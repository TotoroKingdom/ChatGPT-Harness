use crate::audit::{AuditEvent, AuditEventType as InternalAuditEventType};
use harness_control_protocol::{ActivityEvent, ActivityEventType, DecimalU64, RuntimeLimits};
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
    use harness_control_protocol::ActivityEventType;

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
