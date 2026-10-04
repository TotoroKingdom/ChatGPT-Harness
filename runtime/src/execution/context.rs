use super::state::RequestLifecycle;
use super::{RequestState, TransitionError};
use crate::audit::AuditFailureCategory;
use crate::policy::{PolicyContext, PolicyEvaluation};
use harness_control_protocol::{ApprovalRequest, BashApprovalDetails, SecurityMode};
use uuid::Uuid;

/// Non-secret identity of the immutable initialized workspace/security snapshot.
#[derive(Clone, Debug)]
#[allow(dead_code)] // retained for the M6-C hook/audit metadata seam
pub(crate) struct SecurityContext {
    pub(crate) workspace_id: Uuid,
    pub(crate) mode: SecurityMode,
}

/// Owned, sanitized Bash lifecycle evidence suitable for a cancellation
/// callback.  It deliberately contains no raw command, exact grant scope,
/// environment or process output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BashEvidenceSnapshot {
    pub(crate) request_id: Uuid,
    pub(crate) tool_name: &'static str,
    pub(crate) tool_enabled: bool,
    pub(crate) capability: Option<harness_control_protocol::control_read::ToolCapability>,
    pub(crate) sanitized_arguments: Option<harness_control_protocol::SanitizedArgumentsEvidence>,
    pub(crate) started_at: std::time::Instant,
    pub(crate) mode: SecurityMode,
    pub(crate) details: Option<BashApprovalDetails>,
    pub(crate) policy_evaluation: Option<PolicyEvaluation>,
    pub(crate) approval_request_id: Option<String>,
    pub(crate) session_grant_hit: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration_ms: Option<u64>,
}

/// Bounded request metadata and safe logical paths. No raw payload, physical
/// paths, credentials, environment or tool output bodies.
#[derive(Debug)]
#[allow(dead_code)] // request_id/context are intentionally observable only through hooks/reports
pub(crate) struct RequestContext {
    pub(crate) request_id: Uuid,
    pub(crate) tool_name: &'static str,
    pub(crate) security: SecurityContext,
    pub(crate) policy: PolicyContext<'static>,
    pub(crate) policy_evaluation: Option<PolicyEvaluation>,
    pub(crate) failure_category: Option<AuditFailureCategory>,
    /// Sanitized Bash display facts populated by the Pipeline. These fields
    /// never contain the raw command, exact normalized grant scope, env, or
    /// process output.
    pub(crate) bash_details: Option<BashApprovalDetails>,
    pub(crate) bash_exit_code: Option<i32>,
    pub(crate) bash_duration_ms: Option<u64>,
    pub(crate) approval_request_id: Option<String>,
    pub(crate) session_grant_hit: bool,
    pub(crate) tool_enabled: bool,
    pub(crate) capability: Option<harness_control_protocol::control_read::ToolCapability>,
    pub(crate) sanitized_arguments: Option<harness_control_protocol::SanitizedArgumentsEvidence>,
    pub(crate) result_evidence: Option<harness_control_protocol::ExecutionResultEvidence>,
    pub(crate) approval_decision: Option<harness_control_protocol::ApprovalDecision>,
    pub(crate) started_at: std::time::Instant,
    lifecycle: RequestLifecycle,
}

impl RequestContext {
    pub(super) fn new(tool_name: &'static str, security: SecurityContext) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            tool_name,
            policy: PolicyContext::new(tool_name, security.mode.clone(), ""),
            security,
            policy_evaluation: None,
            failure_category: None,
            bash_details: None,
            bash_exit_code: None,
            bash_duration_ms: None,
            approval_request_id: None,
            session_grant_hit: false,
            tool_enabled: true,
            capability: None,
            sanitized_arguments: None,
            result_evidence: None,
            approval_decision: None,
            started_at: std::time::Instant::now(),
            lifecycle: RequestLifecycle::new(),
        }
    }

    pub(crate) fn state(&self) -> RequestState {
        self.lifecycle.state()
    }

    pub(super) fn workspace_validated(&mut self) {
        self.policy = PolicyContext::new(
            self.tool_name,
            self.security.mode.clone(),
            "active_workspace",
        );
    }

    #[cfg(test)]
    pub(crate) fn for_evidence_test(tool_name: &'static str, mode: SecurityMode) -> Self {
        Self::new(
            tool_name,
            SecurityContext {
                workspace_id: Uuid::new_v4(),
                mode,
            },
        )
    }

    pub(crate) fn set_bash_details(&mut self, details: BashApprovalDetails) {
        self.bash_details = Some(details);
    }

    pub(crate) fn set_bash_result(&mut self, exit_code: i32, duration_ms: u64) {
        self.bash_exit_code = Some(exit_code);
        self.bash_duration_ms = Some(duration_ms);
    }

    pub(crate) fn set_bash_duration(&mut self, duration_ms: u64) {
        self.bash_duration_ms = Some(duration_ms);
    }

    pub(crate) fn set_approval_request_id(&mut self, request: &ApprovalRequest) {
        self.approval_request_id = Some(request.request_id.clone());
    }

    pub(crate) fn mark_session_grant_hit(&mut self) {
        self.session_grant_hit = true;
    }

    pub(crate) fn bash_evidence_snapshot(&self) -> Option<BashEvidenceSnapshot> {
        Some(BashEvidenceSnapshot {
            request_id: self.request_id,
            tool_name: self.tool_name,
            tool_enabled: self.tool_enabled,
            capability: self.capability,
            sanitized_arguments: self.sanitized_arguments.clone(),
            started_at: self.started_at,
            mode: self.security.mode.clone(),
            details: self.bash_details.clone(),
            policy_evaluation: self.policy_evaluation.clone(),
            approval_request_id: self.approval_request_id.clone(),
            session_grant_hit: self.session_grant_hit,
            exit_code: self.bash_exit_code,
            duration_ms: self.bash_duration_ms,
        })
    }

    pub(super) fn mark_failure_category(&mut self, category: AuditFailureCategory) {
        self.failure_category = Some(category);
    }

    pub(super) fn transition(&mut self, next: RequestState) -> Result<(), TransitionError> {
        self.lifecycle.transition(next)
    }
}
