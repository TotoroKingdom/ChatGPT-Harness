use crate::events::EventBus;
use crate::mcp::{McpContextError, McpExecutionContext, McpService};
use crate::tools::{
    execute_glob, execute_read_file, GlobError, GlobOutput, GlobRequest, ReadFileError,
    ReadFileOutput, ReadFileRequest, ToolRegistry,
};
use crate::workspace::{Workspace, WorkspaceError, WorkspaceGuard};
use harness_control_protocol::{
    ActivityEvent, ActivityEventType, RuntimeConfigSnapshot, RuntimeErrorSummary, RuntimeLifecycle,
    RuntimeReadiness, RuntimeState,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    WaitingInitialize,
    InitializedPendingAck,
    Ready,
    Stopping,
}

pub struct RuntimeCore {
    pub runtime_id: String,
    pub started_at: String,
    pub phase: Phase,
    pub config: Option<RuntimeConfigSnapshot>,
    pub events: Option<EventBus>,
    workspace_guard: Option<Arc<WorkspaceGuard>>,
    tool_registry: ToolRegistry,
    pub last_error: Option<RuntimeErrorSummary>,
}

impl RuntimeCore {
    pub fn new() -> Self {
        Self {
            runtime_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            phase: Phase::WaitingInitialize,
            config: None,
            events: None,
            workspace_guard: None,
            tool_registry: ToolRegistry::normal_mode(),
            last_error: None,
        }
    }

    pub fn initialize(&mut self, config: RuntimeConfigSnapshot) -> Result<(), WorkspaceError> {
        let workspace = Workspace::new(&config.workspace.root_path)?;
        let workspace_guard = Arc::new(WorkspaceGuard::new(&workspace));
        let events = EventBus::from_limits(&config.runtime_limits);

        self.events = Some(events);
        self.config = Some(config);
        self.workspace_guard = Some(workspace_guard);
        self.phase = Phase::InitializedPendingAck;
        self.last_error = None;
        Ok(())
    }

    #[allow(dead_code)]
    /// Executes the runtime-internal `glob` capability using the one guard
    /// created by the successful initialize snapshot.
    pub fn glob(&self, request: &GlobRequest) -> Result<GlobOutput, GlobError> {
        let workspace_guard = self
            .workspace_guard
            .as_ref()
            .ok_or(GlobError::RuntimeNotInitialized)?;

        execute_glob(workspace_guard, request)
    }
    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
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
        let workspace_guard = self
            .workspace_guard
            .as_ref()
            .cloned()
            .ok_or(McpContextError::RuntimeNotInitialized)?;
        let config = self
            .config
            .as_ref()
            .ok_or(McpContextError::RuntimeNotInitialized)?;

        Ok(McpExecutionContext::new(
            workspace_guard,
            config.runtime_limits.max_read_bytes,
        ))
    }

    #[allow(dead_code)]
    pub fn mcp_service(&self) -> Result<McpService, McpContextError> {
        self.mcp_context().map(McpService::new)
    }

    #[allow(dead_code)]
    /// Executes the runtime-internal `read_file` capability using the guard and
    /// read limit established by the successful initialize snapshot.
    pub fn read_file(&self, request: &ReadFileRequest) -> Result<ReadFileOutput, ReadFileError> {
        let workspace_guard = self
            .workspace_guard
            .as_ref()
            .ok_or(ReadFileError::RuntimeNotInitialized)?;
        let config = self
            .config
            .as_ref()
            .ok_or(ReadFileError::RuntimeNotInitialized)?;

        execute_read_file(
            workspace_guard,
            config.runtime_limits.max_read_bytes,
            request,
        )
    }

    pub fn acknowledge(&mut self) -> Option<ActivityEvent> {
        if self.phase != Phase::InitializedPendingAck {
            return None;
        }
        self.phase = Phase::Ready;
        Some(self.publish(
            ActivityEventType::RuntimeStarted,
            "Runtime started.",
            [("source".into(), "runtime".into())],
            None,
        ))
    }

    pub fn stop(&mut self) -> Option<ActivityEvent> {
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

    pub fn status(&self) -> Option<RuntimeState> {
        let config = self.config.as_ref()?;
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
            exposed_tool_catalog: self.tool_registry.exposed_catalog(),
            active_requests: 0,
            pending_approvals: 0,
            running_executions: 0,
            last_error: self.last_error.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_control_protocol::{
        DecimalU64, RuntimeLimits, SecurityConfig, SecurityMode, WorkspaceConfig,
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
        assert_eq!(
            core.status().unwrap().exposed_tool_catalog,
            core.tool_registry().exposed_catalog()
        );
    }

    #[test]
    fn glob_requires_initialization_and_uses_the_initialized_workspace_guard() {
        let workspace = TestWorkspace::new();
        fs::write(workspace.root.join("main.rs"), "fn main() {}\n").expect("fixture");
        let mut core = RuntimeCore::new();

        assert!(matches!(
            core.glob(&GlobRequest::new("*.rs")),
            Err(GlobError::RuntimeNotInitialized)
        ));

        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        let output = core
            .glob(&GlobRequest::new("*.rs"))
            .expect("glob workspace");
        assert_eq!(output.matches[0].path, "main.rs");
    }
    #[test]
    fn read_file_uses_the_initialized_snapshot_limit_and_is_unavailable_after_failed_init() {
        let workspace = TestWorkspace::new();
        fs::write(workspace.root.join("lines.txt"), "a\nb\n").expect("write fixture");
        let mut core = RuntimeCore::new();

        assert!(matches!(
            core.read_file(&ReadFileRequest::new("lines.txt")),
            Err(ReadFileError::RuntimeNotInitialized)
        ));

        let mut config = config_for(&workspace.root);
        config.runtime_limits.max_read_bytes = 2;
        core.initialize(config).expect("initialize runtime");

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
            Err(ReadFileError::RuntimeNotInitialized)
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
