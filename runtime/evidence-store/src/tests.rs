use super::*;

use harness_control_protocol::v1::{
    EvidenceLifecycleState, EvidenceStage, EvidenceWorkspaceValidation, HistoryOutcome,
    PolicyDecision, PolicyReason,
};
use rusqlite::{Connection, OptionalExtension};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;

fn database() -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let path = directory.path().join("history.sqlite3");
    (directory, path)
}

fn event(
    runtime_id: &str,
    request_id: &str,
    event_id: &str,
    occurred_at_utc: &str,
    stage: EvidenceStage,
    outcome: HistoryOutcome,
) -> EvidenceEventV1 {
    EvidenceEventV1 {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        event_id: event_id.to_owned(),
        runtime_id: runtime_id.to_owned(),
        request_id: request_id.to_owned(),
        host_session_id: Some("host-test".to_owned()),
        supervisor_generation_id: Some("generation-test".to_owned()),
        approval_id: None,
        tool_name: "read_file".to_owned(),
        occurred_at_utc: occurred_at_utc.to_owned(),
        recorded_at_utc: occurred_at_utc.to_owned(),
        stage,
        lifecycle_state: match stage {
            EvidenceStage::RequestReceived => EvidenceLifecycleState::Received,
            EvidenceStage::WorkspaceValidated => EvidenceLifecycleState::WorkspaceValidating,
            EvidenceStage::PolicyEvaluated => EvidenceLifecycleState::PolicyEvaluating,
            EvidenceStage::ApprovalRequired => EvidenceLifecycleState::AwaitingApproval,
            EvidenceStage::ApprovalResolved => EvidenceLifecycleState::Completed,
            EvidenceStage::ApprovalExpired | EvidenceStage::ApprovalCancelled => {
                EvidenceLifecycleState::Failed
            }
            EvidenceStage::SessionGrantMatched => EvidenceLifecycleState::Executing,
            EvidenceStage::ExecutionStarted => EvidenceLifecycleState::Executing,
            EvidenceStage::ExecutionCompleted => EvidenceLifecycleState::Completed,
            EvidenceStage::ExecutionFailed | EvidenceStage::RequestDenied => {
                EvidenceLifecycleState::Failed
            }
        },
        workspace_validation: EvidenceWorkspaceValidation::Passed,
        policy_decision: Some(PolicyDecision::Allow),
        policy_reason: Some(PolicyReason::ToolDefault),
        matched_rule_ids: Vec::new(),
        approval_decision: None,
        outcome,
        failure_category: None,
        duration_ms: None,
        sanitized_arguments: None,
        result_evidence: None,
        coverage: EvidenceCoverage::Complete,
    }
}

fn success_event(
    runtime_id: &str,
    request_id: &str,
    event_id: &str,
    timestamp: &str,
) -> EvidenceEventV1 {
    event(
        runtime_id,
        request_id,
        event_id,
        timestamp,
        EvidenceStage::ExecutionCompleted,
        HistoryOutcome::Success,
    )
}

fn create_v1_fixture(path: &std::path::Path) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(SCHEMA_V1_SQL).unwrap();
    connection
        .execute(
            "INSERT INTO schema_migrations(version, checksum, applied_at_utc)
             VALUES (1, ?1, '2026-01-01T00:00:00.000Z')",
            [SCHEMA_V1_CHECKSUM],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO evidence_health(id, schema_version, drop_count)
             VALUES (1, 1, 0)",
            [],
        )
        .unwrap();
}

#[test]
fn schema_v2_checksum_tables_and_indexes_are_frozen() {
    let (_directory, path) = database();
    let writer = EvidenceWriter::open(&path).expect("create schema");
    assert_eq!(SCHEMA_VERSION, 2);
    assert_eq!(computed_schema_checksum(), SCHEMA_CHECKSUM);
    drop(writer);

    let connection = Connection::open(&path).expect("inspect schema");
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
             AND name IN ('schema_migrations', 'runtime_sessions', 'request_summary',
                          'evidence_events', 'approval_history', 'evidence_health')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(table_count, 6);
    let request_spans: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'request_spans'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(request_spans, 1);
    for index in [
        "idx_request_summary_first_seen",
        "idx_request_summary_tool_first_seen",
        "idx_request_summary_outcome_first_seen",
        "idx_request_summary_policy_first_seen",
        "idx_request_summary_approval_first_seen",
        "idx_evidence_events_request_order",
        "idx_evidence_events_occurred",
        "idx_evidence_events_approval",
        "idx_approval_history_terminal",
    ] {
        let exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "missing index {index}");
    }
    let checksum: String = connection
        .query_row(
            "SELECT checksum FROM schema_migrations WHERE version = 2",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(checksum, SCHEMA_CHECKSUM);
}

#[test]
fn v1_migrates_to_v2_without_inventing_trace_rows() {
    let (_directory, path) = database();
    create_v1_fixture(&path);
    let writer = EvidenceWriter::open(&path).expect("v1 fixture migrates");
    drop(writer);
    let connection = Connection::open(&path).unwrap();
    let version: i64 = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, 2);
    let trace_id: Option<String> = connection
        .query_row("SELECT trace_id FROM request_summary LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()
        .unwrap()
        .flatten();
    assert!(trace_id.is_none());
    let spans: i64 = connection
        .query_row("SELECT COUNT(*) FROM request_spans", [], |row| row.get(0))
        .unwrap();
    assert_eq!(spans, 0);
    assert!(migration_backup_path(&path).exists());
}

#[test]
fn v1_migration_failure_preserves_sibling_backup() {
    let (_directory, path) = database();
    create_v1_fixture(&path);
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE request_spans(bad INTEGER);")
            .unwrap();
    }
    let before = fs::read(&path).unwrap();
    let error = EvidenceWriter::open(&path).expect_err("malformed span table must fail");
    let backup = match error {
        StoreError::MigrationFailed { backup_path, .. } => backup_path,
        other => panic!("expected migration failure, got {other:?}"),
    };
    assert!(backup.exists());
    assert_eq!(fs::read(backup).unwrap(), before);
}

#[test]
fn trace_spans_share_request_identity_and_are_bounded() {
    let (_directory, path) = database();
    let mut writer = EvidenceWriter::open(&path).unwrap();
    writer
        .record_event(&event(
            "runtime-trace",
            "request-trace",
            "event-trace",
            "2026-01-01T00:00:00.000Z",
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
        ))
        .unwrap();
    let span = RequestTraceSpan {
        span_id: "span-root".into(),
        trace_id: "trace-local".into(),
        parent_span_id: None,
        runtime_id: "runtime-trace".into(),
        request_id: "request-trace".into(),
        span_sequence: 0,
        span_name: harness_control_protocol::TraceSpanName::RequestWall,
        started_at_utc: "2026-01-01T00:00:00.000Z".into(),
        ended_at_utc: None,
        started_offset_ms: 0,
        duration_ms: None,
        status: harness_control_protocol::TraceSpanStatus::Open,
        failure_category: None,
        coverage: EvidenceCoverage::Complete,
    };
    assert_eq!(writer.record_spans(std::slice::from_ref(&span)).unwrap(), 1);
    let mut closed = span.clone();
    closed.ended_at_utc = Some("2026-01-01T00:00:00.050Z".into());
    closed.duration_ms = Some(50);
    closed.status = harness_control_protocol::TraceSpanStatus::Complete;
    assert_eq!(
        writer.record_spans(std::slice::from_ref(&closed)).unwrap(),
        1
    );
    // A retried open snapshot cannot reopen a span that was already closed.
    assert_eq!(writer.record_spans(std::slice::from_ref(&span)).unwrap(), 0);
    drop(writer);
    let connection = Connection::open(&path).unwrap();
    let (trace_id, count, duration, status): (String, i64, Option<i64>, String) = connection
        .query_row(
            "SELECT request_summary.trace_id, COUNT(request_spans.span_id),
                     MAX(request_spans.duration_ms), MAX(request_spans.status)
             FROM request_summary
             LEFT JOIN request_spans USING(runtime_id, request_id)
             WHERE request_summary.runtime_id = 'runtime-trace'
               AND request_summary.request_id = 'request-trace'
             GROUP BY request_summary.trace_id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(trace_id, "trace-local");
    assert_eq!(count, 1);
    assert_eq!(duration, Some(50));
    assert_eq!(status, "complete");
}

#[test]
fn newer_and_corrupt_databases_are_preserved() {
    let (_directory, path) = database();
    let writer = EvidenceWriter::open(&path).unwrap();
    drop(writer);
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations(version, checksum, applied_at_utc)
                VALUES (3, 'future', '2099-01-01T00:00:00.000Z')",
                [],
            )
            .unwrap();
    }
    let before = fs::read(&path).unwrap();
    let error = EvidenceWriter::open(&path).expect_err("newer schema must be rejected");
    assert!(matches!(error, StoreError::IncompatibleSchema { found: 3 }));
    assert!(matches!(
        EvidenceReader::open(&path),
        Err(StoreError::IncompatibleSchema { found: 3 })
    ));
    assert_eq!(fs::read(&path).unwrap(), before);

    let (_corrupt_directory, corrupt_path) = database();
    let bytes = b"not a sqlite database".to_vec();
    fs::write(&corrupt_path, &bytes).unwrap();
    let error = EvidenceWriter::open(&corrupt_path).expect_err("corrupt source must be preserved");
    assert!(matches!(error, StoreError::Corrupt { .. }));
    assert_eq!(fs::read(&corrupt_path).unwrap(), bytes);

    assert!(matches!(
        EvidenceReader::open(&corrupt_path),
        Err(StoreError::Corrupt { .. })
    ));
}

#[test]
fn checksum_mismatch_is_preserved_and_never_recreated() {
    let (_directory, path) = database();
    drop(EvidenceWriter::open(&path).unwrap());
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE schema_migrations SET checksum = 'tampered' WHERE version = 2",
                [],
            )
            .unwrap();
    }
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        EvidenceWriter::open(&path),
        Err(StoreError::SchemaChecksumMismatch { .. })
    ));
    assert!(matches!(
        EvidenceReader::open(&path),
        Err(StoreError::SchemaChecksumMismatch { .. })
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn sqlite_busy_maps_to_retryable_status_without_becoming_policy_state() {
    let busy = rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error {
            code: rusqlite::ErrorCode::DatabaseBusy,
            extended_code: 5,
        },
        Some("database is locked".to_owned()),
    );
    let error = StoreError::from_sqlite(busy);
    assert!(matches!(error, StoreError::Busy));
    let status = status_for_error(&error, HistoryConfig::default());
    assert_eq!(status.state, EvidenceStoreState::Busy);
    assert_eq!(
        status.last_error_code,
        Some(EvidenceErrorCode::EvidenceBusy)
    );
}

#[test]
fn legacy_migration_uses_backup_and_failed_migration_preserves_source() {
    let (_directory, path) = database();
    migration::create_legacy_v0(&path).unwrap();
    let writer = EvidenceWriter::open(&path).expect("legacy v0 migrates");
    drop(writer);
    assert!(migration_backup_path(&path).exists());

    let (_failed_directory, failed_path) = database();
    migration::create_legacy_v0(&failed_path).unwrap();
    {
        let connection = Connection::open(&failed_path).unwrap();
        connection
            .execute("CREATE TABLE runtime_sessions(bad INTEGER)", [])
            .unwrap();
    }
    let before_backup_source = fs::read(&failed_path).unwrap();
    let error = EvidenceWriter::open(&failed_path).expect_err("bad legacy shape must fail");
    let backup = match error {
        StoreError::MigrationFailed { backup_path, .. } => backup_path,
        other => panic!("expected migration failure, got {other:?}"),
    };
    assert!(backup.exists());
    assert_eq!(fs::read(&backup).unwrap(), before_backup_source);
}

#[test]
fn second_writer_is_rejected_and_wal_reader_is_query_only() {
    let (_directory, path) = database();
    let mut writer = EvidenceWriter::open(&path).unwrap();
    assert!(!writer.connection_is_query_only().unwrap());
    let error = EvidenceWriter::open(&path).expect_err("second writer must fail");
    assert!(matches!(error, StoreError::WriterBusy { .. }));

    let reader = EvidenceReader::open(&path).unwrap();
    assert!(reader.is_initialized());
    assert!(reader.connection_is_query_only().unwrap());
    writer
        .record_event(&success_event(
            "runtime-1",
            "request-1",
            "event-1",
            "2026-01-01T00:00:00.000Z",
        ))
        .unwrap();
    let result = reader.query(&HistoryQueryParams::default()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.store_status.schema_version, Some(2));
}

#[test]
fn offline_maintenance_shares_writer_ownership_lock() {
    let (_directory, path) = database();
    let writer = EvidenceWriter::open(&path).unwrap();
    let error = OfflineMaintenance::open(&path).expect_err("maintenance must not take writer lock");
    assert!(matches!(error, StoreError::WriterBusy { .. }));
    drop(writer);
    let maintenance = OfflineMaintenance::open(&path).expect("maintenance after writer exits");
    drop(maintenance);
}

#[test]
fn duplicate_event_is_a_noop_and_batch_failure_rolls_back() {
    let (_directory, path) = database();
    let mut writer = EvidenceWriter::open(&path).unwrap();
    let first = event(
        "runtime-1",
        "request-1",
        "event-1",
        "2026-01-01T00:00:00.000Z",
        EvidenceStage::RequestReceived,
        HistoryOutcome::InProgress,
    );
    assert!(writer.record_event(&first).unwrap().inserted);
    let mut duplicate = first.clone();
    duplicate.outcome = HistoryOutcome::Success;
    duplicate.stage = EvidenceStage::ExecutionCompleted;
    assert!(!writer.record_event(&duplicate).unwrap().inserted);
    let mut cross_request_duplicate = first.clone();
    cross_request_duplicate.runtime_id = "runtime-2".to_owned();
    cross_request_duplicate.request_id = "request-other".to_owned();
    assert!(
        !writer
            .record_event(&cross_request_duplicate)
            .unwrap()
            .inserted
    );

    let mut invalid = first.clone();
    invalid.event_id.clear();
    let second = success_event(
        "runtime-1",
        "request-2",
        "event-2",
        "2026-01-01T00:00:01.000Z",
    );
    assert!(matches!(
        writer.record_events(&[second, invalid]),
        Err(StoreError::InvalidEvidence(_))
    ));
    writer.checkpoint().unwrap();
    drop(writer);
    let reader = EvidenceReader::open(&path).unwrap();
    let first_detail = reader.get("runtime-1", "request-1").unwrap().unwrap();
    assert_eq!(first_detail.events.len(), 1);
    assert_eq!(
        first_detail.summary.terminal_state,
        HistoryTerminalState::Open
    );
    assert!(reader.get("runtime-1", "request-2").unwrap().is_none());
    assert!(reader.get("runtime-2", "request-other").unwrap().is_none());
}

#[test]
fn runtime_queue_health_methods_do_not_create_authority_or_synthetic_events() {
    let (_directory, path) = database();
    let mut writer = EvidenceWriter::open(&path).unwrap();
    writer
        .record_event(&event(
            "runtime-health",
            "request-health",
            "event-health",
            "2026-01-01T00:00:00.000Z",
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
        ))
        .unwrap();
    writer
        .mark_request_incomplete("runtime-health", "request-health")
        .unwrap();
    writer
        .record_event(&success_event(
            "runtime-health",
            "request-health",
            "event-health-terminal",
            "2026-01-01T00:00:01.500Z",
        ))
        .unwrap();
    writer
        .mark_coverage_unknown("2026-01-01T00:00:01.000Z")
        .unwrap();
    writer
        .record_drop_count(7, "2026-01-01T00:00:02.000Z", "EVIDENCE_QUEUE_OVERFLOW")
        .unwrap();
    let status = writer.status().unwrap();
    assert_eq!(status.drop_count, 7);
    assert_eq!(
        status.coverage_unknown_since_utc.as_deref(),
        Some("2026-01-01T00:00:01.000Z")
    );
    assert_eq!(status.state, EvidenceStoreState::Degraded);
    drop(writer);

    let reader = EvidenceReader::open(&path).unwrap();
    let detail = reader
        .get("runtime-health", "request-health")
        .unwrap()
        .unwrap();
    assert_eq!(detail.events.len(), 2);
    assert_eq!(
        detail.summary.evidence_coverage,
        EvidenceCoverage::Incomplete
    );
    let connection = Connection::open(&path).unwrap();
    let health: (i64, String) = connection
        .query_row(
            "SELECT drop_count, coverage_unknown_since_utc FROM evidence_health WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(health, (7, "2026-01-01T00:00:01.000Z".to_owned()));
}

#[test]
fn terminal_approval_is_persisted_once_and_detail_is_safe() {
    let (_directory, path) = database();
    let mut writer = EvidenceWriter::open(&path).unwrap();
    let mut required = event(
        "runtime-1",
        "request-approval",
        "approval-event-1",
        "2026-01-01T00:00:00.000Z",
        EvidenceStage::ApprovalRequired,
        HistoryOutcome::ApprovalRequired,
    );
    required.approval_id = Some("approval-1".to_owned());
    writer.record_event(&required).unwrap();
    let mut resolved = success_event(
        "runtime-1",
        "request-approval",
        "approval-event-2",
        "2026-01-01T00:00:01.000Z",
    );
    resolved.stage = EvidenceStage::ApprovalResolved;
    resolved.approval_id = Some("approval-1".to_owned());
    resolved.approval_decision = Some(ApprovalDecision::AllowOnce);
    writer.record_event(&resolved).unwrap();
    let mut duplicate_terminal = resolved.clone();
    duplicate_terminal.event_id = "approval-event-3".to_owned();
    writer.record_event(&duplicate_terminal).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);

    let reader = EvidenceReader::open(&path).unwrap();
    let detail = reader
        .get("runtime-1", "request-approval")
        .unwrap()
        .unwrap();
    assert_eq!(detail.events.len(), 3);
    assert_eq!(
        detail.summary.approval_state,
        HistoryApprovalState::AllowOnce
    );
    assert_eq!(
        detail
            .terminal_approval
            .as_ref()
            .map(|approval| approval.approval_id.as_str()),
        Some("approval-1")
    );
    let connection = Connection::open(&path).unwrap();
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM approval_history", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn cursor_filter_fingerprint_order_and_limits_are_deterministic() {
    let (_directory, path) = database();
    let mut writer = EvidenceWriter::open(&path).unwrap();
    for index in 0..5 {
        let request = format!("request-{index}");
        let tool = if index % 2 == 0 { "read_file" } else { "glob" };
        let mut value = success_event(
            "runtime-1",
            &request,
            &format!("event-{index}"),
            &format!("2026-01-01T00:00:0{index}.000Z"),
        );
        value.tool_name = tool.to_owned();
        value.policy_decision = Some(if index % 2 == 0 {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Deny
        });
        value.outcome = if index == 3 {
            HistoryOutcome::Denied
        } else {
            HistoryOutcome::Success
        };
        writer.record_event(&value).unwrap();
    }
    writer.checkpoint().unwrap();
    drop(writer);

    let reader = EvidenceReader::open(&path).unwrap();
    let first = reader
        .query(&HistoryQueryParams {
            limit: Some(2),
            ..HistoryQueryParams::default()
        })
        .unwrap();
    assert_eq!(first.items.len(), 2);
    let cursor = first.next_cursor.clone().expect("second page cursor");
    let second = reader
        .query(&HistoryQueryParams {
            cursor: Some(cursor.clone()),
            limit: Some(2),
            ..HistoryQueryParams::default()
        })
        .unwrap();
    assert_eq!(second.items.len(), 2);
    assert!(first.items.iter().all(|item| !second
        .items
        .iter()
        .any(|other| other.request_id == item.request_id)));
    let mismatch = reader.query(&HistoryQueryParams {
        cursor: Some(cursor),
        tool: Some("glob".to_owned()),
        ..HistoryQueryParams::default()
    });
    assert!(matches!(mismatch, Err(StoreError::InvalidCursor)));
    assert!(matches!(
        reader.query(&HistoryQueryParams {
            limit: Some(0),
            ..HistoryQueryParams::default()
        }),
        Err(StoreError::InvalidLimit { requested: 0 })
    ));
    let clamped = reader
        .query(&HistoryQueryParams {
            limit: Some(999),
            outcome: Some(HistoryOutcome::Denied),
            policy_decision: Some(PolicyDecision::Deny),
            tool: Some("glob".to_owned()),
            ..HistoryQueryParams::default()
        })
        .unwrap();
    assert_eq!(clamped.items.len(), 1);
    assert_eq!(filter_fingerprint(&HistoryQueryParams::default()).len(), 64);
}

#[test]
fn missing_reader_is_empty_and_offline_clear_and_retention_are_bounded() {
    let (_directory, path) = database();
    let reader = EvidenceReader::open(&path).unwrap();
    assert!(!reader.is_initialized());
    assert!(reader
        .query(&HistoryQueryParams::default())
        .unwrap()
        .items
        .is_empty());
    assert!(!path.exists());
    drop(reader);

    let mut writer = EvidenceWriter::open(&path).unwrap();
    writer
        .record_event(&success_event(
            "runtime-1",
            "old-request",
            "old-event",
            "2020-01-01T00:00:00.000Z",
        ))
        .unwrap();
    writer
        .record_event(&event(
            "runtime-1",
            "open-request",
            "open-event",
            "2026-01-01T00:00:00.000Z",
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
        ))
        .unwrap();
    let report = writer
        .apply_retention(
            "2026-01-02T00:00:00.000Z",
            &RetentionPolicy {
                retention_days: 1,
                max_storage_mib: 32,
            },
        )
        .unwrap();
    assert_eq!(report.age_cleanup_requests, 1);
    drop(writer);

    let mut maintenance = OfflineMaintenance::open(&path).unwrap();
    let clear = maintenance.clear_all().unwrap();
    assert_eq!(clear.deleted_request_count, 1);
    assert_eq!(clear.deleted_event_count, 1);
    drop(maintenance);
    let reader = EvidenceReader::open(&path).unwrap();
    assert!(reader
        .query(&HistoryQueryParams::default())
        .unwrap()
        .items
        .is_empty());
}

#[test]
fn capacity_retention_runs_hysteresis_and_protects_current_open_requests() {
    let (_directory, path) = database();
    let mut writer = EvidenceWriter::open(&path).unwrap();
    writer
        .open_session(&RuntimeSession {
            runtime_id: "runtime-current".to_owned(),
            host_session_id: None,
            supervisor_generation_id: None,
            started_at_utc: "2026-01-01T00:00:00.000Z".to_owned(),
            runtime_version: "test".to_owned(),
            config_revision: "test".to_owned(),
        })
        .unwrap();
    let payload = "x".repeat(24 * 1024);
    for index in 0..2_000u32 {
        let mut value = success_event(
            "runtime-old",
            &format!("capacity-{index:04}"),
            &format!("capacity-event-{index:04}"),
            &format!(
                "2026-01-01T{:02}:{:02}:00.000Z",
                index / 3600,
                (index / 60) % 60
            ),
        );
        value.sanitized_arguments = Some(DurableSanitizedArgumentsEvidence::Glob {
            pattern: payload.clone(),
        });
        writer.record_event(&value).unwrap();
    }
    writer
        .record_event(&event(
            "runtime-current",
            "current-open",
            "current-open-event",
            "2026-01-01T01:00:00.000Z",
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
        ))
        .unwrap();
    let before = file_footprint_bytes(&path).unwrap();
    assert!(
        before > 32 * 1024 * 1024,
        "fixture must exceed cap: {before}"
    );
    let report = writer
        .apply_retention(
            "2026-01-02T00:00:00.000Z",
            &RetentionPolicy {
                retention_days: 3650,
                max_storage_mib: 32,
            },
        )
        .unwrap();
    assert!(report.capacity_cleanup_requests > 0);
    assert!(!report.capacity_degraded);
    assert!(report.footprint_bytes <= 32 * 1024 * 1024);
    drop(writer);
    let reader = EvidenceReader::open(&path).unwrap();
    assert!(reader
        .get("runtime-current", "current-open")
        .unwrap()
        .is_some());
}

#[test]
fn unclean_restart_marks_open_session_and_request_incomplete_without_synthetic_event() {
    let (_directory, path) = database();
    {
        let mut writer = EvidenceWriter::open(&path).unwrap();
        writer
            .open_session(&RuntimeSession {
                runtime_id: "runtime-crash".to_owned(),
                host_session_id: Some("host-crash".to_owned()),
                supervisor_generation_id: Some("generation-crash".to_owned()),
                started_at_utc: "2026-01-01T00:00:00.000Z".to_owned(),
                runtime_version: "test".to_owned(),
                config_revision: "test".to_owned(),
            })
            .unwrap();
        writer
            .record_event(&event(
                "runtime-crash",
                "request-crash",
                "crash-event-1",
                "2026-01-01T00:00:01.000Z",
                EvidenceStage::RequestReceived,
                HistoryOutcome::InProgress,
            ))
            .unwrap();
        // Drop without close_session to model an unclean Runtime exit.
    }
    let recovery = EvidenceWriter::open(&path).unwrap();
    drop(recovery);
    let reader = EvidenceReader::open(&path).unwrap();
    let detail = reader
        .get("runtime-crash", "request-crash")
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.summary.terminal_state,
        HistoryTerminalState::Incomplete
    );
    assert_eq!(
        detail.summary.evidence_coverage,
        EvidenceCoverage::Incomplete
    );
    assert_eq!(detail.events.len(), 1);
    let connection = Connection::open(&path).unwrap();
    let (ended, reason): (String, String) = connection
        .query_row(
            "SELECT ended_at_utc, end_reason FROM runtime_sessions WHERE runtime_id = 'runtime-crash'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(!ended.is_empty());
    assert_eq!(reason, "unclean_restart");
}

#[test]
#[ignore = "250k-event performance fixture; run explicitly for P95 evidence"]
fn performance_50k_requests_250k_events_p95() {
    let (_directory, path) = database();
    let mut writer = EvidenceWriter::open(&path).unwrap();
    let session = RuntimeSession {
        runtime_id: "runtime-perf".to_owned(),
        host_session_id: None,
        supervisor_generation_id: None,
        started_at_utc: "2026-01-01T00:00:00.000Z".to_owned(),
        runtime_version: "perf".to_owned(),
        config_revision: "perf".to_owned(),
    };
    writer.open_session(&session).unwrap();
    let mut batch = Vec::with_capacity(64);
    for request_index in 0..50_000u32 {
        let request_id = format!("request-{request_index:05}");
        for stage_index in 0..5u32 {
            let (stage, outcome) = match stage_index {
                0 => (EvidenceStage::RequestReceived, HistoryOutcome::InProgress),
                1 => (EvidenceStage::PolicyEvaluated, HistoryOutcome::InProgress),
                2 => (EvidenceStage::ExecutionStarted, HistoryOutcome::InProgress),
                _ => (EvidenceStage::ExecutionCompleted, HistoryOutcome::Success),
            };
            let mut value = event(
                "runtime-perf",
                &request_id,
                &format!("event-{request_index:05}-{stage_index}"),
                &format!(
                    "2026-01-01T00:{:02}:{:02}.000Z",
                    request_index / 3600,
                    request_index % 60
                ),
                stage,
                outcome,
            );
            value.tool_name = if request_index % 2 == 0 {
                "read_file".to_owned()
            } else {
                "glob".to_owned()
            };
            batch.push(value);
            if batch.len() == 64 {
                writer.record_events(&batch).unwrap();
                batch.clear();
            }
        }
    }
    if !batch.is_empty() {
        writer.record_events(&batch).unwrap();
    }
    writer.checkpoint().unwrap();
    drop(writer);

    let reader = EvidenceReader::open(&path).unwrap();
    let params = HistoryQueryParams {
        tool: Some("read_file".to_owned()),
        limit: Some(50),
        ..HistoryQueryParams::default()
    };
    for _ in 0..2 {
        reader.query(&params).unwrap();
    }
    let mut query_samples = Vec::with_capacity(30);
    let mut get_samples = Vec::with_capacity(30);
    for _ in 0..30 {
        let start = Instant::now();
        reader.query(&params).unwrap();
        query_samples.push(start.elapsed());
        let start = Instant::now();
        reader.get("runtime-perf", "request-25000").unwrap();
        get_samples.push(start.elapsed());
    }
    query_samples.sort_unstable();
    get_samples.sort_unstable();
    let query_p95 = query_samples[28];
    let get_p95 = get_samples[28];
    println!(
        "history.query P95={}ms history.get P95={}ms footprint={} bytes",
        query_p95.as_secs_f64() * 1000.0,
        get_p95.as_secs_f64() * 1000.0,
        file_footprint_bytes(&path).unwrap()
    );
    assert!(query_p95 <= std::time::Duration::from_millis(500));
    assert!(get_p95 <= std::time::Duration::from_millis(250));
}
