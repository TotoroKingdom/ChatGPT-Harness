use crate::audit::{AuditEvent, AuditEventType as InternalAuditEventType};
use harness_control_protocol::{
    ActivityEvent, ActivityEventType, DecimalU64, PolicyDecision, PolicyReason, RuntimeLimits,
};
use std::collections::VecDeque;
use std::sync::mpsc::Sender;

/// Sanitized activity evidence emitted by the execution pipeline and
/// materialized by the control thread's EventBus.
///
/// MCP requests execute on the dedicated HTTP thread, while the control
/// protocol owns the mutable EventBus. Keeping this message intentionally
/// smaller than the internal audit event preserves the existing exposure
/// boundary: no paths, payloads, temporary names, or raw OS errors cross it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActivityEventInput {
    pub(crate) event_type: ActivityEventType,
    pub(crate) summary: &'static str,
    pub(crate) metadata: Vec<(String, String)>,
    pub(crate) request_id: Option<String>,
}

impl ActivityEventInput {
    pub(crate) fn from_audit(event: &AuditEvent) -> Option<Self> {
        let event_type = match event.event_type {
            InternalAuditEventType::RequestReceived => ActivityEventType::ToolRequested,
            InternalAuditEventType::WorkspaceValidated => {
                if event.workspace_validation == crate::audit::WorkspaceValidation::Failed {
                    ActivityEventType::WorkspaceViolation
                } else {
                    return None;
                }
            }
            InternalAuditEventType::PolicyEvaluated => ActivityEventType::PolicyEvaluated,
            InternalAuditEventType::ExecutionStarted => ActivityEventType::ExecutionStarted,
            InternalAuditEventType::ExecutionCompleted => ActivityEventType::ExecutionCompleted,
            InternalAuditEventType::ExecutionFailed => ActivityEventType::ExecutionFailed,
            InternalAuditEventType::RequestDenied => ActivityEventType::ExecutionFailed,
            InternalAuditEventType::ApprovalRequired => ActivityEventType::ApprovalRequested,
        };

        let mut metadata = vec![("tool".into(), event.tool_name.clone())];
        if let Some(decision) = &event.policy_decision {
            metadata.push(("decision".into(), policy_decision_name(decision).into()));
            let matched_rule_ids = event
                .matched_rule_ids
                .iter()
                .map(|id| redact_rule_id(id))
                .collect::<Vec<_>>();
            metadata.push((
                "matchedRuleIds".into(),
                serde_json::to_string(&matched_rule_ids).unwrap_or_else(|_| "[]".into()),
            ));
        }
        if let Some(reason) = event.policy_reason {
            metadata.push(("policyReason".into(), policy_reason_name(reason).into()));
        }
        if let Some(category) = event.failure_category {
            metadata.push((
                "failureCategory".into(),
                failure_category_name(category).into(),
            ));
        }

        let summary = activity_summary(event_type.clone());
        Some(Self {
            event_type,
            summary,
            metadata,
            request_id: Some(event.request_id.to_string()),
        })
    }
}

fn policy_decision_name(decision: &PolicyDecision) -> &'static str {
    match decision {
        PolicyDecision::Allow => "allow",
        PolicyDecision::Ask => "ask",
        PolicyDecision::Deny => "deny",
    }
}

fn policy_reason_name(reason: PolicyReason) -> &'static str {
    match reason {
        PolicyReason::SystemGuardrail => "system_guardrail",
        PolicyReason::MatchedUserRules => "matched_user_rules",
        PolicyReason::ToolDefault => "tool_default",
        PolicyReason::InvalidPolicySnapshot => "invalid_policy_snapshot",
        PolicyReason::InvalidContext => "invalid_context",
        PolicyReason::UnsupportedCapability => "unsupported_capability",
        PolicyReason::ToolDisabled => "tool_disabled",
    }
}

fn redact_rule_id(id: &str) -> String {
    if id.len() <= 64
        && id.bytes().any(|byte| byte.is_ascii_lowercase())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        id.to_owned()
    } else {
        "<redacted>".into()
    }
}

/// Clones the sender held by an initialized execution pipeline.
pub(crate) type ActivityEventSender = Sender<ActivityEventInput>;

fn activity_summary(event_type: ActivityEventType) -> &'static str {
    match event_type {
        ActivityEventType::ToolRequested => "Tool request received.",
        ActivityEventType::WorkspaceViolation => "Workspace boundary rejected the request.",
        ActivityEventType::PolicyEvaluated => "Tool policy evaluated.",
        ActivityEventType::ExecutionStarted => "Tool execution started.",
        ActivityEventType::ExecutionCompleted => "Tool execution completed.",
        ActivityEventType::ExecutionFailed => "Tool execution failed.",
        ActivityEventType::ApprovalRequested => "Tool approval requested.",
        ActivityEventType::RuntimeStarted
        | ActivityEventType::RuntimeStopped
        | ActivityEventType::ValidationFailed
        | ActivityEventType::ApprovalResolved
        | ActivityEventType::ExecutionTimedOut
        | ActivityEventType::ExecutionCancelled => "Runtime activity observed.",
    }
}

fn failure_category_name(category: crate::audit::AuditFailureCategory) -> &'static str {
    match category {
        crate::audit::AuditFailureCategory::ToolDisabled => "tool_disabled",
        crate::audit::AuditFailureCategory::TargetChanged => "target_changed",
        crate::audit::AuditFailureCategory::AtomicFailure => "atomic_failure",
        crate::audit::AuditFailureCategory::IoFailure => "io_failure",
        crate::audit::AuditFailureCategory::WorkspaceViolation => "workspace_violation",
        crate::audit::AuditFailureCategory::InvalidArguments => "invalid_arguments",
        crate::audit::AuditFailureCategory::ExecutionFailed => "execution_failed",
    }
}

#[derive(Debug)]
pub struct EventBus {
    max_events: usize,
    next_sequence: u64,
    history: VecDeque<ActivityEvent>,
}

impl EventBus {
    pub fn new(max_events: u32) -> Self {
        Self {
            max_events: max_events.max(1) as usize,
            next_sequence: 0,
            history: VecDeque::new(),
        }
    }

    pub fn publish(
        &mut self,
        event_type: ActivityEventType,
        summary: impl Into<String>,
        metadata: impl IntoIterator<Item = (String, String)>,
        request_id: Option<String>,
    ) -> ActivityEvent {
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("activity sequence exhausted");
        let event = ActivityEvent {
            event_id: format!("event_{}", uuid::Uuid::new_v4()),
            sequence: DecimalU64::new(self.next_sequence),
            event_type,
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            summary: summary.into(),
            metadata: metadata.into_iter().collect(),
        };
        self.history.push_back(event.clone());
        while self.history.len() > self.max_events {
            self.history.pop_front();
        }
        event
    }

    pub fn latest_sequence(&self) -> DecimalU64 {
        DecimalU64::new(self.next_sequence)
    }

    pub fn after(&self, after: Option<DecimalU64>, limit: Option<u32>) -> Vec<ActivityEvent> {
        let cursor = after.unwrap_or(DecimalU64::ZERO).get();
        let max = limit.unwrap_or(self.max_events as u32).max(1) as usize;
        self.history
            .iter()
            .filter(|event| event.sequence.get() > cursor)
            .take(max)
            .cloned()
            .collect()
    }

    pub fn from_limits(limits: &RuntimeLimits) -> Self {
        Self::new(limits.max_activity_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{
        AuditEvent, AuditEventType, AuditLifecycleState, AuditOutcome, WorkspaceValidation,
    };
    use harness_control_protocol::ActivityEventType;
    use uuid::Uuid;

    #[test]
    fn policy_evidence_is_projected_without_raw_or_unsafe_rule_ids() {
        let input = ActivityEventInput::from_audit(&AuditEvent {
            event_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            timestamp: "2026-10-02T00:00:00.000Z".into(),
            event_type: AuditEventType::PolicyEvaluated,
            tool_name: "write".into(),
            lifecycle_state: AuditLifecycleState::PolicyEvaluating,
            workspace_id: "active".into(),
            workspace_validation: WorkspaceValidation::Passed,
            policy_decision: Some(PolicyDecision::Allow),
            matched_rule_ids: vec![
                "user-write-allow".into(),
                "M3_SECRET_CONTENT_SENTINEL".into(),
            ],
            policy_reason: Some(PolicyReason::MatchedUserRules),
            outcome: AuditOutcome::InProgress,
            failure_category: None,
        })
        .expect("policy audit event projects to activity");

        let metadata = input
            .metadata
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(metadata.get("tool"), Some(&"write".into()));
        assert_eq!(metadata.get("decision"), Some(&"allow".into()));
        assert_eq!(
            metadata.get("matchedRuleIds"),
            Some(&r#"["user-write-allow","<redacted>"]"#.into())
        );
        assert_eq!(
            metadata.get("policyReason"),
            Some(&"matched_user_rules".into())
        );
        assert!(!metadata
            .values()
            .any(|value| value.contains("M3_SECRET_CONTENT_SENTINEL")));
    }

    #[test]
    fn subscribe_after_sequence_only_replays_missing_events() {
        let mut bus = EventBus::new(3);
        for index in 1..=3 {
            bus.publish(
                ActivityEventType::RuntimeStarted,
                format!("event {index}"),
                std::iter::empty::<(String, String)>(),
                None,
            );
        }

        let events = bus.after(Some(DecimalU64::new(1)), None);
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![DecimalU64::new(2), DecimalU64::new(3)]
        );
    }

    #[test]
    fn history_is_bounded_and_sequences_remain_monotonic() {
        let mut bus = EventBus::new(2);
        for index in 1..=3 {
            bus.publish(
                ActivityEventType::RuntimeStarted,
                format!("event {index}"),
                std::iter::empty::<(String, String)>(),
                None,
            );
        }

        let events = bus.after(None, None);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, DecimalU64::new(2));
        assert_eq!(events[1].sequence, DecimalU64::new(3));
        assert_eq!(bus.latest_sequence(), DecimalU64::new(3));
    }
}
