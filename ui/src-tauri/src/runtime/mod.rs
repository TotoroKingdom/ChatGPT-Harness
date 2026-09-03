use harness_control_protocol::{
    ActivityEvent, ActivityListParams, ActivityListResult, ActivitySubscribeParams,
    ActivitySubscribeResult, AppConfig, ApprovalsListParams, ApprovalsListResult,
    ApprovalsResolveParams, ClientInfo, DiagnosticRun, InitializeParams, InitializeResult,
    PoliciesListResult, RpcMessage, RpcNotification, RpcRequest, RuntimeConfigSnapshot,
    RuntimeLimits, RuntimeState, RuntimeStopResult, ToolsListResult, PROTOCOL_VERSION,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Condvar, Mutex},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Runtime};

trait ProcessEventSink: Send + Sync {
    fn emit_process(&self, phase: RuntimeProcessPhase, error: Option<&SupervisorError>);
    fn emit_activity(&self, event: ActivityEvent);
}

struct TauriProcessEventSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> ProcessEventSink for TauriProcessEventSink<R> {
    fn emit_process(&self, phase: RuntimeProcessPhase, error: Option<&SupervisorError>) {
        emit_process(&self.app, phase, error);
    }

    fn emit_activity(&self, event: ActivityEvent) {
        let _ = self.app.emit("runtime:activity", event);
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
}

struct SupervisorInner {
    process: Option<RuntimeProcess>,
    process_status: RuntimeProcessStatus,
    active_generation: Option<uuid::Uuid>,
    last_identity: Option<RuntimeProcessIdentity>,
}

pub struct RuntimeSupervisor {
    inner: Arc<Mutex<SupervisorInner>>,
    #[cfg(test)]
    test_command: Option<TestRuntimeCommand>,
    #[cfg(test)]
    stop_timeout: Duration,
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
            })),
            #[cfg(test)]
            test_command: None,
            #[cfg(test)]
            stop_timeout: STOP_TIMEOUT,
        }
    }

    pub fn start<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        config: &AppConfig,
    ) -> Result<RuntimeState, SupervisorError> {
        self.start_with_sink(Arc::new(TauriProcessEventSink { app: app.clone() }), config)
    }

    #[cfg(test)]
    fn start_for_test(&self, config: &AppConfig) -> Result<RuntimeState, SupervisorError> {
        self.start_with_sink(Arc::new(NoopProcessEventSink), config)
    }

    fn start_with_sink(
        &self,
        sink: Arc<dyn ProcessEventSink>,
        config: &AppConfig,
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

        let process = match self.spawn_process(sink.clone(), config, generation) {
            Ok(process) => process,
            Err(error) => {
                self.fail_start(generation, error.clone());
                sink.emit_process(RuntimeProcessPhase::Stopped, Some(&error));
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

        match self.initialize(&waiter_process, config) {
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
                Ok(state)
            }
            Err(error) => {
                if self.abort_start(generation, error.clone()) {
                    sink.emit_process(RuntimeProcessPhase::Stopped, Some(&error));
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

    pub fn stop<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<RuntimeStopResult, SupervisorError> {
        self.stop_with_sink(Arc::new(TauriProcessEventSink { app: app.clone() }))
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
                RuntimeProcessPhase::Stopped | RuntimeProcessPhase::Crashed => {
                    return Ok(RuntimeStopResult { accepted: false });
                }
                RuntimeProcessPhase::Starting => {
                    return Err(SupervisorError::conflict("Runtime is still starting."));
                }
                RuntimeProcessPhase::Stopping => {
                    return Err(SupervisorError::conflict("Runtime is already stopping."));
                }
                RuntimeProcessPhase::Running => {
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
    ) -> Result<RuntimeState, SupervisorError> {
        let snapshot = RuntimeConfigSnapshot {
            config_revision: config.config_revision,
            workspace: config.workspace.clone(),
            security: config.security.clone(),
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
                max_activity_events: DEFAULT_MAX_ACTIVITY_EVENTS,
            },
        };
        let initialize = InitializeParams {
            client_info: ClientInfo {
                name: "chatgpt-harness-ui".into(),
                title: "ChatGPT-Harness UI".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            protocol_version: PROTOCOL_VERSION,
            capabilities: Default::default(),
            runtime_config: snapshot,
        };
        let params = serde_json::to_value(initialize)
            .map_err(|error| SupervisorError::spawn(error.to_string()))?;
        let result: InitializeResult = self.request_on_process(process, "initialize", params)?;
        if result.protocol_version != PROTOCOL_VERSION {
            return Err(SupervisorError::disconnected(
                "Runtime protocol version mismatch.",
            ));
        }
        if let Ok(mut identity) = process.identity.lock() {
            identity.runtime_id = result.runtime_id;
            identity.runtime_version = result.server_info.version;
        }
        self.notify_on_process(process, "initialized", json!({}))?;
        self.request_on_process(process, "runtime.status", json!({}))
    }

    fn spawn_process(
        &self,
        sink: Arc<dyn ProcessEventSink>,
        config: &AppConfig,
        generation: uuid::Uuid,
    ) -> Result<RuntimeProcess, SupervisorError> {
        #[cfg(test)]
        let executable_path = self
            .test_command
            .as_ref()
            .map(|command| command.executable_path.clone())
            .map_or_else(discover_runtime_executable, Ok)?;
        #[cfg(not(test))]
        let executable_path = discover_runtime_executable()?;
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
        let mut child = command.spawn().map_err(|error| {
            SupervisorError::spawn(format!(
                "Could not start {}: {error}",
                executable_path.display()
            ))
        })?;
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
    }
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
        if let Some((phase, error)) = transition {
            sink.emit_process(phase, error.as_ref());
        }
        let (lock, cvar) = &*exited;
        if let Ok(mut completed) = lock.lock() {
            *completed = true;
            cvar.notify_all();
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

fn discover_runtime_executable() -> Result<PathBuf, SupervisorError> {
    if let Ok(path) = env::var("CHATGPT_HARNESS_RUNTIME_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(SupervisorError::spawn(format!(
            "CHATGPT_HARNESS_RUNTIME_PATH does not point to a file: {}",
            path.display()
        )));
    }
    let filename = if cfg!(windows) {
        "chatgpt-harness-runtime.exe"
    } else {
        "chatgpt-harness-runtime"
    };
    let current = env::current_exe().map_err(|error| SupervisorError::spawn(error.to_string()))?;
    let sibling = current
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(filename);
    if sibling.is_file() {
        return Ok(sibling);
    }
    Err(SupervisorError::spawn(format!("Runtime executable not found at {}. Build it with cargo build -p chatgpt-harness-runtime or set CHATGPT_HARNESS_RUNTIME_PATH.", sibling.display())))
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
        harness_control_protocol::MachineErrorCode::InternalError => "INTERNAL_ERROR",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_control_protocol::{
        DecimalU64, RpcResponse, RuntimeLifecycle, RuntimeReadiness, SecurityConfig, SecurityMode,
        ServerInfo, TunnelConfig, UiConfig, WorkspaceConfig,
    };
    use std::io::{BufRead, BufReader, BufWriter};
    use std::sync::Arc;
    use std::time::Instant;

    fn config(revision: u64) -> AppConfig {
        AppConfig {
            schema_version: 1,
            config_revision: DecimalU64::new(revision),
            workspace: WorkspaceConfig {
                root_path: format!("D:/workspace-{revision}"),
            },
            tunnel: TunnelConfig {
                tunnel_id: "test-tunnel".into(),
                tunnel_client_path: None,
                auto_detect: false,
                api_key_ref: None,
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            ui: UiConfig::default(),
        }
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
}
