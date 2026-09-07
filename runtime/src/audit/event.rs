use crate::execution::{PipelineEvent, RequestContext, RequestState};
use harness_control_protocol::PolicyDecision;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The deliberately small set of externally useful execution events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuditEventType {
    RequestReceived,
    WorkspaceValidated,
    PolicyEvaluated,
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    RequestDenied,
    ApprovalRequired,
}

/// A safe projection of the internal request lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuditLifecycleState {
    Received,
    Validating,
    WorkspaceValidating,
    PolicyEvaluating,
    Denied,
    AwaitingApproval,
    Executing,
    Completed,
    Failed,
}

impl From<RequestState> for AuditLifecycleState {
    fn from(state: RequestState) -> Self {
        match state {
            RequestState::Received => Self::Received,
            RequestState::Validating => Self::Validating,
            RequestState::WorkspaceValidating => Self::WorkspaceValidating,
            RequestState::PolicyEvaluating => Self::PolicyEvaluating,
            RequestState::Denied => Self::Denied,
            RequestState::AwaitingApproval => Self::AwaitingApproval,
            RequestState::Executing => Self::Executing,
            RequestState::Completed => Self::Completed,
            RequestState::Failed => Self::Failed,
        }
    }
}

/// Result of the single WorkspaceGuard check. No path is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkspaceValidation {
    NotAttempted,
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuditOutcome {
    InProgress,
    Success,
    Failed,
    Denied,
    ApprovalRequired,
}

/// Structured, serde-serializable audit evidence for one pipeline observation.
///
/// `workspace_id` is a logical identifier only; it is never a filesystem path.
/// There is intentionally no arguments, payload, error, or result field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuditEvent {
    pub(crate) event_id: Uuid,
    pub(crate) request_id: Uuid,
    pub(crate) timestamp: String,
    pub(crate) event_type: AuditEventType,
    pub(crate) tool_name: String,
    pub(crate) lifecycle_state: AuditLifecycleState,
    pub(crate) workspace_id: String,
    pub(crate) workspace_validation: WorkspaceValidation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) policy_decision: Option<PolicyDecision>,
    pub(crate) matched_rule_ids: Vec<String>,
    pub(crate) outcome: AuditOutcome,
}

impl AuditEvent {
    pub(crate) fn from_pipeline_event(
        context: &RequestContext,
        event: PipelineEvent,
    ) -> Option<Self> {
        let (event_type, outcome, workspace_validation) = match event {
            PipelineEvent::RequestReceived => (
                AuditEventType::RequestReceived,
                AuditOutcome::InProgress,
                WorkspaceValidation::NotAttempted,
            ),
            PipelineEvent::WorkspaceValidated { passed } => (
                AuditEventType::WorkspaceValidated,
                if passed {
                    AuditOutcome::Success
                } else {
                    AuditOutcome::Failed
                },
                if passed {
                    WorkspaceValidation::Passed
                } else {
                    WorkspaceValidation::Failed
                },
            ),
            PipelineEvent::PolicyDecision => (
                AuditEventType::PolicyEvaluated,
                AuditOutcome::InProgress,
                WorkspaceValidation::Passed,
            ),
            PipelineEvent::ExecutionStarted => (
                AuditEventType::ExecutionStarted,
                AuditOutcome::InProgress,
                WorkspaceValidation::Passed,
            ),
            PipelineEvent::ExecutionCompleted => (
                AuditEventType::ExecutionCompleted,
                AuditOutcome::Success,
                WorkspaceValidation::Passed,
            ),
            PipelineEvent::ExecutionFailed => (
                AuditEventType::ExecutionFailed,
                AuditOutcome::Failed,
                WorkspaceValidation::Passed,
            ),
            PipelineEvent::RequestDenied => (
                AuditEventType::RequestDenied,
                AuditOutcome::Denied,
                WorkspaceValidation::Passed,
            ),
            PipelineEvent::ApprovalRequired => (
                AuditEventType::ApprovalRequired,
                AuditOutcome::ApprovalRequired,
                WorkspaceValidation::Passed,
            ),
            PipelineEvent::StateChanged { .. } | PipelineEvent::RequestFinished => return None,
        };

        let evaluation = context.policy_evaluation.as_ref();
        Some(Self {
            event_id: Uuid::new_v4(),
            request_id: context.request_id,
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            event_type,
            tool_name: context.tool_name.to_owned(),
            lifecycle_state: context.state().into(),
            // This is deliberately a stable logical label, not the configured
            // workspace root or any canonicalized path.
            workspace_id: "active".into(),
            workspace_validation,
            policy_decision: evaluation.map(|item| item.decision.clone()),
            matched_rule_ids: evaluation
                .map(|item| item.matched_rule_ids.clone())
                .unwrap_or_default(),
            outcome,
        })
    }
}
