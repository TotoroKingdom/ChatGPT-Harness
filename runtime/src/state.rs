use crate::activity::{ActivityEntry, ActivityStore};
use crate::approval::ApprovalManager;
use crate::audit::{
    AuditEvent, AuditEventType, AuditFailureCategory, AuditLifecycleState, AuditOutcome, AuditSink,
    WorkspaceValidation,
};
use crate::bash::{resolve_bash, BashExecutableState, BashRuntime};
use crate::events::{ActivityEventInput, ActivityEventSender, EventBus};
use crate::execution::{
    BashCancellationKind, BashCancellationObserver, BashEvidenceSnapshot, ExecutionError,
    ExecutionPipeline, SecurityContext,
};
use crate::mcp::{McpContextError, McpExecutionContext, McpService};
use crate::policy::PolicyEngine;
use crate::session_grant::SessionGrantStore;
use crate::tools::{
    EditOutput, EditRequest, EffectiveTool, EffectiveToolCatalog, GlobOutput, GlobRequest,
    ReadFileOutput, ReadFileRequest, ToolAvailability, ToolRegistry, WriteOutput, WriteRequest,
};
use crate::workspace::{Workspace, WorkspaceError, WorkspaceGuard};
use harness_control_protocol::{
    ActivityEvent, ActivityEventType, RuntimeConfigSnapshot, RuntimeErrorSummary, RuntimeLifecycle,
    RuntimeReadiness, RuntimeState,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    WaitingInitialize,
    InitializedPendingAck,
    Ready,
    Stopping,
}

struct RuntimeBashCancellationObserver {
    audit_sink: Arc<dyn AuditSink>,
    activity_sender: ActivityEventSender,
}

impl BashCancellationObserver for RuntimeBashCancellationObserver {
    fn on_bash_cancelled(&self, snapshot: BashEvidenceSnapshot, kind: BashCancellationKind) {
        let approval_wait = matches!(kind, BashCancellationKind::ApprovalWait);
        let bash = snapshot.details.map(|details| {
            harness_control_protocol::control_read::BashActivityEvidence {
                mode: snapshot.mode.clone(),
                mode_eligible: snapshot.mode
                    == harness_control_protocol::SecurityMode::DeveloperTest,
                capability: harness_control_protocol::control_read::ToolCapability::DeveloperShell,
                cwd: details.cwd,
                command_preview: Some(details.command_preview),
                classification: Some(details.classification),
                risk_flags: details.risk_flags,
                approval_request_id: snapshot.approval_request_id.clone(),
                session_grant_hit: snapshot.session_grant_hit,
                exit_code: snapshot.exit_code,
                duration_ms: snapshot.duration_ms,
            }
        });
        let event = AuditEvent {
            event_id: uuid::Uuid::new_v4(),
            request_id: snapshot.request_id,
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            event_type: if approval_wait {
                AuditEventType::ApprovalCancelled
            } else {
                AuditEventType::ExecutionFailed
            },
            tool_name: "bash".into(),
            lifecycle_state: AuditLifecycleState::Failed,
            workspace_id: "active".into(),
            workspace_validation: WorkspaceValidation::NotAttempted,
            policy_decision: snapshot
                .policy_evaluation
                .as_ref()
                .map(|evaluation| evaluation.decision.clone()),
            matched_rule_ids: snapshot
                .policy_evaluation
                .as_ref()
                .map(|evaluation| evaluation.matched_rule_ids.clone())
                .unwrap_or_default(),
            policy_reason: snapshot
                .policy_evaluation
                .as_ref()
                .map(|evaluation| crate::policy::to_wire_reason(evaluation.reason)),
            approval_decision: None,
            outcome: AuditOutcome::Failed,
            failure_category: (!approval_wait).then_some(AuditFailureCategory::BashCancelled),
            bash,
        };
        self.audit_sink.on_audit_event(&event);
        if let Some(input) = ActivityEventInput::from_audit(&event) {
            let _ = self.activity_sender.send(input);
        }
    }
}

pub struct RuntimeCore {
    pub runtime_id: String,
    pub started_at: String,
    pub phase: Phase,
    pub config: Option<RuntimeConfigSnapshot>,
    bash_executable: BashExecutableState,
    bash_runtime: Option<Arc<BashRuntime>>,
    pub events: Option<EventBus>,
    activity_event_receiver: Option<Receiver<ActivityEventInput>>,
    workspace_guard: Option<Arc<WorkspaceGuard>>,
    mcp_readiness: Arc<AtomicBool>,
    tool_registry: Arc<ToolRegistry>,
    policy_engine: PolicyEngine,
    execution_pipeline: Option<Arc<ExecutionPipeline>>,
    approval_manager: Arc<ApprovalManager>,
    session_grants: Arc<SessionGrantStore>,
    activity: Arc<ActivityStore>,
    pub last_error: Option<RuntimeErrorSummary>,
}

impl RuntimeCore {
    pub fn new() -> Self {
        let runtime_id = uuid::Uuid::new_v4().to_string();
        let tool_registry = Arc::new(ToolRegistry::normal_mode());
        let policy_engine = PolicyEngine::compile(
            &harness_control_protocol::PolicyConfig::default(),
            &tool_registry,
        );
        Self {
            runtime_id: runtime_id.clone(),
            started_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            phase: Phase::WaitingInitialize,
            config: None,
            bash_executable: BashExecutableState::unavailable(
                crate::bash::discovery::BashUnavailableReason::AutoDetectDisabled,
            ),
            bash_runtime: None,
            events: None,
            activity_event_receiver: None,
            workspace_guard: None,
            mcp_readiness: Arc::new(AtomicBool::new(false)),
            activity: Arc::new(ActivityStore::new(Arc::clone(&tool_registry))),
            tool_registry,
            policy_engine,
            execution_pipeline: None,
            approval_manager: ApprovalManager::new(runtime_id.clone()),
            session_grants: Arc::new(SessionGrantStore::new(runtime_id)),
            last_error: None,
        }
    }

    pub fn initialize(&mut self, config: RuntimeConfigSnapshot) -> Result<(), WorkspaceError> {
        let workspace = Workspace::new(&config.workspace.root_path)?;
        let workspace_guard = Arc::new(WorkspaceGuard::new(&workspace));
        let bash_executable = resolve_bash(&config.bash);
        let workspace_root = workspace_guard
            .validate_workspace_root()
            .map_err(|error| error)?
            .to_path_buf();
        let bash_runtime = Arc::new(BashRuntime::new(
            bash_executable.canonical_path().map(ToOwned::to_owned),
            workspace_root,
        ));
        let tool_registry = Arc::new(ToolRegistry::from_config_with_bash(
            &config.tools,
            &bash_executable,
        ));
        let policy_engine = PolicyEngine::compile(&config.policies, &tool_registry);
        let activity = Arc::new(ActivityStore::with_previous_entries(
            &self.activity,
            Arc::clone(&tool_registry),
        ));
        let audit_sink: Arc<dyn crate::audit::AuditSink> = activity.clone();
        let events = EventBus::from_limits(&config.runtime_limits);
        let (activity_event_sender, activity_event_receiver) = mpsc::channel();
        let bash_cancellation_observer = Arc::new(RuntimeBashCancellationObserver {
            audit_sink: Arc::clone(&audit_sink),
            activity_sender: activity_event_sender.clone(),
        });
        let approval_manager = ApprovalManager::new(self.runtime_id.clone());
        let session_grants = Arc::new(SessionGrantStore::new(self.runtime_id.clone()));

        let pipeline = ExecutionPipeline::new(
            Arc::clone(&workspace_guard),
            Arc::clone(&tool_registry),
            policy_engine.clone(),
            SecurityContext {
                workspace_id: uuid::Uuid::new_v4(),
                mode: config.security.mode.clone(),
            },
            config.runtime_limits.max_read_bytes,
            Arc::clone(&self.mcp_readiness),
        )
        .with_bash_runtime(Arc::clone(&bash_runtime))
        .with_bash_cancellation_observer(bash_cancellation_observer)
        .with_approval_state(Arc::clone(&approval_manager), Arc::clone(&session_grants))
        .with_audit_sink(audit_sink);
        let pipeline = pipeline.with_activity_event_sender(activity_event_sender);
        self.execution_pipeline = Some(Arc::new(pipeline));
        self.tool_registry = tool_registry;
        self.bash_executable = bash_executable;
        self.bash_runtime = Some(bash_runtime);
        self.policy_engine = policy_engine;
        self.approval_manager = approval_manager;
        self.session_grants = session_grants;
        self.activity = activity;
        self.events = Some(events);
        self.activity_event_receiver = Some(activity_event_receiver);
        self.config = Some(config);
        self.workspace_guard = Some(workspace_guard);
        self.phase = Phase::InitializedPendingAck;
        self.mcp_readiness.store(false, Ordering::Release);
        self.last_error = None;
        Ok(())
    }

    /// Reverts a successful initialization when a process-owned service cannot
    /// start. Process identity is intentionally retained.
    pub(crate) fn rollback_initialize(&mut self) {
        self.mcp_readiness.store(false, Ordering::Release);
        self.approval_manager.cancel_all();
        if let Some(runtime) = &self.bash_runtime {
            runtime.stop();
        }
        self.session_grants.clear();
        self.phase = Phase::WaitingInitialize;
        self.config = None;
        self.bash_executable = BashExecutableState::unavailable(
            crate::bash::discovery::BashUnavailableReason::AutoDetectDisabled,
        );
        self.bash_runtime = None;
        self.events = None;
        self.activity_event_receiver = None;
        self.workspace_guard = None;
        self.execution_pipeline = None;
        let tool_registry = Arc::new(ToolRegistry::normal_mode());
        self.policy_engine = PolicyEngine::compile(
            &harness_control_protocol::PolicyConfig::default(),
            &tool_registry,
        );
        self.activity = Arc::new(ActivityStore::with_previous_entries(
            &self.activity,
            Arc::clone(&tool_registry),
        ));
        self.tool_registry = tool_registry;
        self.approval_manager = ApprovalManager::new(self.runtime_id.clone());
        self.session_grants = Arc::new(SessionGrantStore::new(self.runtime_id.clone()));
        self.last_error = None;
    }

    #[allow(dead_code)]
    /// Typed internal calls use the same readiness, boundary and policy gates as MCP.
    pub(crate) fn glob(&self, request: &GlobRequest) -> Result<GlobOutput, ExecutionError> {
        self.execution_pipeline
            .as_ref()
            .ok_or(ExecutionError::RuntimeNotInitialized)?
            .execute(|| Ok(request.clone()))
            .result
    }

    /// Typed internal calls use the same readiness, boundary and policy gates as MCP.
    #[allow(dead_code)]
    pub(crate) fn write(&self, request: &WriteRequest) -> Result<WriteOutput, ExecutionError> {
        self.execution_pipeline
            .as_ref()
            .ok_or(ExecutionError::RuntimeNotInitialized)?
            .execute(|| Ok(request.clone()))
            .result
    }

    /// Typed internal calls use the same readiness, boundary and policy gates as MCP.
    #[allow(dead_code)]
    pub(crate) fn edit(&self, request: &EditRequest) -> Result<EditOutput, ExecutionError> {
        self.execution_pipeline
            .as_ref()
            .ok_or(ExecutionError::RuntimeNotInitialized)?
            .execute(|| Ok(request.clone()))
            .result
    }
    /// Latest N observations in receipt order (oldest to newest), detached
    /// from the store. This is not a readiness or authorization query.
    #[allow(dead_code)] // Internal contract; transport/UI consumption is deferred.
    pub(crate) fn activity_snapshot(&self, limit: usize) -> Vec<ActivityEntry> {
        self.activity.snapshot(limit)
    }

    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    /// Runtime-owned executable readiness for the current immutable
    /// generation.  The path is never sourced directly from persisted config.
    pub(crate) fn bash_executable(&self) -> &BashExecutableState {
        &self.bash_executable
    }

    /// Returns the current generation's Runtime-owned effective policy
    /// projection. It is observational only; callers cannot mutate the engine.
    pub fn effective_policy(&self) -> harness_control_protocol::PolicyEffectiveSnapshot {
        self.policy_engine.effective_snapshot(&self.tool_registry)
    }

    pub fn policy_rules(&self) -> Vec<harness_control_protocol::PolicyRule> {
        self.policy_engine
            .legacy_rules_for_list(&self.tool_registry)
    }

    /// Returns the Runtime's current effective exposure view.
    ///
    /// The MCP context consumes the same initialized Runtime snapshot. Before
    /// the Runtime is ready, every tool remains unavailable by runtime
    /// capability.
    #[allow(dead_code)]
    pub fn effective_tools(&self) -> Vec<EffectiveTool> {
        if let Some(pipeline) = &self.execution_pipeline {
            return pipeline.effective_tools();
        }
        EffectiveToolCatalog::compute(
            &self.tool_registry,
            &self.policy_engine,
            harness_control_protocol::SecurityMode::Normal,
            false,
        )
    }

    #[allow(dead_code)]
    pub fn workspace_guard(&self) -> Option<&WorkspaceGuard> {
        self.workspace_guard.as_deref()
    }

    /// Builds the MCP execution context from the successfully initialized
    /// runtime snapshot. The context shares the single WorkspaceGuard rather
    /// than reconstructing workspace security state for an MCP request.
    #[allow(dead_code)]
    pub fn mcp_context(&self) -> Result<McpExecutionContext, McpContextError> {
        self.execution_pipeline
            .as_ref()
            .cloned()
            .map(McpExecutionContext::new)
            .ok_or(McpContextError::RuntimeNotInitialized)
    }
    #[allow(dead_code)]
    pub fn mcp_service(&self) -> Result<McpService, McpContextError> {
        self.mcp_context().map(McpService::new)
    }

    pub(crate) fn approval_manager(&self) -> Arc<ApprovalManager> {
        Arc::clone(&self.approval_manager)
    }

    pub(crate) fn session_grants(&self) -> Arc<SessionGrantStore> {
        Arc::clone(&self.session_grants)
    }

    pub(crate) fn bash_runtime(&self) -> Option<Arc<BashRuntime>> {
        self.bash_runtime.as_ref().map(Arc::clone)
    }

    #[allow(dead_code)]
    /// Typed internal calls use the same initialized enforcement snapshot as MCP.
    pub(crate) fn read_file(
        &self,
        request: &ReadFileRequest,
    ) -> Result<ReadFileOutput, ExecutionError> {
        self.execution_pipeline
            .as_ref()
            .ok_or(ExecutionError::RuntimeNotInitialized)?
            .execute(|| Ok(request.clone()))
            .result
    }
    pub fn acknowledge(&mut self) -> Option<ActivityEvent> {
        if self.phase != Phase::InitializedPendingAck {
            return None;
        }
        self.phase = Phase::Ready;
        self.mcp_readiness.store(true, Ordering::Release);
        Some(self.publish(
            ActivityEventType::RuntimeStarted,
            "Runtime started.",
            [("source".into(), "runtime".into())],
            None,
        ))
    }

    pub fn stop(&mut self) -> Option<ActivityEvent> {
        self.mcp_readiness.store(false, Ordering::Release);
        self.approval_manager.cancel_all();
        if let Some(runtime) = &self.bash_runtime {
            runtime.stop();
        }
        self.session_grants.clear();
        if self.phase == Phase::Stopping {
            return None;
        }
        self.phase = Phase::Stopping;
        Some(self.publish(
            ActivityEventType::RuntimeStopped,
            "Runtime stopped.",
            [("source".into(), "runtime".into())],
            None,
        ))
    }

    #[cfg(test)]
    fn mcp_is_ready(&self) -> bool {
        self.mcp_readiness.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn set_mcp_readiness_for_test(&self, ready: bool) {
        self.mcp_readiness.store(ready, Ordering::Release);
    }

    pub fn publish(
        &mut self,
        event_type: ActivityEventType,
        summary: impl Into<String>,
        metadata: impl IntoIterator<Item = (String, String)>,
        request_id: Option<String>,
    ) -> ActivityEvent {
        self.events
            .as_mut()
            .expect("event bus exists after initialize")
            .publish(event_type, summary, metadata, request_id)
    }

    /// Materializes activity messages emitted by the HTTP execution thread
    /// into the control-owned EventBus before a control read or stop.
    pub fn drain_activity_events(&mut self) {
        let pending = self
            .activity_event_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in pending {
            self.publish(
                event.event_type,
                event.summary,
                event.metadata,
                event.request_id,
            );
        }
    }

    /// Safe observation only. This does not replace RuntimeState or authorize requests.
    /// A future local control-plane adapter can serialize this contract unchanged.
    #[allow(dead_code)] // M8-A freezes the internal contract; IPC/UI wiring is deferred.
    pub(crate) fn diagnostics(&self) -> crate::diagnostics::RuntimeDiagnosticsSnapshot {
        crate::diagnostics::RuntimeDiagnosticsSnapshot::capture(
            self,
            self.execution_pipeline.is_some(),
            self.mcp_readiness.load(Ordering::Acquire),
        )
    }

    pub fn status(&self) -> Option<RuntimeState> {
        let config = self.config.as_ref()?;
        let pending_approvals = self.approval_manager.pending_count() as u32;
        let (lifecycle, readiness) = match self.phase {
            Phase::WaitingInitialize => {
                (RuntimeLifecycle::Initializing, RuntimeReadiness::NotReady)
            }
            Phase::InitializedPendingAck => (
                RuntimeLifecycle::Initializing,
                RuntimeReadiness::Initializing,
            ),
            Phase::Ready => (RuntimeLifecycle::Ready, RuntimeReadiness::Ready),
            Phase::Stopping => (RuntimeLifecycle::Stopping, RuntimeReadiness::NotReady),
        };
        Some(RuntimeState {
            runtime_id: self.runtime_id.clone(),
            started_at: self.started_at.clone(),
            lifecycle,
            readiness,
            config_revision: config.config_revision,
            active_workspace: config.workspace.root_path.clone(),
            security_mode: config.security.mode.clone(),
            exposed_tool_catalog: self.exposed_tool_catalog(),
            active_requests: 0,
            pending_approvals,
            running_executions: 0,
            last_error: self.last_error.clone(),
        })
    }

    fn exposed_tool_catalog(&self) -> Vec<harness_control_protocol::ToolDescriptor> {
        let mut catalog = self.tool_registry.exposed_catalog();
        let bash_exposed = self
            .execution_pipeline
            .as_ref()
            .and_then(|pipeline| {
                pipeline
                    .effective_tools()
                    .into_iter()
                    .find(|tool| tool.name == "bash")
            })
            .is_some_and(|tool| {
                matches!(
                    tool.availability,
                    ToolAvailability::Available | ToolAvailability::AvailableWithApproval
                )
            });
        if !bash_exposed {
            catalog.retain(|tool| tool.name != "bash");
        }
        catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_control_protocol::{
        DecimalU64, PolicyDecision, PolicyMatcherConfig, RuntimeLimits, SecurityConfig,
        SecurityMode, ToolConfig, UserPolicyRuleConfig, WorkspaceConfig,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "chatgpt-harness-runtime-state-{}",
                uuid::Uuid::new_v4()
            ));
            let root = base.join("workspace");
            fs::create_dir_all(&root).expect("create workspace");
            Self { base, root }
        }

        fn missing_path(&self) -> PathBuf {
            self.base.join("missing")
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn config_for(root: &Path) -> RuntimeConfigSnapshot {
        RuntimeConfigSnapshot {
            config_revision: DecimalU64::new(7),
            workspace: WorkspaceConfig {
                root_path: root.to_string_lossy().into_owned(),
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: 4,
                max_activity_events: 2,
                max_read_bytes: 1_048_576,
            },
            tools: ToolConfig::default(),
            bash: harness_control_protocol::BashConfig::default(),
            policies: harness_control_protocol::PolicyConfig::default(),
        }
    }

    fn user_rule(id: &str, decision: PolicyDecision, tool: &str) -> UserPolicyRuleConfig {
        UserPolicyRuleConfig {
            id: id.into(),
            name: format!("Rule {id}"),
            enabled: true,
            decision,
            matcher: PolicyMatcherConfig::tool(tool),
            reason: "Runtime test policy.".into(),
        }
    }

    #[test]
    fn initialize_with_valid_workspace_sets_all_initialized_state() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();

        core.initialize(config_for(&workspace.root))
            .expect("valid workspace initializes runtime");

        let initialized_config = core.config.as_ref().expect("initialized config");
        assert_eq!(initialized_config.runtime_limits.max_read_bytes, 1_048_576);
        assert!(core.events.is_some());
        assert!(core.workspace_guard().is_some());
        assert_eq!(core.phase, Phase::InitializedPendingAck);
        assert!(!core.mcp_is_ready());
        let catalog = core.status().unwrap().exposed_tool_catalog;
        assert!(!catalog.iter().any(|tool| tool.name == "bash"));
        assert_eq!(
            catalog
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob", "write", "edit"]
        );
    }

    #[test]
    fn effective_tools_are_available_only_after_runtime_acknowledgement() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();

        assert!(core
            .effective_tools()
            .iter()
            .filter(|tool| tool.name != "bash")
            .all(|tool| tool.availability == crate::tools::ToolAvailability::UnavailableByRuntime));
        assert_eq!(
            core.effective_tools()
                .iter()
                .find(|tool| tool.name == "bash")
                .map(|tool| tool.availability.clone()),
            Some(crate::tools::ToolAvailability::UnavailableByMode)
        );

        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        assert!(core
            .effective_tools()
            .iter()
            .filter(|tool| tool.name != "bash")
            .all(|tool| tool.availability == crate::tools::ToolAvailability::UnavailableByRuntime));

        core.acknowledge().expect("acknowledge runtime");
        let effective = core.effective_tools();
        assert_eq!(
            effective
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob", "write", "edit", "bash"]
        );
        assert!(effective
            .iter()
            .filter(|tool| tool.name != "bash")
            .all(|tool| {
                tool.availability == crate::tools::ToolAvailability::Available
                    && tool.decision == harness_control_protocol::PolicyDecision::Allow
            }));
        assert_eq!(
            effective
                .iter()
                .find(|tool| tool.name == "bash")
                .map(|tool| tool.availability.clone()),
            Some(crate::tools::ToolAvailability::UnavailableByMode)
        );

        core.stop().expect("stop runtime");
        assert!(core
            .effective_tools()
            .iter()
            .filter(|tool| tool.name != "bash")
            .all(|tool| tool.availability == crate::tools::ToolAvailability::UnavailableByRuntime));
    }

    #[test]
    fn initialized_registry_uses_snapshot_tool_state_and_registers_bash_last() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.tools.read_file.enabled = false;
        config.tools.glob.enabled = false;
        config.tools.write.enabled = false;
        config.tools.edit.enabled = true;
        config.tools.bash.enabled = true;
        let mut core = RuntimeCore::new();

        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");

        let descriptors = core.tool_registry().list();
        assert_eq!(
            descriptors
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob", "write", "edit", "bash"]
        );
        assert!(!descriptors[0].enabled);
        assert!(!descriptors[1].enabled);
        assert!(!descriptors[2].enabled);
        assert!(descriptors[3].enabled);
        assert!(descriptors[4].enabled);
        assert_eq!(
            core.tool_registry().declared_capability("bash"),
            Some(crate::tools::ToolCapability::DeveloperShell)
        );
        assert_eq!(
            core.tool_registry().declared_capability("write"),
            Some(crate::tools::ToolCapability::WorkspaceWrite)
        );
        assert_eq!(core.tool_registry().capability("write"), None);

        let effective = core.effective_tools();
        assert_eq!(
            effective
                .iter()
                .map(|tool| tool.availability.clone())
                .collect::<Vec<_>>(),
            [
                crate::tools::ToolAvailability::UnavailableDisabled,
                crate::tools::ToolAvailability::UnavailableDisabled,
                crate::tools::ToolAvailability::UnavailableDisabled,
                crate::tools::ToolAvailability::Available,
                crate::tools::ToolAvailability::UnavailableByMode,
            ]
        );
    }

    #[test]
    fn current_generation_keeps_snapshot_after_external_config_changes() {
        let workspace = TestWorkspace::new();
        let config_a = config_for(&workspace.root);
        let mut config_b = config_a.clone();
        config_b.tools.write.enabled = false;

        let mut generation_a = RuntimeCore::new();
        generation_a
            .initialize(config_a)
            .expect("initialize generation A");
        generation_a
            .acknowledge()
            .expect("acknowledge generation A");
        assert!(generation_a.tool_registry().capability("write").is_some());

        let mut generation_b = RuntimeCore::new();
        generation_b
            .initialize(config_b)
            .expect("initialize generation B");
        generation_b
            .acknowledge()
            .expect("acknowledge generation B");
        assert_eq!(generation_b.tool_registry().capability("write"), None);
        assert!(generation_a.tool_registry().capability("write").is_some());
    }

    #[test]
    fn current_generation_keeps_compiled_policy_snapshot_immutable() {
        let workspace = TestWorkspace::new();
        let mut config_a = config_for(&workspace.root);
        config_a.policies.tool_defaults.write = PolicyDecision::Deny;
        let mut generation_a = RuntimeCore::new();
        generation_a
            .initialize(config_a)
            .expect("initialize generation A");
        generation_a
            .acknowledge()
            .expect("acknowledge generation A");

        let mut config_b = config_for(&workspace.root);
        config_b.policies.tool_defaults.write = PolicyDecision::Allow;
        config_b
            .policies
            .user_rules
            .push(user_rule("write-ask", PolicyDecision::Ask, "write"));
        let mut generation_b = RuntimeCore::new();
        generation_b
            .initialize(config_b)
            .expect("initialize generation B");
        generation_b
            .acknowledge()
            .expect("acknowledge generation B");

        let write_a = generation_a
            .effective_policy()
            .per_tool
            .into_iter()
            .find(|tool| tool.tool == "write")
            .expect("write policy A");
        let write_b = generation_b
            .effective_policy()
            .per_tool
            .into_iter()
            .find(|tool| tool.tool == "write")
            .expect("write policy B");
        assert_eq!(write_a.decision, PolicyDecision::Deny);
        assert_eq!(write_b.decision, PolicyDecision::Ask);
        assert_eq!(write_b.matched_rule_ids, ["write-ask"]);
    }

    #[test]
    fn invalid_policy_snapshot_is_observable_and_denies_without_executor_side_effect() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.user_rules.push(user_rule(
            "unknown",
            PolicyDecision::Allow,
            "unknown_tool",
        ));
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");

        let result = core.write(&WriteRequest::new("blocked.txt", "secret"));
        assert!(matches!(result, Err(ExecutionError::PolicyDenied)));
        assert!(!workspace.root.join("blocked.txt").exists());
        let snapshot = core.effective_policy();
        assert!(!snapshot.valid);
        assert_eq!(
            snapshot.status,
            harness_control_protocol::PolicySnapshotStatus::Invalid
        );
        assert!(snapshot
            .per_tool
            .iter()
            .filter(|tool| tool.tool != "bash")
            .all(|tool| {
                tool.decision == PolicyDecision::Deny
                    && tool.reason == harness_control_protocol::PolicyReason::InvalidPolicySnapshot
            }));
    }

    #[test]
    fn ask_policy_returns_approval_required_without_executor_side_effect_and_keeps_evidence() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config
            .policies
            .user_rules
            .push(user_rule("write-review", PolicyDecision::Ask, "write"));
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");

        let result = core.write(&WriteRequest::new("approval.txt", "secret"));
        assert!(matches!(result, Err(ExecutionError::ApprovalRequired)));
        assert!(!workspace.root.join("approval.txt").exists());
        let entries = core.activity_snapshot(20);
        assert!(entries.iter().any(|entry| {
            entry.event_type == crate::audit::AuditEventType::PolicyEvaluated
                && entry.decision_summary == Some(PolicyDecision::Ask)
                && entry.matched_rule_ids == ["write-review"]
                && entry.policy_reason
                    == Some(harness_control_protocol::PolicyReason::MatchedUserRules)
        }));
    }

    #[test]
    fn glob_requires_initialization_and_uses_the_initialized_workspace_guard() {
        let workspace = TestWorkspace::new();
        fs::write(workspace.root.join("main.rs"), "fn main() {}\n").expect("fixture");
        let mut core = RuntimeCore::new();

        assert!(matches!(
            core.glob(&GlobRequest::new("*.rs")),
            Err(ExecutionError::RuntimeNotInitialized)
        ));

        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let output = core
            .glob(&GlobRequest::new("*.rs"))
            .expect("glob workspace");
        assert_eq!(output.matches[0].path, "main.rs");
    }

    #[test]
    fn write_and_edit_use_the_same_initialized_pipeline_as_mcp() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();

        assert!(matches!(
            core.write(&WriteRequest::new("file.txt", "created")),
            Err(ExecutionError::RuntimeNotInitialized)
        ));
        assert!(matches!(
            core.edit(&EditRequest::new("file.txt", "created", "edited")),
            Err(ExecutionError::RuntimeNotInitialized)
        ));

        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let write = core.write(&WriteRequest::new("nested/file.txt", "created"));
        assert!(matches!(
            write,
            Err(ExecutionError::Write(crate::tools::WriteError::Workspace(
                WorkspaceError::TargetNotFound { .. }
            )))
        ));

        fs::create_dir_all(workspace.root.join("nested")).expect("parent");
        let write = core
            .write(&WriteRequest::new("nested/file.txt", "created"))
            .expect("write through pipeline");
        assert_eq!(write.normalized_path, PathBuf::from("nested/file.txt"));
        assert_eq!(
            fs::read_to_string(workspace.root.join("nested/file.txt")).unwrap(),
            "created"
        );

        let edit = core
            .edit(&EditRequest::new("nested/file.txt", "created", "edited"))
            .expect("edit through pipeline");
        assert_eq!(edit.replacements, 1);
        assert_eq!(
            fs::read_to_string(workspace.root.join("nested/file.txt")).unwrap(),
            "edited"
        );
    }

    #[test]
    fn read_file_uses_the_initialized_snapshot_limit_and_is_unavailable_after_failed_init() {
        let workspace = TestWorkspace::new();
        fs::write(workspace.root.join("lines.txt"), "a\nb\n").expect("write fixture");
        let mut core = RuntimeCore::new();

        assert!(matches!(
            core.read_file(&ReadFileRequest::new("lines.txt")),
            Err(ExecutionError::RuntimeNotInitialized)
        ));

        let mut config = config_for(&workspace.root);
        config.runtime_limits.max_read_bytes = 2;
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");

        let output = core
            .read_file(&ReadFileRequest::new("lines.txt"))
            .expect("read within configured limit");
        assert_eq!(output.content, "a\n");
        assert_eq!(output.bytes_read, 2);
        assert!(output.truncated);
        assert_eq!(output.next_offset, Some(1));

        let mut failed_core = RuntimeCore::new();
        assert!(failed_core
            .initialize(config_for(&workspace.missing_path()))
            .is_err());
        assert!(matches!(
            failed_core.read_file(&ReadFileRequest::new("lines.txt")),
            Err(ExecutionError::RuntimeNotInitialized)
        ));
    }
    #[test]
    fn failed_workspace_initialization_leaves_core_uninitialized() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();

        assert!(core
            .initialize(config_for(&workspace.missing_path()))
            .is_err());

        assert!(core.config.is_none());
        assert!(core.events.is_none());
        assert!(core.workspace_guard().is_none());
        assert_eq!(core.phase, Phase::WaitingInitialize);
        assert!(core.status().is_none());
    }

    #[test]
    fn lifecycle_is_the_only_writer_of_mcp_readiness() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        assert!(!core.mcp_is_ready());

        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        assert_eq!(core.phase, Phase::InitializedPendingAck);
        assert!(!core.mcp_is_ready());

        core.acknowledge().expect("acknowledge runtime");
        assert_eq!(core.phase, Phase::Ready);
        assert!(core.mcp_is_ready());

        core.stop().expect("stop runtime");
        assert_eq!(core.phase, Phase::Stopping);
        assert!(!core.mcp_is_ready());
    }

    #[test]
    fn rollback_initialize_restores_the_uninitialized_business_state() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        core.rollback_initialize();

        assert_eq!(core.phase, Phase::WaitingInitialize);
        assert!(core.config.is_none());
        assert!(core.events.is_none());
        assert!(core.workspace_guard().is_none());
        assert!(!core.mcp_is_ready());
        core.initialize(config_for(&workspace.root))
            .expect("retry initialize runtime");
    }

    #[test]
    fn workspace_file_is_rejected_during_initialization() {
        let workspace = TestWorkspace::new();
        let file = workspace.base.join("workspace-file");
        fs::write(&file, "not a directory").expect("create workspace file");
        let mut core = RuntimeCore::new();

        assert!(matches!(
            core.initialize(config_for(&file)),
            Err(WorkspaceError::WorkspaceNotDirectory { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn unsupported_unc_workspace_is_rejected_without_accessing_it() {
        let mut core = RuntimeCore::new();

        assert!(matches!(
            core.initialize(config_for(Path::new(r"\\server\share\project"))),
            Err(WorkspaceError::UnsupportedPathForm { .. })
        ));
    }
}
