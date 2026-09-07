use super::{RequestContext, RequestState, SecurityContext, ToolExecutor, TransitionError};
use crate::policy::PolicyEngine;
use crate::tools::{GlobError, ReadFileError, ToolRegistry};
use crate::workspace::WorkspaceGuard;
use harness_control_protocol::PolicyDecision;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)] // variants are retained for typed adapter error preservation
pub(crate) enum ExecutionError {
    RuntimeNotInitialized,
    RuntimeNotReady,
    InvalidArguments,
    PolicyDenied,
    ApprovalRequired,
    InvalidTransition(TransitionError),
    ReadFile(ReadFileError),
    Glob(GlobError),
}

/// No public constructor or Clone/Default. Only this module grants execution.
pub(crate) struct ExecutionPermit {
    _private: (),
}

impl ExecutionPermit {
    // Low-level M5 tests deliberately isolate executor behavior from enforcement.
    #[cfg(test)]
    pub(crate) fn for_executor_test() -> Self {
        Self { _private: () }
    }
}

/// Hooks observe only non-secret metadata, state and policy evidence, never raw
/// errors/arguments/results. They do not decide authorization or resume requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PipelineEvent {
    RequestReceived,
    StateChanged {
        from: RequestState,
        to: RequestState,
    },
    PolicyDecision,
    ExecutionStarted,
    RequestFinished,
}

pub(crate) trait ExecutionHook: Send + Sync {
    fn on_event(&self, context: &RequestContext, event: PipelineEvent);
}

struct NoopHook;
impl ExecutionHook for NoopHook {
    fn on_event(&self, _context: &RequestContext, _event: PipelineEvent) {}
}

#[allow(dead_code)] // context is retained for the M6-C observation seam
pub(crate) struct ExecutionReport<T> {
    pub(crate) context: RequestContext,
    pub(crate) result: Result<T, ExecutionError>,
}

/// One immutable initialized Runtime snapshot shared by every execution entry.
pub(crate) struct ExecutionPipeline {
    guard: Arc<WorkspaceGuard>,
    registry: Arc<ToolRegistry>,
    policy: PolicyEngine,
    security: SecurityContext,
    max_read_bytes: u32,
    readiness: Arc<AtomicBool>,
    hook: Arc<dyn ExecutionHook>,
}

impl ExecutionPipeline {
    pub(crate) fn new(
        guard: Arc<WorkspaceGuard>,
        registry: Arc<ToolRegistry>,
        policy: PolicyEngine,
        security: SecurityContext,
        max_read_bytes: u32,
        readiness: Arc<AtomicBool>,
    ) -> Self {
        Self {
            guard,
            registry,
            policy,
            security,
            max_read_bytes,
            readiness,
            hook: Arc::new(NoopHook),
        }
    }

    /// Internal M6-C observation seam, not user-defined executable hooks.
    #[allow(dead_code)]
    pub(crate) fn with_hook(mut self, hook: Arc<dyn ExecutionHook>) -> Self {
        self.hook = hook;
        self
    }

    /// Parsing happens inside Validating, so invalid MCP payloads get the same
    /// lifecycle as typed Runtime calls without copying payloads into context.
    pub(crate) fn execute<E: ToolExecutor>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
    ) -> ExecutionReport<E::Output> {
        let mut context = RequestContext::new(E::TOOL_NAME, self.security.clone());
        self.hook.on_event(&context, PipelineEvent::RequestReceived);
        let mut result = self.run(&mut context, prepare);
        if result.is_err()
            && !matches!(
                context.state(),
                RequestState::Denied | RequestState::AwaitingApproval
            )
        {
            if let Err(error) = self.transition(&mut context, RequestState::Failed) {
                result = Err(error);
            }
        }
        // Finished means this invocation returned, NOT approval/completion.
        self.hook.on_event(&context, PipelineEvent::RequestFinished);
        ExecutionReport { context, result }
    }

    fn run<E: ToolExecutor>(
        &self,
        context: &mut RequestContext,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
    ) -> Result<E::Output, ExecutionError> {
        self.transition(context, RequestState::Validating)?;
        self.require_ready()?;
        let executor = prepare()?;
        executor.validate()?;
        self.transition(context, RequestState::WorkspaceValidating)?;
        self.guard
            .approve(executor.workspace_target())
            .map_err(E::workspace_error)?;
        context.workspace_validated();
        self.transition(context, RequestState::PolicyEvaluating)?;
        let evaluation = self.policy.evaluate(&context.policy, &self.registry);
        let decision = evaluation.decision.clone();
        context.policy_evaluation = Some(evaluation);
        self.hook.on_event(context, PipelineEvent::PolicyDecision);
        match decision {
            PolicyDecision::Deny => {
                self.transition(context, RequestState::Denied)?;
                Err(ExecutionError::PolicyDenied)
            }
            PolicyDecision::Ask => {
                self.transition(context, RequestState::AwaitingApproval)?;
                Err(ExecutionError::ApprovalRequired)
            }
            PolicyDecision::Allow => {
                // A stop/rollback while validating must not open a new execution.
                self.require_ready()?;
                self.transition(context, RequestState::Executing)?;
                self.hook.on_event(context, PipelineEvent::ExecutionStarted);
                let permit = ExecutionPermit { _private: () };
                let output = executor.execute(&permit, &self.guard, self.max_read_bytes)?;
                self.transition(context, RequestState::Completed)?;
                Ok(output)
            }
        }
    }

    fn require_ready(&self) -> Result<(), ExecutionError> {
        if self.readiness.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(ExecutionError::RuntimeNotReady)
        }
    }

    fn transition(
        &self,
        context: &mut RequestContext,
        to: RequestState,
    ) -> Result<(), ExecutionError> {
        let from = context.state();
        context
            .transition(to)
            .map_err(ExecutionError::InvalidTransition)?;
        self.hook
            .on_event(context, PipelineEvent::StateChanged { from, to });
        Ok(())
    }
}
