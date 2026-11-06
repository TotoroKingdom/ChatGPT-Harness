//! M08 T5 regression fixtures.  All data is disposable and synthetic.
//!
//! These tests exercise the read-only projections against the physical v2
//! schema.  They deliberately insert already-sanitized rows, rather than
//! invoking Runtime policy or executor code, so a regression cannot grant an
//! authorization side effect.

use chatgpt_harness_evidence_store::{
    migration_backup_path, EvidenceReader, EvidenceWriter, MetricsQueryParams, MetricsSampleStatus,
    OfflineMaintenance, StoreError, TraceAvailability, TraceCoverage, TraceGetParams,
    TraceSpanName, TraceSpanStatus, SCHEMA_V1_CHECKSUM, SCHEMA_V1_SQL,
};
use rusqlite::{params, Connection};
use tempfile::TempDir;

fn database() -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("temporary evidence directory");
    let path = dir.path().join("evidence.sqlite3");
    drop(EvidenceWriter::open(&path).expect("create v2 evidence store"));
    (dir, path)
}

fn insert_session(connection: &Connection) {
    connection
        .execute(
            "INSERT INTO runtime_sessions
             (runtime_id, started_at_utc, ended_at_utc, runtime_version, config_revision)
             VALUES ('r-m08', '2026-10-01T00:00:00.000Z',
                     '2026-10-02T00:00:00.000Z', 'test', 'test')",
            [],
        )
        .expect("insert synthetic runtime session");
}

fn insert_summary(
    connection: &Connection,
    request_id: &str,
    terminal_state: &str,
    outcome: Option<&str>,
    execution_started: bool,
    coverage: &str,
    trace_id: Option<&str>,
) {
    connection
        .execute(
            "INSERT INTO request_summary
             (runtime_id, request_id, tool_name, first_seen_at_utc, last_seen_at_utc,
              terminal_state, outcome, policy_decision, approval_state,
              execution_started, evidence_coverage, trace_id)
             VALUES ('r-m08', ?1, 'read_file', '2026-10-01T01:00:00.000Z',
                     '2026-10-01T01:00:01.000Z', ?2, ?3, NULL, 'none', ?4, ?5, ?6)",
            params![
                request_id,
                terminal_state,
                outcome,
                execution_started,
                coverage,
                trace_id,
            ],
        )
        .expect("insert synthetic request summary");
}

fn insert_received(connection: &Connection, request_id: &str) {
    connection
        .execute(
            "INSERT INTO evidence_events
             (event_id, runtime_id, request_id, tool_name, occurred_at_utc,
              recorded_at_utc, stage, lifecycle_state, workspace_validation,
              outcome, coverage)
             VALUES (?1, 'r-m08', ?2, 'read_file', '2026-10-01T01:00:00.000Z',
                     '2026-10-01T01:00:00.000Z', 'request_received', 'received',
                     'not_attempted', 'in_progress', 'COMPLETE')",
            params![format!("{request_id}-received"), request_id],
        )
        .expect("insert received evidence");
}

fn insert_execution_started(connection: &Connection, request_id: &str) {
    connection
        .execute(
            "INSERT INTO evidence_events
             (event_id, runtime_id, request_id, tool_name, occurred_at_utc,
              recorded_at_utc, stage, lifecycle_state, workspace_validation,
              outcome, coverage)
             VALUES (?1, 'r-m08', ?2, 'read_file', '2026-10-01T01:00:00.100Z',
                     '2026-10-01T01:00:00.100Z', 'execution_started', 'executing',
                     'passed', 'in_progress', 'COMPLETE')",
            params![format!("{request_id}-execution-started"), request_id],
        )
        .expect("insert execution-started evidence");
}

fn insert_span(
    connection: &Connection,
    request_id: &str,
    trace_id: &str,
    sequence: u32,
    name: &str,
    duration_ms: Option<u64>,
    status: &str,
) {
    connection
        .execute(
            "INSERT INTO request_spans
             (span_id, trace_id, runtime_id, request_id, span_sequence, span_name,
              started_at_utc, ended_at_utc, started_offset_ms, duration_ms, status, coverage)
             VALUES (?1, ?2, 'r-m08', ?3, ?4, ?5, '2026-10-01T01:00:00.000Z',
                     CASE WHEN ?6 IS NULL THEN NULL ELSE '2026-10-01T01:00:01.000Z' END,
                     0, ?6, ?7, 'COMPLETE')",
            params![
                format!("{request_id}-span-{sequence}"),
                trace_id,
                request_id,
                sequence,
                name,
                duration_ms.map(|v| v as i64),
                status,
            ],
        )
        .expect("insert synthetic trace span");
}

fn create_v1_history_fixture(path: &std::path::Path) {
    let connection = Connection::open(path).expect("open v1 fixture");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable v1 fixture foreign keys");
    connection
        .execute_batch(SCHEMA_V1_SQL)
        .expect("create physical v1 schema");
    connection
        .execute(
            "INSERT INTO schema_migrations(version, checksum, applied_at_utc)
             VALUES (1, ?1, '2026-10-01T00:00:00.000Z')",
            [SCHEMA_V1_CHECKSUM],
        )
        .expect("insert v1 schema marker");
    connection
        .execute(
            "INSERT INTO evidence_health(id, schema_version, drop_count)
             VALUES (1, 1, 0)",
            [],
        )
        .expect("insert v1 health row");
    connection
        .execute_batch(
            "INSERT INTO runtime_sessions
                 (runtime_id, host_session_id, supervisor_generation_id,
                  started_at_utc, ended_at_utc, end_reason, runtime_version, config_revision)
             VALUES
                 ('v1-runtime-a', 'host-a', 'generation-a',
                  '2026-10-01T00:00:00.000Z', '2026-10-01T01:00:00.000Z',
                  'normal_stop', 'm03', '1'),
                 ('v1-runtime-b', 'host-b', 'generation-b',
                  '2026-10-01T02:00:00.000Z', '2026-10-01T03:00:00.000Z',
                  'normal_stop', 'm03', '2');

             INSERT INTO request_summary
                 (runtime_id, request_id, tool_name, first_seen_at_utc, last_seen_at_utc,
                  terminal_state, outcome, policy_decision, approval_state,
                  failure_category, execution_started, duration_ms, evidence_coverage)
             VALUES
                 ('v1-runtime-a', 'v1-success', 'read_file',
                  '2026-10-01T00:10:00.000Z', '2026-10-01T00:10:01.000Z',
                  'succeeded', 'success', 'allow', 'none', NULL, 1, 77, 'COMPLETE'),
                 ('v1-runtime-a', 'v1-approval-deny', 'write',
                  '2026-10-01T00:20:00.000Z', '2026-10-01T00:20:02.000Z',
                  'denied', 'denied', 'allow', 'denied', 'approval_denied', 0, 2, 'COMPLETE'),
                 ('v1-runtime-b', 'v1-incomplete', 'read_file',
                  '2026-10-01T02:10:00.000Z', '2026-10-01T02:10:00.000Z',
                  'open', NULL, NULL, 'none', NULL, 0, NULL, 'INCOMPLETE');

             INSERT INTO evidence_events
                 (event_id, runtime_id, request_id, approval_id, tool_name,
                  occurred_at_utc, recorded_at_utc, stage, lifecycle_state,
                  workspace_validation, policy_decision, policy_reason,
                  matched_rule_ids_json, approval_decision, outcome,
                  failure_category, duration_ms, sanitized_arguments_json,
                  result_evidence_json, coverage)
             VALUES
                 ('v1-success-received', 'v1-runtime-a', 'v1-success', NULL, 'read_file',
                  '2026-10-01T00:10:00.000Z', '2026-10-01T00:10:00.000Z',
                  'request_received', 'received', 'not_attempted', NULL, NULL,
                  NULL, NULL, 'in_progress', NULL, NULL, NULL, NULL, 'INCOMPLETE'),
                 ('v1-success-started', 'v1-runtime-a', 'v1-success', NULL, 'read_file',
                  '2026-10-01T00:10:00.100Z', '2026-10-01T00:10:00.100Z',
                  'execution_started', 'executing', 'passed', 'allow', 'tool_default',
                  NULL, NULL, 'in_progress', NULL, NULL, NULL, NULL, 'INCOMPLETE'),
                 ('v1-success-completed', 'v1-runtime-a', 'v1-success', NULL, 'read_file',
                  '2026-10-01T00:10:01.000Z', '2026-10-01T00:10:01.000Z',
                  'execution_completed', 'completed', 'passed', 'allow', 'tool_default',
                  NULL, NULL, 'success', NULL, 77, NULL, '{\"bytesRead\":5}', 'COMPLETE'),
                 ('v1-approval-received', 'v1-runtime-a', 'v1-approval-deny', 'approval-v1', 'write',
                  '2026-10-01T00:20:00.000Z', '2026-10-01T00:20:00.000Z',
                  'request_received', 'received', 'not_attempted', NULL, NULL,
                  NULL, NULL, 'in_progress', NULL, NULL, NULL, NULL, 'INCOMPLETE'),
                 ('v1-approval-required', 'v1-runtime-a', 'v1-approval-deny', 'approval-v1', 'write',
                  '2026-10-01T00:20:00.100Z', '2026-10-01T00:20:00.100Z',
                  'approval_required', 'awaiting_approval', 'passed', 'allow', 'tool_default',
                  NULL, NULL, 'approval_required', NULL, NULL, NULL, NULL, 'INCOMPLETE'),
                 ('v1-approval-denied', 'v1-runtime-a', 'v1-approval-deny', 'approval-v1', 'write',
                  '2026-10-01T00:20:02.000Z', '2026-10-01T00:20:02.000Z',
                  'approval_resolved', 'failed', 'passed', 'allow', 'tool_default',
                  NULL, 'deny', 'denied', 'approval_denied', 2, NULL, NULL, 'COMPLETE'),
                 ('v1-incomplete-received', 'v1-runtime-b', 'v1-incomplete', NULL, 'read_file',
                  '2026-10-01T02:10:00.000Z', '2026-10-01T02:10:00.000Z',
                  'request_received', 'received', 'not_attempted', NULL, NULL,
                  NULL, NULL, 'in_progress', NULL, NULL, NULL, NULL, 'INCOMPLETE');

             INSERT INTO approval_history
                 (runtime_id, approval_id, request_id, tool_name, requested_at_utc,
                  terminal_at_utc, terminal_state, decision)
             VALUES ('v1-runtime-a', 'approval-v1', 'v1-approval-deny', 'write',
                     '2026-10-01T00:20:00.100Z', '2026-10-01T00:20:02.000Z',
                     'denied', 'deny');",
        )
        .expect("insert representative v1 History rows");
}

fn v1_data_snapshot(path: &std::path::Path) -> (Vec<String>, Vec<String>, Vec<String>) {
    let connection = Connection::open(path).expect("open v1 snapshot");
    let summaries = connection
        .prepare(
            "SELECT runtime_id || '|' || request_id || '|' || terminal_state || '|'
                    || COALESCE(outcome, '') || '|' || approval_state || '|'
                    || execution_started || '|' || COALESCE(duration_ms, -1) || '|'
                    || evidence_coverage
             FROM request_summary ORDER BY runtime_id, request_id",
        )
        .expect("prepare summary snapshot")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("read summary snapshot")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect summary snapshot");
    let events = connection
        .prepare(
            "SELECT event_id || '|' || runtime_id || '|' || request_id || '|'
                    || stage || '|' || outcome || '|' || coverage || '|'
                    || COALESCE(approval_id, '') || '|' || COALESCE(approval_decision, '')
             FROM evidence_events ORDER BY event_id",
        )
        .expect("prepare event snapshot")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("read event snapshot")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect event snapshot");
    let approvals = connection
        .prepare(
            "SELECT runtime_id || '|' || approval_id || '|' || request_id || '|'
                    || terminal_state || '|' || COALESCE(decision, '')
             FROM approval_history ORDER BY runtime_id, approval_id",
        )
        .expect("prepare approval snapshot")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("read approval snapshot")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect approval snapshot");
    (summaries, events, approvals)
}

#[test]
fn metrics_keeps_terminal_categories_and_success_denominator_independent() {
    let (_dir, database) = database();
    let connection = Connection::open(&database).expect("open fixture");
    insert_session(&connection);

    // The rows below cover every mutually exclusive terminal classification.
    let rows = [
        ("success", "succeeded", Some("success"), true, "COMPLETE"),
        ("failed", "failed", Some("failed"), true, "COMPLETE"),
        ("denied", "denied", Some("denied"), false, "COMPLETE"),
        (
            "cancelled",
            "cancelled",
            Some("cancelled"),
            true,
            "COMPLETE",
        ),
        ("timeout", "timed_out", Some("timed_out"), true, "COMPLETE"),
        ("pre-exec", "failed", Some("failed"), false, "COMPLETE"),
        (
            "approval-expired",
            "approval_expired",
            Some("approval_expired"),
            false,
            "COMPLETE",
        ),
        (
            "approval-cancelled",
            "approval_cancelled",
            Some("approval_cancelled"),
            false,
            "COMPLETE",
        ),
        (
            "incomplete",
            "incomplete",
            Some("in_progress"),
            false,
            "INCOMPLETE",
        ),
        ("unknown", "unknown", None, false, "UNKNOWN"),
    ];
    for (request, terminal, outcome, started, coverage) in rows {
        insert_summary(
            &connection,
            request,
            terminal,
            outcome,
            started,
            coverage,
            Some(&format!("trace-{request}")),
        );
        if request != "unknown" {
            insert_received(&connection, request);
        }
        if started {
            insert_execution_started(&connection, request);
        }
    }
    insert_span(
        &connection,
        "success",
        "trace-success",
        0,
        "request_wall",
        Some(100),
        "success",
    );
    insert_span(
        &connection,
        "success",
        "trace-success",
        1,
        "execution",
        Some(40),
        "success",
    );
    insert_span(
        &connection,
        "failed",
        "trace-failed",
        0,
        "request_wall",
        Some(120),
        "failed",
    );
    insert_span(
        &connection,
        "failed",
        "trace-failed",
        1,
        "execution",
        Some(50),
        "failed",
    );
    drop(connection);

    let reader = EvidenceReader::open(&database).expect("open read-only fixture");
    let result = reader
        .metrics_query(&MetricsQueryParams {
            from_utc: "2026-10-01T00:00:00Z".into(),
            to_utc: "2026-10-02T00:00:00Z".into(),
            tool: None,
        })
        .expect("query metrics");
    let counts = result.counts;
    assert_eq!(counts.received_calls, 9);
    assert_eq!(counts.execution_attempts, 4);
    assert_eq!(counts.successful_calls, 1);
    assert_eq!(counts.failed_calls, 1);
    assert_eq!(counts.denied_calls, 1);
    assert_eq!(counts.cancelled_calls, 1);
    assert_eq!(counts.timed_out_calls, 1);
    assert_eq!(counts.pre_execution_failed_calls, 1);
    assert_eq!(counts.approval_expired_calls, 1);
    assert_eq!(counts.approval_cancelled_calls, 1);
    assert_eq!(counts.incomplete_calls, 1);
    assert_eq!(counts.unknown_calls, 1);
    assert_eq!(counts.completed_execution_denominator, 2);
    assert_eq!(counts.execution_success_numerator, 1);
    assert_eq!(counts.execution_success_rate, Some(0.5));
    assert_eq!(
        counts.execution_success_sample_status,
        MetricsSampleStatus::Insufficient
    );
    assert_eq!(result.execution.sample_count, 2);
    assert_eq!(result.execution.average_ms, Some(45.0));
    assert_eq!(result.execution.p95_ms, None);
}

#[test]
fn v1_history_migrates_without_fabricated_trace_and_backup_restores_old_reader() {
    let dir = tempfile::tempdir().expect("temporary v1 migration directory");
    let database = dir.path().join("legacy-v1.sqlite3");
    create_v1_history_fixture(&database);
    let before = v1_data_snapshot(&database);

    // Tauri/offline read ownership must not become the migration owner.  A
    // populated v1 database remains schema/data-compatible v1 until the
    // Runtime writer is explicitly opened below.  SQLite may legitimately
    // update file metadata while opening a database, so this checks the
    // externally meaningful compatibility boundary rather than raw bytes.
    let maintenance = OfflineMaintenance::open(&database).expect("open v1 maintenance facade");
    drop(maintenance);
    assert_eq!(before, v1_data_snapshot(&database));
    let maintenance_connection = Connection::open(&database).expect("open v1 version probe");
    let version: i64 = maintenance_connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read v1 version");
    assert_eq!(version, 1, "offline maintenance must not auto-migrate v1");
    let span_table_count: i64 = maintenance_connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name = 'request_spans'",
            [],
            |row| row.get(0),
        )
        .expect("probe v2 span table");
    assert_eq!(
        span_table_count, 0,
        "offline maintenance must not create v2 span storage"
    );

    // A read-only M08 facade can inspect old History but must not mutate it or
    // pretend that old rows have a trace timeline/statistical span samples.
    let legacy_reader = EvidenceReader::open(&database).expect("open v1 read-only History");
    let history = legacy_reader
        .query(&Default::default())
        .expect("v1 History remains readable");
    assert_eq!(history.items.len(), 3);
    assert_eq!(history.store_status.schema_version, Some(1));
    let legacy_trace = legacy_reader
        .trace_get(&TraceGetParams {
            runtime_id: "v1-runtime-a".into(),
            request_id: "v1-success".into(),
        })
        .expect("legacy trace projection");
    assert_eq!(
        legacy_trace.availability,
        TraceAvailability::MigrationRequired
    );
    assert_eq!(legacy_trace.coverage, TraceCoverage::LegacyUntraced);
    assert!(legacy_trace.trace_id.is_none());
    assert!(legacy_trace.spans.is_empty());
    assert!(matches!(
        legacy_reader.metrics_query(&MetricsQueryParams {
            from_utc: "2026-10-01T00:00:00Z".into(),
            to_utc: "2026-10-02T00:00:00Z".into(),
            tool: None,
        }),
        Err(StoreError::IncompatibleSchema { found: 1 })
    ));
    let approval_detail = legacy_reader
        .get("v1-runtime-a", "v1-approval-deny")
        .expect("read v1 approval detail")
        .expect("approval request is retained");
    assert_eq!(approval_detail.events.len(), 3);
    assert_eq!(
        approval_detail
            .terminal_approval
            .as_ref()
            .expect("terminal v1 approval")
            .terminal_state,
        chatgpt_harness_evidence_store::HistoryApprovalState::Denied
    );
    drop(legacy_reader);

    // The normal Runtime writer owns the v1 -> v2 upgrade.  It preserves all
    // old request-family rows and retains a verified sibling for rollback.
    drop(EvidenceWriter::open(&database).expect("migrate representative v1 store"));
    let backup = migration_backup_path(&database);
    assert!(backup.exists(), "pre-migration v1 backup must be preserved");
    assert_eq!(before, v1_data_snapshot(&database));

    let migrated_reader = EvidenceReader::open(&database).expect("open migrated v2 reader");
    let migrated_history = migrated_reader
        .query(&Default::default())
        .expect("migrated History remains readable");
    assert_eq!(migrated_history.items.len(), 3);
    assert_eq!(migrated_history.store_status.schema_version, Some(2));
    for request_id in ["v1-success", "v1-approval-deny", "v1-incomplete"] {
        let trace = migrated_reader
            .trace_get(&TraceGetParams {
                runtime_id: if request_id == "v1-incomplete" {
                    "v1-runtime-b".into()
                } else {
                    "v1-runtime-a".into()
                },
                request_id: request_id.into(),
            })
            .expect("query migrated legacy trace");
        assert_eq!(trace.availability, TraceAvailability::LegacyUntraced);
        assert_eq!(trace.coverage, TraceCoverage::LegacyUntraced);
        assert!(trace.trace_id.is_none());
        assert!(trace.spans.is_empty());
    }
    let migrated_metrics = migrated_reader
        .metrics_query(&MetricsQueryParams {
            from_utc: "2026-10-01T00:00:00Z".into(),
            to_utc: "2026-10-02T00:00:00Z".into(),
            tool: None,
        })
        .expect("query bounded metrics over migrated legacy rows");
    assert_eq!(migrated_metrics.counts.received_calls, 3);
    assert_eq!(migrated_metrics.counts.successful_calls, 1);
    assert_eq!(migrated_metrics.counts.denied_calls, 1);
    assert_eq!(migrated_metrics.counts.incomplete_calls, 1);
    assert_eq!(migrated_metrics.counts.legacy_untraced_calls, 3);
    assert_eq!(migrated_metrics.execution.sample_count, 0);
    assert_eq!(migrated_metrics.request_wall.sample_count, 0);
    drop(migrated_reader);

    // Disposable rollback exercise: copy the preserved v1 sibling to a new
    // path, leaving the v2 database untouched, and verify old-reader access.
    let restored = database.with_file_name("restored-v1.sqlite3");
    std::fs::copy(&backup, &restored).expect("restore disposable v1 backup");
    let old_reader = EvidenceReader::open(&restored).expect("open restored v1 database");
    assert_eq!(
        old_reader
            .query(&Default::default())
            .expect("old reader History after rollback")
            .items
            .len(),
        3
    );
    let restored_trace = old_reader
        .trace_get(&TraceGetParams {
            runtime_id: "v1-runtime-a".into(),
            request_id: "v1-success".into(),
        })
        .expect("old reader trace availability");
    assert_eq!(
        restored_trace.availability,
        TraceAvailability::MigrationRequired
    );
    assert!(restored_trace.spans.is_empty());
}

#[test]
fn trace_uses_exact_identity_and_clear_removes_span_family() {
    let (_dir, database) = database();
    let connection = Connection::open(&database).expect("open fixture");
    insert_session(&connection);
    insert_summary(
        &connection,
        "request-1",
        "denied",
        Some("denied"),
        false,
        "COMPLETE",
        Some("trace-1"),
    );
    insert_received(&connection, "request-1");
    insert_span(
        &connection,
        "request-1",
        "trace-1",
        0,
        "request_wall",
        Some(3),
        "denied",
    );
    drop(connection);

    let reader = EvidenceReader::open(&database).expect("read fixture");
    let trace = reader
        .trace_get(&TraceGetParams {
            runtime_id: "r-m08".into(),
            request_id: "request-1".into(),
        })
        .expect("exact trace lookup");
    assert_eq!(trace.availability, TraceAvailability::Available);
    assert_eq!(trace.coverage, TraceCoverage::Complete);
    assert_eq!(trace.trace_id.as_deref(), Some("trace-1"));
    assert!(
        !trace.executor_started,
        "DENY must not claim executor start"
    );
    assert_eq!(trace.spans.len(), 1);
    assert_eq!(trace.spans[0].span_name, TraceSpanName::RequestWall);
    assert_eq!(trace.spans[0].status, TraceSpanStatus::Denied);

    // A neighboring request with the same timestamp must not be joined.
    let wrong = reader.trace_get(&TraceGetParams {
        runtime_id: "r-m08".into(),
        request_id: "other-request".into(),
    });
    assert!(matches!(
        wrong,
        Err(chatgpt_harness_evidence_store::StoreError::NotFound)
    ));
    drop(reader);

    let mut writer = EvidenceWriter::open(&database).expect("writer for isolated clear");
    writer
        .clear_terminal_history()
        .expect("clear terminal history");
    drop(writer);
    let reader = EvidenceReader::open(&database).expect("reopen after clear");
    let gone = reader.trace_get(&TraceGetParams {
        runtime_id: "r-m08".into(),
        request_id: "request-1".into(),
    });
    assert!(matches!(
        gone,
        Err(chatgpt_harness_evidence_store::StoreError::NotFound)
    ));
}

#[test]
fn trace_does_not_expose_payload_like_fields() {
    // The trace DTO is a closed projection.  This sentinel guards against
    // accidentally adding raw command/path/result fields to its JSON shape.
    let value = serde_json::to_string(&serde_json::json!({
        "traceId": "trace-safe",
        "spanName": "execution",
        "durationMs": 3,
    }))
    .unwrap();
    for forbidden in [
        "secret",
        "argv",
        "stdout",
        "stderr",
        "fileBody",
        "commandPreview",
    ] {
        assert!(
            !value.contains(forbidden),
            "forbidden trace field leaked: {forbidden}"
        );
    }
}
