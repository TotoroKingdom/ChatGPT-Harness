use super::state::RequestLifecycle;
use super::{RequestState, TransitionError};
use crate::audit::AuditFailureCategory;
use crate::policy::{PolicyContext, PolicyEvaluation};
use harness_control_protocol::SecurityMode;
use uuid::Uuid;

/// Non-secret identity of the immutable initialized workspace/security snapshot.
#[derive(Clone, Debug)]
#[allow(dead_code)] // retained for the M6-C hook/audit metadata seam
pub(crate) struct SecurityContext {
    pub(crate) workspace_id: Uuid,
    pub(crate) mode: SecurityMode,
}

/// Bounded request metadata. No payload, paths, credentials, or tool output.
#[derive(Debug)]
#[allow(dead_code)] // request_id/context are intentionally observable only through hooks/reports
pub(crate) struct RequestContext {
    pub(crate) request_id: Uuid,
    pub(crate) tool_name: &'static str,
    pub(crate) security: SecurityContext,
    pub(crate) policy: PolicyContext<'static>,
    pub(crate) policy_evaluation: Option<PolicyEvaluation>,
    pub(crate) failure_category: Option<AuditFailureCategory>,
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

    pub(super) fn mark_failure_category(&mut self, category: AuditFailureCategory) {
        self.failure_category = Some(category);
    }

    pub(super) fn transition(&mut self, next: RequestState) -> Result<(), TransitionError> {
        self.lifecycle.transition(next)
    }
}
