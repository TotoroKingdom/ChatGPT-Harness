use super::{
    RequestContext, RequestState, SecurityContext, ToolExecutor, TransitionError, WorkspaceTarget,
};
use crate::approval::{ApprovalFacts, ApprovalManager, ApprovalResolution};
use crate::audit::{AuditEvent, AuditFailureCategory, AuditSink};
use crate::events::{ActivityEventInput, ActivityEventSender};
use crate::policy::PolicyEngine;
use crate::session_grant::SessionGrantStore;
use crate::tools::{
    EditError, EffectiveTool, EffectiveToolCatalog, GlobError, ReadFileError, ToolRegistry,
    WriteError,
};
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
    ToolDisabled,
    InvalidArguments,
    PolicyDenied,
    ApprovalRequired,
    ApprovalDenied,
    ApprovalExpired,
    ApprovalCancelled,
    InvalidTransition(TransitionError),
    ReadFile(ReadFileError),
    Glob(GlobError),
    Write(crate::tools::WriteError),
    Edit(crate::tools::EditError),
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
    ApprovalResolved {
        decision: harness_control_protocol::ApprovalDecision,
    },
    ApprovalExpired,
    ApprovalCancelled,
    SessionGrantMatched,
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
    activity_event_sender: Option<ActivityEventSender>,
    approval_manager: Arc<ApprovalManager>,
    session_grants: Arc<SessionGrantStore>,
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
            activity_event_sender: None,
            approval_manager: ApprovalManager::new("unbound-runtime"),
            session_grants: Arc::new(SessionGrantStore::new("unbound-runtime")),
        }
    }

    pub(crate) fn with_approval_state(
        mut self,
        approval_manager: Arc<ApprovalManager>,
        session_grants: Arc<SessionGrantStore>,
    ) -> Self {
        self.approval_manager = approval_manager;
        self.session_grants = session_grants;
        self
    }

    /// Observes the existing audit emission; does not add lifecycle events.
    pub(crate) fn with_audit_sink(mut self, sink: Arc<dyn AuditSink>) -> Self {
        self.audit_sink = Some(sink);
        self
    }

    /// Projects only sanitized lifecycle evidence to the control EventBus.
    /// The sender is owned by the process-local RuntimeCore and does not add
    /// a new control or MCP surface.
    pub(crate) fn with_activity_event_sender(mut self, sender: ActivityEventSender) -> Self {
        self.activity_event_sender = Some(sender);
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

    /// Async execution path used by MCP. The approval wait is represented by
    /// a Tokio receiver/timer and never blocks the MCP current-thread runtime.
    pub(crate) async fn execute_async<E: ToolExecutor + Send>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
    ) -> ExecutionReport<E::Output> {
        let mut context = RequestContext::new(E::TOOL_NAME, self.security.clone());
        let mut audit_events = Vec::new();
        self.emit(&context, PipelineEvent::RequestReceived, &mut audit_events);
        let mut result = self
            .run_async(&mut context, prepare, &mut audit_events)
            .await;
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
        if self.registry.is_disabled(E::TOOL_NAME) {
            context.mark_failure_category(AuditFailureCategory::ToolDisabled);
            self.transition(context, RequestState::Failed)?;
            self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
            return Err(ExecutionError::ToolDisabled);
        }
        let executor = prepare()?;
        executor.validate()?;
        self.transition(context, RequestState::WorkspaceValidating)?;
        let workspace_validation = match executor.workspace_target() {
            WorkspaceTarget::ExistingPath(path) => self.guard.approve(path).map(|_| ()),
            WorkspaceTarget::WriteTarget(path) => self.guard.approve_write_target(path).map(|_| ()),
        };
        match workspace_validation {
            Ok(_) => {
                context.workspace_validated();
                self.emit(
                    context,
                    PipelineEvent::WorkspaceValidated { passed: true },
                    audit_events,
                );
            }
            Err(error) => {
                context.mark_failure_category(AuditFailureCategory::WorkspaceViolation);
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
                        context.mark_failure_category(audit_failure_category(&error));
                        self.transition(context, RequestState::Failed)?;
                        self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                        Err(error)
                    }
                }
            }
        }
    }

    async fn run_async<E: ToolExecutor + Send>(
        &self,
        context: &mut RequestContext,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
        audit_events: &mut Vec<AuditEvent>,
    ) -> Result<E::Output, ExecutionError> {
        self.transition(context, RequestState::Validating)?;
        self.require_ready()?;
        if self.registry.is_disabled(E::TOOL_NAME) {
            context.mark_failure_category(AuditFailureCategory::ToolDisabled);
            self.transition(context, RequestState::Failed)?;
            self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
            return Err(ExecutionError::ToolDisabled);
        }
        let executor = prepare()?;
        executor.validate()?;
        self.transition(context, RequestState::WorkspaceValidating)?;
        let workspace_validation = match executor.workspace_target() {
            WorkspaceTarget::ExistingPath(path) => self.guard.approve(path).map(|_| ()),
            WorkspaceTarget::WriteTarget(path) => self.guard.approve_write_target(path).map(|_| ()),
        };
        match workspace_validation {
            Ok(_) => {
                context.workspace_validated();
                self.emit(
                    context,
                    PipelineEvent::WorkspaceValidated { passed: true },
                    audit_events,
                );
            }
            Err(error) => {
                context.mark_failure_category(AuditFailureCategory::WorkspaceViolation);
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
            PolicyDecision::Allow => self.execute_allowed(context, executor, audit_events),
            PolicyDecision::Ask => {
                let tool_facts = executor.approval_facts(&self.guard)?;
                if self.session_grants.matches(
                    self.approval_manager.generation_id(),
                    E::TOOL_NAME,
                    &tool_facts.scope_summary,
                ) {
                    self.emit(context, PipelineEvent::SessionGrantMatched, audit_events);
                    return self.execute_allowed(context, executor, audit_events);
                }

                let registration = self
                    .approval_manager
                    .register(ApprovalFacts {
                        request_id: context.request_id,
                        tool: E::TOOL_NAME.into(),
                        sanitized_arguments: tool_facts.sanitized_arguments,
                        scope_summary: tool_facts.scope_summary,
                        fingerprint: tool_facts.fingerprint,
                        matched_rule_ids: context
                            .policy_evaluation
                            .as_ref()
                            .map(|evaluation| evaluation.matched_rule_ids.clone())
                            .unwrap_or_default(),
                        policy_reason: context
                            .policy_evaluation
                            .as_ref()
                            .map(|evaluation| crate::policy::to_wire_reason(evaluation.reason))
                            .unwrap_or(harness_control_protocol::PolicyReason::InvalidContext),
                        risk_level: self
                            .registry
                            .list()
                            .into_iter()
                            .find(|descriptor| descriptor.name == E::TOOL_NAME)
                            .map(|descriptor| descriptor.risk_level)
                            .unwrap_or(harness_control_protocol::RiskLevel::High),
                    })
                    .map_err(|_| ExecutionError::ApprovalRequired)?;
                let registration_fingerprint = registration.fingerprint.clone();
                self.transition(context, RequestState::AwaitingApproval)?;
                self.emit(context, PipelineEvent::ApprovalRequired, audit_events);
                let resolution = self
                    .await_approval(registration, context.request_id.to_string())
                    .await?;
                if resolution.request_id != context.request_id.to_string()
                    || resolution.fingerprint != registration_fingerprint
                {
                    self.transition(context, RequestState::Failed)?;
                    self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                    return Err(ExecutionError::ApprovalCancelled);
                }
                match resolution.status {
                    harness_control_protocol::ApprovalStatus::Approved => {
                        let decision = resolution
                            .decision
                            .ok_or(ExecutionError::ApprovalCancelled)?;
                        self.emit(
                            context,
                            PipelineEvent::ApprovalResolved { decision },
                            audit_events,
                        );
                        if let Err(error) = self.revalidate_after_approval(context, &executor) {
                            if context.state() == RequestState::AwaitingApproval {
                                context.mark_failure_category(audit_failure_category(&error));
                                self.transition(context, RequestState::Failed)?;
                                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                            }
                            return Err(error);
                        }
                        self.execute_allowed(context, executor, audit_events)
                    }
                    harness_control_protocol::ApprovalStatus::Denied => {
                        self.emit(
                            context,
                            PipelineEvent::ApprovalResolved {
                                decision: harness_control_protocol::ApprovalDecision::Deny,
                            },
                            audit_events,
                        );
                        self.transition(context, RequestState::Denied)?;
                        self.emit(context, PipelineEvent::RequestDenied, audit_events);
                        Err(ExecutionError::ApprovalDenied)
                    }
                    harness_control_protocol::ApprovalStatus::Expired => {
                        self.transition(context, RequestState::Failed)?;
                        self.emit(context, PipelineEvent::ApprovalExpired, audit_events);
                        Err(ExecutionError::ApprovalExpired)
                    }
                    harness_control_protocol::ApprovalStatus::Cancelled => {
                        self.transition(context, RequestState::Failed)?;
                        self.emit(context, PipelineEvent::ApprovalCancelled, audit_events);
                        Err(ExecutionError::ApprovalCancelled)
                    }
                    harness_control_protocol::ApprovalStatus::Pending => {
                        self.transition(context, RequestState::Failed)?;
                        self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                        Err(ExecutionError::ApprovalCancelled)
                    }
                }
            }
        }
    }

    async fn await_approval(
        &self,
        registration: crate::approval::RegisteredApproval,
        request_id: String,
    ) -> Result<ApprovalResolution, ExecutionError> {
        let cancellation_guard = ApprovalWaitGuard {
            manager: Arc::clone(&self.approval_manager),
            request_id,
        };
        let receiver = registration.receiver;
        tokio::pin!(receiver);
        let timer = tokio::time::sleep(self.approval_manager.ttl());
        tokio::pin!(timer);
        let result = tokio::select! {
            result = &mut receiver => result.map_err(|_| ExecutionError::ApprovalCancelled),
            _ = &mut timer => {
                self.approval_manager.expire_due();
                receiver.await.map_err(|_| ExecutionError::ApprovalExpired)
            }
        };
        drop(cancellation_guard);
        result
    }

    fn revalidate_after_approval<E: ToolExecutor>(
        &self,
        context: &mut RequestContext,
        executor: &E,
    ) -> Result<(), ExecutionError> {
        self.require_ready()?;
        if self.registry.is_disabled(E::TOOL_NAME) {
            context.mark_failure_category(AuditFailureCategory::ToolDisabled);
            return Err(ExecutionError::ToolDisabled);
        }
        match executor.workspace_target() {
            WorkspaceTarget::ExistingPath(path) => self
                .guard
                .approve(path)
                .map(|_| ())
                .map_err(E::workspace_error)?,
            WorkspaceTarget::WriteTarget(path) => self
                .guard
                .approve_write_target(path)
                .map(|_| ())
                .map_err(E::workspace_error)?,
        }
        let evaluation = self.policy.evaluate(&context.policy, &self.registry);
        context.policy_evaluation = Some(evaluation.clone());
        if evaluation.decision == PolicyDecision::Deny {
            self.transition(context, RequestState::Denied)?;
            return Err(ExecutionError::PolicyDenied);
        }
        Ok(())
    }

    fn execute_allowed<E: ToolExecutor>(
        &self,
        context: &mut RequestContext,
        executor: E,
        audit_events: &mut Vec<AuditEvent>,
    ) -> Result<E::Output, ExecutionError> {
        if let Err(error) = self.require_ready() {
            if context.state() == RequestState::AwaitingApproval {
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
            }
            return Err(error);
        }
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
                context.mark_failure_category(audit_failure_category(&error));
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                Err(error)
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
            if let Some(sender) = &self.activity_event_sender {
                if let Some(input) = ActivityEventInput::from_audit(&audit_event) {
                    let _ = sender.send(input);
                }
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

struct ApprovalWaitGuard {
    manager: Arc<ApprovalManager>,
    request_id: String,
}

impl Drop for ApprovalWaitGuard {
    fn drop(&mut self) {
        self.manager.cancel(&self.request_id);
    }
}

fn audit_failure_category(error: &ExecutionError) -> AuditFailureCategory {
    match error {
        ExecutionError::ReadFile(ReadFileError::Workspace(_))
        | ExecutionError::Glob(GlobError::Workspace(_))
        | ExecutionError::Write(WriteError::Workspace(_))
        | ExecutionError::Edit(EditError::Workspace(_)) => AuditFailureCategory::WorkspaceViolation,
        ExecutionError::Write(WriteError::TargetChanged { .. })
        | ExecutionError::Edit(EditError::TargetChanged { .. }) => {
            AuditFailureCategory::TargetChanged
        }
        ExecutionError::Write(WriteError::AtomicFailure { .. })
        | ExecutionError::Edit(EditError::AtomicFailure { .. }) => {
            AuditFailureCategory::AtomicFailure
        }
        ExecutionError::ReadFile(ReadFileError::IoFailure { .. })
        | ExecutionError::Glob(GlobError::IoFailure { .. })
        | ExecutionError::Write(WriteError::IoFailure { .. })
        | ExecutionError::Edit(EditError::IoFailure { .. }) => AuditFailureCategory::IoFailure,
        ExecutionError::ToolDisabled => AuditFailureCategory::ToolDisabled,
        ExecutionError::InvalidArguments
        | ExecutionError::ReadFile(ReadFileError::InvalidArguments { .. })
        | ExecutionError::Write(WriteError::InvalidArguments { .. })
        | ExecutionError::Edit(EditError::InvalidArguments { .. }) => {
            AuditFailureCategory::InvalidArguments
        }
        _ => AuditFailureCategory::ExecutionFailed,
    }
}
