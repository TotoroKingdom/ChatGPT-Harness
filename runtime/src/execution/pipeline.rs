use super::{RequestContext, RequestState, SecurityContext, ToolExecutor, TransitionError};
use crate::audit::{AuditEvent, AuditSink};
use crate::policy::PolicyEngine;
use crate::tools::{EffectiveTool, EffectiveToolCatalog, GlobError, ReadFileError, ToolRegistry};
use crate::workspace::WorkspaceGuard;
use harness_control_protocol::PolicyDecision;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
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
    #[cfg(test)]
    pub(crate) fn for_executor_test() -> Self {
        Self { _private: () }
    }
}

/// Hooks observe only non-secret metadata, state and policy evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PipelineEvent {
    RequestReceived,
    StateChanged {
        from: RequestState,
        to: RequestState,
    },
    WorkspaceValidated {
        passed: bool,
    },
    PolicyDecision,
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    RequestDenied,
    ApprovalRequired,
    RequestFinished,
}

pub(crate) trait ExecutionHook: Send + Sync {
    fn on_event(&self, context: &RequestContext, event: PipelineEvent);
}

struct NoopHook;
impl ExecutionHook for NoopHook {
    fn on_event(&self, _context: &RequestContext, _event: PipelineEvent) {}
}

#[allow(dead_code)]
pub(crate) struct ExecutionReport<T> {
    pub(crate) context: RequestContext,
    pub(crate) result: Result<T, ExecutionError>,
    /// Per-invocation emission; this is not audit storage.
    pub(crate) audit_events: Vec<AuditEvent>,
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
    audit_sink: Option<Arc<dyn AuditSink>>,
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
            audit_sink: None,
        }
    }

    /// Observes the existing audit emission; does not add lifecycle events.
    pub(crate) fn with_audit_sink(mut self, sink: Arc<dyn AuditSink>) -> Self {
        self.audit_sink = Some(sink);
        self
    }

    /// Internal observation seam, not user-defined executable hooks.
    #[allow(dead_code)]
    pub(crate) fn with_hook(mut self, hook: Arc<dyn ExecutionHook>) -> Self {
        self.hook = hook;
        self
    }
    /// Returns the exposure view derived from this immutable Runtime snapshot.
    /// MCP consumes this view rather than re-evaluating policy or reading the
    /// declared registry directly.
    pub(crate) fn effective_tools(&self) -> Vec<EffectiveTool> {
        EffectiveToolCatalog::compute(
            &self.registry,
            &self.policy,
            self.security.mode.clone(),
            self.readiness.load(Ordering::Acquire),
        )
    }

    /// Parsing happens inside Validating, so invalid MCP payloads get the same
    /// lifecycle as typed Runtime calls without copying payloads into context.
    pub(crate) fn execute<E: ToolExecutor>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
    ) -> ExecutionReport<E::Output> {
        let mut context = RequestContext::new(E::TOOL_NAME, self.security.clone());
        let mut audit_events = Vec::new();
        self.emit(&context, PipelineEvent::RequestReceived, &mut audit_events);
        let mut result = self.run(&mut context, prepare, &mut audit_events);
        if result.is_err()
            && !matches!(
                context.state(),
                RequestState::Denied | RequestState::AwaitingApproval | RequestState::Failed
            )
        {
            if let Err(error) = self.transition(&mut context, RequestState::Failed) {
                result = Err(error);
            }
        }
        // Finished means this invocation returned, NOT approval/completion.
        self.emit(&context, PipelineEvent::RequestFinished, &mut audit_events);
        ExecutionReport {
            context,
            result,
            audit_events,
        }
    }

    fn run<E: ToolExecutor>(
        &self,
        context: &mut RequestContext,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
        audit_events: &mut Vec<AuditEvent>,
    ) -> Result<E::Output, ExecutionError> {
        self.transition(context, RequestState::Validating)?;
        self.require_ready()?;
        let executor = prepare()?;
        executor.validate()?;
        self.transition(context, RequestState::WorkspaceValidating)?;
        match self.guard.approve(executor.workspace_target()) {
            Ok(_) => {
                context.workspace_validated();
                self.emit(
                    context,
                    PipelineEvent::WorkspaceValidated { passed: true },
                    audit_events,
                );
            }
            Err(error) => {
                self.emit(
                    context,
                    PipelineEvent::WorkspaceValidated { passed: false },
                    audit_events,
                );
                return Err(E::workspace_error(error));
            }
        }
        self.transition(context, RequestState::PolicyEvaluating)?;
        let evaluation = self.policy.evaluate(&context.policy, &self.registry);
        let decision = evaluation.decision.clone();
        context.policy_evaluation = Some(evaluation);
        self.emit(context, PipelineEvent::PolicyDecision, audit_events);
        match decision {
            PolicyDecision::Deny => {
                self.transition(context, RequestState::Denied)?;
                self.emit(context, PipelineEvent::RequestDenied, audit_events);
                Err(ExecutionError::PolicyDenied)
            }
            PolicyDecision::Ask => {
                self.transition(context, RequestState::AwaitingApproval)?;
                self.emit(context, PipelineEvent::ApprovalRequired, audit_events);
                Err(ExecutionError::ApprovalRequired)
            }
            PolicyDecision::Allow => {
                self.require_ready()?;
                self.transition(context, RequestState::Executing)?;
                self.emit(context, PipelineEvent::ExecutionStarted, audit_events);
                let permit = ExecutionPermit { _private: () };
                match executor.execute(&permit, &self.guard, self.max_read_bytes) {
                    Ok(output) => {
                        self.transition(context, RequestState::Completed)?;
                        self.emit(context, PipelineEvent::ExecutionCompleted, audit_events);
                        Ok(output)
                    }
                    Err(error) => {
                        self.transition(context, RequestState::Failed)?;
                        self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                        Err(error)
                    }
                }
            }
        }
    }

    fn emit(
        &self,
        context: &RequestContext,
        event: PipelineEvent,
        audit_events: &mut Vec<AuditEvent>,
    ) {
        self.hook.on_event(context, event);
        if let Some(audit_event) = AuditEvent::from_pipeline_event(context, event) {
            if let Some(sink) = &self.audit_sink {
                sink.on_audit_event(&audit_event);
            }
            audit_events.push(audit_event);
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
