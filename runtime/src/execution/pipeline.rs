use super::{
    BashEvidenceSnapshot, RequestContext, RequestState, SecurityContext, ToolExecutor,
    TransitionError, WorkspaceTarget,
};
use crate::approval::{ApprovalFacts, ApprovalManager, ApprovalResolution};
use crate::audit::{AuditEvent, AuditFailureCategory, AuditSink};
use crate::bash::{BashRuntime, BashRuntimeError};
use crate::events::{ActivityEventInput, ActivityEventSender};
use crate::policy::PolicyEngine;
use crate::session_grant::SessionGrantStore;
use crate::tools::{
    EditError, EffectiveTool, EffectiveToolCatalog, GlobError, GrepError, ReadFileError,
    ToolRegistry, WriteError,
};
use crate::workspace::WorkspaceGuard;
use harness_control_protocol::PolicyDecision;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio_util::sync::CancellationToken;

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
    Grep(GrepError),
    Write(crate::tools::WriteError),
    Edit(crate::tools::EditError),
    BashWorkspace(crate::workspace::WorkspaceError),
    BashRuntime(BashRuntimeError),
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
    ExecutionCancelled,
    SessionGrantMatched,
    RequestFinished,
}

pub(crate) trait ExecutionHook: Send + Sync {
    fn on_event(&self, context: &RequestContext, event: PipelineEvent);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BashCancellationKind {
    ApprovalWait,
    ExecutionWait,
}

/// Optional Runtime-owned sink for the one cancellation path where an MCP
/// future is dropped before `execute_async` can emit its normal terminal event.
/// The observer receives only the owned sanitized snapshot.
pub(crate) trait BashCancellationObserver: Send + Sync {
    fn on_bash_cancelled(&self, snapshot: BashEvidenceSnapshot, kind: BashCancellationKind);
}

struct NoopBashCancellationObserver;
impl BashCancellationObserver for NoopBashCancellationObserver {
    fn on_bash_cancelled(&self, _snapshot: BashEvidenceSnapshot, _kind: BashCancellationKind) {}
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
    counters: Arc<super::RequestCounters>,
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
    bash_runtime: Option<Arc<BashRuntime>>,
    bash_cancellation_observer: Arc<dyn BashCancellationObserver>,
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
            counters: Arc::new(super::RequestCounters::default()),
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
            bash_runtime: None,
            bash_cancellation_observer: Arc::new(NoopBashCancellationObserver),
        }
    }

    pub(crate) fn with_counters(mut self, counters: Arc<super::RequestCounters>) -> Self {
        self.counters = counters;
        self
    }
    #[cfg(test)]
    pub(crate) fn request_counts(&self) -> (u32, u32) {
        self.counters.snapshot()
    }

    pub(crate) fn with_bash_runtime(mut self, bash_runtime: Arc<BashRuntime>) -> Self {
        self.bash_runtime = Some(bash_runtime);
        self
    }

    pub(crate) fn with_bash_cancellation_observer(
        mut self,
        observer: Arc<dyn BashCancellationObserver>,
    ) -> Self {
        self.bash_cancellation_observer = observer;
        self
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

    /// Records a well-formed MCP `tools/call` whose capability cannot be
    /// resolved.  The resolver deliberately uses the bounded logical label
    /// `unknown` rather than copying the caller-supplied name into durable
    /// evidence.  No WorkspaceGuard, PolicyEngine, ApprovalManager or
    /// executor is invoked for this observation.
    pub(crate) fn observe_unknown_tool(&self) -> uuid::Uuid {
        let _active = self.counters.active_guard();
        let mut context = RequestContext::new("unknown", self.security.clone());
        context.tool_enabled = false;
        context.capability = None;
        let mut audit_events = Vec::new();
        self.emit(&context, PipelineEvent::RequestReceived, &mut audit_events);
        context.mark_failure_category(AuditFailureCategory::ExecutionFailed);
        let _ = self.transition(&mut context, RequestState::Validating);
        let _ = self.transition(&mut context, RequestState::Failed);
        self.emit(&context, PipelineEvent::ExecutionFailed, &mut audit_events);
        self.emit(&context, PipelineEvent::RequestFinished, &mut audit_events);
        context.request_id
    }

    /// Parsing happens inside Validating, so invalid MCP payloads get the same
    /// lifecycle as typed Runtime calls without copying payloads into context.
    pub(crate) fn execute<E: ToolExecutor>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
    ) -> ExecutionReport<E::Output> {
        let _active = self.counters.active_guard();
        let mut context = RequestContext::new(E::TOOL_NAME, self.security.clone());
        context.tool_enabled = !self.registry.is_disabled(E::TOOL_NAME);
        context.capability = self
            .registry
            .declared_capability(E::TOOL_NAME)
            .map(|capability| {
                use crate::tools::ToolCapability as R;
                use harness_control_protocol::control_read::ToolCapability as W;
                match capability {
                    R::WorkspaceRead => W::WorkspaceRead,
                    R::WorkspaceWrite => W::WorkspaceWrite,
                    R::DeveloperShell => W::DeveloperShell,
                }
            });
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
        if let Err(error) = &result {
            if context.state() == RequestState::Failed
                && !audit_events.last().is_some_and(|e| e.duration_ms.is_some())
            {
                context.mark_failure_category(audit_failure_category(error));
                self.emit(&context, PipelineEvent::ExecutionFailed, &mut audit_events);
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
    pub(crate) async fn execute_async<E: ToolExecutor + Send + Sync>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
    ) -> ExecutionReport<E::Output>
    where
        E::Output: Send,
    {
        self.execute_async_cancellable(prepare, CancellationToken::new())
            .await
    }

    pub(crate) async fn execute_async_cancellable<E: ToolExecutor + Send + Sync>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
        request_cancel: CancellationToken,
    ) -> ExecutionReport<E::Output>
    where
        E::Output: Send,
    {
        let _active = self.counters.active_guard();
        let mut context = RequestContext::new(E::TOOL_NAME, self.security.clone());
        context.tool_enabled = !self.registry.is_disabled(E::TOOL_NAME);
        context.capability = self
            .registry
            .declared_capability(E::TOOL_NAME)
            .map(|capability| {
                use crate::tools::ToolCapability as R;
                use harness_control_protocol::control_read::ToolCapability as W;
                match capability {
                    R::WorkspaceRead => W::WorkspaceRead,
                    R::WorkspaceWrite => W::WorkspaceWrite,
                    R::DeveloperShell => W::DeveloperShell,
                }
            });
        let mut audit_events = Vec::new();
        let cancellation_guard = RequestCancellationGuard::new(
            request_cancel.clone(),
            Arc::clone(&self.bash_cancellation_observer),
        );
        self.emit(&context, PipelineEvent::RequestReceived, &mut audit_events);
        let mut result = self
            .run_async(
                &mut context,
                prepare,
                &mut audit_events,
                request_cancel,
                cancellation_guard.state(),
            )
            .await;
        cancellation_guard.disarm();
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
        if let Err(error) = &result {
            if context.state() == RequestState::Failed
                && !audit_events.last().is_some_and(|e| e.duration_ms.is_some())
            {
                context.mark_failure_category(audit_failure_category(error));
                self.emit(&context, PipelineEvent::ExecutionFailed, &mut audit_events);
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
            WorkspaceTarget::WorkspaceRoot => self.guard.validate_workspace_root().map(|_| ()),
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
        if E::TOOL_NAME == "bash" {
            if let Some(runtime) = &self.bash_runtime {
                if let Err(error) = runtime.generation_readiness() {
                    let error = ExecutionError::BashRuntime(error);
                    context.mark_failure_category(audit_failure_category(&error));
                    self.transition(context, RequestState::Failed)?;
                    self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                    return Err(error);
                }
            } else {
                let error = ExecutionError::BashRuntime(BashRuntimeError::Unavailable);
                context.mark_failure_category(audit_failure_category(&error));
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                return Err(error);
            }
        }
        if E::TOOL_NAME == "bash" {
            if let Ok(facts) = executor.approval_facts(&self.guard) {
                if let Some(details) = facts.bash_details {
                    context.set_bash_details(details);
                }
            }
        }
        context.sanitized_arguments = executor.arguments_evidence(&self.guard);
        self.transition(context, RequestState::PolicyEvaluating)?;
        let policy_context = executor.policy_context(&self.security);
        let evaluation = self.policy.evaluate(&policy_context, &self.registry);
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
                let _running = self.counters.running_guard();
                self.emit(context, PipelineEvent::ExecutionStarted, audit_events);
                let permit = ExecutionPermit { _private: () };
                match executor.execute(&permit, &self.guard, self.max_read_bytes) {
                    Ok(output) => {
                        executor.record_output(context, &output);
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
        request_cancel: CancellationToken,
        cancellation_state: Arc<BashCancellationState>,
    ) -> Result<E::Output, ExecutionError>
    where
        E: Sync,
        E::Output: Send,
    {
        self.transition(context, RequestState::Validating)?;
        if E::TOOL_NAME == "bash" {
            if let Some(snapshot) = context.bash_evidence_snapshot() {
                cancellation_state.set_snapshot(snapshot);
            }
        }
        let executor = if E::TOOL_NAME == "bash" {
            let executor = match prepare() {
                Ok(executor) => executor,
                Err(error) => {
                    context.mark_failure_category(audit_failure_category(&error));
                    self.transition(context, RequestState::Failed)?;
                    self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                    return Err(error);
                }
            };
            if let Err(error) = executor.validate() {
                context.mark_failure_category(audit_failure_category(&error));
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                return Err(error);
            }
            if !matches!(
                &self.security.mode,
                harness_control_protocol::SecurityMode::DeveloperTest
            ) {
                let error = ExecutionError::BashRuntime(BashRuntimeError::Unavailable);
                context.mark_failure_category(audit_failure_category(&error));
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                return Err(error);
            }
            if self.registry.is_disabled(E::TOOL_NAME) {
                context.mark_failure_category(AuditFailureCategory::ToolDisabled);
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                return Err(ExecutionError::ToolDisabled);
            }
            if let Err(error) = self.require_ready() {
                context.mark_failure_category(audit_failure_category(&error));
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                return Err(error);
            }
            executor
        } else {
            self.require_ready()?;
            if self.registry.is_disabled(E::TOOL_NAME) {
                context.mark_failure_category(AuditFailureCategory::ToolDisabled);
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                return Err(ExecutionError::ToolDisabled);
            }
            let executor = prepare()?;
            executor.validate()?;
            executor
        };
        self.transition(context, RequestState::WorkspaceValidating)?;
        let workspace_validation = match executor.workspace_target() {
            WorkspaceTarget::ExistingPath(path) => self.guard.approve(path).map(|_| ()),
            WorkspaceTarget::WriteTarget(path) => self.guard.approve_write_target(path).map(|_| ()),
            WorkspaceTarget::WorkspaceRoot => self.guard.validate_workspace_root().map(|_| ()),
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
                if E::TOOL_NAME == "bash" {
                    self.transition(context, RequestState::Failed)?;
                    self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                }
                return Err(E::workspace_error(error));
            }
        }
        if E::TOOL_NAME == "bash" {
            if let Some(runtime) = &self.bash_runtime {
                if let Err(error) = runtime.generation_readiness() {
                    let error = ExecutionError::BashRuntime(error);
                    context.mark_failure_category(audit_failure_category(&error));
                    self.transition(context, RequestState::Failed)?;
                    self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                    return Err(error);
                }
            } else {
                let error = ExecutionError::BashRuntime(BashRuntimeError::Unavailable);
                context.mark_failure_category(audit_failure_category(&error));
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                return Err(error);
            }
        }
        let bash_facts = if E::TOOL_NAME == "bash" {
            let facts = executor.approval_facts(&self.guard)?;
            if let Some(details) = facts.bash_details.clone() {
                context.set_bash_details(details);
            }
            Some(facts)
        } else {
            None
        };
        if let Some(snapshot) = context.bash_evidence_snapshot() {
            cancellation_state.set_snapshot(snapshot);
        }
        context.sanitized_arguments = executor.arguments_evidence(&self.guard);
        self.transition(context, RequestState::PolicyEvaluating)?;
        let policy_context = executor.policy_context(&self.security);
        let evaluation = self.policy.evaluate(&policy_context, &self.registry);
        let decision = evaluation.decision.clone();
        context.policy_evaluation = Some(evaluation);
        if let Some(snapshot) = context.bash_evidence_snapshot() {
            cancellation_state.set_snapshot(snapshot);
        }
        self.emit(context, PipelineEvent::PolicyDecision, audit_events);
        match decision {
            PolicyDecision::Deny => {
                self.transition(context, RequestState::Denied)?;
                self.emit(context, PipelineEvent::RequestDenied, audit_events);
                Err(ExecutionError::PolicyDenied)
            }
            PolicyDecision::Allow => {
                cancellation_state.set_kind(BashCancellationKind::ExecutionWait);
                self.execute_allowed_async(context, executor, audit_events, request_cancel)
                    .await
            }
            PolicyDecision::Ask => {
                let tool_facts = match bash_facts {
                    Some(facts) => facts,
                    None => executor.approval_facts(&self.guard)?,
                };
                let grant_scope = tool_facts.grant_scope.as_deref();
                if grant_scope.is_some_and(|scope| {
                    self.session_grants.matches(
                        self.approval_manager.generation_id(),
                        E::TOOL_NAME,
                        scope,
                    )
                }) {
                    context.mark_session_grant_hit();
                    if let Some(snapshot) = context.bash_evidence_snapshot() {
                        cancellation_state.set_snapshot(snapshot);
                    }
                    self.emit(context, PipelineEvent::SessionGrantMatched, audit_events);
                    cancellation_state.set_kind(BashCancellationKind::ExecutionWait);
                    return self
                        .execute_allowed_async(context, executor, audit_events, request_cancel)
                        .await;
                }

                let registration = self
                    .approval_manager
                    .register(ApprovalFacts {
                        request_id: context.request_id,
                        tool: E::TOOL_NAME.into(),
                        sanitized_arguments: tool_facts.sanitized_arguments,
                        scope_summary: tool_facts.scope_summary,
                        fingerprint: tool_facts.fingerprint,
                        grant_scope: tool_facts.grant_scope,
                        bash_details: tool_facts.bash_details,
                        allow_for_session_eligible: tool_facts.allow_for_session_eligible,
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
                context.set_approval_request_id(&registration.request);
                if let Some(snapshot) = context.bash_evidence_snapshot() {
                    cancellation_state.set_snapshot(snapshot);
                }
                cancellation_state.set_kind(BashCancellationKind::ApprovalWait);
                let registration_fingerprint = registration.fingerprint.clone();
                self.transition(context, RequestState::AwaitingApproval)?;
                self.emit(context, PipelineEvent::ApprovalRequired, audit_events);
                let resolution = match self
                    .await_approval(
                        registration,
                        context.request_id.to_string(),
                        request_cancel.clone(),
                    )
                    .await
                {
                    Ok(resolution) => resolution,
                    Err(error) => {
                        if context.state() == RequestState::AwaitingApproval {
                            context.mark_failure_category(audit_failure_category(&error));
                            self.transition(context, RequestState::Failed)?;
                            if matches!(error, ExecutionError::ApprovalExpired) {
                                self.emit(context, PipelineEvent::ApprovalExpired, audit_events);
                            } else {
                                self.emit(context, PipelineEvent::ApprovalCancelled, audit_events);
                            }
                        }
                        return Err(error);
                    }
                };
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
                        context.approval_decision = Some(decision);
                        self.emit(
                            context,
                            PipelineEvent::ApprovalResolved { decision },
                            audit_events,
                        );
                        if let Err(error) = self.revalidate_after_approval(
                            context,
                            &executor,
                            Some(&registration_fingerprint),
                        ) {
                            if context.state() == RequestState::AwaitingApproval {
                                context.mark_failure_category(audit_failure_category(&error));
                                self.transition(context, RequestState::Failed)?;
                                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                            }
                            return Err(error);
                        }
                        cancellation_state.set_kind(BashCancellationKind::ExecutionWait);
                        self.execute_allowed_async(context, executor, audit_events, request_cancel)
                            .await
                    }
                    harness_control_protocol::ApprovalStatus::Denied => {
                        context.approval_decision =
                            Some(harness_control_protocol::ApprovalDecision::Deny);
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
        request_cancel: CancellationToken,
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
            _ = request_cancel.cancelled() => Err(ExecutionError::ApprovalCancelled),
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
        expected_fingerprint: Option<&str>,
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
            WorkspaceTarget::WorkspaceRoot => self
                .guard
                .validate_workspace_root()
                .map(|_| ())
                .map_err(E::workspace_error)?,
        }
        if E::TOOL_NAME == "bash" {
            if !matches!(
                &self.security.mode,
                harness_control_protocol::SecurityMode::DeveloperTest
            ) {
                return Err(ExecutionError::BashRuntime(BashRuntimeError::Unavailable));
            }
            let runtime = self
                .bash_runtime
                .as_ref()
                .ok_or(ExecutionError::BashRuntime(BashRuntimeError::Unavailable))?;
            runtime
                .generation_readiness()
                .map_err(ExecutionError::BashRuntime)?;
            executor.validate()?;
            if let Some(expected) = expected_fingerprint {
                let actual = executor.approval_facts(&self.guard)?.fingerprint;
                if actual != expected {
                    return Err(ExecutionError::ApprovalCancelled);
                }
            }
        }
        let policy_context = executor.policy_context(&self.security);
        let evaluation = self.policy.evaluate(&policy_context, &self.registry);
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
        let _running = self.counters.running_guard();
        self.emit(context, PipelineEvent::ExecutionStarted, audit_events);
        let permit = ExecutionPermit { _private: () };
        match executor.execute(&permit, &self.guard, self.max_read_bytes) {
            Ok(output) => {
                executor.record_output(context, &output);
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

    async fn execute_allowed_async<E: ToolExecutor + Send + Sync>(
        &self,
        context: &mut RequestContext,
        executor: E,
        audit_events: &mut Vec<AuditEvent>,
        request_cancel: CancellationToken,
    ) -> Result<E::Output, ExecutionError>
    where
        E::Output: Send,
    {
        if let Err(error) = self.require_ready() {
            if context.state() == RequestState::AwaitingApproval {
                self.transition(context, RequestState::Failed)?;
                self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
            }
            return Err(error);
        }
        self.transition(context, RequestState::Executing)?;
        let _running = self.counters.running_guard();
        self.emit(context, PipelineEvent::ExecutionStarted, audit_events);
        let permit = ExecutionPermit { _private: () };
        let started = std::time::Instant::now();
        let result = executor
            .execute_async(
                &permit,
                &self.guard,
                self.max_read_bytes,
                self.bash_runtime.clone(),
                request_cancel,
            )
            .await;
        match result {
            Ok(output) => {
                executor.record_output(context, &output);
                self.transition(context, RequestState::Completed)?;
                self.emit(context, PipelineEvent::ExecutionCompleted, audit_events);
                Ok(output)
            }
            Err(error) => {
                if E::TOOL_NAME == "bash" {
                    context.set_bash_duration(
                        started.elapsed().as_millis().min(u64::MAX as u128) as u64
                    );
                }
                context.mark_failure_category(audit_failure_category(&error));
                self.transition(context, RequestState::Failed)?;
                if matches!(
                    error,
                    ExecutionError::BashRuntime(BashRuntimeError::Cancelled)
                        | ExecutionError::Grep(GrepError::Cancelled)
                ) {
                    self.emit(context, PipelineEvent::ExecutionCancelled, audit_events);
                } else {
                    self.emit(context, PipelineEvent::ExecutionFailed, audit_events);
                }
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

struct RequestCancellationGuard {
    token: CancellationToken,
    armed: std::sync::atomic::AtomicBool,
    state: Arc<BashCancellationState>,
    observer: Arc<dyn BashCancellationObserver>,
}

struct BashCancellationState {
    snapshot: std::sync::Mutex<Option<BashEvidenceSnapshot>>,
    kind: std::sync::Mutex<BashCancellationKind>,
    execution_started_at: std::sync::Mutex<Option<std::time::Instant>>,
}

impl BashCancellationState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            snapshot: std::sync::Mutex::new(None),
            kind: std::sync::Mutex::new(BashCancellationKind::ExecutionWait),
            execution_started_at: std::sync::Mutex::new(None),
        })
    }

    fn set_snapshot(&self, snapshot: BashEvidenceSnapshot) {
        *self
            .snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(snapshot);
    }

    fn set_kind(&self, kind: BashCancellationKind) {
        *self.kind.lock().unwrap_or_else(|error| error.into_inner()) = kind;
        if kind == BashCancellationKind::ExecutionWait {
            self.execution_started_at
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get_or_insert_with(std::time::Instant::now);
        }
    }
}

impl RequestCancellationGuard {
    fn new(token: CancellationToken, observer: Arc<dyn BashCancellationObserver>) -> Self {
        Self {
            token,
            armed: std::sync::atomic::AtomicBool::new(true),
            state: BashCancellationState::new(),
            observer,
        }
    }

    fn state(&self) -> Arc<BashCancellationState> {
        Arc::clone(&self.state)
    }

    fn disarm(&self) {
        self.armed.store(false, Ordering::Release);
    }
}

impl Drop for RequestCancellationGuard {
    fn drop(&mut self) {
        if self.armed.swap(false, Ordering::AcqRel) {
            self.token.cancel();
            let snapshot = self
                .state
                .snapshot
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            let kind = *self
                .state
                .kind
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(mut snapshot) = snapshot {
                snapshot.duration_ms = if snapshot.tool_name == "bash"
                    && kind == BashCancellationKind::ExecutionWait
                {
                    self.state
                        .execution_started_at
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .as_ref()
                        .map(|start| start.elapsed().as_millis().min(u64::MAX as u128) as u64)
                } else {
                    None
                };
                self.observer.on_bash_cancelled(snapshot, kind);
            }
        }
    }
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
        | ExecutionError::Grep(GrepError::Workspace(_))
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
        | ExecutionError::Grep(GrepError::IoFailure { .. })
        | ExecutionError::Write(WriteError::IoFailure { .. })
        | ExecutionError::Edit(EditError::IoFailure { .. }) => AuditFailureCategory::IoFailure,
        ExecutionError::BashWorkspace(_) => AuditFailureCategory::WorkspaceViolation,
        ExecutionError::BashRuntime(error) => match error {
            BashRuntimeError::Unavailable => AuditFailureCategory::BashUnavailable,
            BashRuntimeError::InvalidCommand => AuditFailureCategory::BashInvalidCommand,
            BashRuntimeError::CommandTooLong => AuditFailureCategory::BashCommandTooLong,
            BashRuntimeError::Timeout => AuditFailureCategory::BashTimeout,
            BashRuntimeError::OutputLimit => AuditFailureCategory::BashOutputLimit,
            BashRuntimeError::ConcurrencyLimit => AuditFailureCategory::BashConcurrencyLimit,
            BashRuntimeError::SpawnFailed => AuditFailureCategory::BashSpawnFailed,
            BashRuntimeError::Cancelled => AuditFailureCategory::BashCancelled,
            BashRuntimeError::ProcessFailed => AuditFailureCategory::BashProcessFailed,
        },
        ExecutionError::Grep(GrepError::SearchTimeout) => AuditFailureCategory::GrepTimeout,
        ExecutionError::Grep(GrepError::Cancelled) => AuditFailureCategory::GrepCancelled,
        ExecutionError::ToolDisabled => AuditFailureCategory::ToolDisabled,
        ExecutionError::InvalidArguments
        | ExecutionError::ReadFile(ReadFileError::InvalidArguments { .. })
        | ExecutionError::Grep(GrepError::InvalidArguments { .. })
        | ExecutionError::Write(WriteError::InvalidArguments { .. })
        | ExecutionError::Edit(EditError::InvalidArguments { .. }) => {
            AuditFailureCategory::InvalidArguments
        }
        _ => AuditFailureCategory::ExecutionFailed,
    }
}
