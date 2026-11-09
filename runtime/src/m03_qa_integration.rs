use crate::audit::{
    AuditEvent, AuditEventType, AuditFailureCategory, AuditLifecycleState, AuditOutcome,
    WorkspaceValidation,
};
use crate::evidence::{EvidenceService, RuntimeEvidenceContext, SafeEvidenceProjector};
use crate::state::RuntimeCore;
use crate::tools::WriteRequest;
use chatgpt_harness_evidence_store::{
    computed_schema_checksum, file_footprint_bytes, migration_backup_path, EvidenceReader,
    EvidenceWriter, OfflineMaintenance, RetentionPolicy, StoreError, StorePaths, WriterLock,
    SCHEMA_CHECKSUM, SCHEMA_VERSION,
};
use harness_control_protocol::control_read::{BashActivityEvidence, ToolCapability};
use harness_control_protocol::{
    ApprovalDecision, BashWorkingDirectory, DurableSanitizedArgumentsEvidence, EvidenceCoverage,
    EvidenceEventV1, EvidenceLifecycleState, EvidenceStage, EvidenceStoreState,
    ExecutionResultEvidence, HistoryConfig, HistoryOutcome, PolicyConfig, PolicyDecision,
    PolicyReason, RuntimeConfigSnapshot, RuntimeHostContext, RuntimeLimits, SecurityConfig,
    SecurityMode, ToolConfig, WorkspaceConfig,
};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use uuid::Uuid;

fn temp_database() -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("create disposable M03 QA directory");
    let path = directory.path().join("history.sqlite3");
    (directory, path)
}

fn context(runtime_id: &str) -> RuntimeEvidenceContext {
    RuntimeEvidenceContext {
        runtime_id: runtime_id.to_owned(),
        host_session_id: Some("m03-qa-host".to_owned()),
        supervisor_generation_id: Some("m03-qa-generation".to_owned()),
        runtime_version: env!("CARGO_PKG_VERSION").to_owned(),
        config_revision: "m03-qa".to_owned(),
    }
}

fn timestamp(index: usize) -> String {
    format!("2026-11-01T00:00:{:02}.000Z", index % 60)
}

fn lifecycle_for(stage: EvidenceStage) -> EvidenceLifecycleState {
    match stage {
        EvidenceStage::RequestReceived => EvidenceLifecycleState::Received,
        EvidenceStage::WorkspaceValidated => EvidenceLifecycleState::WorkspaceValidating,
        EvidenceStage::PolicyEvaluated => EvidenceLifecycleState::PolicyEvaluating,
        EvidenceStage::ApprovalRequired => EvidenceLifecycleState::AwaitingApproval,
        EvidenceStage::ApprovalResolved => EvidenceLifecycleState::Completed,
        EvidenceStage::ApprovalExpired | EvidenceStage::ApprovalCancelled => {
            EvidenceLifecycleState::Failed
        }
        EvidenceStage::SessionGrantMatched | EvidenceStage::ExecutionStarted => {
            EvidenceLifecycleState::Executing
        }
        EvidenceStage::ExecutionCompleted => EvidenceLifecycleState::Completed,
        EvidenceStage::ExecutionFailed | EvidenceStage::RequestDenied => {
            EvidenceLifecycleState::Failed
        }
    }
}

fn is_terminal_outcome(outcome: HistoryOutcome) -> bool {
    matches!(
        outcome,
        HistoryOutcome::Success
            | HistoryOutcome::Failed
            | HistoryOutcome::Denied
            | HistoryOutcome::Cancelled
            | HistoryOutcome::TimedOut
            | HistoryOutcome::ApprovalExpired
            | HistoryOutcome::ApprovalCancelled
    )
}

fn event(
    runtime_id: &str,
    request_id: &str,
    event_id: &str,
    stage: EvidenceStage,
    outcome: HistoryOutcome,
    occurred_at_utc: &str,
) -> EvidenceEventV1 {
    EvidenceEventV1 {
        schema_version: 1,
        event_id: event_id.to_owned(),
        runtime_id: runtime_id.to_owned(),
        request_id: request_id.to_owned(),
        host_session_id: Some("m03-qa-host".to_owned()),
        supervisor_generation_id: Some("m03-qa-generation".to_owned()),
        approval_id: None,
        tool_name: "read_file".to_owned(),
        occurred_at_utc: occurred_at_utc.to_owned(),
        recorded_at_utc: occurred_at_utc.to_owned(),
        stage,
        lifecycle_state: lifecycle_for(stage),
        workspace_validation: if stage == EvidenceStage::RequestReceived {
            harness_control_protocol::EvidenceWorkspaceValidation::NotAttempted
        } else {
            harness_control_protocol::EvidenceWorkspaceValidation::Passed
        },
        policy_decision: Some(PolicyDecision::Allow),
        policy_reason: Some(PolicyReason::ToolDefault),
        matched_rule_ids: vec!["m03-qa-rule".to_owned()],
        approval_decision: None,
        outcome,
        failure_category: None,
        duration_ms: is_terminal_outcome(outcome).then_some(7),
        sanitized_arguments: None,
        result_evidence: None,
        coverage: if is_terminal_outcome(outcome) {
            EvidenceCoverage::Complete
        } else {
            EvidenceCoverage::Incomplete
        },
    }
}

fn success_event(runtime_id: &str, request_id: &str, event_id: &str, at: &str) -> EvidenceEventV1 {
    event(
        runtime_id,
        request_id,
        event_id,
        EvidenceStage::ExecutionCompleted,
        HistoryOutcome::Success,
        at,
    )
}

fn sqlite_script(database: &Path, script: &str) -> String {
    let output = Command::new("sqlite3")
        .arg("-batch")
        .arg("-noheader")
        .arg(database)
        .arg(script)
        .output()
        .unwrap_or_else(|error| {
            panic!("M03 QA requires the sqlite3 executable for raw SQLite inspection: {error}")
        });
    assert!(
        output.status.success(),
        "sqlite3 script failed for {}: stdout={} stderr={}",
        database.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn sqlite_value(database: &Path, query: &str) -> String {
    let value = sqlite_script(database, query);
    assert!(
        !value.is_empty(),
        "sqlite query returned no scalar: {query}"
    );
    value.lines().next().unwrap().trim().to_owned()
}

fn all_store_bytes(database: &Path) -> Vec<u8> {
    let mut bytes = fs::read(database).expect("read SQLite main database");
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", database.display()));
        if let Ok(sidecar_bytes) = fs::read(sidecar) {
            bytes.extend_from_slice(&sidecar_bytes);
        }
    }
    bytes
}

fn assert_foreign_keys_clean(database: &Path) {
    assert!(
        sqlite_script(database, "PRAGMA foreign_key_check;").is_empty(),
        "SQLite foreign-key check reported an orphan"
    );
    assert_eq!(sqlite_value(database, "PRAGMA integrity_check;"), "ok");
}

fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    predicate()
}

fn assert_status(
    status: &harness_control_protocol::EvidenceStoreStatus,
    state: EvidenceStoreState,
    code: Option<harness_control_protocol::EvidenceErrorCode>,
) {
    assert_eq!(status.state, state);
    assert_eq!(status.last_error_code, code);
    let json = serde_json::to_string(status).expect("serialize UI-facing evidence status");
    assert!(!json.is_empty());
    assert_ne!(json, "[]");
}

struct SqliteBusyGuard {
    child: Child,
    _stdin: ChildStdin,
}

impl SqliteBusyGuard {
    fn hold(database: &Path) -> Self {
        let mut child = Command::new("sqlite3")
            .arg("-batch")
            .arg(database)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|error| panic!("start SQLite lock holder: {error}"));
        let mut stdin = child.stdin.take().expect("SQLite lock-holder stdin");
        let stdout = child.stdout.take().expect("SQLite lock-holder stdout");
        writeln!(stdin, "PRAGMA journal_mode=WAL;").expect("write SQLite WAL setup to lock holder");
        writeln!(stdin, "BEGIN IMMEDIATE;").expect("write SQLite transaction setup to lock holder");
        writeln!(stdin, "SELECT 'M03_SQLITE_BUSY_READY';").expect("write SQLite lock marker");
        stdin.flush().expect("flush SQLite lock marker");

        let mut reader = BufReader::new(stdout);
        let mut output = String::new();
        loop {
            let mut line = String::new();
            let read = reader
                .read_line(&mut line)
                .expect("read SQLite lock marker");
            if read == 0 {
                break;
            }
            output.push_str(&line);
            if output.contains("M03_SQLITE_BUSY_READY") {
                break;
            }
        }
        assert!(
            output.contains("M03_SQLITE_BUSY_READY"),
            "SQLite lock holder did not reach BEGIN IMMEDIATE: {output}"
        );
        Self {
            child,
            _stdin: stdin,
        }
    }
}

impl Drop for SqliteBusyGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn runtime_config(workspace: &Path, policy: PolicyDecision) -> RuntimeConfigSnapshot {
    let mut policies = PolicyConfig::default();
    policies.tool_defaults.write = policy;
    RuntimeConfigSnapshot {
        config_revision: harness_control_protocol::DecimalU64::new(1),
        workspace: WorkspaceConfig {
            root_path: workspace.to_string_lossy().into_owned(),
        },
        security: SecurityConfig {
            mode: SecurityMode::Normal,
            developer_test_mode: false,
        },
        runtime_limits: RuntimeLimits {
            max_concurrent_requests: 4,
            max_activity_events: 256,
            max_read_bytes: 1_048_576,
        },
        tools: ToolConfig::default(),
        bash: harness_control_protocol::BashConfig::default(),
        policies,
        search: harness_control_protocol::SearchConfig::default(),
    }
}

fn initialize_runtime_with_evidence(
    workspace: &Path,
    evidence_path: &Path,
    policy: PolicyDecision,
) -> RuntimeCore {
    let mut core = RuntimeCore::new();
    core.initialize_with_host(
        runtime_config(workspace, policy),
        Some(RuntimeHostContext {
            host_session_id: None,
            supervisor_generation_id: None,
            evidence_store: Some(harness_control_protocol::EvidenceStoreLaunchConfig {
                enabled: true,
                database_path: evidence_path.to_string_lossy().into_owned(),
                retention_days: 30,
                max_storage_mib: 256,
            }),
        }),
    )
    .expect("evidence fault must not abort Runtime initialization");
    core.acknowledge().expect("acknowledge Runtime");
    core
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn int_04() {
    let (_directory, database) = temp_database();
    let service = EvidenceService::start(
        &database,
        HistoryConfig::default(),
        context("runtime-int-04"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    let projector = SafeEvidenceProjector::new(context("runtime-int-04"));

    let allowed_event_id = Uuid::new_v4();
    let allowed_request_id = Uuid::new_v4();
    let allowed = AuditEvent {
        tool_enabled: Some(true),
        capability: Some(ToolCapability::WorkspaceRead),
        sanitized_arguments: Some(
            harness_control_protocol::SanitizedArgumentsEvidence::ReadFile {
                logical_path: "nested/allowed.txt".to_owned(),
            },
        ),
        approval_id: None,
        duration_ms: Some(17),
        result_evidence: Some(ExecutionResultEvidence::ReadFile { bytes_read: 17 }),
        event_id: allowed_event_id,
        request_id: allowed_request_id,
        timestamp: "2026-11-01T00:00:01.000Z".to_owned(),
        event_type: AuditEventType::ExecutionCompleted,
        tool_name: "read_file".to_owned(),
        lifecycle_state: AuditLifecycleState::Completed,
        workspace_id: "active".to_owned(),
        workspace_validation: WorkspaceValidation::Passed,
        policy_decision: Some(PolicyDecision::Allow),
        matched_rule_ids: vec!["allow-read".to_owned()],
        policy_reason: Some(PolicyReason::ToolDefault),
        approval_decision: None,
        outcome: AuditOutcome::Success,
        failure_category: None,
        bash: None,
    };

    let forbidden_request_id = Uuid::new_v4();
    let forbidden = AuditEvent {
        tool_enabled: Some(true),
        capability: Some(ToolCapability::DeveloperShell),
        sanitized_arguments: Some(
            harness_control_protocol::SanitizedArgumentsEvidence::Bash {
                command_preview: "M03_BASH_PREVIEW_SENTINEL".to_owned(),
            },
        ),
        approval_id: None,
        duration_ms: Some(1),
        result_evidence: None,
        event_id: Uuid::new_v4(),
        request_id: forbidden_request_id,
        timestamp: "2026-11-01T00:00:02.000Z".to_owned(),
        event_type: AuditEventType::ExecutionFailed,
        tool_name: "bash".to_owned(),
        lifecycle_state: AuditLifecycleState::Failed,
        workspace_id: "M03_API_KEY_SENTINEL M03_SECRETSTORE_SENTINEL M03_RAW_FILE_CONTENT_SENTINEL M03_WORKSPACE_PATH_SENTINEL M03_RAW_ENV_SENTINEL M03_STDOUT_SENTINEL M03_STDERR_SENTINEL M03_PROXY_CREDENTIAL_SENTINEL M03_RAW_ERROR_PATH_SENTINEL M03_ARBITRARY_JSON_SENTINEL".to_owned(),
        workspace_validation: WorkspaceValidation::NotAttempted,
        policy_decision: None,
        matched_rule_ids: vec!["M03_ARBITRARY_JSON_SENTINEL".to_owned()],
        policy_reason: None,
        approval_decision: None,
        outcome: AuditOutcome::Failed,
        failure_category: Some(AuditFailureCategory::ExecutionFailed),
        bash: Some(BashActivityEvidence {
            mode: SecurityMode::DeveloperTest,
            mode_eligible: true,
            capability: ToolCapability::DeveloperShell,
            cwd: BashWorkingDirectory::ActiveWorkspace,
            command_preview: Some(
                "M03_RAW_ENV_SENTINEL M03_STDOUT_SENTINEL M03_STDERR_SENTINEL M03_PROXY_CREDENTIAL_SENTINEL M03_RAW_ERROR_PATH_SENTINEL".to_owned(),
            ),
            classification: None,
            risk_flags: Vec::new(),
            approval_request_id: None,
            session_grant_hit: false,
            exit_code: Some(7),
            duration_ms: Some(1),
        }),
    };

    let allowed_projection = projector.project(&allowed);
    assert_eq!(allowed_projection.event_id, allowed_event_id.to_string());
    assert_eq!(
        allowed_projection.request_id,
        allowed_request_id.to_string()
    );
    assert_eq!(
        allowed_projection.sanitized_arguments,
        Some(DurableSanitizedArgumentsEvidence::ReadFile {
            logical_path: "nested/allowed.txt".to_owned()
        })
    );
    assert_eq!(
        allowed_projection.result_evidence,
        Some(ExecutionResultEvidence::ReadFile { bytes_read: 17 })
    );
    let forbidden_projection = projector.project(&forbidden);
    assert!(matches!(
        forbidden_projection.sanitized_arguments,
        Some(DurableSanitizedArgumentsEvidence::Bash { .. })
    ));
    assert!(!serde_json::to_string(&forbidden_projection)
        .unwrap()
        .contains("M03_BASH_PREVIEW_SENTINEL"));

    service.enqueue(allowed_projection.clone());
    service.enqueue(forbidden_projection);
    service.stop();

    let reader = EvidenceReader::open(&database).expect("open persisted evidence reader");
    let query = reader
        .query(&Default::default())
        .expect("query persisted evidence");
    assert_eq!(query.items.len(), 2);
    assert_eq!(query.items[0].evidence_coverage, EvidenceCoverage::Complete);
    assert!(query.items.iter().any(|item| {
        item.request_id == allowed_request_id.to_string()
            && item.policy_decision == Some(PolicyDecision::Allow)
    }));
    let detail = reader
        .get("runtime-int-04", &allowed_request_id.to_string())
        .expect("get persisted evidence")
        .expect("allowed request detail");
    let allowed_stored = detail
        .events
        .iter()
        .find(|item| item.event_id == allowed_event_id.to_string())
        .expect("allowed typed event");
    assert_eq!(
        allowed_stored.sanitized_arguments,
        Some(DurableSanitizedArgumentsEvidence::ReadFile {
            logical_path: "nested/allowed.txt".to_owned()
        })
    );
    assert_eq!(
        allowed_stored.result_evidence,
        Some(ExecutionResultEvidence::ReadFile { bytes_read: 17 })
    );
    assert_eq!(allowed_stored.matched_rule_ids, vec!["allow-read"]);

    let projection_json =
        serde_json::to_string(&allowed_projection).expect("serialize EvidenceEventV1");
    let query_json = serde_json::to_string(&query).expect("serialize history query DTO");
    let detail_json = serde_json::to_string(&detail).expect("serialize history get DTO");
    let raw_bytes = all_store_bytes(&database);
    let raw_text = String::from_utf8_lossy(&raw_bytes).into_owned();
    for value in [projection_json, query_json, detail_json, raw_text] {
        for sentinel in [
            "M03_API_KEY_SENTINEL",
            "M03_SECRETSTORE_SENTINEL",
            "M03_RAW_FILE_CONTENT_SENTINEL",
            "M03_WORKSPACE_PATH_SENTINEL",
            "M03_BASH_PREVIEW_SENTINEL",
            "M03_RAW_ENV_SENTINEL",
            "M03_STDOUT_SENTINEL",
            "M03_STDERR_SENTINEL",
            "M03_PROXY_CREDENTIAL_SENTINEL",
            "M03_RAW_ERROR_PATH_SENTINEL",
            "M03_ARBITRARY_JSON_SENTINEL",
        ] {
            assert!(
                !value.contains(sentinel),
                "forbidden sentinel leaked: {sentinel}"
            );
        }
    }
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn int_06() {
    let (_directory, database) = temp_database();
    let missing_reader = EvidenceReader::open(&database).expect("open missing reader");
    assert!(!missing_reader.is_initialized());
    assert!(!database.exists(), "query-only reader created a database");
    drop(missing_reader);

    let writer = EvidenceWriter::open(&database).expect("create v2 schema through writer");
    assert!(!writer
        .connection_is_query_only()
        .expect("writer query_only pragma"));
    drop(writer);

    let tables = sqlite_script(
        &database,
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name;",
    );
    for table in [
        "approval_history",
        "evidence_events",
        "evidence_health",
        "request_summary",
        "runtime_sessions",
        "schema_migrations",
    ] {
        assert!(
            tables.lines().any(|value| value == table),
            "missing table {table}"
        );
    }
    assert_eq!(
        sqlite_value(
            &database,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='request_spans';",
        ),
        "1"
    );
    let indexes = sqlite_script(
        &database,
        "SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%' ORDER BY name;",
    );
    for index in [
        "idx_approval_history_terminal",
        "idx_evidence_events_approval",
        "idx_evidence_events_occurred",
        "idx_evidence_events_request_order",
        "idx_request_summary_approval_first_seen",
        "idx_request_summary_first_seen",
        "idx_request_summary_outcome_first_seen",
        "idx_request_summary_policy_first_seen",
        "idx_request_summary_tool_first_seen",
        "idx_request_spans_name_duration",
        "idx_request_spans_request_order",
        "idx_request_spans_trace_order",
    ] {
        assert!(
            indexes.lines().any(|value| value == index),
            "missing index {index}"
        );
    }
    assert_eq!(
        sqlite_value(&database, "PRAGMA foreign_keys = ON; PRAGMA foreign_keys;"),
        "1"
    );
    for (table, primary_key_columns) in [
        ("runtime_sessions", "1"),
        ("request_summary", "2"),
        ("evidence_events", "1"),
        ("approval_history", "2"),
        ("evidence_health", "1"),
    ] {
        assert_eq!(
            sqlite_value(
                &database,
                &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE pk > 0;"),
            ),
            primary_key_columns,
            "unexpected primary-key shape for {table}"
        );
    }
    assert_eq!(
        sqlite_value(
            &database,
            "SELECT COUNT(*) FROM pragma_foreign_key_list('request_summary') WHERE \"table\"='runtime_sessions' AND on_delete='CASCADE';",
        ),
        "1"
    );
    assert_eq!(
        sqlite_value(
            &database,
            "SELECT COUNT(*) FROM pragma_foreign_key_list('evidence_events') WHERE \"table\"='request_summary' AND on_delete='CASCADE';",
        ),
        "2"
    );
    assert_eq!(
        sqlite_value(
            &database,
            "SELECT COUNT(*) FROM pragma_foreign_key_list('approval_history') WHERE \"table\"='request_summary' AND on_delete='CASCADE';",
        ),
        "2"
    );
    assert_eq!(SCHEMA_VERSION, 2);
    assert_eq!(computed_schema_checksum(), SCHEMA_CHECKSUM);
    assert_eq!(
        sqlite_value(
            &database,
            "SELECT checksum FROM schema_migrations WHERE version=2;"
        ),
        SCHEMA_CHECKSUM
    );
    assert_eq!(sqlite_value(&database, "PRAGMA integrity_check;"), "ok");

    let (_newer_directory, newer_database) = temp_database();
    drop(EvidenceWriter::open(&newer_database).expect("create newer-schema fixture"));
    sqlite_script(
        &newer_database,
        "INSERT INTO schema_migrations(version, checksum, applied_at_utc) VALUES (3, 'future', '2099-01-01T00:00:00.000Z');",
    );
    let newer_before = fs::read(&newer_database).expect("read newer schema source");
    assert!(matches!(
        EvidenceWriter::open(&newer_database),
        Err(StoreError::IncompatibleSchema { found: 3 })
    ));
    assert!(matches!(
        EvidenceReader::open(&newer_database),
        Err(StoreError::IncompatibleSchema { found: 3 })
    ));
    assert_eq!(fs::read(&newer_database).unwrap(), newer_before);

    let (_checksum_directory, checksum_database) = temp_database();
    drop(EvidenceWriter::open(&checksum_database).expect("create checksum fixture"));
    sqlite_script(
        &checksum_database,
        "UPDATE schema_migrations SET checksum='tampered' WHERE version=2;",
    );
    let checksum_before = fs::read(&checksum_database).expect("read checksum source");
    assert!(matches!(
        EvidenceWriter::open(&checksum_database),
        Err(StoreError::SchemaChecksumMismatch { .. })
    ));
    assert!(matches!(
        EvidenceReader::open(&checksum_database),
        Err(StoreError::SchemaChecksumMismatch { .. })
    ));
    assert_eq!(fs::read(&checksum_database).unwrap(), checksum_before);

    let (_corrupt_directory, corrupt_database) = temp_database();
    let corrupt_bytes = b"not an SQLite database".to_vec();
    fs::write(&corrupt_database, &corrupt_bytes).expect("write corrupt fixture");
    assert!(matches!(
        EvidenceWriter::open(&corrupt_database),
        Err(StoreError::Corrupt { .. })
    ));
    assert_eq!(fs::read(&corrupt_database).unwrap(), corrupt_bytes);

    let (_migrated_directory, migrated_database) = temp_database();
    sqlite_script(
        &migrated_database,
        "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at_utc TEXT NOT NULL); INSERT INTO schema_migrations VALUES(0, 'legacy-v0', '1970-01-01T00:00:00.000Z');",
    );
    drop(EvidenceWriter::open(&migrated_database).expect("transactional v0 migration"));
    assert!(migration_backup_path(&migrated_database).exists());
    assert_eq!(
        sqlite_value(
            &migrated_database,
            "SELECT checksum FROM schema_migrations WHERE version=2;"
        ),
        SCHEMA_CHECKSUM
    );
    assert_eq!(
        sqlite_value(&migrated_database, "PRAGMA integrity_check;"),
        "ok"
    );

    let (_failed_directory, failed_database) = temp_database();
    sqlite_script(
        &failed_database,
        "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at_utc TEXT NOT NULL); INSERT INTO schema_migrations VALUES(0, 'legacy-v0', '1970-01-01T00:00:00.000Z'); CREATE TABLE runtime_sessions(bad INTEGER);",
    );
    let failed_before = fs::read(&failed_database).expect("read migration source");
    let backup = match EvidenceWriter::open(&failed_database) {
        Err(StoreError::MigrationFailed { backup_path, .. }) => backup_path,
        other => panic!("expected preserved migration failure, got {other:?}"),
    };
    assert!(backup.exists());
    assert_eq!(fs::read(&failed_database).unwrap(), failed_before);
    assert_eq!(fs::read(&backup).unwrap(), failed_before);
    assert_eq!(
        sqlite_value(
            &failed_database,
            "SELECT MAX(version) FROM schema_migrations;"
        ),
        "0"
    );
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn int_07() {
    let (_directory, database) = temp_database();
    let mut writer = EvidenceWriter::open(&database).expect("open Runtime writer");
    assert!(!writer.connection_is_query_only().unwrap());
    assert_eq!(sqlite_value(&database, "PRAGMA journal_mode;"), "wal");

    let lock_path = StorePaths::from_database(&database).lock;
    assert!(matches!(
        WriterLock::try_acquire(&lock_path),
        Err(StoreError::WriterBusy { .. })
    ));
    let reader = EvidenceReader::open(&database).expect("open concurrent history reader");
    assert!(reader.is_initialized());
    assert!(reader.connection_is_query_only().unwrap());

    for index in 0..4 {
        writer
            .record_event(&success_event(
                "runtime-int-07",
                &format!("request-{index}"),
                &format!("event-{index}"),
                &timestamp(index),
            ))
            .expect("commit while query-only reader is open");
        let result = reader
            .query(&Default::default())
            .expect("read concurrently committed row");
        assert_eq!(result.items.len(), index + 1);
    }
    assert!(matches!(
        EvidenceWriter::open(&database),
        Err(StoreError::WriterBusy { .. })
    ));
    assert!(matches!(
        OfflineMaintenance::open(&database),
        Err(StoreError::WriterBusy { .. })
    ));

    drop(reader);
    drop(writer);
    let mut maintenance = OfflineMaintenance::open(&database).expect("maintenance after stop");
    let cleared = maintenance
        .clear_all()
        .expect("maintenance clear after lock release");
    assert_eq!(cleared.deleted_request_count, 4);
    assert_eq!(cleared.deleted_event_count, 4);
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn int_08() {
    let trigger_cases = [
        (
            "summary_insert",
            "CREATE TRIGGER m03_fail BEFORE INSERT ON request_summary BEGIN SELECT RAISE(ABORT, 'm03-summary-insert'); END;",
            event(
                "runtime-int-08",
                "request-summary-insert",
                "event-summary-insert",
                EvidenceStage::ExecutionCompleted,
                HistoryOutcome::Success,
                "2026-11-01T00:01:00.000Z",
            ),
        ),
        (
            "event_insert",
            "CREATE TRIGGER m03_fail BEFORE INSERT ON evidence_events BEGIN SELECT RAISE(ABORT, 'm03-event-insert'); END;",
            event(
                "runtime-int-08",
                "request-event-insert",
                "event-event-insert",
                EvidenceStage::ExecutionCompleted,
                HistoryOutcome::Success,
                "2026-11-01T00:01:01.000Z",
            ),
        ),
        (
            "summary_update",
            "CREATE TRIGGER m03_fail BEFORE UPDATE ON request_summary BEGIN SELECT RAISE(ABORT, 'm03-summary-update'); END;",
            event(
                "runtime-int-08",
                "request-summary-update",
                "event-summary-update",
                EvidenceStage::ExecutionCompleted,
                HistoryOutcome::Success,
                "2026-11-01T00:01:02.000Z",
            ),
        ),
        (
            "approval_insert",
            "CREATE TRIGGER m03_fail BEFORE INSERT ON approval_history BEGIN SELECT RAISE(ABORT, 'm03-approval-insert'); END;",
            {
                let mut value = event(
                    "runtime-int-08",
                    "request-approval-insert",
                    "event-approval-insert",
                    EvidenceStage::ApprovalResolved,
                    HistoryOutcome::InProgress,
                    "2026-11-01T00:01:03.000Z",
                );
                value.approval_id = Some("approval-08".to_owned());
                value.approval_decision = Some(ApprovalDecision::AllowOnce);
                value
            },
        ),
        (
            "health_update",
            "CREATE TRIGGER m03_fail BEFORE UPDATE ON evidence_health BEGIN SELECT RAISE(ABORT, 'm03-health-update'); END;",
            event(
                "runtime-int-08",
                "request-health-update",
                "event-health-update",
                EvidenceStage::ExecutionCompleted,
                HistoryOutcome::Success,
                "2026-11-01T00:01:04.000Z",
            ),
        ),
    ];

    for (name, trigger, value) in trigger_cases {
        let (_directory, database) = temp_database();
        let mut writer = EvidenceWriter::open(&database).expect("open trigger-fault writer");
        sqlite_script(&database, trigger);
        assert!(
            writer.record_event(&value).is_err(),
            "fault at {name} unexpectedly committed"
        );
        drop(writer);
        assert_eq!(
            sqlite_value(&database, "SELECT COUNT(*) FROM runtime_sessions;"),
            "0"
        );
        assert_eq!(
            sqlite_value(&database, "SELECT COUNT(*) FROM request_summary;"),
            "0"
        );
        assert_eq!(
            sqlite_value(&database, "SELECT COUNT(*) FROM evidence_events;"),
            "0"
        );
        assert_eq!(
            sqlite_value(&database, "SELECT COUNT(*) FROM approval_history;"),
            "0"
        );
        assert_foreign_keys_clean(&database);
    }

    let (_directory, database) = temp_database();
    let mut writer = EvidenceWriter::open(&database).expect("open atomicity writer");
    let success = success_event(
        "runtime-int-08",
        "request-success",
        "event-success",
        "2026-11-01T00:02:00.000Z",
    );
    writer.record_event(&success).expect("commit success event");
    let mut duplicate = success.clone();
    duplicate.stage = EvidenceStage::RequestDenied;
    duplicate.lifecycle_state = EvidenceLifecycleState::Denied;
    duplicate.outcome = HistoryOutcome::Denied;
    assert!(!writer.record_event(&duplicate).unwrap().inserted);

    let deny = event(
        "runtime-int-08",
        "request-deny",
        "event-deny",
        EvidenceStage::RequestDenied,
        HistoryOutcome::Denied,
        "2026-11-01T00:02:01.000Z",
    );
    writer.record_event(&deny).expect("commit deny event");

    let mut approval_required = event(
        "runtime-int-08",
        "request-approval",
        "event-approval-required",
        EvidenceStage::ApprovalRequired,
        HistoryOutcome::ApprovalRequired,
        "2026-11-01T00:02:02.000Z",
    );
    approval_required.approval_id = Some("approval-idempotent".to_owned());
    writer
        .record_event(&approval_required)
        .expect("commit approval-required event");
    let mut approval_terminal = event(
        "runtime-int-08",
        "request-approval",
        "event-approval-terminal",
        EvidenceStage::ApprovalResolved,
        HistoryOutcome::InProgress,
        "2026-11-01T00:02:03.000Z",
    );
    approval_terminal.approval_id = Some("approval-idempotent".to_owned());
    approval_terminal.approval_decision = Some(ApprovalDecision::AllowOnce);
    writer
        .record_event(&approval_terminal)
        .expect("commit terminal approval event");
    let mut duplicate_terminal = approval_terminal.clone();
    duplicate_terminal.event_id = "event-approval-terminal-duplicate".to_owned();
    writer
        .record_event(&duplicate_terminal)
        .expect("commit duplicate terminal approval event");
    writer.checkpoint().expect("checkpoint atomicity database");
    drop(writer);

    let reader = EvidenceReader::open(&database).expect("open atomicity reader");
    let success_detail = reader
        .get("runtime-int-08", "request-success")
        .unwrap()
        .unwrap();
    assert_eq!(success_detail.events.len(), 1);
    assert_eq!(
        success_detail.summary.terminal_state,
        harness_control_protocol::HistoryTerminalState::Succeeded
    );
    let deny_detail = reader
        .get("runtime-int-08", "request-deny")
        .unwrap()
        .unwrap();
    assert_eq!(deny_detail.events.len(), 1);
    assert_eq!(
        deny_detail.summary.terminal_state,
        harness_control_protocol::HistoryTerminalState::Denied
    );
    let approval_detail = reader
        .get("runtime-int-08", "request-approval")
        .unwrap()
        .unwrap();
    assert_eq!(approval_detail.events.len(), 3);
    assert_eq!(
        approval_detail
            .terminal_approval
            .as_ref()
            .unwrap()
            .approval_id,
        "approval-idempotent"
    );
    assert_eq!(
        sqlite_value(&database, "SELECT COUNT(*) FROM approval_history;"),
        "1"
    );
    assert_foreign_keys_clean(&database);
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn int_09() {
    let (_directory, database) = temp_database();
    let service = EvidenceService::start(
        &database,
        HistoryConfig::default(),
        context("runtime-int-09"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert!(wait_until(Duration::from_secs(2), || database.exists()));
    let retained_request = event(
        "runtime-int-09",
        "retained-request",
        "retained-request-received",
        EvidenceStage::RequestReceived,
        HistoryOutcome::InProgress,
        "2026-11-01T00:03:00.000Z",
    );
    service.enqueue(retained_request);
    assert!(wait_until(Duration::from_secs(2), || {
        EvidenceReader::open(&database)
            .ok()
            .and_then(|reader| reader.get("runtime-int-09", "retained-request").ok())
            .flatten()
            .is_some()
    }));

    let busy = SqliteBusyGuard::hold(&database);
    let started = Instant::now();
    for index in 0..1_400 {
        service.enqueue(event(
            "runtime-int-09",
            &format!("overflow-request-{index}"),
            &format!("overflow-event-{index}"),
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
            &format!("2026-11-01T00:04:{:02}.000Z", index % 60),
        ));
    }
    let enqueue_duration = started.elapsed();
    assert!(
        enqueue_duration < Duration::from_secs(1),
        "producer enqueue blocked for {:?}",
        enqueue_duration
    );
    assert!(
        wait_until(Duration::from_secs(2), || service.status().drop_count > 256),
        "queue did not overflow with a stalled SQLite writer"
    );
    for index in 0..8 {
        service.enqueue(event(
            "runtime-int-09",
            "retained-request",
            &format!("retained-loss-{index}"),
            EvidenceStage::ExecutionStarted,
            HistoryOutcome::InProgress,
            &format!("2026-11-01T00:05:{:02}.000Z", index),
        ));
    }
    let saturated = service.status();
    assert_eq!(saturated.queue_capacity, 1024);
    assert!(saturated.drop_count > 256);
    assert!(saturated.last_drop_at_utc.is_some());
    assert_eq!(saturated.state, EvidenceStoreState::Degraded);
    assert_eq!(
        saturated.last_error_code,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceQueueOverflow)
    );

    let service_source = include_str!("evidence/service.rs");
    assert!(service_source.contains("const EVIDENCE_BATCH_MAX: usize = 64;"));
    assert!(
        service_source.contains("const EVIDENCE_BATCH_WAIT: Duration = Duration::from_millis(50);")
    );
    drop(busy);
    assert!(wait_until(Duration::from_secs(6), || {
        let status = service.status();
        status.queue_depth == 0 && status.coverage_unknown_since_utc.is_some()
    }));
    let drained_status = service.status();
    assert!(drained_status.coverage_unknown_since_utc.is_some());
    assert!(drained_status.drop_count > 256);
    service.stop();

    let reader = EvidenceReader::open(&database).expect("open overflow reader");
    let detail = reader
        .get("runtime-int-09", "retained-request")
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.summary.evidence_coverage,
        EvidenceCoverage::Incomplete
    );
    assert_eq!(
        detail.summary.terminal_state,
        harness_control_protocol::HistoryTerminalState::Incomplete
    );
    assert_eq!(detail.summary.outcome, None);
    assert_eq!(detail.events.len(), 1);
    assert_eq!(detail.events[0].stage, EvidenceStage::RequestReceived);
    assert!(!detail.events.iter().any(|item| {
        matches!(
            item.stage,
            EvidenceStage::ExecutionCompleted
                | EvidenceStage::ExecutionFailed
                | EvidenceStage::RequestDenied
        )
    }));
    assert_eq!(
        sqlite_value(
            &database,
            "SELECT drop_count FROM evidence_health WHERE id=1;"
        ),
        drained_status.drop_count.to_string()
    );
    assert_eq!(
        detail.notice.as_ref().map(|notice| notice.code),
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceQueueOverflow)
    );
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn int_10() {
    let (_normal_directory, normal_database) = temp_database();
    let normal_service = EvidenceService::start(
        &normal_database,
        HistoryConfig::default(),
        context("runtime-int-10-normal"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    let normal_started = Instant::now();
    normal_service.stop();
    assert!(normal_started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        sqlite_value(
            &normal_database,
            "SELECT end_reason FROM runtime_sessions WHERE runtime_id='runtime-int-10-normal';",
        ),
        "normal_stop"
    );

    let (_queued_directory, queued_database) = temp_database();
    let queued_service = EvidenceService::start(
        &queued_database,
        HistoryConfig::default(),
        context("runtime-int-10-queued"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    queued_service.enqueue(event(
        "runtime-int-10-queued",
        "queued-request",
        "queued-event",
        EvidenceStage::RequestReceived,
        HistoryOutcome::InProgress,
        "2026-11-01T00:06:00.000Z",
    ));
    let queued_started = Instant::now();
    queued_service.stop();
    assert!(queued_started.elapsed() < Duration::from_secs(2));
    let queued_reader = EvidenceReader::open(&queued_database).expect("open drained evidence");
    let queued_detail = queued_reader
        .get("runtime-int-10-queued", "queued-request")
        .unwrap()
        .unwrap();
    assert_eq!(
        queued_detail.summary.terminal_state,
        harness_control_protocol::HistoryTerminalState::Incomplete
    );
    assert_eq!(queued_detail.events.len(), 1);
    assert!(!queued_detail.events.iter().any(|item| {
        matches!(
            item.stage,
            EvidenceStage::ExecutionCompleted
                | EvidenceStage::ExecutionFailed
                | EvidenceStage::RequestDenied
        )
    }));

    let (_timeout_directory, timeout_database) = temp_database();
    let timeout_service = EvidenceService::start(
        &timeout_database,
        HistoryConfig::default(),
        context("runtime-int-10-timeout"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert!(wait_until(Duration::from_secs(2), || timeout_database.exists()));
    let busy = SqliteBusyGuard::hold(&timeout_database);
    timeout_service.enqueue(event(
        "runtime-int-10-timeout",
        "timeout-request",
        "timeout-event",
        EvidenceStage::RequestReceived,
        HistoryOutcome::InProgress,
        "2026-11-01T00:07:00.000Z",
    ));
    let timeout_started = Instant::now();
    timeout_service.stop();
    let timeout_elapsed = timeout_started.elapsed();
    assert!(timeout_elapsed >= Duration::from_secs(1));
    let timeout_status = timeout_service.status();
    assert_eq!(timeout_status.state, EvidenceStoreState::Degraded);
    assert!(matches!(
        timeout_status.last_error_code,
        Some(
            harness_control_protocol::EvidenceErrorCode::EvidenceIo
                | harness_control_protocol::EvidenceErrorCode::EvidenceQueueOverflow
        )
    ));
    assert!(include_str!("evidence/service.rs")
        .contains("const EVIDENCE_STOP_DRAIN: Duration = Duration::from_secs(2);"));
    drop(busy);
    assert!(wait_until(Duration::from_secs(6), || {
        WriterLock::try_acquire(StorePaths::from_database(&timeout_database).lock)
            .map(|lock| {
                drop(lock);
                true
            })
            .unwrap_or(false)
    }));

    let (_unclean_directory, unclean_database) = temp_database();
    {
        let mut writer = EvidenceWriter::open(&unclean_database).expect("open unclean writer");
        writer
            .open_session(&chatgpt_harness_evidence_store::RuntimeSession {
                runtime_id: "runtime-int-10-unclean".to_owned(),
                host_session_id: None,
                supervisor_generation_id: None,
                started_at_utc: "2026-11-01T00:00:00.000Z".to_owned(),
                runtime_version: "qa".to_owned(),
                config_revision: "qa".to_owned(),
            })
            .unwrap();
        writer
            .record_event(&event(
                "runtime-int-10-unclean",
                "unclean-request",
                "unclean-event",
                EvidenceStage::RequestReceived,
                HistoryOutcome::InProgress,
                "2026-11-01T00:08:00.000Z",
            ))
            .unwrap();
        // Deliberately drop without close_session: this is the real unclean-restart boundary.
    }
    drop(EvidenceWriter::open(&unclean_database).expect("recover unclean writer"));
    let unclean_reader = EvidenceReader::open(&unclean_database).expect("open recovered reader");
    let unclean_detail = unclean_reader
        .get("runtime-int-10-unclean", "unclean-request")
        .unwrap()
        .unwrap();
    assert_eq!(
        unclean_detail.summary.terminal_state,
        harness_control_protocol::HistoryTerminalState::Incomplete
    );
    assert_eq!(unclean_detail.events.len(), 1);
    assert_eq!(
        sqlite_value(
            &unclean_database,
            "SELECT end_reason FROM runtime_sessions WHERE runtime_id='runtime-int-10-unclean';",
        ),
        "unclean_restart"
    );
    assert!(!unclean_detail.events.iter().any(|item| {
        matches!(
            item.stage,
            EvidenceStage::ExecutionCompleted
                | EvidenceStage::ExecutionFailed
                | EvidenceStage::RequestDenied
        )
    }));
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn int_16() {
    let (_directory, database) = temp_database();
    let workspace = tempfile::tempdir().expect("create retention workspace");
    let workspace_file = workspace.path().join("must-survive.txt");
    fs::write(&workspace_file, b"workspace sentinel").expect("write workspace sentinel");
    let mut writer = EvidenceWriter::open(&database).expect("open retention writer");

    writer
        .record_event(&success_event(
            "runtime-age-success",
            "age-success",
            "age-success-event",
            "2020-01-01T00:00:00.000Z",
        ))
        .unwrap();
    let mut old_terminal_approval = event(
        "runtime-age-approval",
        "age-approval",
        "age-approval-event",
        EvidenceStage::ApprovalResolved,
        HistoryOutcome::Denied,
        "2020-01-01T00:00:00.500Z",
    );
    old_terminal_approval.approval_id = Some("age-approval-id".to_owned());
    old_terminal_approval.approval_decision = Some(ApprovalDecision::Deny);
    writer.record_event(&old_terminal_approval).unwrap();
    writer
        .record_event(&event(
            "runtime-age-incomplete",
            "age-incomplete",
            "age-incomplete-event",
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
            "2020-01-01T00:00:01.000Z",
        ))
        .unwrap();
    writer
        .close_session(
            "runtime-age-incomplete",
            "2020-01-01T00:00:02.000Z",
            "test_incomplete",
        )
        .unwrap();
    writer
        .record_event(&event(
            "runtime-current",
            "current-open",
            "current-open-event",
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
            "2026-11-01T00:00:00.000Z",
        ))
        .unwrap();
    let age_report = writer
        .apply_retention(
            "2026-11-02T00:00:00.000Z",
            &RetentionPolicy {
                retention_days: 1,
                max_storage_mib: 32,
            },
        )
        .unwrap();
    assert_eq!(age_report.age_cleanup_requests, 3);
    assert!(age_report.deleted_event_count >= 3);
    assert_eq!(age_report.deleted_approval_count, 1);

    let payload = "m03-capacity-payload".repeat(1_200);
    let mut batch = Vec::with_capacity(64);
    for index in 0..1_800usize {
        let mut value = success_event(
            "runtime-capacity-old",
            &format!("capacity-old-{index:04}"),
            &format!("capacity-old-event-{index:04}"),
            &format!(
                "2020-01-02T00:{:02}:{:02}.000Z",
                (index / 60) % 60,
                index % 60
            ),
        );
        value.sanitized_arguments = Some(DurableSanitizedArgumentsEvidence::Glob {
            pattern: payload.clone(),
        });
        batch.push(value);
        if batch.len() == 64 {
            writer.record_events(&batch).expect("write capacity batch");
            batch.clear();
        }
    }
    if !batch.is_empty() {
        writer
            .record_events(&batch)
            .expect("write final capacity batch");
    }
    let before_capacity = file_footprint_bytes(&database).unwrap();
    assert!(
        before_capacity > 32 * 1024 * 1024,
        "fixture did not exceed 32 MiB: {before_capacity}"
    );
    let capacity_report = writer
        .apply_retention(
            "2026-11-02T00:00:00.000Z",
            &RetentionPolicy {
                retention_days: 3650,
                max_storage_mib: 32,
            },
        )
        .unwrap();
    assert!(capacity_report.capacity_cleanup_requests > 0);
    assert!(!capacity_report.capacity_degraded);
    assert!(capacity_report.footprint_bytes <= 32 * 1024 * 1024 * 90 / 100);
    assert_eq!(fs::read(&workspace_file).unwrap(), b"workspace sentinel");

    let reader_after_capacity = EvidenceReader::open(&database).unwrap();
    assert!(reader_after_capacity
        .get("runtime-current", "current-open")
        .unwrap()
        .is_some());
    assert!(reader_after_capacity
        .get("runtime-age-success", "age-success")
        .unwrap()
        .is_none());
    assert!(reader_after_capacity
        .get("runtime-age-incomplete", "age-incomplete")
        .unwrap()
        .is_none());
    assert!(reader_after_capacity
        .get("runtime-age-approval", "age-approval")
        .unwrap()
        .is_none());
    drop(reader_after_capacity);
    assert_foreign_keys_clean(&database);
    assert_eq!(
        sqlite_value(
            &database,
            "SELECT COUNT(*) FROM evidence_events AS e LEFT JOIN request_summary AS s ON s.runtime_id=e.runtime_id AND s.request_id=e.request_id WHERE s.runtime_id IS NULL;",
        ),
        "0"
    );

    let open_payload = "m03-open-capacity-payload".repeat(1_100);
    let mut open_batch = Vec::with_capacity(64);
    for index in 0..1_600usize {
        let mut value = event(
            "runtime-uncapped-open",
            "uncapped-open",
            &format!("uncapped-open-event-{index:04}"),
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
            &format!(
                "2026-11-01T01:{:02}:{:02}.000Z",
                (index / 60) % 60,
                index % 60
            ),
        );
        value.sanitized_arguments = Some(DurableSanitizedArgumentsEvidence::Glob {
            pattern: open_payload.clone(),
        });
        open_batch.push(value);
        if open_batch.len() == 64 {
            writer
                .record_events(&open_batch)
                .expect("write open capacity batch");
            open_batch.clear();
        }
    }
    if !open_batch.is_empty() {
        writer
            .record_events(&open_batch)
            .expect("write final open capacity batch");
    }
    let degraded_report = writer
        .apply_retention(
            "2026-11-02T00:00:00.000Z",
            &RetentionPolicy {
                retention_days: 3650,
                max_storage_mib: 32,
            },
        )
        .unwrap();
    assert!(degraded_report.capacity_degraded);
    assert!(degraded_report.footprint_bytes > 32 * 1024 * 1024);
    let degraded_status = writer.status().unwrap();
    assert_status(
        &degraded_status,
        EvidenceStoreState::Degraded,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceCapacityDegraded),
    );
    assert_eq!(HistoryConfig::default().enabled, true);
    assert_eq!(HistoryConfig::default().retention_days, 30);
    assert_eq!(HistoryConfig::default().max_storage_mib, 256);
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn int_17() {
    let (_missing_directory, missing_database) = temp_database();
    let missing_reader = EvidenceReader::open(&missing_database).expect("open missing reader");
    assert!(!missing_reader.is_initialized());
    assert_status(
        &missing_reader.status().unwrap(),
        EvidenceStoreState::Unavailable,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceIo),
    );

    let (_disabled_directory, disabled_database) = temp_database();
    let disabled = EvidenceService::disabled(HistoryConfig {
        enabled: false,
        ..HistoryConfig::default()
    });
    assert_status(&disabled.status(), EvidenceStoreState::Disabled, None);
    assert!(!disabled_database.exists());

    let (_healthy_directory, healthy_database) = temp_database();
    let healthy = EvidenceService::start(
        &healthy_database,
        HistoryConfig::default(),
        context("runtime-int-17-healthy"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert!(wait_until(Duration::from_secs(2), || {
        healthy.status().state == EvidenceStoreState::Healthy
    }));
    assert_status(&healthy.status(), EvidenceStoreState::Healthy, None);
    healthy.stop();

    let (_ownership_directory, ownership_database) = temp_database();
    let ownership_lock =
        WriterLock::try_acquire(StorePaths::from_database(&ownership_database).lock)
            .expect("hold writer ownership fault");
    let ownership_service = EvidenceService::start(
        &ownership_database,
        HistoryConfig::default(),
        context("runtime-int-17-ownership"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert_status(
        &ownership_service.status(),
        EvidenceStoreState::Degraded,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceWriterOwnership),
    );
    drop(ownership_service);
    drop(ownership_lock);

    let (_busy_directory, busy_database) = temp_database();
    drop(EvidenceWriter::open(&busy_database).expect("create busy fixture"));
    let busy = SqliteBusyGuard::hold(&busy_database);
    let busy_service = EvidenceService::start(
        &busy_database,
        HistoryConfig::default(),
        context("runtime-int-17-busy"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert_status(
        &busy_service.status(),
        EvidenceStoreState::Busy,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceBusy),
    );
    drop(busy_service);
    drop(busy);

    let (io_directory, _io_database) = temp_database();
    let blocked_parent = io_directory.path().join("not-a-directory");
    fs::write(&blocked_parent, b"blocking file").expect("create deterministic IO fault");
    let io_database = blocked_parent.join("history.sqlite3");
    let io_service = EvidenceService::start(
        &io_database,
        HistoryConfig::default(),
        context("runtime-int-17-io"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert_status(
        &io_service.status(),
        EvidenceStoreState::Degraded,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceIo),
    );
    drop(io_service);

    let (_corrupt_directory, corrupt_database) = temp_database();
    fs::write(&corrupt_database, b"corrupt M03 database").expect("create corrupt DB");
    let corrupt_before = fs::read(&corrupt_database).unwrap();
    let corrupt_service = EvidenceService::start(
        &corrupt_database,
        HistoryConfig::default(),
        context("runtime-int-17-corrupt"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert_status(
        &corrupt_service.status(),
        EvidenceStoreState::Corrupt,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceCorrupt),
    );
    assert_eq!(fs::read(&corrupt_database).unwrap(), corrupt_before);
    drop(corrupt_service);

    let (_incompatible_directory, incompatible_database) = temp_database();
    drop(EvidenceWriter::open(&incompatible_database).expect("create incompatible DB"));
    sqlite_script(
        &incompatible_database,
        "INSERT INTO schema_migrations(version, checksum, applied_at_utc) VALUES(3, 'future', '2099-01-01T00:00:00.000Z');",
    );
    let incompatible_before = fs::read(&incompatible_database).unwrap();
    let incompatible_service = EvidenceService::start(
        &incompatible_database,
        HistoryConfig::default(),
        context("runtime-int-17-incompatible"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert_status(
        &incompatible_service.status(),
        EvidenceStoreState::Incompatible,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceIncompatibleSchema),
    );
    assert_eq!(
        fs::read(&incompatible_database).unwrap(),
        incompatible_before
    );
    drop(incompatible_service);

    let (_checksum_directory, checksum_database) = temp_database();
    drop(EvidenceWriter::open(&checksum_database).expect("create checksum DB"));
    sqlite_script(
        &checksum_database,
        "UPDATE schema_migrations SET checksum='wrong' WHERE version=2;",
    );
    let checksum_service = EvidenceService::start(
        &checksum_database,
        HistoryConfig::default(),
        context("runtime-int-17-checksum"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert_status(
        &checksum_service.status(),
        EvidenceStoreState::Incompatible,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceIncompatibleSchema),
    );
    drop(checksum_service);

    let (_migration_directory, migration_database) = temp_database();
    sqlite_script(
        &migration_database,
        "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at_utc TEXT NOT NULL); INSERT INTO schema_migrations VALUES(0, 'legacy-v0', '1970-01-01T00:00:00.000Z'); CREATE TABLE runtime_sessions(bad INTEGER);",
    );
    let migration_service = EvidenceService::start(
        &migration_database,
        HistoryConfig::default(),
        context("runtime-int-17-migration"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert_status(
        &migration_service.status(),
        EvidenceStoreState::Degraded,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceMigrationFailed),
    );
    assert!(migration_backup_path(&migration_database).exists());
    drop(migration_service);

    let (_queue_directory, queue_database) = temp_database();
    let queue_service = EvidenceService::start(
        &queue_database,
        HistoryConfig::default(),
        context("runtime-int-17-queue"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    assert!(wait_until(Duration::from_secs(2), || queue_database.exists()));
    let queue_busy = SqliteBusyGuard::hold(&queue_database);
    for index in 0..1_400 {
        queue_service.enqueue(event(
            "runtime-int-17-queue",
            &format!("queue-request-{index}"),
            &format!("queue-event-{index}"),
            EvidenceStage::RequestReceived,
            HistoryOutcome::InProgress,
            &timestamp(index),
        ));
    }
    assert!(wait_until(Duration::from_secs(2), || {
        queue_service.status().drop_count > 0
    }));
    let queue_status = queue_service.status();
    assert_status(
        &queue_status,
        EvidenceStoreState::Degraded,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceQueueOverflow),
    );
    drop(queue_busy);
    queue_service.stop();

    let (_capacity_directory, capacity_database) = temp_database();
    let capacity_service = EvidenceService::start(
        &capacity_database,
        HistoryConfig {
            enabled: true,
            retention_days: 3650,
            max_storage_mib: 32,
        },
        context("runtime-int-17-capacity"),
        "2026-11-01T00:00:00.000Z".to_owned(),
    );
    let capacity_payload = "m03-service-open-capacity-payload".repeat(900);
    for batch_index in 0..48usize {
        for item_index in 0..32usize {
            let index = batch_index * 32 + item_index;
            let mut value = event(
                "runtime-int-17-capacity",
                "open-capacity-request",
                &format!("open-capacity-event-{index:04}"),
                EvidenceStage::RequestReceived,
                HistoryOutcome::InProgress,
                &format!(
                    "2026-11-01T02:{:02}:{:02}.000Z",
                    (index / 60) % 60,
                    index % 60
                ),
            );
            value.sanitized_arguments = Some(DurableSanitizedArgumentsEvidence::Glob {
                pattern: capacity_payload.clone(),
            });
            capacity_service.enqueue(value);
        }
        assert!(wait_until(Duration::from_secs(2), || {
            capacity_service.status().queue_depth == 0
        }));
    }
    assert!(wait_until(Duration::from_secs(8), || {
        capacity_service.status().last_error_code
            == Some(harness_control_protocol::EvidenceErrorCode::EvidenceCapacityDegraded)
    }));
    let capacity_status = capacity_service.status();
    assert_status(
        &capacity_status,
        EvidenceStoreState::Degraded,
        Some(harness_control_protocol::EvidenceErrorCode::EvidenceCapacityDegraded),
    );
    capacity_service.stop();

    let (policy_directory, _policy_database) = temp_database();
    let workspace = policy_directory.path().join("workspace");
    fs::create_dir_all(&workspace).expect("create policy workspace");
    let policy_database = policy_directory.path().join("policy-history.sqlite3");
    let policy_lock = WriterLock::try_acquire(StorePaths::from_database(&policy_database).lock)
        .expect("hold policy evidence ownership fault");
    let allowed_core =
        initialize_runtime_with_evidence(&workspace, &policy_database, PolicyDecision::Allow);
    allowed_core
        .write(&WriteRequest::new(
            "allowed-under-evidence-fault.txt",
            "allowed",
        ))
        .expect("Policy/Tool allow semantics changed under evidence fault");
    assert!(workspace.join("allowed-under-evidence-fault.txt").exists());
    drop(policy_lock);

    let blocked_parent = policy_directory.path().join("policy-parent-file");
    fs::write(&blocked_parent, b"not a directory").expect("create policy IO fault");
    let blocked_database = blocked_parent.join("history.sqlite3");
    let denied_core =
        initialize_runtime_with_evidence(&workspace, &blocked_database, PolicyDecision::Deny);
    assert!(denied_core
        .write(&WriteRequest::new(
            "denied-under-evidence-fault.txt",
            "denied"
        ))
        .is_err());
    assert!(!workspace.join("denied-under-evidence-fault.txt").exists());
}
