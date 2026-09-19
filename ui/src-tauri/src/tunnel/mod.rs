mod discovery;

pub(crate) use discovery::resolve_tunnel;

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const READINESS_MONITOR_INTERVAL: Duration = Duration::from_millis(500);
const READINESS_LOSS_TOLERANCE: u32 = 6;
const TUNNEL_API_KEY_ENV: &str = "HARNESS_TUNNEL_API_KEY";
const TUNNEL_CLIENT_FILE_NAME: &str = "tunnel-client.exe";
const CLOUDFLARED_FILE_NAME: &str = "cloudflared.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TunnelInstance {
    pub instance_id: String,
    pub pid: u32,
    pub tunnel_id: Option<String>,
    pub started_at: String,
    pub executable_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TunnelExit {
    pub exit_code: Option<i32>,
    pub exited_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TunnelProcessError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TunnelReadinessEvidence {
    pub control_plane_connected: bool,
    pub mcp_target_reachable: bool,
    pub health_check_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TunnelStatus {
    pub state: TunnelState,
    pub instance: Option<TunnelInstance>,
    pub readiness: Option<TunnelReadinessEvidence>,
    pub last_exit: Option<TunnelExit>,
    pub last_error: Option<TunnelProcessError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TunnelStopResult {
    pub accepted: bool,
    pub forced: bool,
}

/// The raw API key is intentionally kept in a type without Debug or Serialize.
/// It is consumed only while constructing the managed child's environment.
#[derive(Clone)]
pub(crate) struct TunnelCredential {
    value: String,
}

impl TunnelCredential {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// Public launch data plus a short-lived credential. The credential is never
/// copied into status, errors, events, or serialized state.
pub struct TunnelLaunch {
    pub executable_path: PathBuf,
    pub cloudflared_path: PathBuf,
    pub tunnel_id: Option<String>,
    pub runtime_endpoint: String,
    credential: Option<TunnelCredential>,
    health_url_file: Option<PathBuf>,
    startup_timeout: Duration,
}

impl TunnelLaunch {
    pub fn new(executable_path: PathBuf, runtime_endpoint: impl Into<String>) -> Self {
        Self {
            executable_path,
            cloudflared_path: PathBuf::new(),
            tunnel_id: None,
            runtime_endpoint: runtime_endpoint.into(),
            credential: None,
            health_url_file: None,
            startup_timeout: STARTUP_TIMEOUT,
        }
    }

    pub fn with_tunnel_id(mut self, tunnel_id: impl Into<String>) -> Self {
        self.tunnel_id = Some(tunnel_id.into());
        self
    }

    pub fn with_cloudflared_path(mut self, path: PathBuf) -> Self {
        self.cloudflared_path = path;
        self
    }

    pub(crate) fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.credential = Some(TunnelCredential::new(api_key));
        self
    }

    pub(crate) fn with_health_url_file(mut self, path: PathBuf) -> Self {
        self.health_url_file = Some(path);
        self
    }

    #[cfg(test)]
    fn with_startup_timeout(mut self, timeout: Duration) -> Self {
        self.startup_timeout = timeout;
        self
    }
}

#[derive(Clone)]
struct ManagedTunnelProcess {
    instance: TunnelInstance,
    child: Arc<Mutex<Child>>,
    exited: Arc<(Mutex<bool>, Condvar)>,
    health_url_file: PathBuf,
    #[cfg(windows)]
    job: Arc<WindowsJobObject>,
}

struct TunnelInner {
    process: Option<ManagedTunnelProcess>,
    status: TunnelStatus,
    observer: Option<Arc<dyn TunnelEventSink>>,
}

pub(crate) trait TunnelEventSink: Send + Sync {
    fn tunnel_event(
        &self,
        instance_id: Option<&str>,
        state: TunnelState,
        error: Option<&TunnelProcessError>,
    );
}

pub struct TunnelProcessManager {
    inner: Arc<Mutex<TunnelInner>>,
    #[cfg(test)]
    test_command: Option<TestTunnelCommand>,
    #[cfg(test)]
    stop_timeout: Duration,
}

impl Clone for TunnelProcessManager {
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
struct TestTunnelCommand {
    executable_path: PathBuf,
    args: Vec<String>,
}

impl Default for TunnelProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TunnelProcessManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TunnelInner {
                process: None,
                status: TunnelStatus {
                    state: TunnelState::Stopped,
                    instance: None,
                    readiness: None,
                    last_exit: None,
                    last_error: None,
                },
                observer: None,
            })),
            #[cfg(test)]
            test_command: None,
            #[cfg(test)]
            stop_timeout: STOP_TIMEOUT,
        }
    }

    pub(crate) fn set_event_sink(&self, sink: Arc<dyn TunnelEventSink>) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.observer = Some(sink);
        }
    }

    pub fn start(&self, launch: TunnelLaunch) -> Result<TunnelStatus, TunnelProcessError> {
        #[cfg(test)]
        let test_mode = self.test_command.is_some();
        #[cfg(not(test))]
        let test_mode = false;
        let executable_path = self.resolve_executable(&launch)?;
        let cloudflared_path = if test_mode {
            launch.cloudflared_path.clone()
        } else {
            canonical_file(
                &launch.cloudflared_path,
                CLOUDFLARED_FILE_NAME,
                "Cloudflared executable is invalid.",
            )?
        };
        validate_launch(&launch, test_mode)?;
        let health_url_file = prepare_health_url_file(launch.health_url_file.clone())?;

        let instance_id = {
            let mut inner = self.lock_inner()?;
            match inner.status.state {
                TunnelState::Stopped | TunnelState::Failed => {}
                TunnelState::Starting => {
                    return Err(TunnelProcessError::conflict(
                        "Tunnel client is already starting.",
                    ))
                }
                TunnelState::Ready | TunnelState::Degraded => {
                    return Err(TunnelProcessError::conflict(
                        "Tunnel client is already running.",
                    ))
                }
                TunnelState::Stopping => {
                    return Err(TunnelProcessError::conflict("Tunnel client is stopping."))
                }
            }
            inner.process = None;
            inner.status = TunnelStatus {
                state: TunnelState::Starting,
                instance: None,
                readiness: None,
                last_exit: None,
                last_error: None,
            };
            uuid::Uuid::new_v4().to_string()
        };
        self.notify(None, TunnelState::Starting, None);

        let child = match self.spawn_process(
            &executable_path,
            &cloudflared_path,
            &launch,
            &health_url_file,
        ) {
            Ok(child) => child,
            Err(error) => {
                remove_health_file(&health_url_file);
                self.set_failed(None, error.clone());
                return Err(error);
            }
        };

        #[cfg(windows)]
        let job = match create_job_for_child(&child) {
            Ok(job) => Arc::new(job),
            Err(error) => {
                let mut child = child;
                let _ = child.kill();
                let _ = child.wait();
                remove_health_file(&health_url_file);
                self.set_failed(None, error.clone());
                return Err(error);
            }
        };
        #[cfg(windows)]
        if let Err(error) = resume_suspended_process(&child) {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            remove_health_file(&health_url_file);
            self.set_failed(None, error.clone());
            return Err(error);
        }

        let instance = TunnelInstance {
            instance_id,
            pid: child.id(),
            tunnel_id: launch
                .tunnel_id
                .clone()
                .filter(|value| !value.trim().is_empty()),
            started_at: timestamp(),
            executable_path,
        };
        let process = ManagedTunnelProcess {
            instance: instance.clone(),
            child: Arc::new(Mutex::new(child)),
            exited: Arc::new((Mutex::new(false), Condvar::new())),
            health_url_file,
            #[cfg(windows)]
            job,
        };

        {
            let mut inner = self.lock_inner()?;
            if inner.status.state != TunnelState::Starting {
                drop(inner);
                terminate_process(&process);
                return Err(TunnelProcessError::conflict(
                    "Tunnel lifecycle changed while starting.",
                ));
            }
            inner.process = Some(process.clone());
            inner.status.instance = Some(instance.clone());
        }
        spawn_exit_watcher(self.inner.clone(), process.clone());

        let readiness = if test_mode {
            Ok(TunnelReadinessEvidence {
                control_plane_connected: true,
                mcp_target_reachable: true,
                health_check_passed: true,
            })
        } else {
            wait_for_ready(&process, launch.startup_timeout)
        };
        let readiness = match readiness {
            Ok(readiness) => readiness,
            Err(error) => {
                self.set_failed(Some(&process.instance.instance_id), error.clone());
                let _ = self.stop_process(&process);
                return Err(error);
            }
        };

        if let Ok(mut inner) = self.lock_inner() {
            if inner
                .process
                .as_ref()
                .is_none_or(|active| active.instance.instance_id != process.instance.instance_id)
                || inner.status.state != TunnelState::Starting
            {
                drop(inner);
                let _ = self.stop_process(&process);
                return Err(TunnelProcessError::conflict(
                    "Tunnel lifecycle changed while waiting for readiness.",
                ));
            }
            inner.status.state = TunnelState::Ready;
            inner.status.readiness = Some(readiness);
            inner.status.last_error = None;
        }
        self.notify(
            Some(&process.instance.instance_id),
            TunnelState::Ready,
            None,
        );
        if !test_mode {
            spawn_readiness_watcher(self.inner.clone(), process);
        }
        self.status()
    }

    pub fn stop(&self) -> Result<TunnelStopResult, TunnelProcessError> {
        let instance_id = {
            let inner = self.lock_inner()?;
            inner
                .process
                .as_ref()
                .map(|process| process.instance.instance_id.clone())
        };
        match instance_id {
            Some(instance_id) => self.stop_instance(&instance_id),
            None => Ok(TunnelStopResult {
                accepted: false,
                forced: false,
            }),
        }
    }

    pub fn stop_instance(
        &self,
        expected_instance_id: &str,
    ) -> Result<TunnelStopResult, TunnelProcessError> {
        let process = {
            let mut inner = self.lock_inner()?;
            match inner.status.state {
                TunnelState::Stopped => {
                    return Ok(TunnelStopResult {
                        accepted: false,
                        forced: false,
                    })
                }
                TunnelState::Stopping => {
                    return Err(TunnelProcessError::conflict(
                        "Tunnel client is already stopping.",
                    ))
                }
                TunnelState::Starting
                | TunnelState::Ready
                | TunnelState::Degraded
                | TunnelState::Failed => {}
            }
            let process = inner.process.clone().ok_or_else(|| {
                TunnelProcessError::state_unavailable("Managed tunnel process is missing.")
            })?;
            if process.instance.instance_id != expected_instance_id {
                return Err(TunnelProcessError::instance_mismatch());
            }
            inner.status.state = TunnelState::Stopping;
            inner.status.last_error = None;
            self.notify_locked(
                &inner,
                Some(expected_instance_id),
                TunnelState::Stopping,
                None,
            );
            process
        };

        let result = self.stop_process(&process);
        match result {
            Ok(forced) => {
                remove_health_file(&process.health_url_file);
                Ok(TunnelStopResult {
                    accepted: true,
                    forced,
                })
            }
            Err(error) => {
                self.set_failed(Some(expected_instance_id), error.clone());
                Err(error)
            }
        }
    }

    pub fn restart(&self, launch: TunnelLaunch) -> Result<TunnelStatus, TunnelProcessError> {
        let active_instance_id = {
            let inner = self.lock_inner()?;
            inner
                .process
                .as_ref()
                .map(|process| process.instance.instance_id.clone())
        };
        if let Some(instance_id) = active_instance_id {
            let stopped = self.stop_instance(&instance_id)?;
            if !stopped.accepted {
                return Err(TunnelProcessError::conflict(
                    "Tunnel client could not be stopped before restart.",
                ));
            }
        } else {
            let state = self.status()?.state;
            if matches!(state, TunnelState::Starting | TunnelState::Stopping) {
                return Err(TunnelProcessError::conflict(
                    "Tunnel lifecycle is transitioning; restart is not safe.",
                ));
            }
        }
        self.start(launch)
    }

    pub fn status(&self) -> Result<TunnelStatus, TunnelProcessError> {
        Ok(self.lock_inner()?.status.clone())
    }

    fn resolve_executable(&self, launch: &TunnelLaunch) -> Result<PathBuf, TunnelProcessError> {
        #[cfg(test)]
        let candidate = self
            .test_command
            .as_ref()
            .map(|command| command.executable_path.clone())
            .unwrap_or_else(|| launch.executable_path.clone());
        #[cfg(not(test))]
        let candidate = launch.executable_path.clone();
        #[cfg(test)]
        if self.test_command.is_some() {
            return fs::canonicalize(&candidate)
                .map_err(|_| TunnelProcessError::executable_not_found(candidate));
        }
        canonical_file(
            &candidate,
            TUNNEL_CLIENT_FILE_NAME,
            "Tunnel client executable is invalid.",
        )
    }

    fn spawn_process(
        &self,
        executable_path: &Path,
        cloudflared_path: &Path,
        launch: &TunnelLaunch,
        health_url_file: &Path,
    ) -> Result<Child, TunnelProcessError> {
        let mut command = Command::new(executable_path);
        #[cfg(test)]
        if let Some(test_command) = &self.test_command {
            command.args(&test_command.args);
        } else {
            configure_real_command(&mut command, cloudflared_path, launch, health_url_file)?;
        }
        #[cfg(not(test))]
        configure_real_command(&mut command, cloudflared_path, launch, health_url_file)?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const CREATE_SUSPENDED: u32 = 0x0000_0004;
            command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW | CREATE_SUSPENDED);
        }
        command.spawn().map_err(|error| {
            TunnelProcessError::spawn(format!(
                "Could not start Tunnel client {}: {error}",
                executable_path.display()
            ))
        })
    }

    fn stop_process(&self, process: &ManagedTunnelProcess) -> Result<bool, TunnelProcessError> {
        let _ = request_graceful_termination(process.instance.pid);
        if wait_for_exit(&process.exited, self.stop_timeout()) {
            return Ok(false);
        }

        #[cfg(windows)]
        let force_result = terminate_job(&process.job);
        #[cfg(not(windows))]
        let force_result = process
            .child
            .lock()
            .map_err(|_| {
                TunnelProcessError::state_unavailable("Tunnel child handle is unavailable.")
            })
            .and_then(|mut child| child.kill().map_err(|error| error.to_string()));
        if let Err(error) = force_result {
            let _ = process
                .child
                .lock()
                .ok()
                .and_then(|mut child| child.kill().ok());
            if !wait_for_exit(&process.exited, Duration::from_millis(100)) {
                return Err(TunnelProcessError::stop_failed(format!(
                    "Could not force-stop owned Tunnel process: {error}"
                )));
            }
        }
        if !wait_for_exit(&process.exited, self.stop_timeout()) {
            return Err(TunnelProcessError::stop_failed(
                "Owned Tunnel process tree did not exit after force termination.",
            ));
        }
        Ok(true)
    }

    fn set_failed(&self, instance_id: Option<&str>, error: TunnelProcessError) {
        if let Ok(mut inner) = self.lock_inner() {
            if instance_id.is_none()
                || inner.process.as_ref().is_some_and(|process| {
                    Some(process.instance.instance_id.as_str()) == instance_id
                })
            {
                inner.status.state = TunnelState::Failed;
                inner.status.last_error = Some(error.clone());
                inner.status.readiness = None;
                self.notify_locked(&inner, instance_id, TunnelState::Failed, Some(&error));
            }
        }
    }

    fn notify(
        &self,
        instance_id: Option<&str>,
        state: TunnelState,
        error: Option<&TunnelProcessError>,
    ) {
        if let Ok(inner) = self.lock_inner() {
            self.notify_locked(&inner, instance_id, state, error);
        }
    }

    fn notify_locked(
        &self,
        inner: &TunnelInner,
        instance_id: Option<&str>,
        state: TunnelState,
        error: Option<&TunnelProcessError>,
    ) {
        notify_inner(inner, instance_id, state, error);
    }

    fn lock_inner(&self) -> Result<MutexGuard<'_, TunnelInner>, TunnelProcessError> {
        self.inner.lock().map_err(|_| {
            TunnelProcessError::state_unavailable("Tunnel lifecycle state is unavailable.")
        })
    }

    #[cfg(test)]
    fn with_test_command(command: TestTunnelCommand) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TunnelInner {
                process: None,
                status: TunnelStatus {
                    state: TunnelState::Stopped,
                    instance: None,
                    readiness: None,
                    last_exit: None,
                    last_error: None,
                },
                observer: None,
            })),
            test_command: Some(command),
            stop_timeout: Duration::from_millis(50),
        }
    }

    #[cfg(test)]
    fn stop_timeout(&self) -> Duration {
        self.stop_timeout
    }

    #[cfg(not(test))]
    fn stop_timeout(&self) -> Duration {
        STOP_TIMEOUT
    }
}

fn validate_launch(launch: &TunnelLaunch, test_mode: bool) -> Result<(), TunnelProcessError> {
    if test_mode {
        return Ok(());
    }
    if launch
        .tunnel_id
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err(TunnelProcessError::invalid_launch(
            "Tunnel ID is required before starting the managed Tunnel.",
        ));
    }
    validate_runtime_endpoint(&launch.runtime_endpoint)?;
    if launch
        .credential
        .as_ref()
        .is_none_or(|credential| credential.value.trim().is_empty())
    {
        return Err(TunnelProcessError::invalid_launch(
            "Tunnel API key is unavailable in the OS credential store.",
        ));
    }
    Ok(())
}

fn configure_real_command(
    command: &mut Command,
    cloudflared_path: &Path,
    launch: &TunnelLaunch,
    health_url_file: &Path,
) -> Result<(), TunnelProcessError> {
    let tunnel_id = launch
        .tunnel_id
        .as_deref()
        .ok_or_else(|| TunnelProcessError::invalid_launch("Tunnel ID is required."))?;
    let credential = launch
        .credential
        .as_ref()
        .ok_or_else(|| TunnelProcessError::invalid_launch("Tunnel API key is unavailable."))?;
    let mcp_target = format!("url={},channel=main", launch.runtime_endpoint);
    let cloudflared = cloudflared_path.display().to_string();
    let api_key_reference = format!("env:{TUNNEL_API_KEY_ENV}");
    let health_file = health_url_file.display().to_string();
    command
        .arg("run")
        .args(["--control-plane.tunnel-id", tunnel_id])
        .args(["--control-plane.api-key", &api_key_reference])
        .args(["--mcp.server-url", &mcp_target])
        .args(["--cloudflared.path", &cloudflared])
        .args(["--health.listen-addr", "127.0.0.1:0"])
        .args(["--health.url-file", &health_file])
        .env(TUNNEL_API_KEY_ENV, &credential.value);
    Ok(())
}

fn validate_runtime_endpoint(url: &str) -> Result<(), TunnelProcessError> {
    let authority_and_path = url.strip_prefix("http://").ok_or_else(|| {
        TunnelProcessError::invalid_launch("Runtime MCP endpoint must use loopback HTTP.")
    })?;
    let (authority, path) = authority_and_path.split_once('/').ok_or_else(|| {
        TunnelProcessError::invalid_launch("Runtime MCP endpoint path is invalid.")
    })?;
    let address: SocketAddr = authority.parse().map_err(|_| {
        TunnelProcessError::invalid_launch("Runtime MCP endpoint authority is invalid.")
    })?;
    if !address.ip().is_loopback() || address.port() == 0 || path != "mcp" {
        return Err(TunnelProcessError::invalid_launch(
            "Runtime MCP endpoint must be a loopback /mcp URL.",
        ));
    }
    Ok(())
}

fn prepare_health_url_file(path: Option<PathBuf>) -> Result<PathBuf, TunnelProcessError> {
    let path = path.unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "chatgpt-harness-tunnel-{}.url",
            uuid::Uuid::new_v4()
        ))
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| {
            TunnelProcessError::readiness("Tunnel health discovery directory is unavailable.")
        })?;
    }
    remove_health_file(&path);
    Ok(path)
}

fn wait_for_ready(
    process: &ManagedTunnelProcess,
    timeout: Duration,
) -> Result<TunnelReadinessEvidence, TunnelProcessError> {
    let deadline = Instant::now() + timeout;
    let mut discovered_url: Option<String> = None;
    loop {
        if wait_for_exit(&process.exited, Duration::from_millis(0)) {
            return Err(TunnelProcessError::readiness(
                "Tunnel process exited before readiness was established.",
            ));
        }
        if discovered_url.is_none() && process.health_url_file.is_file() {
            let value = fs::read_to_string(&process.health_url_file).map_err(|_| {
                TunnelProcessError::readiness("Tunnel health URL discovery could not be read.")
            })?;
            let value = value.trim().to_owned();
            validate_health_url(&value)?;
            discovered_url = Some(value);
        }
        if let Some(url) = discovered_url.as_deref() {
            if let Ok(evidence) = probe_ready(url) {
                return Ok(evidence);
            }
        }
        if Instant::now() >= deadline {
            return Err(TunnelProcessError::readiness(
                "Tunnel readiness timed out before control-plane, MCP target, and health checks passed.",
            ));
        }
        thread::sleep(READINESS_POLL_INTERVAL);
    }
}

fn spawn_readiness_watcher(inner: Arc<Mutex<TunnelInner>>, process: ManagedTunnelProcess) {
    thread::spawn(move || {
        let mut failures = 0;
        loop {
            thread::sleep(READINESS_MONITOR_INTERVAL);
            if wait_for_exit(&process.exited, Duration::from_millis(0)) {
                return;
            }
            let is_current = inner.lock().ok().is_some_and(|inner| {
                (inner.status.state == TunnelState::Ready
                    || inner.status.state == TunnelState::Degraded)
                    && inner.process.as_ref().is_some_and(|active| {
                        active.instance.instance_id == process.instance.instance_id
                    })
            });
            if !is_current {
                return;
            }
            let health_url = fs::read_to_string(&process.health_url_file)
                .ok()
                .map(|value| value.trim().to_owned());
            let result = health_url.as_deref().and_then(|url| probe_ready(url).ok());
            if let Some(evidence) = result {
                failures = 0;
                if let Ok(mut inner) = inner.lock() {
                    if inner.process.as_ref().is_some_and(|active| {
                        active.instance.instance_id == process.instance.instance_id
                    }) && inner.status.state == TunnelState::Degraded
                    {
                        inner.status.state = TunnelState::Ready;
                        inner.status.readiness = Some(evidence);
                        notify_inner(
                            &inner,
                            Some(&process.instance.instance_id),
                            TunnelState::Ready,
                            None,
                        );
                    }
                }
            } else {
                failures += 1;
                if let Ok(mut inner) = inner.lock() {
                    if inner.process.as_ref().is_none_or(|active| {
                        active.instance.instance_id != process.instance.instance_id
                    }) {
                        return;
                    }
                    let error = TunnelProcessError::readiness(
                        "Tunnel readiness probe is temporarily unhealthy.",
                    );
                    if failures >= READINESS_LOSS_TOLERANCE {
                        inner.status.state = TunnelState::Failed;
                        inner.status.readiness = None;
                        inner.status.last_error = Some(error.clone());
                        notify_inner(
                            &inner,
                            Some(&process.instance.instance_id),
                            TunnelState::Failed,
                            Some(&error),
                        );
                    } else {
                        inner.status.state = TunnelState::Degraded;
                        notify_inner(
                            &inner,
                            Some(&process.instance.instance_id),
                            TunnelState::Degraded,
                            Some(&error),
                        );
                    }
                }
                if failures >= READINESS_LOSS_TOLERANCE {
                    return;
                }
            }
        }
    });
}

fn probe_ready(url: &str) -> Result<TunnelReadinessEvidence, TunnelProcessError> {
    let address = validate_health_url(url)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(250))
        .map_err(|_| TunnelProcessError::readiness("Tunnel health endpoint is unreachable."))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
    let request = format!(
        "GET /readyz HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        address
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|_| TunnelProcessError::readiness("Tunnel readiness request failed."))?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|_| {
        TunnelProcessError::readiness("Tunnel readiness response could not be read.")
    })?;
    let response = String::from_utf8_lossy(&response);
    let status_line = response.lines().next().unwrap_or_default();
    if !status_line.contains(" 200 ") {
        return Err(TunnelProcessError::readiness(
            "Tunnel /readyz did not report ready.",
        ));
    }
    let lowercase = response.to_ascii_lowercase().replace(' ', "");
    for marker in [
        "\"ready\":false",
        "\"control_plane_connected\":false",
        "\"mcp_target_reachable\":false",
        "\"healthy\":false",
    ] {
        if lowercase.contains(marker) {
            return Err(TunnelProcessError::readiness(
                "Tunnel /readyz reported an unhealthy dependency.",
            ));
        }
    }
    Ok(TunnelReadinessEvidence {
        control_plane_connected: true,
        mcp_target_reachable: true,
        health_check_passed: true,
    })
}

fn validate_health_url(url: &str) -> Result<SocketAddr, TunnelProcessError> {
    let authority_and_path = url.strip_prefix("http://").ok_or_else(|| {
        TunnelProcessError::readiness("Tunnel health URL must use loopback HTTP.")
    })?;
    let authority = authority_and_path.trim_end_matches('/');
    if authority.contains('/') {
        return Err(TunnelProcessError::readiness(
            "Tunnel health URL must be a base loopback URL.",
        ));
    }
    let address: SocketAddr = authority
        .parse()
        .map_err(|_| TunnelProcessError::readiness("Tunnel health URL is malformed."))?;
    if !address.ip().is_loopback() || address.port() == 0 {
        return Err(TunnelProcessError::readiness(
            "Tunnel health URL must bind to loopback.",
        ));
    }
    Ok(address)
}

fn spawn_exit_watcher(inner: Arc<Mutex<TunnelInner>>, process: ManagedTunnelProcess) {
    thread::spawn(move || {
        let exit_code = loop {
            let result = process
                .child
                .lock()
                .ok()
                .and_then(|mut child| child.try_wait().ok().flatten());
            if let Some(status) = result {
                break status.code();
            }
            thread::sleep(Duration::from_millis(25));
        };
        let exit = TunnelExit {
            exit_code,
            exited_at: timestamp(),
        };

        if let Ok(mut inner) = inner.lock() {
            let is_current = inner
                .process
                .as_ref()
                .is_some_and(|active| active.instance.instance_id == process.instance.instance_id);
            if is_current {
                let stopped_by_manager = inner.status.state == TunnelState::Stopping;
                inner.process = None;
                inner.status = TunnelStatus {
                    state: if stopped_by_manager {
                        TunnelState::Stopped
                    } else {
                        TunnelState::Failed
                    },
                    instance: Some(process.instance.clone()),
                    readiness: None,
                    last_exit: Some(exit),
                    last_error: if stopped_by_manager {
                        None
                    } else {
                        Some(TunnelProcessError::crashed(exit_code))
                    },
                };
                let error = inner.status.last_error.clone();
                notify_inner(
                    &inner,
                    Some(&process.instance.instance_id),
                    inner.status.state,
                    error.as_ref(),
                );
            }
        }
        remove_health_file(&process.health_url_file);
        let (lock, wake) = &*process.exited;
        if let Ok(mut exited) = lock.lock() {
            *exited = true;
            wake.notify_all();
        }
    });
}

fn notify_inner(
    inner: &TunnelInner,
    instance_id: Option<&str>,
    state: TunnelState,
    error: Option<&TunnelProcessError>,
) {
    if let Some(observer) = &inner.observer {
        observer.tunnel_event(instance_id, state, error);
    }
}

fn wait_for_exit(exited: &Arc<(Mutex<bool>, Condvar)>, timeout: Duration) -> bool {
    let (lock, wake) = &**exited;
    let Ok(exited) = lock.lock() else {
        return false;
    };
    if *exited {
        return true;
    }
    wake.wait_timeout_while(exited, timeout, |value| !*value)
        .map(|(value, _)| *value)
        .unwrap_or(false)
}

fn terminate_process(process: &ManagedTunnelProcess) {
    #[cfg(windows)]
    {
        let _ = terminate_job(&process.job);
    }
    if let Ok(mut child) = process.child.lock() {
        let _ = child.kill();
    }
}

fn remove_health_file(path: &Path) {
    if !path.as_os_str().is_empty() {
        let _ = fs::remove_file(path);
    }
}

fn canonical_file(
    path: &Path,
    expected_name: &str,
    message: &str,
) -> Result<PathBuf, TunnelProcessError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| TunnelProcessError::executable_not_found(path.to_path_buf()))?;
    if !metadata.is_file() || is_link_or_reparse_point(&metadata) {
        return Err(TunnelProcessError::distribution(message));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|_| TunnelProcessError::executable_not_found(path.to_path_buf()))?;
    if canonical.file_name().and_then(|name| name.to_str()) != Some(expected_name) {
        return Err(TunnelProcessError::distribution(message));
    }
    Ok(canonical)
}

#[cfg(windows)]
fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || (metadata.file_attributes() & 0x400) != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

impl TunnelProcessError {
    pub(crate) fn distribution(message: impl Into<String>) -> Self {
        Self {
            code: "TUNNEL_DISTRIBUTION_INVALID".into(),
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: "LIFECYCLE_CONFLICT".into(),
            message: message.into(),
        }
    }

    fn invalid_launch(message: impl Into<String>) -> Self {
        Self {
            code: "TUNNEL_LAUNCH_INVALID".into(),
            message: message.into(),
        }
    }

    fn executable_not_found(path: PathBuf) -> Self {
        Self {
            code: "TUNNEL_EXECUTABLE_NOT_FOUND".into(),
            message: format!("Tunnel executable is unavailable at {}.", path.display()),
        }
    }

    fn spawn(message: impl Into<String>) -> Self {
        Self {
            code: "TUNNEL_SPAWN_FAILED".into(),
            message: message.into(),
        }
    }

    fn instance_mismatch() -> Self {
        Self {
            code: "TUNNEL_INSTANCE_MISMATCH".into(),
            message: "The requested tunnel instance is not the currently managed process.".into(),
        }
    }

    fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "TUNNEL_STATE_UNAVAILABLE".into(),
            message: message.into(),
        }
    }

    fn stop_failed(message: impl Into<String>) -> Self {
        Self {
            code: "TUNNEL_STOP_FAILED".into(),
            message: message.into(),
        }
    }

    fn readiness(message: impl Into<String>) -> Self {
        Self {
            code: "TUNNEL_NOT_READY".into(),
            message: message.into(),
        }
    }

    fn crashed(exit_code: Option<i32>) -> Self {
        Self {
            code: "TUNNEL_PROCESS_EXITED".into(),
            message: match exit_code {
                Some(code) => format!("Tunnel process exited unexpectedly with exit code {code}."),
                None => "Tunnel process exited unexpectedly.".into(),
            },
        }
    }
}

#[cfg(windows)]
struct WindowsJobObject {
    handle: *mut std::ffi::c_void,
}

#[cfg(windows)]
unsafe impl Send for WindowsJobObject {}
#[cfg(windows)]
unsafe impl Sync for WindowsJobObject {}

#[cfg(windows)]
impl Drop for WindowsJobObject {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
#[repr(C)]
struct IoCounters {
    read_operations: u64,
    write_operations: u64,
    other_operations: u64,
    read_bytes: u64,
    write_bytes: u64,
    other_bytes: u64,
}

#[cfg(windows)]
#[repr(C)]
struct JobBasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[cfg(windows)]
#[repr(C)]
struct JobExtendedLimitInformation {
    basic_limit_information: JobBasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[cfg(windows)]
extern "system" {
    fn CreateJobObjectW(
        attributes: *mut std::ffi::c_void,
        name: *const u16,
    ) -> *mut std::ffi::c_void;
    fn SetInformationJobObject(
        job: *mut std::ffi::c_void,
        info_class: u32,
        info: *mut std::ffi::c_void,
        info_length: u32,
    ) -> i32;
    fn AssignProcessToJobObject(job: *mut std::ffi::c_void, process: *mut std::ffi::c_void) -> i32;
    fn TerminateJobObject(job: *mut std::ffi::c_void, exit_code: u32) -> i32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut std::ffi::c_void;
    fn Thread32First(snapshot: *mut std::ffi::c_void, entry: *mut ThreadEntry32) -> i32;
    fn Thread32Next(snapshot: *mut std::ffi::c_void, entry: *mut ThreadEntry32) -> i32;
    fn OpenThread(
        desired_access: u32,
        inherit_handle: i32,
        thread_id: u32,
    ) -> *mut std::ffi::c_void;
    fn ResumeThread(thread: *mut std::ffi::c_void) -> u32;
}

#[cfg(windows)]
#[repr(C)]
struct ThreadEntry32 {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}

#[cfg(windows)]
fn create_job_for_child(child: &Child) -> Result<WindowsJobObject, TunnelProcessError> {
    use std::os::windows::io::AsRawHandle;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
    let handle = unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) };
    if handle.is_null() {
        return Err(TunnelProcessError::spawn(
            "Could not create the Tunnel process Job Object.",
        ));
    }
    let mut limits = JobExtendedLimitInformation {
        basic_limit_information: JobBasicLimitInformation {
            per_process_user_time_limit: 0,
            per_job_user_time_limit: 0,
            limit_flags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            minimum_working_set_size: 0,
            maximum_working_set_size: 0,
            active_process_limit: 0,
            affinity: 0,
            priority_class: 0,
            scheduling_class: 0,
        },
        io_info: IoCounters {
            read_operations: 0,
            write_operations: 0,
            other_operations: 0,
            read_bytes: 0,
            write_bytes: 0,
            other_bytes: 0,
        },
        process_memory_limit: 0,
        job_memory_limit: 0,
        peak_process_memory_used: 0,
        peak_job_memory_used: 0,
    };
    let configured = unsafe {
        SetInformationJobObject(
            handle,
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
            (&mut limits as *mut JobExtendedLimitInformation).cast(),
            std::mem::size_of::<JobExtendedLimitInformation>() as u32,
        ) != 0
    };
    if !configured {
        unsafe { CloseHandle(handle) };
        return Err(TunnelProcessError::spawn(
            "Could not configure the Tunnel process Job Object.",
        ));
    }
    let assigned =
        unsafe { AssignProcessToJobObject(handle, child.as_raw_handle() as *mut _) != 0 };
    if !assigned {
        unsafe { CloseHandle(handle) };
        return Err(TunnelProcessError::spawn(
            "Could not assign the Tunnel process Job Object.",
        ));
    }
    Ok(WindowsJobObject { handle })
}

#[cfg(windows)]
fn terminate_job(job: &WindowsJobObject) -> Result<(), String> {
    if unsafe { TerminateJobObject(job.handle, 1) } == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn resume_suspended_process(child: &Child) -> Result<(), TunnelProcessError> {
    const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
    const THREAD_SUSPEND_RESUME: u32 = 0x0002;
    const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = -1isize as *mut std::ffi::c_void;
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
        return Err(TunnelProcessError::spawn(
            "Could not enumerate the suspended Tunnel process thread.",
        ));
    }
    let mut entry = ThreadEntry32 {
        size: std::mem::size_of::<ThreadEntry32>() as u32,
        usage: 0,
        thread_id: 0,
        owner_process_id: 0,
        base_priority: 0,
        delta_priority: 0,
        flags: 0,
    };
    let mut resumed = false;
    let mut found = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    while found {
        if entry.owner_process_id == child.id() {
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.thread_id) };
            if !thread.is_null() {
                let result = unsafe { ResumeThread(thread) };
                unsafe { CloseHandle(thread) };
                if result != u32::MAX {
                    resumed = true;
                    break;
                }
            }
        }
        found = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    if resumed {
        Ok(())
    } else {
        Err(TunnelProcessError::spawn(
            "Could not resume the managed Tunnel process.",
        ))
    }
}

#[cfg(windows)]
fn request_graceful_termination(process_group_id: u32) -> std::io::Result<()> {
    extern "system" {
        fn GenerateConsoleCtrlEvent(control_type: u32, process_group_id: u32) -> i32;
    }
    const CTRL_BREAK_EVENT: u32 = 1;
    let result = unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, process_group_id) };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn request_graceful_termination(_process_group_id: u32) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "graceful Tunnel termination is only implemented for Windows",
    ))
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::time::{Duration, Instant};

    fn cmd_path() -> PathBuf {
        std::env::var_os("ComSpec")
            .map(PathBuf::from)
            .expect("Windows command processor must be available")
    }

    fn sleeping_manager() -> TunnelProcessManager {
        TunnelProcessManager::with_test_command(TestTunnelCommand {
            executable_path: cmd_path(),
            args: vec!["/C".into(), "ping 127.0.0.1 -n 30 > NUL".into()],
        })
    }

    fn crashing_manager() -> TunnelProcessManager {
        TunnelProcessManager::with_test_command(TestTunnelCommand {
            executable_path: cmd_path(),
            args: vec!["/C".into(), "exit /b 7".into()],
        })
    }

    fn launch() -> TunnelLaunch {
        TunnelLaunch::new(cmd_path(), "http://127.0.0.1:39001/mcp")
            .with_tunnel_id("tunnel-a")
            .with_startup_timeout(Duration::from_millis(250))
    }

    fn wait_for_state(manager: &TunnelProcessManager, expected: TunnelState) -> TunnelStatus {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = manager.status().unwrap();
            if status.state == expected {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "state did not become {expected:?}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn start_creates_a_ready_owned_instance_with_readiness_evidence() {
        let manager = sleeping_manager();
        let status = manager.start(launch()).unwrap();
        let instance = status.instance.clone().unwrap();

        assert_eq!(status.state, TunnelState::Ready);
        assert!(!instance.instance_id.is_empty());
        assert!(instance.pid > 0);
        assert_eq!(instance.tunnel_id.as_deref(), Some("tunnel-a"));
        assert_eq!(
            status.readiness,
            Some(TunnelReadinessEvidence {
                control_plane_connected: true,
                mcp_target_reachable: true,
                health_check_passed: true,
            })
        );
        assert!(manager.stop().unwrap().accepted);
        assert_eq!(
            wait_for_state(&manager, TunnelState::Stopped).state,
            TunnelState::Stopped
        );
    }

    #[test]
    fn old_instance_id_cannot_stop_a_new_process() {
        let manager = sleeping_manager();
        let old_instance_id = manager
            .start(launch())
            .unwrap()
            .instance
            .unwrap()
            .instance_id;
        assert!(manager.stop_instance(&old_instance_id).unwrap().accepted);
        wait_for_state(&manager, TunnelState::Stopped);

        let new_instance_id = manager
            .start(launch())
            .unwrap()
            .instance
            .unwrap()
            .instance_id;
        let error = manager.stop_instance(&old_instance_id).unwrap_err();
        assert_eq!(error.code, "TUNNEL_INSTANCE_MISMATCH");
        assert_eq!(manager.status().unwrap().state, TunnelState::Ready);
        assert_ne!(old_instance_id, new_instance_id);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn unexpected_exit_transitions_ready_to_failed() {
        let manager = crashing_manager();
        manager.start(launch()).unwrap();

        let status = wait_for_state(&manager, TunnelState::Failed);
        assert_eq!(status.last_exit.unwrap().exit_code, Some(7));
        assert_eq!(status.last_error.unwrap().code, "TUNNEL_PROCESS_EXITED");
    }

    #[test]
    fn duplicate_start_is_rejected() {
        let manager = sleeping_manager();
        manager.start(launch()).unwrap();

        let error = manager.start(launch()).unwrap_err();
        assert_eq!(error.code, "LIFECYCLE_CONFLICT");
        assert_eq!(manager.status().unwrap().state, TunnelState::Ready);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn real_command_uses_secret_reference_and_not_the_raw_key_in_argv() {
        let mut command = Command::new(cmd_path());
        let health = std::env::temp_dir().join("harness-test-health.url");
        let launch = launch().with_api_key("raw-api-key");
        configure_real_command(
            &mut command,
            Path::new(r"C:\tunnel\cloudflared.exe"),
            &launch,
            &health,
        )
        .unwrap();
        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(args
            .iter()
            .any(|value| value == "env:HARNESS_TUNNEL_API_KEY"));
        assert!(!args.iter().any(|value| value.contains("raw-api-key")));
        let envs = command
            .get_envs()
            .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value.to_owned())))
            .collect::<Vec<(OsString, OsString)>>();
        assert!(envs
            .iter()
            .any(|(key, value)| { key == "HARNESS_TUNNEL_API_KEY" && value == "raw-api-key" }));
    }

    #[test]
    fn readiness_requires_loopback_health_and_http_200() {
        assert!(validate_health_url("http://127.0.0.1:39001").is_ok());
        assert!(validate_health_url("http://0.0.0.0:39001").is_err());
        assert!(validate_health_url("https://127.0.0.1:39001").is_err());
        assert!(probe_ready("http://127.0.0.1:1").is_err());
    }
}
