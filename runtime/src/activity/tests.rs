use super::*;
use crate::audit::WorkspaceValidation;
use crate::state::RuntimeCore;
use crate::tools::{GlobRequest, ReadFileRequest};
use harness_control_protocol::{
    DecimalU64, RuntimeConfigSnapshot, RuntimeLimits, SecurityConfig, SecurityMode, WorkspaceConfig,
};
use std::{fs, path::PathBuf, thread};

fn event(id: u128) -> AuditEvent {
    AuditEvent {
        event_id: Uuid::from_u128(id),
        request_id: Uuid::from_u128(900),
        timestamp: "2026-09-08T01:02:03.004Z".into(),
        event_type: AuditEventType::RequestReceived,
        tool_name: "read_file".into(),
        lifecycle_state: AuditLifecycleState::Received,
        workspace_id: "active".into(),
        workspace_validation: WorkspaceValidation::NotAttempted,
        policy_decision: None,
        matched_rule_ids: vec![],
        outcome: AuditOutcome::InProgress,
    }
}

fn store() -> ActivityStore {
    ActivityStore::new(Arc::new(ToolRegistry::normal_mode()))
}

#[test]
fn projects_every_event_category_and_preserves_audit_evidence() {
    use ActivityCategory::*;
    for (kind, category) in [
        (AuditEventType::RequestReceived, Request),
        (AuditEventType::WorkspaceValidated, Workspace),
        (AuditEventType::PolicyEvaluated, Policy),
        (AuditEventType::RequestDenied, Policy),
        (AuditEventType::ExecutionStarted, Execution),
        (AuditEventType::ExecutionCompleted, Execution),
        (AuditEventType::ExecutionFailed, Execution),
        (AuditEventType::ApprovalRequired, Approval),
    ] {
        let mut source = event(1);
        source.event_type = kind;
        let entry = ActivityEntry::from_audit(&source, &ToolRegistry::normal_mode());
        assert_eq!(entry.event_id, source.event_id);
        assert_eq!(entry.request_id, source.request_id);
        assert_eq!(entry.timestamp.as_deref(), Some(source.timestamp.as_str()));
        assert_eq!(entry.category, category);
        assert_eq!(entry.event_type, kind);
        assert_eq!(entry.lifecycle_state, source.lifecycle_state);
        assert_eq!(entry.tool_name.as_deref(), Some("read_file"));
        assert_eq!(entry.decision_summary, None);
        assert_eq!(entry.outcome_summary, source.outcome);
    }
    for decision in [
        PolicyDecision::Allow,
        PolicyDecision::Deny,
        PolicyDecision::Ask,
    ] {
        let mut source = event(2);
        source.policy_decision = Some(decision.clone());
        assert_eq!(
            ActivityEntry::from_audit(&source, &ToolRegistry::normal_mode()).decision_summary,
            Some(decision)
        );
    }
}

#[test]
fn redaction_drops_all_free_text_and_has_a_closed_serialized_shape() {
    let store = store();
    for secret in [
        "API-KEY-VALUE",
        "TOKEN-VALUE",
        "SECRET-VALUE",
        "ENVIRONMENT=PRIVATE",
        r"C:\Users\private\sensitive.txt",
        "/private/sensitive/file",
        "raw arguments",
        "tool result",
    ] {
        let mut source = event(1);
        source.timestamp = secret.into();
        source.tool_name = secret.into();
        source.workspace_id = secret.into();
        source.matched_rule_ids = vec![secret.into()];
        store.on_audit_event(&source);
        let snapshot = store.snapshot(1);
        assert_eq!(snapshot[0].timestamp, None);
        assert_eq!(snapshot[0].tool_name, None);
        assert!(!serde_json::to_string(&snapshot).unwrap().contains(secret));
        assert!(!format!("{snapshot:?}").contains(secret));
        let value = serde_json::to_value(&snapshot[0]).unwrap();
        let mut keys: Vec<_> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "category",
                "decisionSummary",
                "eventId",
                "eventType",
                "lifecycleState",
                "outcomeSummary",
                "requestId",
                "timestamp",
                "toolName"
            ]
        );
    }
}

#[test]
fn normalizes_timestamp_and_rejects_oversized_or_suffix_text() {
    let registry = ToolRegistry::normal_mode();
    let mut source = event(1);
    source.timestamp = "2026-09-08T09:02:03.004+08:00".into();
    assert_eq!(
        ActivityEntry::from_audit(&source, &registry).timestamp,
        event(1).timestamp.into()
    );
    for text in ["s".repeat(100_000), "2026-09-08T01:02:03Z SECRET".into()] {
        source.timestamp = text;
        assert_eq!(
            ActivityEntry::from_audit(&source, &registry).timestamp,
            None
        );
    }
}

#[test]
fn capacity_evicts_oldest_and_latest_limits_are_bounded() {
    let store = store();
    assert!(store.snapshot(usize::MAX).is_empty());
    for id in 0..ACTIVITY_CAPACITY + 3 {
        store.on_audit_event(&event(id as u128));
    }
    let snapshot = store.snapshot(usize::MAX);
    assert_eq!(snapshot.len(), ACTIVITY_CAPACITY);
    assert_eq!(snapshot[0].event_id, Uuid::from_u128(3));
    assert_eq!(
        snapshot.last().unwrap().event_id,
        Uuid::from_u128((ACTIVITY_CAPACITY + 2) as u128)
    );
    assert!(store.snapshot(0).is_empty());
    assert_eq!(store.snapshot(2), snapshot[ACTIVITY_CAPACITY - 2..]);
    assert_eq!(store.snapshot(ACTIVITY_CAPACITY), snapshot);
}

#[test]
fn ordering_uses_receipt_order_not_timestamps_or_random_ids_and_snapshots_are_detached() {
    let store = store();
    for (id, timestamp) in [
        (9, "2026-09-08T02:00:00Z"),
        (2, "2026-09-08T02:00:00Z"),
        (7, "2026-09-08T01:00:00Z"),
    ] {
        let mut source = event(id);
        source.timestamp = timestamp.into();
        store.on_audit_event(&source);
    }
    let snapshot = store.snapshot(10);
    assert_eq!(
        snapshot
            .iter()
            .map(|entry| entry.event_id.as_u128())
            .collect::<Vec<_>>(),
        [9, 2, 7]
    );
    assert_eq!(store.snapshot(10), snapshot);
    let mut changed = store.snapshot(10);
    changed[0].tool_name = None;
    changed.clear();
    store.on_audit_event(&event(4));
    assert_eq!(snapshot.len(), 3);
    assert_eq!(store.snapshot(10)[..3], snapshot);
}

#[test]
fn concurrent_readers_and_writers_preserve_capacity_uniqueness_and_producer_order() {
    let store = Arc::new(store());
    thread::scope(|scope| {
        for producer in 0..4 {
            let store = Arc::clone(&store);
            scope.spawn(move || {
                for n in 0..100 {
                    store.on_audit_event(&event(producer * 1000 + n));
                    assert!(store.snapshot(usize::MAX).len() <= ACTIVITY_CAPACITY);
                }
            });
        }
        let store = Arc::clone(&store);
        scope.spawn(move || {
            for _ in 0..100 {
                assert!(store.snapshot(usize::MAX).len() <= ACTIVITY_CAPACITY);
            }
        });
    });
    let snapshot = store.snapshot(usize::MAX);
    assert_eq!(snapshot.len(), ACTIVITY_CAPACITY);
    let ids: std::collections::HashSet<_> = snapshot.iter().map(|entry| entry.event_id).collect();
    assert_eq!(ids.len(), ACTIVITY_CAPACITY);
    for producer in 0..4 {
        let ids: Vec<_> = snapshot
            .iter()
            .map(|entry| entry.event_id.as_u128())
            .filter(|id| id / 1000 == producer)
            .collect();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn poisoned_observation_lock_does_not_panic_on_emit_or_snapshot() {
    let store = Arc::new(store());
    let writer = Arc::clone(&store);
    assert!(thread::spawn(move || {
        let _guard = writer.entries.lock().unwrap();
        panic!("test observation lock poison");
    })
    .join()
    .is_err());
    store.on_audit_event(&event(1));
    assert_eq!(store.snapshot(1)[0].event_id, event(1).event_id);
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("harness-activity-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("private.txt"), "TOOL-RESULT-SECRET").unwrap();
        Self(path)
    }
    fn config(&self) -> RuntimeConfigSnapshot {
        RuntimeConfigSnapshot {
            config_revision: DecimalU64::new(1),
            workspace: WorkspaceConfig {
                root_path: self.0.to_string_lossy().into_owned(),
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: 4,
                max_activity_events: 20,
                max_read_bytes: 4096,
            },
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn runtime_and_mcp_share_the_sink_with_identical_audit_ids_and_no_payload_leaks() {
    let fixture = Fixture::new();
    let mut core = RuntimeCore::new();
    assert!(core.activity_snapshot(10).is_empty());
    core.initialize(fixture.config()).unwrap();
    core.acknowledge().unwrap();
    let before = core.diagnostics();
    let context = core.mcp_context().unwrap();
    let report = context.execute(|| Ok(ReadFileRequest::new("private.txt")));
    assert!(report.result.is_ok());
    let expected: Vec<_> = report
        .audit_events
        .iter()
        .map(|event| ActivityEntry::from_audit(event, core.tool_registry()))
        .collect();
    assert_eq!(expected.len(), 5);
    assert_eq!(core.activity_snapshot(20), expected);
    assert_eq!(
        expected.last().unwrap().outcome_summary,
        AuditOutcome::Success
    );
    core.glob(&GlobRequest::new("*.txt")).unwrap();
    assert_eq!(core.activity_snapshot(20).len(), 10);
    assert!(core
        .read_file(&ReadFileRequest::new("../OUTSIDE-SECRET.txt"))
        .is_err());
    let denied = core.activity_snapshot(1).pop().unwrap();
    assert_eq!(denied.event_type, AuditEventType::WorkspaceValidated);
    assert_eq!(denied.decision_summary, None);
    assert_eq!(denied.outcome_summary, AuditOutcome::Failed);
    let snapshot = core.activity_snapshot(usize::MAX);
    let json = serde_json::to_string(&snapshot).unwrap();
    for secret in [
        "private.txt",
        "OUTSIDE-SECRET",
        "TOOL-RESULT-SECRET",
        fixture.0.to_str().unwrap(),
    ] {
        assert!(!json.contains(secret));
        assert!(!format!("{snapshot:?}").contains(secret));
    }
    let after = core.diagnostics();
    assert_eq!(before.security, after.security);
    assert_eq!(before.tools, after.tools);
    assert_eq!(before.tunnel, after.tunnel);
    core.stop();
    assert_eq!(core.activity_snapshot(usize::MAX), snapshot);
    core.rollback_initialize();
    assert_eq!(core.activity_snapshot(usize::MAX), snapshot);
    core.initialize(fixture.config()).unwrap();
    assert_eq!(core.activity_snapshot(usize::MAX), snapshot);
    assert!(RuntimeCore::new().activity_snapshot(usize::MAX).is_empty());
}
