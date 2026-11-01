use super::{AuditEvent, AuditSink};
use std::sync::Arc;

/// Forwards one canonical audit event to observation-only sinks. It cannot
/// aggregate results, create new events, or feed evidence back into policy.
pub(crate) struct AuditFanout {
    sinks: Vec<Arc<dyn AuditSink>>,
}

impl AuditFanout {
    pub(crate) fn new(sinks: Vec<Arc<dyn AuditSink>>) -> Self {
        Self { sinks }
    }
}

impl AuditSink for AuditFanout {
    fn on_audit_event(&self, event: &AuditEvent) {
        for sink in &self.sinks {
            sink.on_audit_event(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditEventType, AuditLifecycleState, AuditOutcome, WorkspaceValidation};
    use harness_control_protocol::control_read::ToolCapability;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counter(AtomicUsize);

    impl AuditSink for Counter {
        fn on_audit_event(&self, _event: &AuditEvent) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn fanout_sends_the_same_event_once_to_each_observer() {
        let first = Arc::new(Counter(AtomicUsize::new(0)));
        let second = Arc::new(Counter(AtomicUsize::new(0)));
        let fanout = AuditFanout::new(vec![first.clone(), second.clone()]);
        fanout.on_audit_event(&AuditEvent {
            tool_enabled: Some(true),
            capability: Some(ToolCapability::WorkspaceRead),
            sanitized_arguments: None,
            approval_id: None,
            duration_ms: None,
            result_evidence: None,
            event_id: uuid::Uuid::new_v4(),
            request_id: uuid::Uuid::new_v4(),
            timestamp: "2026-11-01T00:00:00.000Z".into(),
            event_type: AuditEventType::RequestReceived,
            tool_name: "read_file".into(),
            lifecycle_state: AuditLifecycleState::Received,
            workspace_id: "active".into(),
            workspace_validation: WorkspaceValidation::NotAttempted,
            policy_decision: None,
            matched_rule_ids: Vec::new(),
            policy_reason: None,
            approval_decision: None,
            outcome: AuditOutcome::InProgress,
            failure_category: None,
            bash: None,
        });
        assert_eq!(first.0.load(Ordering::SeqCst), 1);
        assert_eq!(second.0.load(Ordering::SeqCst), 1);
    }
}
