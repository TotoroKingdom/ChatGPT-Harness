mod control_read;
pub(crate) mod discovery;
pub(crate) mod manifest;
use harness_control_protocol::{
    ActivityEvent, ActivityListParams, ActivityListResult, ActivitySubscribeParams,
    ActivitySubscribeResult, AppConfig, ApprovalsListParams, ApprovalsListResult,
    ApprovalsResolveParams, ClientInfo, DiagnosticRun, EvidenceStoreLaunchConfig, InitializeParams,
    InitializeResult, McpEndpointResult, PoliciesListResult, RpcMessage, RpcNotification,
    RpcRequest, RuntimeConfigSnapshot, RuntimeHostContext, RuntimeLimits, RuntimeState,
    RuntimeStopResult, ToolsListResult, PROTOCOL_VERSION,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Condvar, Mutex},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, Runtime};

trait ProcessEventSink: Send + Sync {
    fn emit_process(&self, phase: RuntimeProcessPhase, error: Option<&SupervisorError>);
    fn emit_activity(&self, event: ActivityEvent);
}

pub(crate) trait RuntimeEventSink: Send + Sync {
    fn runtime_event(
        &self,
        generation: uuid::Uuid,
        phase: RuntimeProcessPhase,
        error: Option<&SupervisorError>,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeStartupPhase {
    Spawned,
    Initialized,
    McpEndpointReady,
}

pub(crate) trait RuntimeStartupSink: Send + Sync {
    fn runtime_startup_event(
        &self,
        generation: uuid::Uuid,
        phase: RuntimeStartupPhase,
        error: Option<&SupervisorError>,
    );
}

struct TauriProcessEventSink<R: Runtime> {
    app: AppHandle<R>,
    activity_sink: Option<Arc<dyn Fn(ActivityEvent) + Send + Sync>>,
}

impl<R: Runtime> ProcessEventSink for TauriProcessEventSink<R> {
    fn emit_process(&self, phase: RuntimeProcessPhase, error: Option<&SupervisorError>) {
        emit_process(&self.app, phase, error);
    }

    fn emit_activity(&self, event: ActivityEvent) {
        let _ = self.app.emit("runtime:activity", event.clone());
        if let Some(sink) = &self.activity_sink {
            sink(event);
        }
    }
}

#[cfg(test)]
struct NoopProcessEventSink;

#[cfg(test)]
impl ProcessEventSink for NoopProcessEventSink {
    fn emit_process(&self, _phase: RuntimeProcessPhase, _error: Option<&SupervisorError>) {}
    fn emit_activity(&self, _event: ActivityEvent) {}
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_MAX_CONCURRENT_REQUESTS: u32 = 16;
const DEFAULT_MAX_ACTIVITY_EVENTS: u32 = 1024;
const DEFAULT_MAX_READ_BYTES: u32 = 1_048_576;

type PendingResult = Result<Value, SupervisorError>;
type PendingMap = Arc<Mutex<HashMap<String, mpsc::Sender<PendingResult>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProcessPhase {
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeProcessStatus {
    pub phase: RuntimeProcessPhase,
    pub last_error: Option<SupervisorError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeProcessEvent {
    pub phase: RuntimeProcessPhase,
    pub error: Option<ControlErrorSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlErrorSummary {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeProcessIdentity {
    pub pid: u32,
    pub runtime_id: String,
    pub runtime_version: String,
    pub executable_path: PathBuf,
    pub started_at: String,
    pub workspace: String,
    pub config_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupervisorError {
    pub code: String,
    pub message: String,
}

impl SupervisorError {
    fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: "LIFECYCLE_CONFLICT".into(),
            message: message.into(),
        }
    }

    fn disconnected(message: impl Into<String>) -> Self {
        Self {
            code: "RUNTIME_DISCONNECTED".into(),
            message: message.into(),
        }
    }

    fn timeout() -> Self {
        Self {
            code: "RUNTIME_TIMEOUT".into(),
            message: "Runtime request timed out.".into(),
        }
    }

    fn graceful_stop_timeout() -> Self {
        Self {
            code: "RUNTIME_GRACEFUL_STOP_TIMEOUT".into(),
            message: "Runtime graceful shutdown timed out; the process was terminated.".into(),
        }
    }

    fn distribution(message: impl Into<String>) -> Self {
        Self {
            code: "RUNTIME_DISTRIBUTION_INVALID".into(),
            message: message.into(),
        }
    }

    fn spawn(message: impl Into<String>) -> Self {
        Self {
            code: "RUNTIME_SPAWN_FAILED".into(),
            message: message.into(),
        }
    }

    fn crashed(status: Option<&std::process::ExitStatus>) -> Self {
        Self {
            code: "RUNTIME_CRASHED".into(),
            message: format!(
                "Runtime process exited unexpectedly{}.",
                status
                    .and_then(|value| value.code())
                    .map(|code| format!(" with exit code {code}"))
                    .unwrap_or_default()
            ),
        }
    }
}

struct Observation {
    alive: bool,
}

struct RuntimeProcess {
    generation: uuid::Uuid,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
    observation: Arc<Mutex<Observation>>,
    exited: Arc<(Mutex<bool>, Condvar)>,
    child: Arc<Mutex<Child>>,
    identity: Arc<Mutex<RuntimeProcessIdentity>>,
    expected_runtime_version: String,
    mcp_endpoint: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
struct RuntimeProcessRef {
    generation: uuid::Uuid,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
    observation: Arc<Mutex<Observation>>,
    exited: Arc<(Mutex<bool>, Condvar)>,
    child: Arc<Mutex<Child>>,
    identity: Arc<Mutex<RuntimeProcessIdentity>>,
    expected_runtime_version: String,
    mcp_endpoint: Arc<Mutex<Option<String>>>,
}

struct SupervisorInner {
    process: Option<RuntimeProcess>,
    process_status: RuntimeProcessStatus,
    active_generation: Option<uuid::Uuid>,
    last_identity: Option<RuntimeProcessIdentity>,
    observer: Option<Arc<dyn RuntimeEventSink>>,
    startup_observer: Option<Arc<dyn RuntimeStartupSink>>,
    activity_sink: Option<Arc<dyn Fn(ActivityEvent) + Send + Sync>>,
}

pub struct RuntimeSupervisor {
    inner: Arc<Mutex<SupervisorInner>>,
    #[cfg(test)]
    test_command: Option<TestRuntimeCommand>,
    #[cfg(test)]
    stop_timeout: Duration,
}

impl Clone for RuntimeSupervisor {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            #[cfg(test)]
            test_command: self.test_command.clone(),
            #[cfg(test)]
            stop_timeout: self.stop_timeout,
        }
    }
}

#[cfg(test)]
#[derive(Clone)]
struct TestRuntimeCommand {
    executable_path: PathBuf,
    args: Vec<String>,
    env: Vec<(String, String)>,
    protocol_on_stderr: bool,
}

impl Default for RuntimeSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeSupervisor {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SupervisorInner {
                process: None,
                process_status: RuntimeProcessStatus {
                    phase: RuntimeProcessPhase::Stopped,
                    last_error: None,
                },
                active_generation: None,
                last_identity: None,
                observer: None,
                startup_observer: None,
                activity_sink: None,
            })),
            #[cfg(test)]
            test_command: None,
            #[cfg(test)]
            stop_timeout: STOP_TIMEOUT,
        }
    }

    pub(crate) fn set_event_sink(&self, sink: Arc<dyn RuntimeEventSink>) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.observer = Some(sink);
        }
    }

    pub(crate) fn set_startup_sink(&self, sink: Arc<dyn RuntimeStartupSink>) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.startup_observer = Some(sink);
        }
    }

    pub(crate) fn set_activity_sink(&self, sink: Arc<dyn Fn(ActivityEvent) + Send + Sync>) {
        // Keep one ordered Host worker for activity callbacks. Spawning one
        // thread per Runtime event allowed ApprovalRequested/Resolved
        // refreshes to race and could regress the Tray projection to an older
        // Pending list. The unbounded channel send is non-blocking for the
        // protocol reader; only the worker performs the potentially blocking
        // Host/Runtime refresh.
        let (sender, receiver) = mpsc::channel::<ActivityEvent>();
        thread::spawn(move || {
            for event in receiver {
                sink(event);
            }
        });
        if let Ok(mut inner) = self.inner.lock() {
            inner.activity_sink = Some(Arc::new(move |event| {
                let _ = sender.send(event);
            }));
        }
    }

    pub(crate) fn active_generation(&self) -> Option<uuid::Uuid> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.active_generation)
    }

    pub fn start<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        config: &AppConfig,
    ) -> Result<RuntimeState, SupervisorError> {
        self.start_with_host_session(app, config, None)
    }

    pub(crate) fn start_with_host_session<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        config: &AppConfig,
        host_session_id: Option<&str>,
    ) -> Result<RuntimeState, SupervisorError> {
        let activity_sink = self
            .inner
            .lock()
            .ok()
            .and_then(|inner| inner.activity_sink.clone());
        let history_directory = app
            .path()
            .app_data_dir()
            .ok()
            .map(|directory| directory.join("history"));
        self.start_with_sink(
            Arc::new(TauriProcessEventSink {
                app: app.clone(),
                activity_sink,
            }),
            config,
            host_session_id,
            history_directory,
        )
    }

    #[cfg(test)]
    fn start_for_test(&self, config: &AppConfig) -> Result<RuntimeState, SupervisorError> {
        self.start_with_sink(Arc::new(NoopProcessEventSink), config, None, None)
    }

    fn start_with_sink(
        &self,
        sink: Arc<dyn ProcessEventSink>,
        config: &AppConfig,
        host_session_id: Option<&str>,
        history_directory: Option<PathBuf>,
    ) -> Result<RuntimeState, SupervisorError> {
        let generation = {
            let mut inner = self.lock_inner()?;
            match inner.process_status.phase {
                RuntimeProcessPhase::Stopped => {}
                RuntimeProcessPhase::Crashed => {
                    if inner
                        .process
                        .as_ref()
                        .is_some_and(|process| process_is_alive(process))
                    {
                        return Err(SupervisorError::conflict(
                            "Runtime process is still exiting.",
                        ));
                    }
                    inner.process = None;
                }
                RuntimeProcessPhase::Starting => {
                    return Err(SupervisorError::conflict("Runtime is already starting."))
                }
                RuntimeProcessPhase::Running => {
                    return Err(SupervisorError::conflict("Runtime is already running."))
                }
                RuntimeProcessPhase::Stopping => {
                    return Err(SupervisorError::conflict("Runtime is stopping."))
                }
            }
            let generation = uuid::Uuid::new_v4();
            inner.active_generation = Some(generation);
            inner.process_status = RuntimeProcessStatus {
                phase: RuntimeProcessPhase::Starting,
                last_error: None,
            };
            generation
        };
        sink.emit_process(RuntimeProcessPhase::Starting, None);
        self.notify_observer(generation, RuntimeProcessPhase::Starting, None);

        let process = match self.spawn_process(sink.clone(), config, generation) {
            Ok(process) => process,
            Err(error) => {
                self.fail_start(generation, error.clone());
                sink.emit_process(RuntimeProcessPhase::Stopped, Some(&error));
                self.notify_observer(generation, RuntimeProcessPhase::Stopped, Some(&error));
                return Err(error);
            }
        };
        {
            let mut inner = self.lock_inner()?;
            if inner.active_generation != Some(generation)
                || inner.process_status.phase != RuntimeProcessPhase::Starting
            {
                drop(inner);
                terminate_process(&process);
                return Err(SupervisorError::conflict("Runtime start was superseded."));
            }
            inner.last_identity = process
                .identity
                .lock()
                .ok()
                .map(|identity| identity.clone());
            inner.process = Some(process);
        }
        self.notify_startup_observer(generation, RuntimeStartupPhase::Spawned, None);
        let waiter_process = self.process_for_generation(generation)?;
        spawn_waiter(
            waiter_process.child.clone(),
            waiter_process.identity.clone(),
            waiter_process.pending.clone(),
            waiter_process.observation.clone(),
            waiter_process.exited.clone(),
            self.inner.clone(),
            sink.clone(),
            generation,
        );

        let host_context = history_directory.map(|directory| RuntimeHostContext {
            host_session_id: host_session_id.map(str::to_owned),
            supervisor_generation_id: Some(generation.to_string()),
            evidence_store: Some(EvidenceStoreLaunchConfig {
                enabled: config.history.enabled,
                database_path: directory
                    .join("history.sqlite3")
                    .to_string_lossy()
                    .into_owned(),
                retention_days: config.history.retention_days,
                max_storage_mib: config.history.max_storage_mib,
            }),
        });
        match self.initialize(&waiter_process, config, host_context.as_ref()) {
            Ok(state) => {
                let should_run = {
                    let mut inner = self.lock_inner()?;
                    if inner.active_generation == Some(generation)
                        && inner.process_status.phase == RuntimeProcessPhase::Starting
                    {
                        inner.process_status.phase = RuntimeProcessPhase::Running;
                        true
                    } else {
                        false
                    }
                };
                if !should_run {
                    return Err(SupervisorError::disconnected(
                        "Runtime exited during initialization.",
                    ));
                }
                sink.emit_process(RuntimeProcessPhase::Running, None);
                self.notify_observer(generation, RuntimeProcessPhase::Running, None);
                Ok(state)
            }
            Err(error) => {
                if self.abort_start(generation, error.clone()) {
                    sink.emit_process(RuntimeProcessPhase::Stopped, Some(&error));
                    self.notify_observer(generation, RuntimeProcessPhase::Stopped, Some(&error));
                }
                Err(error)
            }
        }
    }

    pub fn status(&self) -> Result<RuntimeState, SupervisorError> {
        if self.is_alive() {
            let state: RuntimeState = self.request("runtime.status", json!({}))?;
            Ok(state)
        } else {
            Err(SupervisorError::disconnected("Runtime is not running."))
        }
    }

    pub(crate) fn mcp_endpoint(&self) -> Result<String, SupervisorError> {
        let process = self
            .process()
            .ok_or_else(|| SupervisorError::disconnected("Runtime is not connected."))?;
        let endpoint = process
            .mcp_endpoint
            .lock()
            .map_err(|_| SupervisorError::disconnected("Runtime endpoint state unavailable."))?
            .clone();
        endpoint
            .ok_or_else(|| SupervisorError::disconnected("Runtime MCP endpoint is unavailable."))
    }

    pub fn stop<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<RuntimeStopResult, SupervisorError> {
        let activity_sink = self
            .inner
            .lock()
            .ok()
            .and_then(|inner| inner.activity_sink.clone());
        self.stop_with_sink(Arc::new(TauriProcessEventSink {
            app: app.clone(),
            activity_sink,
        }))
    }

    #[cfg(test)]
    fn stop_for_test(&self) -> Result<RuntimeStopResult, SupervisorError> {
        self.stop_with_sink(Arc::new(NoopProcessEventSink))
    }

    fn stop_with_sink(
        &self,
        sink: Arc<dyn ProcessEventSink>,
    ) -> Result<RuntimeStopResult, SupervisorError> {
        let (generation, process) = {
            let mut inner = self.lock_inner()?;
            match inner.process_status.phase {
                RuntimeProcessPhase::Stopped => {
                    return Ok(RuntimeStopResult { accepted: false });
                }
                RuntimeProcessPhase::Crashed
                    if inner
                        .process
                        .as_ref()
                        .is_none_or(|process| !process_is_alive(process)) =>
                {
                    return Ok(RuntimeStopResult { accepted: false });
                }
                RuntimeProcessPhase::Starting => {
                    return Err(SupervisorError::conflict("Runtime is still starting."));
                }
                RuntimeProcessPhase::Stopping => {
                    return Err(SupervisorError::conflict("Runtime is already stopping."));
                }
                RuntimeProcessPhase::Running | RuntimeProcessPhase::Crashed => {
                    let process = inner.process.as_ref().ok_or_else(|| {
                        SupervisorError::disconnected("Runtime process is not available.")
                    })?;
                    if !process_is_alive(process) {
                        return Err(SupervisorError::disconnected(
                            "Runtime process is not alive.",
                        ));
                    }
                    let process = runtime_process_ref(process);
                    inner.process_status.phase = RuntimeProcessPhase::Stopping;
                    (process.generation, process)
                }
            }
        };
        sink.emit_process(RuntimeProcessPhase::Stopping, None);
        self.notify_observer(generation, RuntimeProcessPhase::Stopping, None);

        let stop_result = self.request_on_process(&process, "runtime.stop", json!({}));
        let stop_timeout = self.stop_timeout();
        let shutdown_error = match &stop_result {
            Ok(_) if wait_for_exit(&process.exited, stop_timeout) => return stop_result,
            Ok(_) => SupervisorError::graceful_stop_timeout(),
            Err(_) if wait_for_exit(&process.exited, Duration::from_millis(100)) => {
                return Ok(RuntimeStopResult { accepted: true });
            }
            Err(error) => error.clone(),
        };

        self.set_last_error_if_current(generation, shutdown_error);
        if let Err(error) = kill_and_wait(&process, stop_timeout) {
            self.mark_crashed(generation, error.clone());
            sink.emit_process(RuntimeProcessPhase::Crashed, Some(&error));
            self.notify_observer(generation, RuntimeProcessPhase::Crashed, Some(&error));
            return Err(error);
        }
        Ok(RuntimeStopResult { accepted: true })
    }

    pub fn tools_list(&self) -> Result<ToolsListResult, SupervisorError> {
        self.request("tools.list", json!({}))
    }
    pub fn policies_list(&self) -> Result<PoliciesListResult, SupervisorError> {
        self.request("policies.list", json!({}))
    }
    pub fn activity_list(
        &self,
        params: ActivityListParams,
    ) -> Result<ActivityListResult, SupervisorError> {
        self.request_serialized("activity.list", params)
    }
    pub fn activity_subscribe(
        &self,
        params: ActivitySubscribeParams,
    ) -> Result<ActivitySubscribeResult, SupervisorError> {
        self.request_serialized("activity.subscribe", params)
    }
    pub fn approvals_list(
        &self,
        params: ApprovalsListParams,
    ) -> Result<ApprovalsListResult, SupervisorError> {
        self.request_serialized("approvals.list", params)
    }
    pub fn approvals_resolve(
        &self,
        params: ApprovalsResolveParams,
    ) -> Result<harness_control_protocol::ApprovalsResolveResult, SupervisorError> {
        self.request_serialized("approvals.resolve", params)
    }
    pub fn diagnostics(&self) -> Result<DiagnosticRun, SupervisorError> {
        self.request("diagnostics.run", json!({}))
    }

    pub fn evidence_status(
        &self,
    ) -> Result<harness_control_protocol::EvidenceStoreStatus, SupervisorError> {
        self.request("evidence.status", json!({}))
    }

    pub fn history_clear(
        &self,
    ) -> Result<harness_control_protocol::HistoryClearResult, SupervisorError> {
        self.request("history.clear", json!({}))
    }

    #[allow(dead_code)]
    pub fn identity(&self) -> Option<RuntimeProcessIdentity> {
        self.inner.lock().ok().and_then(|inner| {
            inner
                .process
                .as_ref()
                .and_then(|process| {
                    process
                        .identity
                        .lock()
                        .ok()
                        .map(|identity| identity.clone())
                })
                .or_else(|| inner.last_identity.clone())
        })
    }

    #[allow(dead_code)]
    pub fn process_status(&self) -> RuntimeProcessStatus {
        self.inner
            .lock()
            .map(|inner| inner.process_status.clone())
            .unwrap_or(RuntimeProcessStatus {
                phase: RuntimeProcessPhase::Crashed,
                last_error: Some(SupervisorError::disconnected(
                    "Runtime supervisor unavailable.",
                )),
            })
    }

    fn stop_timeout(&self) -> Duration {
        #[cfg(test)]
        {
            self.stop_timeout
        }
        #[cfg(not(test))]
        {
            STOP_TIMEOUT
        }
    }

    fn initialize(
        &self,
        process: &RuntimeProcessRef,
        config: &AppConfig,
        host_context: Option<&RuntimeHostContext>,
    ) -> Result<RuntimeState, SupervisorError> {
        let snapshot = runtime_config_snapshot(config);
        let initialize = InitializeParams {
            client_info: ClientInfo {
                name: "chatgpt-harness-ui".into(),
                title: "ChatGPT-Harness UI".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            protocol_version: PROTOCOL_VERSION,
            capabilities: Default::default(),
            runtime_config: snapshot,
            host_context: host_context.cloned(),
        };
        let params = serde_json::to_value(initialize)
            .map_err(|error| SupervisorError::spawn(error.to_string()))?;
        let result: InitializeResult = self.request_on_process(process, "initialize", params)?;
        if result.protocol_version != PROTOCOL_VERSION {
            return Err(SupervisorError::disconnected(
                "Runtime protocol version mismatch.",
            ));
        }
        if result.server_info.version != process.expected_runtime_version {
            return Err(SupervisorError::distribution(
                "Runtime version is incompatible with the installed distribution.",
            ));
        }
        if let Ok(mut identity) = process.identity.lock() {
            identity.runtime_id = result.runtime_id;
            identity.runtime_version = result.server_info.version;
        }
        self.notify_startup_observer(process.generation, RuntimeStartupPhase::Initialized, None);
        self.notify_on_process(process, "initialized", json!({}))?;
        let endpoint: McpEndpointResult =
            self.request_on_process(process, "mcp.endpoint.get", json!({}))?;
        validate_mcp_endpoint(&endpoint.url)?;
        if let Ok(mut active_endpoint) = process.mcp_endpoint.lock() {
            *active_endpoint = Some(endpoint.url);
        }
        self.notify_startup_observer(
            process.generation,
            RuntimeStartupPhase::McpEndpointReady,
            None,
        );
        self.request_on_process(process, "runtime.status", json!({}))
    }

    fn notify_startup_observer(
        &self,
        generation: uuid::Uuid,
        phase: RuntimeStartupPhase,
        error: Option<&SupervisorError>,
    ) {
        let observer = self
            .inner
            .lock()
            .ok()
            .and_then(|inner| inner.startup_observer.clone());
        if let Some(observer) = observer {
            observer.runtime_startup_event(generation, phase, error);
        }
    }

    fn spawn_process(
        &self,
        sink: Arc<dyn ProcessEventSink>,
        config: &AppConfig,
        generation: uuid::Uuid,
    ) -> Result<RuntimeProcess, SupervisorError> {
        let (executable_path, expected_runtime_version) = {
            #[cfg(test)]
            if let Some(test_command) = &self.test_command {
                (test_command.executable_path.clone(), "test".to_string())
            } else {
                let resolved = discovery::resolve_runtime()?;
                {
                    let expected_runtime_version = resolved.expected_runtime_version().to_string();
                    (resolved.executable_path, expected_runtime_version)
                }
            }
            #[cfg(not(test))]
            {
                let resolved = discovery::resolve_runtime()?;
                {
                    let expected_runtime_version = resolved.expected_runtime_version().to_string();
                    (resolved.executable_path, expected_runtime_version)
                }
            }
        };
        let mut command = Command::new(&executable_path);
        #[cfg(test)]
        if let Some(test_command) = &self.test_command {
            command
                .args(&test_command.args)
                .envs(test_command.env.iter().cloned());
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|_| SupervisorError::spawn("Could not start the Runtime process."))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| failed_child_spawn(&mut child, "Runtime stdin was not available."))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| failed_child_spawn(&mut child, "Runtime stdout was not available."))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| failed_child_spawn(&mut child, "Runtime stderr was not available."))?;
        let pid = child.id();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let observation = Arc::new(Mutex::new(Observation { alive: true }));
        let exited = Arc::new((Mutex::new(false), Condvar::new()));
        let child = Arc::new(Mutex::new(child));
        let identity = Arc::new(Mutex::new(RuntimeProcessIdentity {
            pid,
            runtime_id: String::new(),
            runtime_version: String::new(),
            executable_path,
            started_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            workspace: config.workspace.root_path.clone(),
            config_revision: config.config_revision.get().to_string(),
        }));
        let mcp_endpoint = Arc::new(Mutex::new(None));
        #[cfg(test)]
        let protocol_on_stderr = self
            .test_command
            .as_ref()
            .is_some_and(|command| command.protocol_on_stderr);
        #[cfg(not(test))]
        let protocol_on_stderr = false;
        if protocol_on_stderr {
            spawn_rpc_reader(stderr, pending.clone(), observation.clone(), sink);
            spawn_output_drain(stdout, false);
        } else {
            spawn_rpc_reader(stdout, pending.clone(), observation.clone(), sink);
            spawn_output_drain(stderr, true);
        }
        Ok(RuntimeProcess {
            generation,
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            observation,
            exited,
            child,
            identity,
            expected_runtime_version,
            mcp_endpoint,
        })
    }

    fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, SupervisorError> {
        let process = self
            .process()
            .ok_or_else(|| SupervisorError::disconnected("Runtime is not connected."))?;
        self.request_on_process(&process, method, params)
    }

    fn request_on_process<T: serde::de::DeserializeOwned>(
        &self,
        process: &RuntimeProcessRef,
        method: &str,
        params: Value,
    ) -> Result<T, SupervisorError> {
        let id = format!("tauri-{}", uuid::Uuid::new_v4());
        let (sender, receiver) = mpsc::channel();
        process
            .pending
            .lock()
            .map_err(|_| SupervisorError::disconnected("Runtime request state unavailable."))?
            .insert(id.clone(), sender);
        let request = RpcMessage::Request(RpcRequest {
            id: harness_control_protocol::RequestId::String(id.clone()),
            method: method.into(),
            params: Some(params),
        });
        let mut line = serde_json::to_vec(&request)
            .map_err(|error| SupervisorError::spawn(error.to_string()))?;
        line.push(b'\n');
        let write_result = (|| {
            let mut stdin = process
                .stdin
                .lock()
                .map_err(|_| SupervisorError::disconnected("Runtime stdin unavailable."))?;
            stdin.write_all(&line).map_err(|error| {
                SupervisorError::disconnected(format!("Could not send Runtime request: {error}"))
            })?;
            stdin
                .flush()
                .map_err(|error| SupervisorError::disconnected(error.to_string()))
        })();
        if let Err(error) = write_result {
            let _ = process
                .pending
                .lock()
                .map(|mut pending| pending.remove(&id));
            return Err(error);
        }
        let value = match receiver.recv_timeout(REQUEST_TIMEOUT) {
            Ok(result) => result?,
            Err(_) => {
                let _ = process
                    .pending
                    .lock()
                    .map(|mut pending| pending.remove(&id));
                return Err(SupervisorError::timeout());
            }
        };
        serde_json::from_value(value).map_err(|error| SupervisorError {
            code: "RUNTIME_INVALID_RESPONSE".into(),
            message: error.to_string(),
        })
    }

    fn request_serialized<P: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<T, SupervisorError> {
        let value = serde_json::to_value(params)
            .map_err(|error| SupervisorError::spawn(error.to_string()))?;
        self.request(method, value)
    }

    fn notify_on_process(
        &self,
        process: &RuntimeProcessRef,
        method: &str,
        params: Value,
    ) -> Result<(), SupervisorError> {
        let message = RpcMessage::Notification(RpcNotification {
            method: method.into(),
            params: Some(params),
        });
        let mut line = serde_json::to_vec(&message)
            .map_err(|error| SupervisorError::spawn(error.to_string()))?;
        line.push(b'\n');
        let mut stdin = process
            .stdin
            .lock()
            .map_err(|_| SupervisorError::disconnected("Runtime stdin unavailable."))?;
        stdin
            .write_all(&line)
            .map_err(|error| SupervisorError::disconnected(error.to_string()))?;
        stdin
            .flush()
            .map_err(|error| SupervisorError::disconnected(error.to_string()))
    }

    fn process(&self) -> Option<RuntimeProcessRef> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.process.as_ref().map(runtime_process_ref))
    }

    fn process_for_generation(
        &self,
        generation: uuid::Uuid,
    ) -> Result<RuntimeProcessRef, SupervisorError> {
        let inner = self.lock_inner()?;
        let process = inner
            .process
            .as_ref()
            .ok_or_else(|| SupervisorError::disconnected("Runtime process disappeared."))?;
        if process.generation != generation {
            return Err(SupervisorError::disconnected(
                "Runtime process generation changed.",
            ));
        }
        Ok(runtime_process_ref(process))
    }

    fn is_alive(&self) -> bool {
        self.process().is_some_and(|process| {
            process
                .observation
                .lock()
                .map(|observation| observation.alive)
                .unwrap_or(false)
        })
    }

    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, SupervisorInner>, SupervisorError> {
        self.inner
            .lock()
            .map_err(|_| SupervisorError::disconnected("Runtime supervisor unavailable."))
    }

    fn fail_start(&self, generation: uuid::Uuid, error: SupervisorError) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.active_generation == Some(generation) {
                inner.active_generation = None;
                inner.process_status = RuntimeProcessStatus {
                    phase: RuntimeProcessPhase::Stopped,
                    last_error: Some(error),
                };
            }
        }
    }

    fn abort_start(&self, generation: uuid::Uuid, error: SupervisorError) -> bool {
        let process = self.inner.lock().ok().and_then(|mut inner| {
            if inner.active_generation != Some(generation) {
                return None;
            }
            inner.active_generation = None;
            inner.process_status = RuntimeProcessStatus {
                phase: RuntimeProcessPhase::Stopped,
                last_error: Some(error),
            };
            Some(inner.process.take())
        });
        let Some(process) = process else {
            return false;
        };
        if let Some(process) = process {
            terminate_process(&process);
        }
        true
    }

    fn set_last_error_if_current(&self, generation: uuid::Uuid, error: SupervisorError) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.active_generation == Some(generation) {
                inner.process_status.last_error = Some(error);
            }
        }
    }

    fn mark_crashed(&self, generation: uuid::Uuid, error: SupervisorError) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.active_generation == Some(generation) {
                inner.process_status = RuntimeProcessStatus {
                    phase: RuntimeProcessPhase::Crashed,
                    last_error: Some(error),
                };
            }
        }
    }

    fn notify_observer(
        &self,
        generation: uuid::Uuid,
        phase: RuntimeProcessPhase,
        error: Option<&SupervisorError>,
    ) {
        // Clone the observer handle while protected, then invoke user/Host
        // code after releasing supervisor.inner. Host status callbacks may
        // synchronously query Runtime (for example approvals.list); invoking
        // them under this lock creates a self-deadlock during Restart/Stop.
        let observer = self
            .inner
            .lock()
            .ok()
            .and_then(|inner| inner.observer.clone());
        if let Some(observer) = observer {
            observer.runtime_event(generation, phase, error);
        }
    }
}

fn runtime_process_ref(process: &RuntimeProcess) -> RuntimeProcessRef {
    RuntimeProcessRef {
        generation: process.generation,
        stdin: process.stdin.clone(),
        pending: process.pending.clone(),
        observation: process.observation.clone(),
        exited: process.exited.clone(),
        child: process.child.clone(),
        identity: process.identity.clone(),
        expected_runtime_version: process.expected_runtime_version.clone(),
        mcp_endpoint: process.mcp_endpoint.clone(),
    }
}

fn runtime_config_snapshot(config: &AppConfig) -> RuntimeConfigSnapshot {
    RuntimeConfigSnapshot {
        config_revision: config.config_revision,
        workspace: config.workspace.clone(),
        security: config.security.clone(),
        runtime_limits: RuntimeLimits {
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            max_activity_events: DEFAULT_MAX_ACTIVITY_EVENTS,
            max_read_bytes: DEFAULT_MAX_READ_BYTES,
        },
        tools: config.tools.clone(),
        bash: config.bash.clone(),
        policies: config.policies.clone(),
    }
}

fn validate_mcp_endpoint(url: &str) -> Result<(), SupervisorError> {
    let authority_and_path = url.strip_prefix("http://").ok_or_else(|| {
        SupervisorError::disconnected("Runtime MCP endpoint must use loopback HTTP.")
    })?;
    let (authority, path) = authority_and_path
        .split_once('/')
        .ok_or_else(|| SupervisorError::disconnected("Runtime MCP endpoint path is invalid."))?;
    let address: std::net::SocketAddr = authority
        .parse()
        .map_err(|_| SupervisorError::disconnected("Runtime MCP endpoint authority is invalid."))?;
    if !address.ip().is_loopback() || address.port() == 0 || path != "mcp" {
        return Err(SupervisorError::disconnected(
            "Runtime MCP endpoint must be a loopback /mcp URL.",
        ));
    }
    Ok(())
}

fn process_is_alive(process: &RuntimeProcess) -> bool {
    process
        .observation
        .lock()
        .map(|observation| observation.alive)
        .unwrap_or(false)
}

fn emit_process<R: Runtime>(
    app: &AppHandle<R>,
    phase: RuntimeProcessPhase,
    error: Option<&SupervisorError>,
) {
    let _ = app.emit(
        "runtime:process",
        RuntimeProcessEvent {
            phase,
            error: error.map(|value| ControlErrorSummary {
                code: value.code.clone(),
                message: value.message.clone(),
            }),
        },
    );
}

fn wait_for_exit(exited: &Arc<(Mutex<bool>, Condvar)>, timeout: Duration) -> bool {
    let (lock, cvar) = &**exited;
    let Ok(done) = lock.lock() else {
        return false;
    };
    cvar.wait_timeout_while(done, timeout, |value| !*value)
        .map(|(done, _)| *done)
        .unwrap_or(false)
}

fn kill_and_wait(process: &RuntimeProcessRef, timeout: Duration) -> Result<(), SupervisorError> {
    let kill_result = process
        .child
        .lock()
        .map_err(|_| SupervisorError {
            code: "RUNTIME_KILL_FAILED".into(),
            message: "Runtime process handle unavailable.".into(),
        })
        .and_then(|mut child| {
            child.kill().map_err(|error| SupervisorError {
                code: "RUNTIME_KILL_FAILED".into(),
                message: format!("Could not kill Runtime process: {error}"),
            })
        });
    if let Err(error) = kill_result {
        if wait_for_exit(&process.exited, Duration::from_millis(100)) {
            return Ok(());
        }
        return Err(error);
    }
    if wait_for_exit(&process.exited, timeout) {
        Ok(())
    } else {
        Err(SupervisorError {
            code: "RUNTIME_KILL_FAILED".into(),
            message: "Runtime process did not exit after kill.".into(),
        })
    }
}

fn terminate_process(process: &RuntimeProcess) {
    if let Ok(mut child) = process.child.lock() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn spawn_rpc_reader<R: Read + Send + 'static>(
    stdout: R,
    pending: PendingMap,
    observation: Arc<Mutex<Observation>>,
    sink: Arc<dyn ProcessEventSink>,
) {
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Ok(message) = serde_json::from_str::<RpcMessage>(&line) else {
                continue;
            };
            match message {
                RpcMessage::Response(response) => {
                    route_response(response.id, Ok(response.result), &pending)
                }
                RpcMessage::Error(error) => route_response(
                    error.id,
                    Err(SupervisorError {
                        code: format!("RPC_{}", machine_code(&error.error.data.code)),
                        message: error.error.message,
                    }),
                    &pending,
                ),
                RpcMessage::Notification(notification)
                    if notification.method == "activity.event" =>
                {
                    if let Some(params) = notification.params {
                        if let Ok(event) = serde_json::from_value::<ActivityEvent>(params) {
                            sink.emit_activity(event);
                        }
                    }
                }
                _ => {}
            }
        }
        if let Ok(mut observation) = observation.lock() {
            observation.alive = false;
        }
    });
}

fn spawn_output_drain<R: Read + Send + 'static>(output: R, echo: bool) {
    thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            match line {
                Ok(line) if echo => eprintln!("runtime: {line}"),
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
}

fn spawn_waiter(
    child: Arc<Mutex<Child>>,
    identity: Arc<Mutex<RuntimeProcessIdentity>>,
    pending: PendingMap,
    observation: Arc<Mutex<Observation>>,
    exited: Arc<(Mutex<bool>, Condvar)>,
    inner: Arc<Mutex<SupervisorInner>>,
    sink: Arc<dyn ProcessEventSink>,
    generation: uuid::Uuid,
) {
    thread::spawn(move || {
        let status = loop {
            match child.lock() {
                Ok(mut child) => match child.try_wait() {
                    Ok(Some(status)) => break Some(status),
                    Ok(None) => {}
                    Err(_) => break None,
                },
                Err(_) => break None,
            }
            thread::sleep(Duration::from_millis(25));
        };
        if let Ok(mut observation) = observation.lock() {
            observation.alive = false;
        }
        if let Ok(mut pending) = pending.lock() {
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(SupervisorError::disconnected(
                    "Runtime process exited.",
                )));
            }
        }

        let transition = inner.lock().ok().and_then(|mut supervisor| {
            transition_process_exit(
                &mut supervisor,
                generation,
                identity.lock().ok().map(|value| value.clone()),
                status.as_ref(),
            )
        });

        // Publish the terminal supervisor state and signal process exit before
        // invoking the Host observer. Observer callbacks are external code
        // and may block while refreshing UI or authoritative Runtime state;
        // the bounded stop path must be able to return without waiting on it.
        let (lock, cvar) = &*exited;
        if let Ok(mut completed) = lock.lock() {
            *completed = true;
            cvar.notify_all();
        }

        if let Some((phase, error)) = transition {
            sink.emit_process(phase, error.as_ref());
            let observer = inner
                .lock()
                .ok()
                .and_then(|supervisor| supervisor.observer.clone());
            if let Some(observer) = observer {
                observer.runtime_event(generation, phase, error.as_ref());
            }
        }
    });
}

fn transition_process_exit(
    supervisor: &mut SupervisorInner,
    generation: uuid::Uuid,
    identity: Option<RuntimeProcessIdentity>,
    status: Option<&std::process::ExitStatus>,
) -> Option<(RuntimeProcessPhase, Option<SupervisorError>)> {
    if supervisor.active_generation != Some(generation) {
        return None;
    }
    let expected = supervisor.process_status.phase == RuntimeProcessPhase::Stopping;
    let phase = if expected {
        RuntimeProcessPhase::Stopped
    } else {
        RuntimeProcessPhase::Crashed
    };
    let error = if expected {
        supervisor.process_status.last_error.clone()
    } else {
        supervisor
            .process_status
            .last_error
            .clone()
            .or_else(|| Some(SupervisorError::crashed(status)))
    };
    if let Some(process) = supervisor.process.as_ref() {
        if process.generation == generation {
            if let Ok(mut endpoint) = process.mcp_endpoint.lock() {
                *endpoint = None;
            }
        }
    }
    supervisor.process = None;
    supervisor.active_generation = None;
    supervisor.process_status = RuntimeProcessStatus {
        phase,
        last_error: error.clone(),
    };
    supervisor.last_identity = identity;
    Some((phase, error))
}

fn failed_child_spawn(child: &mut Child, message: &str) -> SupervisorError {
    let _ = child.kill();
    let _ = child.wait();
    SupervisorError::spawn(message)
}

fn route_response(
    id: harness_control_protocol::RequestId,
    result: PendingResult,
    pending: &PendingMap,
) {
    let key = match id {
        harness_control_protocol::RequestId::String(value) => value,
        harness_control_protocol::RequestId::Integer(value) => value.to_string(),
    };
    if let Ok(mut pending) = pending.lock() {
        if let Some(sender) = pending.remove(&key) {
            let _ = sender.send(result);
        }
    }
}

fn machine_code(code: &harness_control_protocol::MachineErrorCode) -> &'static str {
    match code {
        harness_control_protocol::MachineErrorCode::NotInitialized => "NOT_INITIALIZED",
        harness_control_protocol::MachineErrorCode::AlreadyInitialized => "ALREADY_INITIALIZED",
        harness_control_protocol::MachineErrorCode::ProtocolVersionMismatch => {
            "PROTOCOL_VERSION_MISMATCH"
        }
        harness_control_protocol::MachineErrorCode::RuntimeNotReady => "RUNTIME_NOT_READY",
        harness_control_protocol::MachineErrorCode::InvalidArgument => "INVALID_ARGUMENT",
        harness_control_protocol::MachineErrorCode::ApprovalNotFound => "APPROVAL_NOT_FOUND",
        harness_control_protocol::MachineErrorCode::ApprovalAlreadyResolved => {
            "APPROVAL_ALREADY_RESOLVED"
        }
        harness_control_protocol::MachineErrorCode::ApprovalSessionGrantUnavailable => {
            "APPROVAL_SESSION_GRANT_UNAVAILABLE"
        }
        harness_control_protocol::MachineErrorCode::BashUnavailable => "BASH_UNAVAILABLE",
        harness_control_protocol::MachineErrorCode::BashInvalidCommand => "BASH_INVALID_COMMAND",
        harness_control_protocol::MachineErrorCode::BashCommandTooLong => "BASH_COMMAND_TOO_LONG",
        harness_control_protocol::MachineErrorCode::BashTimeout => "BASH_TIMEOUT",
        harness_control_protocol::MachineErrorCode::BashOutputLimit => "BASH_OUTPUT_LIMIT",
        harness_control_protocol::MachineErrorCode::BashConcurrencyLimit => {
            "BASH_CONCURRENCY_LIMIT"
        }
        harness_control_protocol::MachineErrorCode::BashSpawnFailed => "BASH_SPAWN_FAILED",
        harness_control_protocol::MachineErrorCode::BashCancelled => "BASH_CANCELLED",
        harness_control_protocol::MachineErrorCode::BashProcessFailed => "BASH_PROCESS_FAILED",
        harness_control_protocol::MachineErrorCode::EvidenceBusy => "EVIDENCE_BUSY",
        harness_control_protocol::MachineErrorCode::EvidenceWriterOwnership => {
            "EVIDENCE_WRITER_OWNERSHIP"
        }
        harness_control_protocol::MachineErrorCode::EvidenceIo => "EVIDENCE_IO",
        harness_control_protocol::MachineErrorCode::EvidenceFull => "EVIDENCE_FULL",
        harness_control_protocol::MachineErrorCode::EvidenceCorrupt => "EVIDENCE_CORRUPT",
        harness_control_protocol::MachineErrorCode::EvidenceIncompatibleSchema => {
            "EVIDENCE_INCOMPATIBLE_SCHEMA"
        }
        harness_control_protocol::MachineErrorCode::EvidenceMigrationFailed => {
            "EVIDENCE_MIGRATION_FAILED"
        }
        harness_control_protocol::MachineErrorCode::EvidenceQueueOverflow => {
            "EVIDENCE_QUEUE_OVERFLOW"
        }
        harness_control_protocol::MachineErrorCode::EvidenceCapacityDegraded => {
            "EVIDENCE_CAPACITY_DEGRADED"
        }
        harness_control_protocol::MachineErrorCode::EvidenceInvalidCursor => {
            "EVIDENCE_INVALID_CURSOR"
        }
        harness_control_protocol::MachineErrorCode::EvidenceNotFound => "EVIDENCE_NOT_FOUND",
        harness_control_protocol::MachineErrorCode::InternalError => "INTERNAL_ERROR",
    }
}

#[cfg(test)]
mod m2_e2e;

#[cfg(test)]
mod m01_integration;

#[cfg(test)]
mod m3_config_integration;

#[cfg(test)]
mod m06_upgrade_integration;

#[cfg(test)]
mod m6_i18n_qa;

#[cfg(test)]
mod m7_e2e;

#[cfg(test)]
mod tests {
    use super::*;
    use harness_control_protocol::{
        DecimalU64, RpcResponse, RuntimeLifecycle, RuntimeReadiness, SecurityConfig, SecurityMode,
        ServerInfo, ToolConfig, TunnelConfig, UiConfig, WorkspaceConfig,
    };
    use std::env;
    use std::io::{BufRead, BufReader, BufWriter};
    use std::sync::Arc;
    use std::time::Instant;

    fn config(revision: u64) -> AppConfig {
        AppConfig {
            schema_version: harness_control_protocol::CURRENT_SCHEMA_VERSION,
            config_revision: DecimalU64::new(revision),
            workspace: WorkspaceConfig {
                root_path: format!("D:/workspace-{revision}"),
            },
            tunnel: TunnelConfig {
                tunnel_id: "test-tunnel".into(),
                control_plane_http_proxy: None,
                tunnel_client_path: None,
                auto_detect: false,
                api_key_ref: None,
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            ui: UiConfig::default(),
            tools: ToolConfig::default(),
            bash: harness_control_protocol::BashConfig::default(),
            policies: harness_control_protocol::PolicyConfig::default(),
            desktop: harness_control_protocol::DesktopConfig::default(),
            history: harness_control_protocol::HistoryConfig::default(),
        }
    }

    #[test]
    fn runtime_snapshot_captures_persisted_tool_state_at_generation_start() {
        let mut config = config(7);
        config.tools.write.enabled = false;
        config.tools.edit.enabled = false;
        config.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
        config.tunnel.api_key_ref = Some(harness_control_protocol::SecretReference {
            provider: "memory-development".into(),
            key: "tunnel-api-key".into(),
        });

        let snapshot = runtime_config_snapshot(&config);

        assert_eq!(snapshot.config_revision, DecimalU64::new(7));
        assert!(!snapshot.tools.write.enabled);
        assert!(!snapshot.tools.edit.enabled);
        assert!(snapshot.tools.read_file.enabled);
        assert!(snapshot.tools.glob.enabled);
        assert!(!snapshot.tools.bash.enabled);
        assert_eq!(snapshot.bash, config.bash);
        assert_eq!(
            snapshot.policies.tool_defaults.write,
            harness_control_protocol::PolicyDecision::Ask
        );
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains("tunnel-api-key"));
        assert!(!serialized.contains("apiKeyRef"));
    }

    fn test_supervisor(mode: &str, stop_timeout: Duration) -> RuntimeSupervisor {
        let mut supervisor = RuntimeSupervisor::new();
        supervisor.test_command = Some(TestRuntimeCommand {
            executable_path: env::current_exe().expect("test executable path"),
            args: vec![
                "--exact".into(),
                "runtime::tests::runtime_fixture_process".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            env: vec![("HARNESS_RUNTIME_FIXTURE".into(), mode.into())],
            protocol_on_stderr: true,
        });
        supervisor.stop_timeout = stop_timeout;
        supervisor
    }

    fn set_fixture_mode(supervisor: &mut RuntimeSupervisor, mode: &str) {
        supervisor.test_command.as_mut().expect("test command").env =
            vec![("HARNESS_RUNTIME_FIXTURE".into(), mode.into())];
    }

    fn wait_for_phase(supervisor: &RuntimeSupervisor, phase: RuntimeProcessPhase) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if supervisor.process_status().phase == phase {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for {phase:?}; current status: {:?}",
            supervisor.process_status()
        );
    }

    struct ReentrantObserver {
        supervisor: RuntimeSupervisor,
        returned: std::sync::mpsc::Sender<()>,
    }

    impl RuntimeEventSink for ReentrantObserver {
        fn runtime_event(
            &self,
            _generation: uuid::Uuid,
            _phase: RuntimeProcessPhase,
            _error: Option<&SupervisorError>,
        ) {
            // Host observers are allowed to query the same supervisor from a
            // status callback (for example, to refresh the authoritative
            // approval projection). This must not run while supervisor.inner
            // is held by the event dispatcher.
            let _ = self.supervisor.process_status();
            let _ = self.returned.send(());
        }
    }

    #[test]
    fn runtime_event_observer_can_reenter_supervisor_without_deadlock() {
        let supervisor = RuntimeSupervisor::new();
        let (returned_sender, returned_receiver) = std::sync::mpsc::channel();
        supervisor.set_event_sink(Arc::new(ReentrantObserver {
            supervisor: supervisor.clone(),
            returned: returned_sender,
        }));

        let dispatch = supervisor.clone();
        let dispatch_thread = thread::spawn(move || {
            dispatch.notify_observer(uuid::Uuid::new_v4(), RuntimeProcessPhase::Stopping, None);
        });

        let returned = returned_receiver
            .recv_timeout(Duration::from_millis(250))
            .is_ok();
        assert!(
            returned,
            "Runtime event observer callback deadlocked on supervisor.inner"
        );
        dispatch_thread.join().expect("event dispatch thread");
    }

    #[test]
    fn runtime_fixture_restart_with_reentrant_observer_completes() {
        let supervisor = test_supervisor("normal", Duration::from_secs(1));
        let (returned_sender, _returned_receiver) = std::sync::mpsc::channel();
        supervisor.set_event_sink(Arc::new(ReentrantObserver {
            supervisor: supervisor.clone(),
            returned: returned_sender,
        }));

        let first = supervisor
            .start_for_test(&config(1))
            .expect("first Runtime start");
        let mut changed = config(2);
        changed.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
        supervisor.stop_for_test().expect("first Runtime stop");
        let second = supervisor
            .start_for_test(&changed)
            .expect("Runtime restart after policy revision");

        assert_ne!(first.runtime_id, second.runtime_id);
        assert_eq!(second.config_revision, DecimalU64::new(2));
        supervisor.stop_for_test().expect("final Runtime stop");
    }

    #[test]
    fn failed_cleanup_state_can_retry_stopping_an_owned_live_process() {
        let supervisor = test_supervisor("normal", Duration::from_secs(1));
        supervisor
            .start_for_test(&config(1))
            .expect("Runtime start");
        // A failed termination can leave the owned child alive with a Crashed
        // projection. Stop must retry cleanup rather than treat it as gone.
        supervisor.inner.lock().unwrap().process_status.phase = RuntimeProcessPhase::Crashed;
        assert!(supervisor.stop_for_test().expect("retry cleanup").accepted);
        assert_eq!(
            supervisor.process_status().phase,
            RuntimeProcessPhase::Stopped
        );
        supervisor
            .start_for_test(&config(2))
            .expect("start after cleanup retry");
        supervisor.stop_for_test().expect("final cleanup");
    }

    struct BlockingStopObserver {
        stopped: std::sync::mpsc::Sender<()>,
        release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
    }

    impl RuntimeEventSink for BlockingStopObserver {
        fn runtime_event(
            &self,
            _generation: uuid::Uuid,
            phase: RuntimeProcessPhase,
            _error: Option<&SupervisorError>,
        ) {
            if phase != RuntimeProcessPhase::Stopped {
                return;
            }
            let _ = self.stopped.send(());
            let _ = self
                .release
                .lock()
                .expect("stop observer release lock")
                .recv();
        }
    }

    #[test]
    fn runtime_stop_returns_before_blocking_terminal_observer() {
        let supervisor = test_supervisor("normal", Duration::from_secs(1));
        let (stopped_sender, stopped_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        supervisor.set_event_sink(Arc::new(BlockingStopObserver {
            stopped: stopped_sender,
            release: std::sync::Mutex::new(release_receiver),
        }));
        supervisor
            .start_for_test(&config(1))
            .expect("Runtime start");

        let stopping = supervisor.clone();
        let (finished_sender, finished_receiver) = std::sync::mpsc::channel();
        let stop_thread = thread::spawn(move || {
            let result = stopping.stop_for_test();
            let _ = finished_sender.send(result);
        });
        stopped_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("terminal observer did not run");
        let result = finished_receiver
            .recv_timeout(Duration::from_millis(300))
            .expect("Runtime stop waited for a blocking observer");
        assert!(result.is_ok(), "Runtime stop returned an error: {result:?}");
        release_sender.send(()).expect("release terminal observer");
        stop_thread.join().expect("stop thread");
    }

    fn write_fixture_result<T: Serialize>(
        output: &mut impl Write,
        id: harness_control_protocol::RequestId,
        value: T,
    ) {
        let response = RpcMessage::Response(RpcResponse {
            id,
            result: serde_json::to_value(value).expect("fixture result"),
        });
        serde_json::to_writer(&mut *output, &response).expect("write fixture response");
        output.write_all(b"\n").expect("finish fixture line");
        output.flush().expect("flush fixture response");
    }

    #[test]
    fn runtime_fixture_process() {
        let Ok(mode) = env::var("HARNESS_RUNTIME_FIXTURE") else {
            return;
        };
        let stdin = std::io::stdin();
        let stderr = std::io::stderr();
        let mut input = BufReader::new(stdin.lock());
        let mut output = BufWriter::new(stderr.lock());
        let runtime_id = format!("fixture-{}", uuid::Uuid::new_v4());
        let started_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut snapshot: Option<RuntimeConfigSnapshot> = None;
        let mut line = String::new();

        loop {
            line.clear();
            if input.read_line(&mut line).expect("read fixture request") == 0 {
                return;
            }
            let Ok(message) = serde_json::from_str::<RpcMessage>(line.trim_end()) else {
                continue;
            };
            match message {
                RpcMessage::Request(request) if request.method == "initialize" => {
                    if mode == "slow_initialize" {
                        thread::sleep(Duration::from_millis(300));
                    }
                    let params: InitializeParams =
                        serde_json::from_value(request.params.expect("initialize params"))
                            .expect("valid initialize params");
                    snapshot = Some(params.runtime_config);
                    write_fixture_result(
                        &mut output,
                        request.id,
                        InitializeResult {
                            server_info: ServerInfo {
                                name: "runtime-fixture".into(),
                                title: "Runtime Fixture".into(),
                                version: "test".into(),
                            },
                            protocol_version: PROTOCOL_VERSION,
                            runtime_id: runtime_id.clone(),
                            capabilities: Default::default(),
                        },
                    );
                }
                RpcMessage::Request(request) if request.method == "runtime.status" => {
                    let snapshot = snapshot.clone().expect("initialized snapshot");
                    write_fixture_result(
                        &mut output,
                        request.id,
                        RuntimeState {
                            runtime_id: runtime_id.clone(),
                            started_at: started_at.clone(),
                            lifecycle: RuntimeLifecycle::Ready,
                            readiness: RuntimeReadiness::Ready,
                            config_revision: snapshot.config_revision,
                            active_workspace: snapshot.workspace.root_path,
                            security_mode: snapshot.security.mode,
                            exposed_tool_catalog: vec![],
                            active_requests: 0,
                            pending_approvals: 0,
                            running_executions: 0,
                            last_error: None,
                        },
                    );
                    if mode == "crash_after_status" {
                        thread::sleep(Duration::from_millis(250));
                        std::process::exit(17);
                    }
                }
                RpcMessage::Request(request) if request.method == "mcp.endpoint.get" => {
                    write_fixture_result(
                        &mut output,
                        request.id,
                        McpEndpointResult {
                            url: "http://127.0.0.1:39001/mcp".into(),
                        },
                    );
                }
                RpcMessage::Request(request) if request.method == "runtime.stop" => {
                    write_fixture_result(
                        &mut output,
                        request.id,
                        RuntimeStopResult { accepted: true },
                    );
                    if mode != "ignore_stop" {
                        return;
                    }
                }
                RpcMessage::Notification(_) => {}
                _ => {}
            }
        }
    }

    #[test]
    fn concurrent_start_only_spawns_one_runtime() {
        let supervisor = Arc::new(test_supervisor(
            "slow_initialize",
            Duration::from_millis(200),
        ));
        let first_supervisor = supervisor.clone();
        let first_config = config(1);
        let first = thread::spawn(move || first_supervisor.start_for_test(&first_config));
        wait_for_phase(&supervisor, RuntimeProcessPhase::Starting);

        let error = supervisor.start_for_test(&config(1)).unwrap_err();
        assert_eq!(error.code, "LIFECYCLE_CONFLICT");
        first.join().expect("start thread").expect("first start");
        assert_eq!(
            supervisor.process_status().phase,
            RuntimeProcessPhase::Running
        );
        supervisor.stop_for_test().expect("stop runtime");
    }

    #[test]
    fn normal_start_status_stop() {
        let supervisor = test_supervisor("normal", Duration::from_millis(250));
        let started = supervisor.start_for_test(&config(7)).expect("start");
        let status = supervisor.status().expect("status");
        assert_eq!(status, started);
        assert_eq!(status.config_revision, DecimalU64::new(7));

        let stopped = supervisor.stop_for_test().expect("stop");
        assert!(stopped.accepted);
        assert_eq!(
            supervisor.process_status().phase,
            RuntimeProcessPhase::Stopped
        );
        assert_eq!(
            supervisor.status().unwrap_err().code,
            "RUNTIME_DISCONNECTED"
        );
    }

    #[test]
    fn crash_marks_process_crashed() {
        let supervisor = test_supervisor("crash_after_status", Duration::from_millis(200));
        supervisor.start_for_test(&config(1)).expect("start");

        wait_for_phase(&supervisor, RuntimeProcessPhase::Crashed);
        let status = supervisor.process_status();
        assert_eq!(
            status.last_error.expect("crash error").code,
            "RUNTIME_CRASHED"
        );
        assert_eq!(
            supervisor.status().unwrap_err().code,
            "RUNTIME_DISCONNECTED"
        );
    }

    #[test]
    fn crash_then_start_succeeds() {
        let mut supervisor = test_supervisor("crash_after_status", Duration::from_millis(200));
        let first = supervisor.start_for_test(&config(1)).expect("first start");
        wait_for_phase(&supervisor, RuntimeProcessPhase::Crashed);
        set_fixture_mode(&mut supervisor, "normal");

        let second = supervisor.start_for_test(&config(1)).expect("restart");
        assert_ne!(first.runtime_id, second.runtime_id);
        supervisor.stop_for_test().expect("stop restarted runtime");
    }

    #[test]
    fn restart_generates_new_runtime_id() {
        let supervisor = test_supervisor("normal", Duration::from_millis(250));
        let first = supervisor.start_for_test(&config(1)).expect("first start");
        supervisor.stop_for_test().expect("first stop");
        let second = supervisor.start_for_test(&config(1)).expect("second start");

        assert_ne!(first.runtime_id, second.runtime_id);
        supervisor.stop_for_test().expect("second stop");
    }

    #[test]
    fn config_change_restart_uses_new_revision() {
        let supervisor = test_supervisor("normal", Duration::from_millis(250));
        let first = supervisor.start_for_test(&config(7)).expect("first start");
        assert_eq!(first.config_revision, DecimalU64::new(7));
        supervisor.stop_for_test().expect("stop");

        let second = supervisor.start_for_test(&config(8)).expect("restart");
        assert_eq!(second.config_revision, DecimalU64::new(8));
        assert_eq!(second.active_workspace, "D:/workspace-8");
        supervisor.stop_for_test().expect("final stop");
    }

    #[test]
    fn stop_timeout_kills_child() {
        let supervisor = test_supervisor("ignore_stop", Duration::from_millis(100));
        supervisor.start_for_test(&config(1)).expect("start");

        let result = supervisor.stop_for_test().expect("fallback stop");
        assert!(result.accepted);
        let status = supervisor.process_status();
        assert_eq!(status.phase, RuntimeProcessPhase::Stopped);
        assert_eq!(
            status.last_error.expect("shutdown warning").code,
            "RUNTIME_GRACEFUL_STOP_TIMEOUT"
        );
        assert_eq!(
            supervisor.status().unwrap_err().code,
            "RUNTIME_DISCONNECTED"
        );
    }

    #[test]
    fn stop_timeout_leaves_runtime_restartable() {
        let mut supervisor = test_supervisor("ignore_stop", Duration::from_millis(100));
        supervisor.start_for_test(&config(1)).expect("start");

        let stopped = supervisor.stop_for_test().expect("bounded stop fallback");
        assert!(stopped.accepted);
        assert_eq!(
            supervisor.process_status().phase,
            RuntimeProcessPhase::Stopped
        );
        assert_eq!(
            supervisor
                .process_status()
                .last_error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("RUNTIME_GRACEFUL_STOP_TIMEOUT")
        );

        // A failed graceful stop must not leave lifecycle state stuck in
        // Stopping/Crashed; the next generation can be started normally.
        set_fixture_mode(&mut supervisor, "normal");
        let restarted = supervisor
            .start_for_test(&config(2))
            .expect("restart after bounded stop failure");
        assert_eq!(restarted.config_revision, DecimalU64::new(2));
        supervisor.stop_for_test().expect("final stop");
    }

    #[test]
    fn stale_waiter_cannot_clear_new_process() {
        let stale_generation = uuid::Uuid::new_v4();
        let active_generation = uuid::Uuid::new_v4();
        let mut inner = SupervisorInner {
            process: None,
            process_status: RuntimeProcessStatus {
                phase: RuntimeProcessPhase::Running,
                last_error: None,
            },
            active_generation: Some(active_generation),
            last_identity: None,
            observer: None,
            startup_observer: None,
            activity_sink: None,
        };

        assert!(transition_process_exit(&mut inner, stale_generation, None, None).is_none());
        assert_eq!(inner.active_generation, Some(active_generation));
        assert_eq!(inner.process_status.phase, RuntimeProcessPhase::Running);
    }

    #[test]
    fn runtime_absent_does_not_create_runtime_state_from_config() {
        let supervisor = RuntimeSupervisor::new();
        let saved_config = config(42);
        assert_eq!(saved_config.workspace.root_path, "D:/workspace-42");

        let error = supervisor.status().unwrap_err();
        assert_eq!(error.code, "RUNTIME_DISCONNECTED");
        assert_eq!(
            supervisor.process_status().phase,
            RuntimeProcessPhase::Stopped
        );
        assert!(supervisor.identity().is_none());
    }

    #[test]
    fn mcp_endpoint_validation_accepts_only_loopback_mcp_urls() {
        assert!(validate_mcp_endpoint("http://127.0.0.1:39001/mcp").is_ok());
        for invalid in [
            "https://127.0.0.1:39001/mcp",
            "http://0.0.0.0:39001/mcp",
            "http://192.0.2.1:39001/mcp",
            "http://127.0.0.1:39001/other",
            "http://127.0.0.1:0/mcp",
            "not-a-url",
        ] {
            assert!(
                validate_mcp_endpoint(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }
}
