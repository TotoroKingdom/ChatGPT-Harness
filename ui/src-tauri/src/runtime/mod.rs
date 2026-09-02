use harness_control_protocol::{
    ActivityEvent, ActivityListParams, ActivityListResult, ActivitySubscribeParams,
    ActivitySubscribeResult, AppConfig, ApprovalsListParams, ApprovalsListResult,
    ApprovalsResolveParams, ClientInfo, DiagnosticRun, InitializeParams, InitializeResult,
    PoliciesListResult, RpcMessage, RpcNotification, RpcRequest, RuntimeConfigSnapshot,
    RuntimeLifecycle, RuntimeLimits, RuntimeReadiness, RuntimeState, RuntimeStopResult,
    ToolsListResult, PROTOCOL_VERSION,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
    sync::{mpsc, Arc, Condvar, Mutex},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_MAX_CONCURRENT_REQUESTS: u32 = 16;
const DEFAULT_MAX_ACTIVITY_EVENTS: u32 = 1024;

type PendingResult = Result<Value, SupervisorError>;
type PendingMap = Arc<Mutex<HashMap<String, mpsc::Sender<PendingResult>>>>;

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

impl RuntimeProcessIdentity {
    fn matches_process(&self, other: &Self) -> bool {
        self.pid == other.pid
            && self.runtime_id == other.runtime_id
            && self.runtime_version == other.runtime_version
            && self.executable_path == other.executable_path
            && self.started_at == other.started_at
            && self.workspace == other.workspace
            && self.config_revision == other.config_revision
    }
}

#[derive(Debug, Clone)]
pub struct SupervisorError {
    pub code: String,
    pub message: String,
}

impl SupervisorError {
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

    fn spawn(message: impl Into<String>) -> Self {
        Self {
            code: "RUNTIME_SPAWN_FAILED".into(),
            message: message.into(),
        }
    }
}

struct Observation {
    alive: bool,
    requested_stop: bool,
    state: Option<RuntimeState>,
}

struct RuntimeProcess {
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
    observation: Arc<Mutex<Observation>>,
    exited: Arc<(Mutex<bool>, Condvar)>,
    child: Arc<Mutex<Child>>,
    identity: Arc<Mutex<RuntimeProcessIdentity>>,
}

struct RuntimeProcessRef {
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
    observation: Arc<Mutex<Observation>>,
    exited: Arc<(Mutex<bool>, Condvar)>,
}

struct SupervisorInner {
    process: Option<RuntimeProcess>,
    last_state: Option<RuntimeState>,
    last_error: Option<SupervisorError>,
}

pub struct RuntimeSupervisor {
    inner: Arc<Mutex<SupervisorInner>>,
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
                last_state: None,
                last_error: None,
            })),
        }
    }

    pub fn start(
        &self,
        app: &AppHandle,
        config: &AppConfig,
    ) -> Result<RuntimeState, SupervisorError> {
        {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| SupervisorError::disconnected("Runtime supervisor unavailable."))?;
            if let Some(process) = inner.process.as_ref() {
                if process
                    .observation
                    .lock()
                    .map(|observation| observation.alive)
                    .unwrap_or(false)
                {
                    return inner.last_state.clone().ok_or_else(|| {
                        SupervisorError::disconnected("Runtime is already starting.")
                    });
                }
            }
            inner.process = None;
            inner.last_error = None;
        }

        let process = self.spawn_process(app, config)?;
        {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| SupervisorError::disconnected("Runtime supervisor unavailable."))?;
            inner.process = Some(process);
            inner.last_state = None;
        }

        let result = self.initialize(config);
        if result.is_err() {
            self.abort_start();
        }
        result
    }

    pub fn status(&self) -> Result<RuntimeState, SupervisorError> {
        if self.is_alive() {
            let state: RuntimeState = self.request("runtime.status", json!({}))?;
            self.record_state(state.clone());
            Ok(state)
        } else {
            let inner = self
                .inner
                .lock()
                .map_err(|_| SupervisorError::disconnected("Runtime supervisor unavailable."))?;
            inner.last_state.clone().ok_or_else(|| {
                inner
                    .last_error
                    .clone()
                    .unwrap_or_else(|| SupervisorError::disconnected("Runtime is not running."))
            })
        }
    }

    pub fn stop(&self) -> Result<RuntimeStopResult, SupervisorError> {
        let Some(process) = self.process() else {
            return Ok(RuntimeStopResult { accepted: false });
        };
        if !process
            .observation
            .lock()
            .map(|observation| observation.alive)
            .unwrap_or(false)
        {
            return Ok(RuntimeStopResult { accepted: false });
        }
        if let Ok(mut observation) = process.observation.lock() {
            observation.requested_stop = true;
        }
        let result: RuntimeStopResult = self.request("runtime.stop", json!({}))?;
        let (lock, cvar) = &*process.exited;
        let complete = lock
            .lock()
            .map_err(|_| SupervisorError::disconnected("Runtime exit state unavailable."))?;
        let (complete, timeout) = cvar
            .wait_timeout_while(complete, STOP_TIMEOUT, |done| !*done)
            .map_err(|_| SupervisorError::disconnected("Runtime exit state unavailable."))?;
        if !*complete && timeout.timed_out() {
            return Err(SupervisorError::timeout());
        }
        Ok(result)
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
            inner.process.as_ref().and_then(|process| {
                process
                    .identity
                    .lock()
                    .ok()
                    .map(|identity| identity.clone())
            })
        })
    }

    fn initialize(&self, config: &AppConfig) -> Result<RuntimeState, SupervisorError> {
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
        let result: InitializeResult = self.request_serialized("initialize", initialize)?;
        if result.protocol_version != PROTOCOL_VERSION {
            return Err(SupervisorError::disconnected(
                "Runtime protocol version mismatch.",
            ));
        }
        if let Ok(inner) = self.inner.lock() {
            if let Some(process) = inner.process.as_ref() {
                if let Ok(mut identity) = process.identity.lock() {
                    identity.runtime_id = result.runtime_id;
                    identity.runtime_version = result.server_info.version;
                }
            }
        }
        self.notify("initialized", json!({}))?;
        let state: RuntimeState = self.request("runtime.status", json!({}))?;
        self.record_state(state.clone());
        Ok(state)
    }

    fn spawn_process(
        &self,
        app: &AppHandle,
        config: &AppConfig,
    ) -> Result<RuntimeProcess, SupervisorError> {
        let executable_path = discover_runtime_executable()?;
        let mut command = Command::new(&executable_path);
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
        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                return Err(failed_child_spawn(
                    &mut child,
                    "Runtime stdin was not available.",
                ))
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                return Err(failed_child_spawn(
                    &mut child,
                    "Runtime stdout was not available.",
                ))
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                return Err(failed_child_spawn(
                    &mut child,
                    "Runtime stderr was not available.",
                ))
            }
        };
        let pid = child.id();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let observation = Arc::new(Mutex::new(Observation {
            alive: true,
            requested_stop: false,
            state: None,
        }));
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

        spawn_stdout_reader(stdout, pending.clone(), observation.clone(), app.clone());
        spawn_stderr_drain(stderr);
        spawn_waiter(
            child.clone(),
            identity.clone(),
            pending.clone(),
            observation.clone(),
            exited.clone(),
            self.inner.clone(),
            app.clone(),
        );

        Ok(RuntimeProcess {
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

    fn notify(&self, method: &str, params: Value) -> Result<(), SupervisorError> {
        let process = self
            .process()
            .ok_or_else(|| SupervisorError::disconnected("Runtime is not connected."))?;
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
        self.inner.lock().ok().and_then(|inner| {
            inner.process.as_ref().map(|process| RuntimeProcessRef {
                stdin: process.stdin.clone(),
                pending: process.pending.clone(),
                observation: process.observation.clone(),
                exited: process.exited.clone(),
            })
        })
    }

    fn is_alive(&self) -> bool {
        self.process()
            .and_then(|process| {
                process
                    .observation
                    .lock()
                    .ok()
                    .map(|observation| observation.alive)
            })
            .unwrap_or(false)
    }

    fn record_state(&self, state: RuntimeState) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(process) = inner.process.as_ref() {
                if let Ok(mut observation) = process.observation.lock() {
                    observation.state = Some(state.clone());
                }
            }
            inner.last_state = Some(state);
        }
    }

    fn abort_start(&self) {
        let process = self
            .inner
            .lock()
            .ok()
            .and_then(|mut inner| inner.process.take());
        if let Some(process) = process {
            if let Ok(mut observation) = process.observation.lock() {
                observation.requested_stop = true;
            }
            if let Ok(mut child) = process.child.lock() {
                let _ = child.kill();
            }
        }
    }
}

fn spawn_stdout_reader(
    stdout: ChildStdout,
    pending: PendingMap,
    observation: Arc<Mutex<Observation>>,
    app: AppHandle,
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
                            let _ = app.emit("runtime:activity", event);
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

fn spawn_stderr_drain(stderr: ChildStderr) {
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            match line {
                Ok(line) => eprintln!("runtime: {line}"),
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
    app: AppHandle,
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
        let exited_identity = identity.lock().ok().map(|identity| identity.clone());
        let (requested_stop, mut state) = observation
            .lock()
            .map(|observation| (observation.requested_stop, observation.state.clone()))
            .unwrap_or((false, None));
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
        if let Some(current) = state.as_mut() {
            if requested_stop {
                current.runtime_id.clear();
                current.lifecycle = RuntimeLifecycle::Initializing;
                current.readiness = RuntimeReadiness::NotReady;
                current.last_error = None;
            } else {
                current.lifecycle = RuntimeLifecycle::Error;
                current.readiness = RuntimeReadiness::Degraded;
                current.last_error = Some(harness_control_protocol::RuntimeErrorSummary {
                    code: "RUNTIME_CRASHED".into(),
                    message: format!(
                        "Runtime process exited unexpectedly{}.",
                        status
                            .and_then(|status| status.code())
                            .map(|code| format!(" with exit code {code}"))
                            .unwrap_or_default()
                    ),
                });
            }

            let current_process = inner
                .lock()
                .map(|mut supervisor| {
                    let is_current = supervisor.process.as_ref().is_some_and(|process| {
                        exited_identity.as_ref().is_some_and(|exited_identity| {
                            process
                                .identity
                                .lock()
                                .map(|identity| identity.matches_process(exited_identity))
                                .unwrap_or(false)
                        })
                    });
                    if is_current {
                        supervisor.process = None;
                        supervisor.last_state = Some(current.clone());
                        supervisor.last_error =
                            current.last_error.as_ref().map(|error| SupervisorError {
                                code: error.code.clone(),
                                message: error.message.clone(),
                            });
                    }
                    is_current
                })
                .unwrap_or(false);

            if current_process {
                if !requested_stop {
                    let _ = app.emit("runtime:crashed", current.clone());
                }
                let _ = app.emit("runtime:state", current.clone());
            }
        }
        let (lock, cvar) = &*exited;
        if let Ok(mut completed) = lock.lock() {
            *completed = true;
            cvar.notify_all();
        }
    });
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
    Err(SupervisorError::spawn(format!(
        "Runtime executable not found at {}. Build it with cargo build -p chatgpt-harness-runtime or set CHATGPT_HARNESS_RUNTIME_PATH.",
        sibling.display()
    )))
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
