mod discovery;

pub(crate) use discovery::resolve_tunnel;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    io::{BufRead, BufReader, Read, Write},
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
#[cfg(not(test))]
const READINESS_MONITOR_INTERVAL: Duration = Duration::from_millis(500);
#[cfg(test)]
const READINESS_MONITOR_INTERVAL: Duration = Duration::from_millis(20);
const READINESS_LOSS_TOLERANCE: u32 = 6;
const CONTROL_PLANE_POLL_FRESHNESS: Duration = Duration::from_secs(90);
const CONTROL_PLANE_RESTART_AGE: Duration = Duration::from_secs(120);
const RECOVERY_BASE_DELAY: Duration = Duration::from_secs(1);
const RECOVERY_MAX_DELAY: Duration = Duration::from_secs(60);
const RECOVERY_WINDOW: Duration = Duration::from_secs(10 * 60);
const RECOVERY_COOLDOWN: Duration = Duration::from_secs(10 * 60);
const RECOVERY_MAX_ATTEMPTS: usize = 6;
const LOG_ROLL_BYTES: u64 = 5 * 1024 * 1024;
const LOG_ROLL_FILES: usize = 3;
const CONTROL_PLANE_POLL_METRIC: &str = "commands_poll_last_successful_timestamp_seconds";
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelRecoveryPhase {
    Disarmed,
    Idle,
    Backoff,
    Cooldown,
    HalfOpen,
    Recovering,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelRecoveryStatus {
    pub phase: TunnelRecoveryPhase,
    pub reason: Option<TunnelProcessError>,
    pub next_attempt_at: Option<String>,
    pub cooldown_until: Option<String>,
}

impl Default for TunnelRecoveryStatus {
    fn default() -> Self {
        Self {
            phase: TunnelRecoveryPhase::Disarmed,
            reason: None,
            next_attempt_at: None,
            cooldown_until: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelControlPlaneRoute {
    pub route_mode: String,
    pub proxy_source: String,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelHealthState {
    Unknown,
    Starting,
    Stopping,
    Ready,
    Degraded,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelHealth {
    pub process: TunnelHealthState,
    pub local_mcp: TunnelHealthState,
    pub control_plane: TunnelHealthState,
}

impl TunnelHealth {
    pub(crate) fn starting() -> Self {
        Self {
            process: TunnelHealthState::Starting,
            local_mcp: TunnelHealthState::Unknown,
            control_plane: TunnelHealthState::Unknown,
        }
    }

    pub(crate) fn stopped() -> Self {
        Self {
            process: TunnelHealthState::Stopped,
            local_mcp: TunnelHealthState::Stopped,
            control_plane: TunnelHealthState::Stopped,
        }
    }

    fn stopping() -> Self {
        Self {
            process: TunnelHealthState::Stopping,
            local_mcp: TunnelHealthState::Stopping,
            control_plane: TunnelHealthState::Stopping,
        }
    }

    pub(crate) fn failed() -> Self {
        Self {
            process: TunnelHealthState::Failed,
            local_mcp: TunnelHealthState::Failed,
            control_plane: TunnelHealthState::Failed,
        }
    }

    fn probe_failed(previous: Option<&Self>) -> Self {
        Self {
            process: TunnelHealthState::Ready,
            local_mcp: previous
                .map(|health| health.local_mcp)
                .unwrap_or(TunnelHealthState::Unknown),
            control_plane: TunnelHealthState::Unknown,
        }
    }

    fn all_ready(self) -> bool {
        self.process == TunnelHealthState::Ready
            && self.local_mcp == TunnelHealthState::Ready
            && self.control_plane == TunnelHealthState::Ready
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelReadinessEvidence {
    pub control_plane_connected: bool,
    pub mcp_target_reachable: bool,
    pub health_check_passed: bool,
    pub control_plane_route: Option<TunnelControlPlaneRoute>,
    pub control_plane_error: Option<String>,
    pub last_successful_poll_timestamp: Option<f64>,
    pub last_successful_poll_age_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct AdminStatus {
    control_plane_route: Option<AdminRoute>,
    tunnel_metadata_error: Option<String>,
    #[serde(default)]
    channels: Vec<AdminChannel>,
}

#[derive(Debug, Deserialize)]
struct AdminRoute {
    route_mode: String,
    proxy_source: String,
    proxy_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdminChannel {
    name: String,
    probe_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TunnelStatus {
    pub state: TunnelState,
    pub instance: Option<TunnelInstance>,
    pub generation: u64,
    pub health: TunnelHealth,
    pub readiness: Option<TunnelReadinessEvidence>,
    pub last_exit: Option<TunnelExit>,
    pub last_error: Option<TunnelProcessError>,
    pub recovery: TunnelRecoveryStatus,
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
#[derive(Clone)]
pub struct TunnelLaunch {
    pub executable_path: PathBuf,
    pub cloudflared_path: PathBuf,
    pub tunnel_id: Option<String>,
    pub control_plane_http_proxy: Option<String>,
    pub runtime_endpoint: String,
    credential: Option<TunnelCredential>,
    health_url_file: Option<PathBuf>,
    log_file: Option<PathBuf>,
    startup_timeout: Duration,
}

impl TunnelLaunch {
    pub fn new(executable_path: PathBuf, runtime_endpoint: impl Into<String>) -> Self {
        Self {
            executable_path,
            cloudflared_path: PathBuf::new(),
            tunnel_id: None,
            control_plane_http_proxy: None,
            runtime_endpoint: runtime_endpoint.into(),
            credential: None,
            health_url_file: None,
            log_file: None,
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

    pub fn with_control_plane_http_proxy(mut self, proxy: Option<String>) -> Self {
        self.control_plane_http_proxy = proxy;
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

    pub(crate) fn with_log_file(mut self, path: PathBuf) -> Self {
        self.log_file = Some(path);
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

#[derive(Clone)]
struct TunnelLogger {
    path: Arc<Mutex<Option<PathBuf>>>,
}

impl TunnelLogger {
    fn new() -> Self {
        Self {
            path: Arc::new(Mutex::new(None)),
        }
    }

    fn set_path(&self, path: Option<PathBuf>) {
        if let Ok(mut current) = self.path.lock() {
            *current = path;
        }
    }

    fn event(&self, event: &str, fields: serde_json::Value) {
        let Ok(path) = self.path.lock() else {
            return;
        };
        let Some(path) = path.as_ref() else {
            return;
        };
        let record = serde_json::json!({
            "time": timestamp(),
            "component": "tunnel-supervisor",
            "event": event,
            "fields": redact_json(fields),
        });
        let Ok(line) = serde_json::to_string(&record) else {
            return;
        };
        let _ = append_rolled_log(path, &line);
    }
}

struct RecoveryState {
    desired_running: bool,
    generation: u64,
    launch: Option<TunnelLaunch>,
    recovering: bool,
    token: u64,
    attempts: VecDeque<Instant>,
    blocked: Option<TunnelProcessError>,
    last_error: Option<TunnelProcessError>,
    next_attempt_at: Option<Instant>,
    next_attempt_at_wall: Option<String>,
    cooldown_until: Option<Instant>,
    cooldown_until_wall: Option<String>,
    half_open: bool,
    active_instance_id: Option<String>,
    active_instance_token: Option<u64>,
    stop_in_progress: bool,
}

impl RecoveryState {
    fn new() -> Self {
        Self {
            desired_running: false,
            generation: 0,
            launch: None,
            recovering: false,
            token: 0,
            attempts: VecDeque::new(),
            blocked: None,
            last_error: None,
            next_attempt_at: None,
            next_attempt_at_wall: None,
            cooldown_until: None,
            cooldown_until_wall: None,
            half_open: false,
            active_instance_id: None,
            active_instance_token: None,
            stop_in_progress: false,
        }
    }

    fn status(&self, now: Instant) -> TunnelRecoveryStatus {
        let phase = if !self.desired_running {
            TunnelRecoveryPhase::Disarmed
        } else if self.blocked.is_some() {
            TunnelRecoveryPhase::Blocked
        } else if self.cooldown_until.is_some_and(|until| until > now) {
            TunnelRecoveryPhase::Cooldown
        } else if self.cooldown_until.is_some() || self.half_open {
            TunnelRecoveryPhase::HalfOpen
        } else if self.next_attempt_at.is_some() {
            TunnelRecoveryPhase::Backoff
        } else if self.recovering {
            TunnelRecoveryPhase::Recovering
        } else {
            TunnelRecoveryPhase::Idle
        };
        let reason = self
            .blocked
            .clone()
            .or_else(|| self.last_error.clone())
            .map(|mut error| {
                error.message = redact_sensitive_text(&error.message);
                error
            });
        let next_attempt_at = match phase {
            TunnelRecoveryPhase::Backoff | TunnelRecoveryPhase::Cooldown => {
                self.next_attempt_at_wall.clone()
            }
            _ => None,
        };
        let cooldown_until = if phase == TunnelRecoveryPhase::Cooldown {
            self.cooldown_until_wall.clone()
        } else {
            None
        };
        TunnelRecoveryStatus {
            phase,
            reason,
            next_attempt_at,
            cooldown_until,
        }
    }

    fn reset_backoff(&mut self) {
        self.last_error = None;
        self.next_attempt_at = None;
        self.next_attempt_at_wall = None;
        self.cooldown_until = None;
        self.cooldown_until_wall = None;
        self.half_open = false;
    }

    fn prepare_attempt(&mut self, now: Instant, generation: u64) -> (Duration, usize, u64) {
        let mut attempt = self.attempts.len() + 1;
        let delay = if self.half_open || self.attempts.len() >= RECOVERY_MAX_ATTEMPTS {
            self.attempts.clear();
            self.half_open = false;
            self.cooldown_until = Some(now + RECOVERY_COOLDOWN);
            attempt = RECOVERY_MAX_ATTEMPTS + 1;
            RECOVERY_COOLDOWN
        } else {
            self.attempts.push_back(now);
            self.half_open = false;
            recovery_delay(attempt, recovery_jitter_seed(generation, attempt))
        };
        let deadline = timestamp_after(delay);
        self.next_attempt_at = Some(now + delay);
        self.next_attempt_at_wall = Some(deadline.clone());
        self.cooldown_until_wall = self.cooldown_until.map(|_| deadline);
        self.recovering = true;
        self.token = self.token.wrapping_add(1);
        (delay, attempt, self.token)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryTrigger {
    ProcessExit,
    HealthProbe,
    ControlPlaneStale,
}

impl RecoveryTrigger {
    fn as_str(self) -> &'static str {
        match self {
            Self::ProcessExit => "process_exit",
            Self::HealthProbe => "health_probe",
            Self::ControlPlaneStale => "control_plane_stale",
        }
    }
}

pub(crate) trait TunnelEventSink: Send + Sync {
    fn tunnel_event(
        &self,
        generation: u64,
        instance_id: Option<&str>,
        state: TunnelState,
        health: TunnelHealth,
        readiness: Option<&TunnelReadinessEvidence>,
        recovery: &TunnelRecoveryStatus,
        error: Option<&TunnelProcessError>,
    );
}

/// Snapshot of a Tunnel lifecycle event. The observer is invoked only after
/// the Tunnel state mutex has been released so Host callbacks may safely query
/// the manager again (for example while projecting Harness status).
struct TunnelNotification {
    observer: Arc<dyn TunnelEventSink>,
    generation: u64,
    instance_id: Option<String>,
    state: TunnelState,
    health: TunnelHealth,
    readiness: Option<TunnelReadinessEvidence>,
    recovery: TunnelRecoveryStatus,
    error: Option<TunnelProcessError>,
}

impl TunnelNotification {
    fn emit(self) {
        self.observer.tunnel_event(
            self.generation,
            self.instance_id.as_deref(),
            self.state,
            self.health,
            self.readiness.as_ref(),
            &self.recovery,
            self.error.as_ref(),
        );
    }
}

pub struct TunnelProcessManager {
    inner: Arc<Mutex<TunnelInner>>,
    recovery: Arc<Mutex<RecoveryState>>,
    operation: Arc<Mutex<()>>,
    logger: TunnelLogger,
    #[cfg(test)]
    test_command: Option<TestTunnelCommand>,
    #[cfg(test)]
    auto_recovery_enabled: bool,
    #[cfg(test)]
    stop_timeout: Duration,
}

impl Clone for TunnelProcessManager {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            recovery: self.recovery.clone(),
            operation: self.operation.clone(),
            logger: self.logger.clone(),
            #[cfg(test)]
            test_command: self.test_command.clone(),
            #[cfg(test)]
            auto_recovery_enabled: self.auto_recovery_enabled,
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
                    generation: 0,
                    health: TunnelHealth::stopped(),
                    readiness: None,
                    last_exit: None,
                    last_error: None,
                    recovery: TunnelRecoveryStatus::default(),
                },
                observer: None,
            })),
            recovery: Arc::new(Mutex::new(RecoveryState::new())),
            operation: Arc::new(Mutex::new(())),
            logger: TunnelLogger::new(),
            #[cfg(test)]
            test_command: None,
            #[cfg(test)]
            auto_recovery_enabled: false,
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
        let _operation = self.try_manual_operation()?;
        self.start_internal(launch)
    }

    fn start_internal(&self, launch: TunnelLaunch) -> Result<TunnelStatus, TunnelProcessError> {
        let (generation, recovery_token) = self.begin_manual_start();
        self.logger.set_path(launch.log_file.clone());
        self.logger.event(
            "manual_start_requested",
            serde_json::json!({
                "generation": generation,
                "tunnel_version": crate::runtime::manifest::TUNNEL_CLIENT_VERSION,
            }),
        );
        match self.start_instance(launch.clone(), generation, recovery_token) {
            Ok(mut status) => {
                self.arm_recovery(generation, launch);
                status.recovery = self.recovery_status();
                let current = self.status()?;
                if current.state == TunnelState::Failed {
                    self.schedule_recovery(
                        generation,
                        RecoveryTrigger::ProcessExit,
                        current.last_error.as_ref(),
                    );
                    status = self.status()?;
                }
                self.logger.event(
                    "manual_start_succeeded",
                    serde_json::json!({
                        "generation": generation,
                        "instance_id": status.instance.as_ref().map(|instance| instance.instance_id.as_str()),
                        "pid": status.instance.as_ref().map(|instance| instance.pid),
                        "state": status.state,
                    }),
                );
                Ok(status)
            }
            Err(error) => {
                self.disarm_recovery(generation);
                self.logger.event(
                    "manual_start_failed",
                    serde_json::json!({
                        "generation": generation,
                        "error_code": error.code,
                        "error": error.message,
                    }),
                );
                Err(error)
            }
        }
    }

    fn start_instance(
        &self,
        launch: TunnelLaunch,
        recovery_generation: u64,
        recovery_token: u64,
    ) -> Result<TunnelStatus, TunnelProcessError> {
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
                generation: recovery_generation,
                health: TunnelHealth::starting(),
                readiness: None,
                last_exit: None,
                last_error: None,
                recovery: TunnelRecoveryStatus::default(),
            };
            uuid::Uuid::new_v4().to_string()
        };
        self.notify(Some(&instance_id), TunnelState::Starting, None);

        let mut child = match self.spawn_process(
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
        let child_pid = child.id();
        if let Some(stdout) = child.stdout.take() {
            spawn_child_output_logger(
                stdout,
                self.logger.clone(),
                child_pid,
                instance_id.clone(),
                "stdout",
            );
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_child_output_logger(
                stderr,
                self.logger.clone(),
                child_pid,
                instance_id.clone(),
                "stderr",
            );
        }

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
        {
            let mut recovery = self.lock_recovery();
            if recovery.generation == recovery_generation {
                recovery.active_instance_id = Some(instance.instance_id.clone());
                recovery.active_instance_token = Some(recovery_token);
            }
        }
        spawn_exit_watcher(
            self.clone(),
            process.clone(),
            recovery_generation,
            recovery_token,
        );

        let readiness = if test_mode {
            Ok(TunnelReadinessEvidence {
                control_plane_connected: true,
                mcp_target_reachable: true,
                health_check_passed: true,
                control_plane_route: None,
                control_plane_error: None,
                last_successful_poll_timestamp: None,
                last_successful_poll_age_seconds: None,
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
            let (state, error) = state_from_readiness(&readiness);
            inner.status.state = state;
            inner.status.health = health_from_readiness(&readiness);
            inner.status.readiness = Some(readiness);
            inner.status.last_error = error;
        }
        let status = self.status()?;
        self.notify(
            Some(&process.instance.instance_id),
            status.state,
            status.last_error.as_ref(),
        );
        if !test_mode {
            spawn_readiness_watcher(self.clone(), process, recovery_generation, recovery_token);
        }
        Ok(status)
    }

    fn begin_manual_start(&self) -> (u64, u64) {
        let mut recovery = self.lock_recovery();
        recovery.generation = recovery.generation.wrapping_add(1);
        recovery.desired_running = false;
        recovery.launch = None;
        recovery.recovering = false;
        recovery.blocked = None;
        recovery.reset_backoff();
        recovery.attempts.clear();
        recovery.active_instance_id = None;
        recovery.active_instance_token = None;
        recovery.stop_in_progress = false;
        recovery.token = recovery.token.wrapping_add(1);
        (recovery.generation, recovery.token)
    }

    fn arm_recovery(&self, generation: u64, launch: TunnelLaunch) {
        let mut recovery = self.lock_recovery();
        if recovery.generation == generation {
            recovery.desired_running = true;
            recovery.launch = Some(launch);
            recovery.blocked = None;
            recovery.recovering = false;
            recovery.reset_backoff();
            recovery.attempts.clear();
            drop(recovery);
            self.notify_recovery();
        }
    }

    fn disarm_recovery(&self, generation: u64) {
        let mut recovery = self.lock_recovery();
        if recovery.generation == generation {
            recovery.desired_running = false;
            recovery.launch = None;
            recovery.recovering = false;
            recovery.blocked = None;
            recovery.reset_backoff();
            recovery.attempts.clear();
            recovery.active_instance_id = None;
            recovery.active_instance_token = None;
            recovery.stop_in_progress = false;
            recovery.token = recovery.token.wrapping_add(1);
            recovery.generation = recovery.generation.wrapping_add(1);
        }
    }

    fn disarm_for_manual_stop(&self) -> u64 {
        let mut recovery = self.lock_recovery();
        recovery.desired_running = false;
        recovery.launch = None;
        recovery.recovering = false;
        recovery.blocked = None;
        recovery.reset_backoff();
        recovery.attempts.clear();
        recovery.active_instance_id = None;
        recovery.active_instance_token = None;
        recovery.stop_in_progress = false;
        recovery.token = recovery.token.wrapping_add(1);
        recovery.generation = recovery.generation.wrapping_add(1);
        recovery.generation
    }

    fn note_recovered(&self, generation: u64, token: u64, instance_id: &str, state: TunnelState) {
        let current_error = match self.inner.lock() {
            Ok(inner)
                if inner
                    .process
                    .as_ref()
                    .is_some_and(|process| process.instance.instance_id == instance_id) =>
            {
                inner.status.last_error.clone()
            }
            _ => return,
        };
        {
            let recovery = self.lock_recovery();
            let token_is_valid = recovery.active_instance_token == Some(token)
                && (recovery.token == token || recovery.next_attempt_at.is_some());
            if recovery.generation != generation
                || recovery.active_instance_id.as_deref() != Some(instance_id)
                || !token_is_valid
                || !recovery.desired_running
                || recovery.stop_in_progress
            {
                return;
            }
        }
        let mut recovery = self.lock_recovery();
        let token_is_valid = recovery.active_instance_token == Some(token)
            && (recovery.token == token || recovery.next_attempt_at.is_some());
        if recovery.generation != generation
            || recovery.active_instance_id.as_deref() != Some(instance_id)
            || !token_is_valid
            || !recovery.desired_running
            || recovery.stop_in_progress
        {
            return;
        }
        recovery.recovering = false;
        recovery.stop_in_progress = false;
        recovery.next_attempt_at = None;
        recovery.next_attempt_at_wall = None;
        if state == TunnelState::Ready {
            recovery.blocked = None;
            recovery.reset_backoff();
            recovery.token = recovery.token.wrapping_add(1);
        } else {
            recovery.last_error = current_error;
        }
        drop(recovery);
        self.notify_recovery();
    }

    fn schedule_recovery(
        &self,
        generation: u64,
        trigger: RecoveryTrigger,
        error: Option<&TunnelProcessError>,
    ) {
        #[cfg(test)]
        if !self.auto_recovery_enabled {
            return;
        }

        let now = Instant::now();
        let (schedule, blocked) = {
            let mut recovery = self.lock_recovery();
            if !recovery.desired_running || recovery.generation != generation || recovery.recovering
            {
                return;
            }
            recovery
                .attempts
                .retain(|attempt| now.duration_since(*attempt) <= RECOVERY_WINDOW);
            if let Some(cooldown_until) = recovery.cooldown_until {
                if cooldown_until > now {
                    return;
                }
                recovery.cooldown_until = None;
                recovery.cooldown_until_wall = None;
                recovery.next_attempt_at = None;
                recovery.next_attempt_at_wall = None;
                recovery.half_open = true;
            }
            if let Some(error) = error {
                if recovery_is_non_retryable(error) {
                    recovery.last_error = Some(error.clone());
                    recovery.blocked = Some(error.clone());
                    recovery.recovering = false;
                    recovery.next_attempt_at = None;
                    recovery.next_attempt_at_wall = None;
                    recovery.cooldown_until = None;
                    recovery.cooldown_until_wall = None;
                    recovery.half_open = false;
                    (None, Some(error.clone()))
                } else {
                    let Some(launch) = recovery.launch.clone() else {
                        return;
                    };
                    let (delay, attempt, token) = recovery.prepare_attempt(now, generation);
                    recovery.last_error = Some(error.clone());
                    (Some((delay, attempt, launch, token)), None)
                }
            } else {
                let Some(launch) = recovery.launch.clone() else {
                    return;
                };
                let (delay, attempt, token) = recovery.prepare_attempt(now, generation);
                recovery.last_error = error.map(Clone::clone);
                (Some((delay, attempt, launch, token)), None)
            }
        };

        if let Some(error) = blocked {
            self.logger.event(
                "recovery_blocked",
                serde_json::json!({
                    "generation": generation,
                    "trigger": trigger.as_str(),
                    "error_code": error.code,
                    "error": error.message,
                }),
            );
            self.notify_recovery();
            return;
        }
        let Some((delay, attempt, launch, token)) = schedule else {
            return;
        };
        if delay == RECOVERY_COOLDOWN {
            self.logger.event(
                "recovery_budget_cooldown",
                serde_json::json!({
                    "generation": generation,
                    "trigger": trigger.as_str(),
                    "cooldown_ms": RECOVERY_COOLDOWN.as_millis(),
                }),
            );
        }
        self.notify_recovery();

        self.logger.event(
            "recovery_scheduled",
            serde_json::json!({
                "generation": generation,
                "trigger": trigger.as_str(),
                "attempt": attempt,
                "delay_ms": delay.as_millis(),
                "error_code": error.map(|error| error.code.as_str()),
                "error": error.map(|error| error.message.as_str()),
            }),
        );
        let manager = self.clone();
        thread::spawn(move || {
            thread::sleep(delay);
            manager.perform_recovery(generation, trigger, launch, token);
        });
    }

    fn perform_recovery(
        &self,
        generation: u64,
        trigger: RecoveryTrigger,
        launch: TunnelLaunch,
        token: u64,
    ) {
        let _operation = self.lock_operation();
        if !self.recovery_is_current(generation, token) {
            return;
        }

        {
            let mut recovery = self.lock_recovery();
            if recovery.generation != generation
                || recovery.token != token
                || !recovery.desired_running
                || recovery.blocked.is_some()
            {
                return;
            }
            if let Some(cooldown_until) = recovery.cooldown_until {
                if cooldown_until > Instant::now() {
                    return;
                }
                recovery.cooldown_until = None;
                recovery.cooldown_until_wall = None;
                recovery.half_open = true;
            }
            recovery.stop_in_progress = true;
            recovery.next_attempt_at = None;
            recovery.next_attempt_at_wall = None;
        }
        self.notify_recovery();

        if let Some(process) = self
            .inner
            .lock()
            .ok()
            .and_then(|inner| inner.process.as_ref().cloned())
        {
            self.logger.event(
                "recovery_stopping_previous_instance",
                serde_json::json!({
                    "generation": generation,
                    "trigger": trigger.as_str(),
                    "instance_id": process.instance.instance_id,
                    "pid": process.instance.pid,
                }),
            );
            let recovery = self.recovery_status();
            let notification = if let Ok(mut inner) = self.lock_inner() {
                inner.status.state = TunnelState::Stopping;
                inner.status.health = TunnelHealth::stopping();
                capture_notification(
                    &inner,
                    Some(&process.instance.instance_id),
                    TunnelState::Stopping,
                    &recovery,
                    None,
                )
            } else {
                None
            };
            if let Some(notification) = notification {
                notification.emit();
            }
            if let Err(error) = self.stop_process(&process) {
                if !self.finish_recovery_attempt(generation, token, Some(error.clone())) {
                    return;
                }
                self.logger.event(
                    "recovery_stop_failed",
                    serde_json::json!({
                        "generation": generation,
                        "trigger": trigger.as_str(),
                        "error_code": error.code,
                        "error": error.message,
                    }),
                );
                self.schedule_recovery(generation, trigger, Some(&error));
                return;
            }
            remove_health_file(&process.health_url_file);
        }

        if !self.recovery_is_current(generation, token) {
            return;
        }
        {
            let mut recovery = self.lock_recovery();
            if recovery.generation != generation
                || recovery.token != token
                || !recovery.desired_running
                || recovery.blocked.is_some()
            {
                return;
            }
            recovery.stop_in_progress = false;
        }

        self.logger.event(
            "recovery_started",
            serde_json::json!({
                "generation": generation,
                "trigger": trigger.as_str(),
            }),
        );
        match self.start_instance(launch, generation, token) {
            Ok(status) => {
                let instance_id = status
                    .instance
                    .as_ref()
                    .map(|instance| instance.instance_id.as_str());
                if let Some(instance_id) = instance_id {
                    self.note_recovered(generation, token, instance_id, status.state);
                }
                self.logger.event(
                    "recovery_succeeded",
                    serde_json::json!({
                        "generation": generation,
                        "trigger": trigger.as_str(),
                        "instance_id": status.instance.as_ref().map(|instance| instance.instance_id.as_str()),
                        "pid": status.instance.as_ref().map(|instance| instance.pid),
                        "state": status.state,
                    }),
                );
                if status.state != TunnelState::Ready {
                    self.schedule_recovery(
                        generation,
                        RecoveryTrigger::HealthProbe,
                        status.last_error.as_ref(),
                    );
                }
            }
            Err(error) => {
                if !self.finish_recovery_attempt(generation, token, Some(error.clone())) {
                    return;
                }
                self.logger.event(
                    "recovery_failed",
                    serde_json::json!({
                        "generation": generation,
                        "trigger": trigger.as_str(),
                        "error_code": error.code,
                        "error": error.message,
                    }),
                );
                self.schedule_recovery(generation, trigger, Some(&error));
            }
        }
    }

    fn recovery_is_current(&self, generation: u64, token: u64) -> bool {
        let recovery = self.lock_recovery();
        recovery.token == token
            && recovery.desired_running
            && recovery.generation == generation
            && recovery.blocked.is_none()
    }

    pub fn stop(&self) -> Result<TunnelStopResult, TunnelProcessError> {
        let _operation = self.try_manual_operation()?;
        self.stop_internal()
    }

    fn stop_internal(&self) -> Result<TunnelStopResult, TunnelProcessError> {
        let generation = self.disarm_for_manual_stop();
        self.logger.event(
            "manual_stop_requested",
            serde_json::json!({ "generation": generation }),
        );
        let instance_id = {
            let inner = self.lock_inner()?;
            inner
                .process
                .as_ref()
                .map(|process| process.instance.instance_id.clone())
        };
        match instance_id {
            Some(instance_id) => self.stop_instance_internal(&instance_id),
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
        let _operation = self.try_manual_operation()?;
        self.stop_instance_internal(expected_instance_id)
    }

    fn stop_instance_internal(
        &self,
        expected_instance_id: &str,
    ) -> Result<TunnelStopResult, TunnelProcessError> {
        let recovery = self.recovery_status();
        let (process, notification) = {
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
            inner.status.health = TunnelHealth::stopping();
            inner.status.last_error = None;
            let notification = capture_notification(
                &inner,
                Some(expected_instance_id),
                TunnelState::Stopping,
                &recovery,
                None,
            );
            (process, notification)
        };
        if let Some(notification) = notification {
            notification.emit();
        }
        let generation = self.disarm_for_manual_stop();
        self.logger.event(
            "manual_stop_instance_requested",
            serde_json::json!({
                "generation": generation,
                "instance_id": expected_instance_id,
            }),
        );

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
        let _operation = self.try_manual_operation()?;
        let active_instance_id = {
            let inner = self.lock_inner()?;
            inner
                .process
                .as_ref()
                .map(|process| process.instance.instance_id.clone())
        };
        if let Some(instance_id) = active_instance_id {
            let stopped = self.stop_instance_internal(&instance_id)?;
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
        self.start_internal(launch)
    }

    pub fn status(&self) -> Result<TunnelStatus, TunnelProcessError> {
        let mut status = self.lock_inner()?.status.clone();
        status.recovery = self.recovery_status();
        Ok(status)
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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
        let recovery = self.recovery_status();
        let notification = if let Ok(mut inner) = self.lock_inner() {
            if instance_id.is_none()
                || inner.process.as_ref().is_some_and(|process| {
                    Some(process.instance.instance_id.as_str()) == instance_id
                })
            {
                inner.status.state = TunnelState::Failed;
                inner.status.health = TunnelHealth::failed();
                inner.status.last_error = Some(error.clone());
                inner.status.readiness = None;
                capture_notification(
                    &inner,
                    instance_id,
                    TunnelState::Failed,
                    &recovery,
                    Some(&error),
                )
            } else {
                None
            }
        } else {
            None
        };
        if let Some(notification) = notification {
            notification.emit();
        }
    }

    fn notify(
        &self,
        instance_id: Option<&str>,
        state: TunnelState,
        error: Option<&TunnelProcessError>,
    ) {
        let recovery = self.recovery_status();
        let notification = self
            .lock_inner()
            .ok()
            .and_then(|inner| capture_notification(&inner, instance_id, state, &recovery, error));
        if let Some(notification) = notification {
            notification.emit();
        }
    }

    fn notify_recovery(&self) {
        let recovery = self.recovery_status();
        let notification = self.lock_inner().ok().and_then(|inner| {
            let instance_id = inner
                .process
                .as_ref()
                .map(|process| process.instance.instance_id.as_str())
                .or_else(|| {
                    inner
                        .status
                        .instance
                        .as_ref()
                        .map(|instance| instance.instance_id.as_str())
                });
            capture_notification(
                &inner,
                instance_id,
                inner.status.state,
                &recovery,
                inner.status.last_error.as_ref(),
            )
        });
        if let Some(notification) = notification {
            notification.emit();
        }
    }

    fn lock_inner(&self) -> Result<MutexGuard<'_, TunnelInner>, TunnelProcessError> {
        self.inner.lock().map_err(|_| {
            TunnelProcessError::state_unavailable("Tunnel lifecycle state is unavailable.")
        })
    }

    fn lock_recovery(&self) -> MutexGuard<'_, RecoveryState> {
        self.recovery
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn recovery_status(&self) -> TunnelRecoveryStatus {
        self.lock_recovery().status(Instant::now())
    }

    fn finish_recovery_attempt(
        &self,
        generation: u64,
        token: u64,
        error: Option<TunnelProcessError>,
    ) -> bool {
        let mut recovery = self.lock_recovery();
        if recovery.generation != generation
            || recovery.token != token
            || !recovery.desired_running
            || recovery.blocked.is_some()
        {
            return false;
        }
        recovery.recovering = false;
        recovery.stop_in_progress = false;
        recovery.next_attempt_at = None;
        recovery.next_attempt_at_wall = None;
        if let Some(error) = error {
            recovery.last_error = Some(error);
        }
        drop(recovery);
        self.notify_recovery();
        true
    }

    fn finish_process_exit(
        &self,
        generation: u64,
        token: u64,
        instance_id: &str,
        error: Option<TunnelProcessError>,
    ) -> bool {
        let mut recovery = self.lock_recovery();
        if recovery.generation != generation
            || recovery.active_instance_id.as_deref() != Some(instance_id)
            || recovery.active_instance_token != Some(token)
            || !recovery.desired_running
        {
            return false;
        }
        recovery.recovering = false;
        recovery.next_attempt_at = None;
        recovery.next_attempt_at_wall = None;
        if let Some(error) = error {
            recovery.last_error = Some(error);
        }
        recovery.token = recovery.token.wrapping_add(1);
        drop(recovery);
        self.notify_recovery();
        true
    }

    fn try_manual_operation(&self) -> Result<MutexGuard<'_, ()>, TunnelProcessError> {
        self.operation.try_lock().map_err(|_| {
            TunnelProcessError::conflict(
                "Another Tunnel operation is in progress. Retry Stop or Restart after it finishes.",
            )
        })
    }

    fn lock_operation(&self) -> MutexGuard<'_, ()> {
        self.operation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[cfg(test)]
    fn with_test_command(command: TestTunnelCommand) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TunnelInner {
                process: None,
                status: TunnelStatus {
                    state: TunnelState::Stopped,
                    instance: None,
                    generation: 0,
                    health: TunnelHealth::stopped(),
                    readiness: None,
                    last_exit: None,
                    last_error: None,
                    recovery: TunnelRecoveryStatus::default(),
                },
                observer: None,
            })),
            recovery: Arc::new(Mutex::new(RecoveryState::new())),
            operation: Arc::new(Mutex::new(())),
            logger: TunnelLogger::new(),
            test_command: Some(command),
            auto_recovery_enabled: false,
            // Allow the exit watcher one polling interval plus process-tree
            // teardown time when recovery tests run in parallel on Windows.
            stop_timeout: Duration::from_millis(250),
        }
    }

    #[cfg(test)]
    fn with_auto_recovery(mut self) -> Self {
        self.auto_recovery_enabled = true;
        self
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
        .args(["--control-plane.api-key", &api_key_reference]);
    if let Some(proxy) = launch.control_plane_http_proxy.as_deref() {
        command.args(["--control-plane.http-proxy", proxy]);
    }
    command
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
                if evidence.mcp_target_reachable {
                    return Ok(evidence);
                }
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

fn spawn_readiness_watcher(
    manager: TunnelProcessManager,
    process: ManagedTunnelProcess,
    recovery_generation: u64,
    recovery_token: u64,
) {
    thread::spawn(move || {
        let mut failures = 0;
        let mut health_recovery_requested = false;
        let mut control_plane_recovery_requested = false;
        let mut local_mcp_failures = 0;
        let mut local_mcp_recovery_requested = false;
        let mut no_success_since: Option<Instant> = None;
        loop {
            thread::sleep(READINESS_MONITOR_INTERVAL);
            if wait_for_exit(&process.exited, Duration::from_millis(0)) {
                return;
            }
            let is_current = manager.inner.lock().ok().is_some_and(|inner| {
                !matches!(
                    inner.status.state,
                    TunnelState::Stopping | TunnelState::Stopped
                ) && inner.process.as_ref().is_some_and(|active| {
                    active.instance.instance_id == process.instance.instance_id
                })
            });
            if !is_current {
                return;
            }
            let health_url = fs::read_to_string(&process.health_url_file)
                .ok()
                .map(|value| value.trim().to_owned());
            let result = health_url
                .as_deref()
                .ok_or_else(|| {
                    TunnelProcessError::readiness("Tunnel health URL discovery could not be read.")
                })
                .and_then(probe_ready);
            if let Ok(evidence) = result {
                failures = 0;
                health_recovery_requested = false;
                let (next_state, next_error) = state_from_readiness(&evidence);
                let next_health = health_from_readiness(&evidence);
                if evidence.mcp_target_reachable {
                    local_mcp_failures = 0;
                    local_mcp_recovery_requested = false;
                } else {
                    local_mcp_failures += 1;
                }
                let recovery = manager.recovery_status();
                let mut current_after_probe = false;
                let mut changed_after_probe = false;
                let notification = if let Ok(mut inner) = manager.inner.lock() {
                    if !matches!(
                        inner.status.state,
                        TunnelState::Stopping | TunnelState::Stopped
                    ) && inner.process.as_ref().is_some_and(|active| {
                        active.instance.instance_id == process.instance.instance_id
                    }) {
                        current_after_probe = true;
                        let changed = inner.status.state != next_state
                            || inner.status.last_error != next_error
                            || inner.status.health != next_health
                            || inner.status.readiness.as_ref() != Some(&evidence);
                        inner.status.state = next_state;
                        inner.status.health = next_health;
                        inner.status.readiness = Some(evidence.clone());
                        inner.status.last_error = next_error.clone();
                        if changed {
                            changed_after_probe = true;
                            capture_notification(
                                &inner,
                                Some(&process.instance.instance_id),
                                next_state,
                                &recovery,
                                next_error.as_ref(),
                            )
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                if !current_after_probe {
                    return;
                }
                if let Some(notification) = notification {
                    notification.emit();
                }
                if changed_after_probe {
                    manager.logger.event(
                        "readiness_transition",
                        serde_json::json!({
                            "generation": recovery_generation,
                            "instance_id": process.instance.instance_id,
                            "state": next_state,
                            "health": next_health,
                            "error_code": next_error.as_ref().map(|error| error.code.as_str()),
                            "error": next_error.as_ref().map(|error| error.message.as_str()),
                        }),
                    );
                }
                if next_state == TunnelState::Ready {
                    manager.note_recovered(
                        recovery_generation,
                        recovery_token,
                        &process.instance.instance_id,
                        next_state,
                    );
                }

                let has_valid_poll_age = evidence.last_successful_poll_timestamp.is_some()
                    && evidence.last_successful_poll_age_seconds.is_some();
                let control_plane_stale = if has_valid_poll_age {
                    no_success_since = None;
                    evidence
                        .last_successful_poll_age_seconds
                        .is_some_and(|age| age >= CONTROL_PLANE_RESTART_AGE.as_secs_f64())
                } else {
                    let since = no_success_since.get_or_insert_with(Instant::now);
                    since.elapsed() >= CONTROL_PLANE_RESTART_AGE
                };
                if control_plane_stale && !control_plane_recovery_requested {
                    control_plane_recovery_requested = true;
                    manager.schedule_recovery(
                        recovery_generation,
                        RecoveryTrigger::ControlPlaneStale,
                        next_error.as_ref(),
                    );
                } else if !control_plane_stale {
                    control_plane_recovery_requested = false;
                }
                if next_state == TunnelState::Failed
                    && next_health.control_plane == TunnelHealthState::Failed
                    && !health_recovery_requested
                {
                    health_recovery_requested = true;
                    manager.schedule_recovery(
                        recovery_generation,
                        RecoveryTrigger::HealthProbe,
                        next_error.as_ref(),
                    );
                }
                if local_mcp_failures >= READINESS_LOSS_TOLERANCE
                    && next_error
                        .as_ref()
                        .is_some_and(|error| error.code == "TUNNEL_MCP_UNAVAILABLE")
                    && !local_mcp_recovery_requested
                {
                    local_mcp_recovery_requested = true;
                    manager.schedule_recovery(
                        recovery_generation,
                        RecoveryTrigger::HealthProbe,
                        next_error.as_ref(),
                    );
                }
            } else {
                failures += 1;
                let error = TunnelProcessError::readiness(
                    "Tunnel readiness probe is temporarily unhealthy.",
                );
                let next_state = if failures >= READINESS_LOSS_TOLERANCE {
                    TunnelState::Failed
                } else {
                    TunnelState::Degraded
                };
                let previous_health = manager.inner.lock().ok().map(|inner| inner.status.health);
                let next_health = TunnelHealth::probe_failed(previous_health.as_ref());
                let recovery = manager.recovery_status();
                let mut changed_after_probe = false;
                let notification = if let Ok(mut inner) = manager.inner.lock() {
                    if matches!(
                        inner.status.state,
                        TunnelState::Stopping | TunnelState::Stopped
                    ) || inner.process.as_ref().is_none_or(|active| {
                        active.instance.instance_id != process.instance.instance_id
                    }) {
                        return;
                    }
                    let changed = inner.status.state != next_state
                        || inner.status.health != next_health
                        || inner.status.last_error.as_ref() != Some(&error);
                    inner.status.state = next_state;
                    inner.status.health = next_health;
                    inner.status.last_error = Some(error.clone());
                    if changed {
                        changed_after_probe = true;
                        capture_notification(
                            &inner,
                            Some(&process.instance.instance_id),
                            next_state,
                            &recovery,
                            Some(&error),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(notification) = notification {
                    notification.emit();
                }
                if changed_after_probe {
                    manager.logger.event(
                        "health_probe_failed",
                        serde_json::json!({
                            "generation": recovery_generation,
                            "instance_id": process.instance.instance_id,
                            "consecutive_failures": failures,
                            "state": next_state,
                            "health": next_health,
                            "error_code": error.code,
                            "error": error.message,
                        }),
                    );
                }
                if failures >= READINESS_LOSS_TOLERANCE && !health_recovery_requested {
                    health_recovery_requested = true;
                    manager.schedule_recovery(
                        recovery_generation,
                        RecoveryTrigger::HealthProbe,
                        Some(&error),
                    );
                }
            }
        }
    });
}

fn probe_ready(url: &str) -> Result<TunnelReadinessEvidence, TunnelProcessError> {
    let (ready_status, ready_body) = health_get(url, "/readyz")?;
    if ready_status != 200 {
        return Err(TunnelProcessError::readiness(
            "Tunnel /readyz did not report ready.",
        ));
    }
    let lowercase = ready_body.to_ascii_lowercase().replace(' ', "");
    for marker in [
        "\"ready\":false",
        "\"mcp_target_reachable\":false",
        "\"healthy\":false",
    ] {
        if lowercase.contains(marker) {
            return Err(TunnelProcessError::readiness(
                "Tunnel /readyz reported an unhealthy local dependency.",
            ));
        }
    }

    let admin_status = health_get(url, "/api/status").and_then(|(status, body)| {
        if status != 200 {
            return Err(TunnelProcessError::readiness(
                "Tunnel admin status endpoint did not return HTTP 200.",
            ));
        }
        serde_json::from_str::<AdminStatus>(&body).map_err(|_| {
            TunnelProcessError::readiness("Tunnel admin status response was malformed.")
        })
    });
    let (route, metadata_error, mcp_target_reachable) = match admin_status {
        Ok(status) => {
            let route = status
                .control_plane_route
                .map(|route| TunnelControlPlaneRoute {
                    route_mode: route.route_mode,
                    proxy_source: route.proxy_source,
                    proxy_url: route.proxy_url,
                });
            let mcp_ready = status
                .channels
                .iter()
                .find(|channel| channel.name == "main")
                .and_then(|channel| channel.probe_status.as_deref())
                == Some("ok");
            (route, status.tunnel_metadata_error, mcp_ready)
        }
        Err(error) => (None, Some(error.message), false),
    };

    let metrics = health_get(url, "/metrics")
        .ok()
        .filter(|(status, _)| *status == 200)
        .map(|(_, body)| body);
    let last_successful_poll_timestamp =
        metrics.as_deref().and_then(last_successful_poll_timestamp);
    let now = unix_timestamp_seconds();
    let last_successful_poll_age_seconds =
        last_successful_poll_timestamp.and_then(|timestamp| poll_age_seconds(timestamp, now));
    let poll_is_fresh = control_plane_poll_is_fresh(last_successful_poll_timestamp, now);
    let control_plane_connected = poll_is_fresh && metadata_error.is_none();
    let control_plane_error = if let Some(error) = metadata_error {
        Some(error)
    } else if poll_is_fresh {
        None
    } else if let Some(timestamp) = last_successful_poll_timestamp {
        if timestamp > now {
            Some("Control-plane poll timestamp is in the future.".into())
        } else if let Some(age_seconds) = last_successful_poll_age_seconds {
            Some(format!(
                "Control-plane poll has been stale for {} seconds.",
                age_seconds.floor() as u64
            ))
        } else {
            Some("Control-plane poll timestamp is invalid.".into())
        }
    } else {
        Some("Control-plane poll has not completed successfully yet.".into())
    };

    Ok(TunnelReadinessEvidence {
        control_plane_connected,
        mcp_target_reachable,
        health_check_passed: true,
        control_plane_route: route,
        control_plane_error,
        last_successful_poll_timestamp,
        last_successful_poll_age_seconds,
    })
}

fn state_from_readiness(
    evidence: &TunnelReadinessEvidence,
) -> (TunnelState, Option<TunnelProcessError>) {
    let health = health_from_readiness(evidence);
    if health.all_ready() {
        return (TunnelState::Ready, None);
    }
    let control_message = evidence
        .control_plane_error
        .clone()
        .unwrap_or_else(|| "Control-plane connection is unavailable.".into());
    let control_error = TunnelProcessError::control_plane(control_message);
    if control_error.code == "CONTROL_PLANE_UNAUTHORIZED" {
        return (TunnelState::Failed, Some(control_error));
    }
    if health.local_mcp != TunnelHealthState::Ready {
        return (
            TunnelState::Degraded,
            Some(TunnelProcessError::mcp_unavailable(
                "Tunnel MCP target is unavailable.",
            )),
        );
    }
    (TunnelState::Degraded, Some(control_error))
}

fn health_from_readiness(evidence: &TunnelReadinessEvidence) -> TunnelHealth {
    let local_mcp = if evidence.health_check_passed && evidence.mcp_target_reachable {
        TunnelHealthState::Ready
    } else {
        TunnelHealthState::Failed
    };
    let control_plane = if evidence.control_plane_connected {
        TunnelHealthState::Ready
    } else if evidence
        .control_plane_error
        .as_deref()
        .is_some_and(|message| {
            message.contains("401") || message.to_ascii_lowercase().contains("unauthorized")
        })
    {
        TunnelHealthState::Failed
    } else {
        TunnelHealthState::Degraded
    };
    TunnelHealth {
        process: TunnelHealthState::Ready,
        local_mcp,
        control_plane,
    }
}

fn unix_timestamp_seconds() -> f64 {
    Utc::now().timestamp_millis() as f64 / 1_000.0
}

fn last_successful_poll_timestamp(metrics: &str) -> Option<f64> {
    metrics.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let metric_name = fields.next()?;
        if metric_name != CONTROL_PLANE_POLL_METRIC
            && !metric_name
                .strip_prefix(CONTROL_PLANE_POLL_METRIC)
                .is_some_and(|suffix| suffix.starts_with('{'))
        {
            return None;
        }
        let value = fields.next()?.parse::<f64>().ok()?;
        (value.is_finite() && value > 0.0).then_some(value)
    })
}

fn poll_age_seconds(timestamp: f64, now: f64) -> Option<f64> {
    if !timestamp.is_finite() || timestamp <= 0.0 || !now.is_finite() || timestamp > now {
        return None;
    }
    let age_seconds = now - timestamp;
    age_seconds.is_finite().then_some(age_seconds)
}

fn control_plane_poll_is_fresh(timestamp: Option<f64>, now: f64) -> bool {
    let Some(timestamp) = timestamp else {
        return false;
    };
    poll_age_seconds(timestamp, now)
        .is_some_and(|age| age <= CONTROL_PLANE_POLL_FRESHNESS.as_secs_f64())
}

fn health_get(url: &str, path: &str) -> Result<(u16, String), TunnelProcessError> {
    let address = validate_health_url(url)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(250))
        .map_err(|_| TunnelProcessError::readiness("Tunnel health endpoint is unreachable."))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|_| TunnelProcessError::readiness("Tunnel readiness request failed."))?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|_| {
        TunnelProcessError::readiness("Tunnel readiness response could not be read.")
    })?;
    let separator = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| TunnelProcessError::readiness("Tunnel HTTP response was malformed."))?;
    let headers = String::from_utf8_lossy(&response[..separator]);
    let raw_body = &response[separator + 4..];
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| TunnelProcessError::readiness("Tunnel HTTP response was malformed."))?;
    let body = if headers
        .lines()
        .any(|line| line.eq_ignore_ascii_case("Transfer-Encoding: chunked"))
    {
        decode_chunked(raw_body).ok_or_else(|| {
            TunnelProcessError::readiness("Tunnel chunked HTTP response was malformed.")
        })?
    } else {
        raw_body.to_vec()
    };
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

fn decode_chunked(mut input: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    loop {
        let line_end = input.windows(2).position(|window| window == b"\r\n")?;
        let size_text = std::str::from_utf8(&input[..line_end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        input = &input[line_end + 2..];
        if size == 0 {
            return Some(output);
        }
        if input.len() < size + 2 || &input[size..size + 2] != b"\r\n" {
            return None;
        }
        output.extend_from_slice(&input[..size]);
        input = &input[size + 2..];
    }
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

fn spawn_exit_watcher(
    manager: TunnelProcessManager,
    process: ManagedTunnelProcess,
    recovery_generation: u64,
    recovery_token: u64,
) {
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

        let mut recovery_error = None;
        let mut should_recover = false;
        let mut notification = None;
        let recovery = manager.recovery_status();
        if let Ok(mut inner) = manager.inner.lock() {
            let is_current = inner
                .process
                .as_ref()
                .is_some_and(|active| active.instance.instance_id == process.instance.instance_id);
            if is_current {
                let stopped_by_manager = inner.status.state == TunnelState::Stopping;
                let generation = inner.status.generation;
                inner.process = None;
                let error = if stopped_by_manager {
                    None
                } else {
                    Some(TunnelProcessError::crashed(exit_code))
                };
                inner.status = TunnelStatus {
                    state: if stopped_by_manager {
                        TunnelState::Stopped
                    } else {
                        TunnelState::Failed
                    },
                    instance: Some(process.instance.clone()),
                    generation,
                    health: if stopped_by_manager {
                        TunnelHealth::stopped()
                    } else {
                        TunnelHealth::failed()
                    },
                    readiness: None,
                    last_exit: Some(exit),
                    last_error: error.clone(),
                    recovery: TunnelRecoveryStatus::default(),
                };
                notification = capture_notification(
                    &inner,
                    Some(&process.instance.instance_id),
                    inner.status.state,
                    &recovery,
                    error.as_ref(),
                );
                should_recover = !stopped_by_manager;
                recovery_error = error;
            }
        }
        // Publish process termination before invoking the Host observer. The
        // stop path must not wait for a callback that re-enters Tunnel state.
        let (lock, wake) = &*process.exited;
        if let Ok(mut exited) = lock.lock() {
            *exited = true;
            wake.notify_all();
        }
        if let Some(notification) = notification {
            notification.emit();
        }
        manager.logger.event(
            "process_exited",
            serde_json::json!({
                "generation": recovery_generation,
                "instance_id": process.instance.instance_id,
                "pid": process.instance.pid,
                "exit_code": exit_code,
                "recover": should_recover,
                "error_code": recovery_error.as_ref().map(|error| error.code.as_str()),
                "error": recovery_error.as_ref().map(|error| error.message.as_str()),
            }),
        );
        if should_recover {
            if manager.finish_process_exit(
                recovery_generation,
                recovery_token,
                &process.instance.instance_id,
                recovery_error.clone(),
            ) {
                manager.schedule_recovery(
                    recovery_generation,
                    RecoveryTrigger::ProcessExit,
                    recovery_error.as_ref(),
                );
            }
        }
        remove_health_file(&process.health_url_file);
    });
}

fn capture_notification(
    inner: &TunnelInner,
    instance_id: Option<&str>,
    state: TunnelState,
    recovery: &TunnelRecoveryStatus,
    error: Option<&TunnelProcessError>,
) -> Option<TunnelNotification> {
    Some(TunnelNotification {
        observer: inner.observer.clone()?,
        generation: inner.status.generation,
        instance_id: instance_id.map(str::to_owned),
        state,
        health: inner.status.health,
        readiness: inner.status.readiness.clone(),
        recovery: recovery.clone(),
        error: error.cloned(),
    })
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

    fn control_plane(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = if message.contains("401") || message.contains("Unauthorized") {
            "CONTROL_PLANE_UNAUTHORIZED"
        } else {
            "CONTROL_PLANE_UNAVAILABLE"
        };
        Self {
            code: code.into(),
            message,
        }
    }

    fn mcp_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "TUNNEL_MCP_UNAVAILABLE".into(),
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

fn recovery_is_non_retryable(error: &TunnelProcessError) -> bool {
    matches!(
        error.code.as_str(),
        "CONTROL_PLANE_UNAUTHORIZED"
            | "TUNNEL_DISTRIBUTION_INVALID"
            | "TUNNEL_EXECUTABLE_NOT_FOUND"
            | "TUNNEL_LAUNCH_INVALID"
    )
}

fn recovery_delay(attempt: usize, jitter_seed: u64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(6) as u32;
    let base_ms = (RECOVERY_BASE_DELAY.as_millis() as u64)
        .saturating_mul(1_u64 << exponent)
        .min(RECOVERY_MAX_DELAY.as_millis() as u64);
    let jitter_ms = jitter_seed % ((base_ms / 4).max(1) + 1);
    Duration::from_millis((base_ms + jitter_ms).min(RECOVERY_MAX_DELAY.as_millis() as u64))
}

fn recovery_jitter_seed(generation: u64, attempt: usize) -> u64 {
    let nanos = Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .unsigned_abs();
    nanos ^ generation.rotate_left(13) ^ (attempt as u64).rotate_left(29)
}

fn append_rolled_log(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::metadata(path).is_ok_and(|metadata| metadata.len() >= LOG_ROLL_BYTES)
        && !rotate_log_file(path)
    {
        return Err(std::io::Error::other(
            "Tunnel supervisor log rotation failed.",
        ));
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")
}

fn spawn_child_output_logger<R: Read + Send + 'static>(
    reader: R,
    logger: TunnelLogger,
    pid: u32,
    instance_id: String,
    stream: &'static str,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            let Ok(count) = reader.read_until(b'\n', &mut buffer) else {
                return;
            };
            if count == 0 {
                return;
            }
            let line = String::from_utf8_lossy(&buffer);
            let line = line.trim_end_matches(['\r', '\n']);
            let line: String = line.chars().take(4096).collect();
            logger.event(
                "child_output",
                serde_json::json!({
                    "instance_id": instance_id,
                    "pid": pid,
                    "stream": stream,
                    "line": line,
                }),
            );
        }
    });
}

fn redact_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(redact_sensitive_text(&text)),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(redact_json).collect())
        }
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    let value = if sensitive_log_key(&key) {
                        serde_json::Value::String("<REDACTED>".into())
                    } else {
                        redact_json(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        value => value,
    }
}

fn redact_sensitive_text(text: &str) -> String {
    let lowercase = text.to_ascii_lowercase();
    let sensitive = [
        "sk-",
        "bearer ",
        "authorization:",
        "api_key=",
        "api-key=",
        "client_secret",
        "token=",
    ];
    let url_with_userinfo = lowercase.contains("://") && lowercase.contains('@');
    if sensitive.iter().any(|marker| lowercase.contains(marker)) || url_with_userinfo {
        "<REDACTED>".into()
    } else {
        text.to_owned()
    }
}

fn sensitive_log_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "authorization",
        "token",
        "api_key",
        "apikey",
        "secret",
        "password",
        "cookie",
        "credential",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

fn rotate_log_file(path: &Path) -> bool {
    let numbered_path = |index: usize| {
        let mut value = path.as_os_str().to_os_string();
        value.push(format!(".{index}"));
        PathBuf::from(value)
    };
    let oldest = numbered_path(LOG_ROLL_FILES);
    if oldest.exists() && fs::remove_file(&oldest).is_err() {
        return false;
    }
    for index in (1..LOG_ROLL_FILES).rev() {
        let source = numbered_path(index);
        if source.is_file() {
            if fs::rename(source, numbered_path(index + 1)).is_err() {
                return false;
            }
        }
    }
    if path.is_file() && fs::rename(path, numbered_path(1)).is_err() {
        return false;
    }
    true
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

fn timestamp_after(delay: Duration) -> String {
    let millis = delay.as_millis().min(i64::MAX as u128) as i64;
    (Utc::now() + chrono::Duration::milliseconds(millis))
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
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

    fn admin_server(metadata_error: Option<&str>, last_poll: Option<f64>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let metadata_error = metadata_error.map(str::to_owned);
        thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let count = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");
                let body = match path {
                    "/readyz" => "ready".to_owned(),
                    "/api/status" => serde_json::json!({
                        "control_plane_route": {
                            "route_mode": "proxy",
                            "proxy_source": "control-plane.http-proxy",
                            "proxy_url": "http://127.0.0.1:7897"
                        },
                        "tunnel_metadata_error": metadata_error,
                        "channels": [{"name": "main", "probe_status": "ok"}]
                    })
                    .to_string(),
                    "/metrics" => last_poll
                        .map(|value| {
                            format!("commands_poll_last_successful_timestamp_seconds{{}} {value}\n")
                        })
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        format!("http://{address}")
    }

    fn toggle_admin_server() -> (String, Arc<AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let healthy = Arc::new(AtomicBool::new(true));
        let healthy_for_thread = healthy.clone();
        thread::spawn(move || loop {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            if !healthy_for_thread.load(Ordering::SeqCst) {
                continue;
            }
            let mut request = [0_u8; 2048];
            let Ok(count) = stream.read(&mut request) else {
                continue;
            };
            let request = String::from_utf8_lossy(&request[..count]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            let body = match path {
                "/readyz" => "ready".to_owned(),
                "/api/status" => serde_json::json!({
                    "control_plane_route": {
                        "route_mode": "proxy",
                        "proxy_source": "control-plane.http-proxy",
                        "proxy_url": "http://127.0.0.1:7897"
                    },
                    "tunnel_metadata_error": null,
                    "channels": [{"name": "main", "probe_status": "ok"}]
                })
                .to_string(),
                "/metrics" => format!(
                    "commands_poll_last_successful_timestamp_seconds{{}} {}\n",
                    unix_timestamp_seconds()
                ),
                _ => String::new(),
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        });
        (format!("http://{address}"), healthy)
    }

    struct ReentrantTunnelObserver {
        manager: TunnelProcessManager,
        returned: std::sync::mpsc::Sender<()>,
    }

    #[test]
    fn manual_stop_returns_conflict_during_recovery_and_allows_retry() {
        let manager = TunnelProcessManager::new();
        let recovery = manager.lock_operation();
        assert_eq!(manager.stop().unwrap_err().code, "LIFECYCLE_CONFLICT");
        drop(recovery);
        assert!(!manager.stop().expect("retry stop").accepted);
    }

    impl TunnelEventSink for ReentrantTunnelObserver {
        fn tunnel_event(
            &self,
            _generation: u64,
            _instance_id: Option<&str>,
            _state: TunnelState,
            _health: TunnelHealth,
            _readiness: Option<&TunnelReadinessEvidence>,
            _recovery: &TunnelRecoveryStatus,
            _error: Option<&TunnelProcessError>,
        ) {
            let _ = self.manager.status();
            let _ = self.returned.send(());
        }
    }

    #[test]
    fn tunnel_event_observer_can_reenter_manager_without_deadlock() {
        let manager = TunnelProcessManager::new();
        let (returned_sender, returned_receiver) = std::sync::mpsc::channel();
        manager.set_event_sink(Arc::new(ReentrantTunnelObserver {
            manager: manager.clone(),
            returned: returned_sender,
        }));

        let dispatch = manager.clone();
        let dispatch_thread = thread::spawn(move || {
            dispatch.notify(None, TunnelState::Stopping, None);
        });
        assert!(
            returned_receiver
                .recv_timeout(Duration::from_millis(250))
                .is_ok(),
            "Tunnel event observer callback deadlocked on tunnel.inner"
        );
        dispatch_thread
            .join()
            .expect("tunnel event dispatch thread");
    }

    #[test]
    fn tunnel_stop_with_reentrant_observer_completes() {
        let manager = sleeping_manager();
        let (returned_sender, returned_receiver) = std::sync::mpsc::channel();
        manager.set_event_sink(Arc::new(ReentrantTunnelObserver {
            manager: manager.clone(),
            returned: returned_sender,
        }));
        manager.start(launch()).expect("Tunnel start");
        assert!(manager.stop().expect("Tunnel stop").accepted);
        wait_for_state(&manager, TunnelState::Stopped);
        assert!(
            returned_receiver
                .recv_timeout(Duration::from_secs(1))
                .is_ok(),
            "Tunnel lifecycle observer did not run during stop"
        );
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
                control_plane_route: None,
                control_plane_error: None,
                last_successful_poll_timestamp: None,
                last_successful_poll_age_seconds: None,
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
        assert!(manager.lock_recovery().desired_running);
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
    fn real_command_scopes_configured_proxy_to_control_plane() {
        let mut command = Command::new(cmd_path());
        let health = std::env::temp_dir().join("harness-test-health-proxy.url");
        let launch = launch()
            .with_api_key("raw-api-key")
            .with_control_plane_http_proxy(Some("http://127.0.0.1:7897".into()));
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
        let proxy_index = args
            .iter()
            .position(|value| value == "--control-plane.http-proxy")
            .unwrap();
        assert_eq!(args[proxy_index + 1], "http://127.0.0.1:7897");
        assert!(!args.iter().any(|value| value == "--http-proxy"));
    }

    #[test]
    fn real_command_without_configured_proxy_preserves_environment_fallback() {
        let mut command = Command::new(cmd_path());
        let health = std::env::temp_dir().join("harness-test-health-no-proxy.url");
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
        assert!(!args
            .iter()
            .any(|value| value == "--control-plane.http-proxy"));
        assert!(!command
            .get_envs()
            .any(|(key, value)| key == "CONTROL_PLANE_HTTP_PROXY" && value.is_none()));
    }

    #[test]
    fn readiness_requires_loopback_health_and_http_200() {
        assert!(validate_health_url("http://127.0.0.1:39001").is_ok());
        assert!(validate_health_url("http://0.0.0.0:39001").is_err());
        assert!(validate_health_url("https://127.0.0.1:39001").is_err());
        assert!(probe_ready("http://127.0.0.1:1").is_err());
    }

    #[test]
    fn poll_metric_parses_only_positive_finite_timestamps() {
        assert_eq!(
            last_successful_poll_timestamp("commands_poll_last_successful_timestamp_seconds{} 0\n"),
            None
        );
        assert_eq!(
            last_successful_poll_timestamp(
                "commands_poll_last_successful_timestamp_seconds{} NaN\n"
            ),
            None
        );
        assert_eq!(
            last_successful_poll_timestamp(
                "commands_poll_last_successful_timestamp_seconds{} -1\n"
            ),
            None
        );
        assert_eq!(last_successful_poll_timestamp("other_metric 10\n"), None);
        assert_eq!(
            last_successful_poll_timestamp(
                "commands_poll_last_successful_timestamp_seconds{} 1789923600\n"
            ),
            Some(1_789_923_600.0)
        );
    }

    #[test]
    fn stale_poll_timestamp_is_not_control_plane_connected() {
        let stale = Utc::now().timestamp() as f64 - 91.0;
        let evidence = probe_ready(&admin_server(None, Some(stale))).unwrap();

        assert!(!evidence.control_plane_connected);
        assert!(evidence
            .last_successful_poll_age_seconds
            .is_some_and(|age| age > CONTROL_PLANE_POLL_FRESHNESS.as_secs_f64()));
        assert!(evidence
            .control_plane_error
            .as_deref()
            .is_some_and(|message| message.contains("stale for")));
        assert_eq!(state_from_readiness(&evidence).0, TunnelState::Degraded);
    }

    #[test]
    fn poll_freshness_accepts_fresh_boundaries_and_rejects_stale_or_future() {
        let now = 1_800_000_000.0;

        assert!(control_plane_poll_is_fresh(Some(now - 10.0), now));
        assert!(control_plane_poll_is_fresh(Some(now - 89.0), now));
        assert!(!control_plane_poll_is_fresh(Some(now - 91.0), now));
        assert!(!control_plane_poll_is_fresh(Some(now + 1.0), now));
        assert!(!control_plane_poll_is_fresh(None, now));
    }

    #[test]
    fn zero_or_missing_poll_timestamp_is_degraded() {
        let zero = probe_ready(&admin_server(None, Some(0.0))).unwrap();
        assert!(!zero.control_plane_connected);
        assert_eq!(state_from_readiness(&zero).0, TunnelState::Degraded);

        let missing = probe_ready(&admin_server(None, None)).unwrap();
        assert!(!missing.control_plane_connected);
        assert_eq!(state_from_readiness(&missing).0, TunnelState::Degraded);
    }

    #[test]
    fn future_poll_timestamp_is_not_considered_fresh() {
        let future = unix_timestamp_seconds() + 10.0;
        let evidence = probe_ready(&admin_server(None, Some(future))).unwrap();

        assert!(!evidence.control_plane_connected);
        assert_eq!(evidence.last_successful_poll_timestamp, Some(future));
        assert_eq!(evidence.last_successful_poll_age_seconds, None);
        assert_eq!(
            evidence.control_plane_error.as_deref(),
            Some("Control-plane poll timestamp is in the future.")
        );
        assert_eq!(state_from_readiness(&evidence).0, TunnelState::Degraded);
    }

    #[test]
    fn stale_poll_recovers_to_ready_after_a_new_successful_poll() {
        let stale =
            probe_ready(&admin_server(None, Some(unix_timestamp_seconds() - 91.0))).unwrap();
        assert_eq!(state_from_readiness(&stale).0, TunnelState::Degraded);

        let recovered =
            probe_ready(&admin_server(None, Some(unix_timestamp_seconds() - 10.0))).unwrap();
        assert!(recovered.control_plane_connected);
        assert!(recovered
            .last_successful_poll_age_seconds
            .is_some_and(|age| age < CONTROL_PLANE_POLL_FRESHNESS.as_secs_f64()));
        assert_eq!(state_from_readiness(&recovered), (TunnelState::Ready, None));
    }

    #[test]
    fn chunked_metrics_are_decoded_before_readiness_parsing() {
        let chunked = b"4\r\ntest\r\n3\r\ning\r\n0\r\n\r\n";
        assert_eq!(decode_chunked(chunked).unwrap(), b"testing");
        assert!(decode_chunked(b"invalid\r\n").is_none());
    }

    #[test]
    fn control_plane_failure_is_degraded_and_unauthorized_is_actionable() {
        let evidence = TunnelReadinessEvidence {
            control_plane_connected: false,
            mcp_target_reachable: true,
            health_check_passed: true,
            control_plane_route: Some(TunnelControlPlaneRoute {
                route_mode: "proxy".into(),
                proxy_source: "control-plane.http-proxy".into(),
                proxy_url: Some("http://127.0.0.1:7897".into()),
            }),
            control_plane_error: Some("401 Unauthorized".into()),
            last_successful_poll_timestamp: None,
            last_successful_poll_age_seconds: None,
        };
        let (state, error) = state_from_readiness(&evidence);
        assert_eq!(state, TunnelState::Failed);
        assert_eq!(error.unwrap().code, "CONTROL_PLANE_UNAUTHORIZED");
        assert_eq!(
            health_from_readiness(&evidence).control_plane,
            TunnelHealthState::Failed
        );
    }

    #[test]
    fn live_admin_evidence_distinguishes_unauthorized_from_connected() {
        let degraded = probe_ready(&admin_server(Some("401 Unauthorized"), Some(0.0))).unwrap();
        assert!(!degraded.control_plane_connected);
        assert_eq!(
            degraded.control_plane_route.as_ref().unwrap().route_mode,
            "proxy"
        );
        assert_eq!(
            state_from_readiness(&degraded).1.unwrap().code,
            "CONTROL_PLANE_UNAUTHORIZED"
        );

        let ready = probe_ready(&admin_server(None, Some(unix_timestamp_seconds() - 1.0))).unwrap();
        assert!(ready.control_plane_connected);
        assert_eq!(state_from_readiness(&ready), (TunnelState::Ready, None));
    }

    #[test]
    fn manual_stop_disarms_automatic_recovery() {
        let manager = sleeping_manager().with_auto_recovery();
        manager.start(launch()).unwrap();
        assert!(manager.lock_recovery().desired_running);

        assert!(manager.stop().unwrap().accepted);
        wait_for_state(&manager, TunnelState::Stopped);

        let recovery = manager.lock_recovery();
        assert!(!recovery.desired_running);
        assert!(recovery.launch.is_none());
    }

    #[test]
    fn unauthorized_error_blocks_recovery_without_scheduling_restart() {
        let manager = sleeping_manager().with_auto_recovery();
        manager.start(launch()).unwrap();
        let generation = manager.lock_recovery().generation;
        let error = TunnelProcessError::control_plane("401 Unauthorized");

        manager.schedule_recovery(generation, RecoveryTrigger::ControlPlaneStale, Some(&error));

        let recovery = manager.lock_recovery();
        assert_eq!(
            recovery.blocked.as_ref().map(|error| error.code.as_str()),
            Some("CONTROL_PLANE_UNAUTHORIZED")
        );
        assert!(!recovery.recovering);
        drop(recovery);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn unauthorized_error_takes_priority_over_local_mcp_failure() {
        let evidence = TunnelReadinessEvidence {
            control_plane_connected: false,
            mcp_target_reachable: false,
            health_check_passed: true,
            control_plane_route: None,
            control_plane_error: Some("401 Unauthorized".into()),
            last_successful_poll_timestamp: None,
            last_successful_poll_age_seconds: None,
        };

        let (state, error) = state_from_readiness(&evidence);
        assert_eq!(state, TunnelState::Failed);
        assert_eq!(error.unwrap().code, "CONTROL_PLANE_UNAUTHORIZED");
        assert_eq!(
            health_from_readiness(&evidence).local_mcp,
            TunnelHealthState::Failed
        );
    }

    #[test]
    fn local_mcp_failure_is_degraded_with_mcp_specific_error() {
        let evidence = TunnelReadinessEvidence {
            control_plane_connected: true,
            mcp_target_reachable: false,
            health_check_passed: true,
            control_plane_route: None,
            control_plane_error: None,
            last_successful_poll_timestamp: Some(unix_timestamp_seconds() - 1.0),
            last_successful_poll_age_seconds: Some(1.0),
        };

        let (state, error) = state_from_readiness(&evidence);
        assert_eq!(state, TunnelState::Degraded);
        assert_eq!(error.unwrap().code, "TUNNEL_MCP_UNAVAILABLE");
        assert_eq!(
            health_from_readiness(&evidence).local_mcp,
            TunnelHealthState::Failed
        );
    }

    #[test]
    fn recovery_window_enters_cooldown_without_permanent_block() {
        let manager = sleeping_manager().with_auto_recovery();
        manager.start(launch()).unwrap();
        let generation = manager.lock_recovery().generation;
        manager.lock_recovery().attempts =
            (0..RECOVERY_MAX_ATTEMPTS).map(|_| Instant::now()).collect();
        let error = TunnelProcessError::readiness("temporary network failure");

        manager.schedule_recovery(generation, RecoveryTrigger::HealthProbe, Some(&error));

        let recovery = manager.recovery_status();
        assert_eq!(recovery.phase, TunnelRecoveryPhase::Cooldown);
        assert_eq!(recovery.reason, Some(error));
        assert!(recovery.cooldown_until.is_some());
        assert!(manager.lock_recovery().blocked.is_none());
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn half_open_failure_reenters_cooldown() {
        let manager = sleeping_manager().with_auto_recovery();
        let generation = {
            let mut recovery = manager.lock_recovery();
            recovery.desired_running = true;
            recovery.generation = 1;
            recovery.launch = Some(launch());
            recovery.half_open = true;
            recovery.recovering = false;
            recovery.generation
        };
        {
            let mut recovery = manager.lock_recovery();
            recovery.half_open = true;
            recovery.recovering = false;
            recovery.attempts.clear();
        }
        let error = TunnelProcessError::readiness("half-open probe failed");
        manager.schedule_recovery(generation, RecoveryTrigger::HealthProbe, Some(&error));

        assert_eq!(
            manager.recovery_status().phase,
            TunnelRecoveryPhase::Cooldown
        );
        assert!(manager.recovery_status().cooldown_until.is_some());
        manager.disarm_for_manual_stop();
    }

    #[test]
    fn successful_recovery_resets_backoff_state() {
        let manager = sleeping_manager().with_auto_recovery();
        let status = manager.start(launch()).unwrap();
        let instance_id = status.instance.unwrap().instance_id;
        let (generation, token) = {
            let mut recovery = manager.lock_recovery();
            recovery.attempts = (0..3).map(|_| Instant::now()).collect();
            recovery.recovering = true;
            (
                recovery.generation,
                recovery.active_instance_token.expect("active token"),
            )
        };

        manager.note_recovered(generation, token, &instance_id, TunnelState::Ready);

        let recovery = manager.recovery_status();
        assert_eq!(recovery.phase, TunnelRecoveryPhase::Idle);
        assert!(recovery.reason.is_none());
        assert!(recovery.next_attempt_at.is_none());
        assert!(recovery.cooldown_until.is_none());
        assert_eq!(manager.lock_recovery().attempts.len(), 3);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn repeated_ready_flaps_reach_the_window_cooldown() {
        let manager = sleeping_manager().with_auto_recovery();
        let status = manager.start(launch()).unwrap();
        let instance_id = status.instance.unwrap().instance_id;
        let generation = manager.lock_recovery().generation;
        let instance_token = manager
            .lock_recovery()
            .active_instance_token
            .expect("active token");
        let error = TunnelProcessError::readiness("temporary network failure");

        for _ in 0..RECOVERY_MAX_ATTEMPTS {
            manager.schedule_recovery(generation, RecoveryTrigger::HealthProbe, Some(&error));
            manager.note_recovered(generation, instance_token, &instance_id, TunnelState::Ready);
            assert!(manager.lock_recovery().blocked.is_none());
        }
        manager.schedule_recovery(generation, RecoveryTrigger::HealthProbe, Some(&error));

        assert_eq!(
            manager.recovery_status().phase,
            TunnelRecoveryPhase::Cooldown
        );
        assert!(manager.lock_recovery().blocked.is_none());
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn expired_cooldown_runs_a_half_open_recovery_and_returns_idle() {
        let manager = sleeping_manager().with_auto_recovery();
        manager.start(launch()).unwrap();
        let generation = manager.lock_recovery().generation;
        let recovery_launch = manager.lock_recovery().launch.clone().unwrap();
        manager.lock_recovery().attempts =
            (0..RECOVERY_MAX_ATTEMPTS).map(|_| Instant::now()).collect();
        manager.schedule_recovery(
            generation,
            RecoveryTrigger::HealthProbe,
            Some(&TunnelProcessError::readiness("temporary network failure")),
        );
        let token = manager.lock_recovery().token;
        {
            let mut recovery = manager.lock_recovery();
            recovery.cooldown_until = Some(Instant::now() - Duration::from_secs(1));
            recovery.cooldown_until_wall = Some(timestamp());
            recovery.next_attempt_at = Some(Instant::now() - Duration::from_secs(1));
            recovery.next_attempt_at_wall = Some(timestamp());
        }

        manager.perform_recovery(
            generation,
            RecoveryTrigger::HealthProbe,
            recovery_launch,
            token,
        );

        let status = manager.status().unwrap();
        assert_eq!(status.state, TunnelState::Ready);
        assert_eq!(status.recovery.phase, TunnelRecoveryPhase::Idle);
        assert!(status.recovery.reason.is_none());
        assert!(manager.lock_recovery().attempts.is_empty());
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn stale_recovery_task_cannot_clear_new_generation_state() {
        let manager = sleeping_manager().with_auto_recovery();
        manager.start(launch()).unwrap();
        let generation = manager.lock_recovery().generation;
        let error = TunnelProcessError::readiness("temporary network failure");
        manager.schedule_recovery(generation, RecoveryTrigger::HealthProbe, Some(&error));
        let (token, next_attempt) = {
            let recovery = manager.lock_recovery();
            (recovery.token, recovery.next_attempt_at)
        };
        assert!(next_attempt.is_some());
        let new_generation = manager.disarm_for_manual_stop();

        manager.perform_recovery(generation, RecoveryTrigger::HealthProbe, launch(), token);

        let recovery = manager.lock_recovery();
        assert_eq!(recovery.generation, new_generation);
        assert!(!recovery.recovering);
        assert!(!recovery.desired_running);
        drop(recovery);
        assert!(manager.stop().is_ok());
    }

    #[test]
    fn current_instance_ready_cancels_only_a_pending_recovery() {
        let manager = sleeping_manager().with_auto_recovery();
        let status = manager.start(launch()).unwrap();
        let instance_id = status.instance.unwrap().instance_id;
        let generation = manager.lock_recovery().generation;
        let instance_token = manager
            .lock_recovery()
            .active_instance_token
            .expect("active token");
        manager.schedule_recovery(
            generation,
            RecoveryTrigger::HealthProbe,
            Some(&TunnelProcessError::readiness("temporary network failure")),
        );
        assert!(manager.lock_recovery().next_attempt_at.is_some());

        manager.note_recovered(generation, instance_token, &instance_id, TunnelState::Ready);

        let recovery = manager.recovery_status();
        assert_eq!(recovery.phase, TunnelRecoveryPhase::Idle);
        assert!(recovery.next_attempt_at.is_none());
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn current_old_instance_ready_cannot_cancel_a_recovery_already_in_progress() {
        let manager = sleeping_manager().with_auto_recovery();
        let status = manager.start(launch()).unwrap();
        let instance_id = status.instance.unwrap().instance_id;
        let (generation, old_token) = {
            let recovery = manager.lock_recovery();
            (
                recovery.generation,
                recovery.active_instance_token.expect("active token"),
            )
        };
        {
            let mut recovery = manager.lock_recovery();
            recovery.recovering = true;
            recovery.token = old_token.wrapping_add(1);
            recovery.next_attempt_at = None;
            recovery.next_attempt_at_wall = None;
        }

        manager.note_recovered(generation, old_token, &instance_id, TunnelState::Ready);

        let recovery = manager.lock_recovery();
        assert!(recovery.recovering);
        assert_eq!(recovery.token, old_token.wrapping_add(1));
        drop(recovery);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn recovery_can_continue_after_more_than_twelve_expired_window_attempts() {
        let manager = sleeping_manager().with_auto_recovery();
        let generation = {
            let mut recovery = manager.lock_recovery();
            recovery.desired_running = true;
            recovery.generation = 1;
            recovery.launch = Some(launch());
            recovery.generation
        };
        let error = TunnelProcessError::readiness("temporary network failure");

        for _ in 0..13 {
            {
                let mut recovery = manager.lock_recovery();
                recovery.recovering = false;
                recovery.attempts = (0..RECOVERY_MAX_ATTEMPTS)
                    .map(|_| Instant::now() - RECOVERY_WINDOW - Duration::from_secs(1))
                    .collect();
                recovery.next_attempt_at = None;
                recovery.next_attempt_at_wall = None;
                recovery.cooldown_until = None;
                recovery.cooldown_until_wall = None;
                recovery.half_open = false;
            }
            manager.schedule_recovery(generation, RecoveryTrigger::HealthProbe, Some(&error));
            assert!(manager.lock_recovery().blocked.is_none());
        }

        manager.disarm_for_manual_stop();
    }

    #[test]
    fn restart_invalidates_a_pending_recovery_task_from_the_old_generation() {
        let manager = sleeping_manager().with_auto_recovery();
        let first = manager.start(launch()).unwrap();
        let first_instance = first.instance.unwrap().instance_id;
        let old_generation = manager.lock_recovery().generation;
        manager.schedule_recovery(
            old_generation,
            RecoveryTrigger::HealthProbe,
            Some(&TunnelProcessError::readiness("temporary network failure")),
        );

        let restarted = manager.restart(launch()).unwrap();
        let restarted_instance = restarted.instance.unwrap().instance_id;
        assert_ne!(first_instance, restarted_instance);
        assert_ne!(manager.lock_recovery().generation, old_generation);
        thread::sleep(Duration::from_secs(2));

        let status = manager.status().unwrap();
        assert_eq!(status.state, TunnelState::Ready);
        assert_eq!(
            status
                .instance
                .as_ref()
                .map(|instance| instance.instance_id.as_str()),
            Some(restarted_instance.as_str())
        );
        assert_eq!(status.recovery.phase, TunnelRecoveryPhase::Idle);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn stop_invalidates_a_pending_recovery_task_without_restarting() {
        let manager = sleeping_manager().with_auto_recovery();
        manager.start(launch()).unwrap();
        let generation = manager.lock_recovery().generation;
        manager.schedule_recovery(
            generation,
            RecoveryTrigger::HealthProbe,
            Some(&TunnelProcessError::readiness("temporary network failure")),
        );

        assert!(manager.stop().unwrap().accepted);
        thread::sleep(Duration::from_secs(2));
        let status = manager.status().unwrap();
        assert_eq!(status.state, TunnelState::Stopped);
        assert_eq!(status.recovery.phase, TunnelRecoveryPhase::Disarmed);
        assert!(!manager.lock_recovery().desired_running);
    }

    #[test]
    fn recovery_delay_is_exponential_bounded_and_jittered() {
        assert!(recovery_delay(1, 0) >= RECOVERY_BASE_DELAY);
        assert!(recovery_delay(1, 1) > recovery_delay(1, 0));
        assert!(recovery_delay(20, u64::MAX) <= RECOVERY_MAX_DELAY);
        assert!(recovery_delay(8, 0) >= Duration::from_secs(60));
    }

    #[test]
    fn structured_logger_redacts_sensitive_values() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tunnel-supervisor.ndjson");
        let logger = TunnelLogger::new();
        logger.set_path(Some(path.clone()));
        logger.event(
            "probe",
            serde_json::json!({
                "generation": 3,
                "attempt": 2,
                "token": "raw-secret-token",
                "proxy_url": "http://user:password@127.0.0.1:7897",
                "error": "Authorization: Bearer secret-value"
            }),
        );

        let output = fs::read_to_string(path).unwrap();
        assert!(output.contains("\"event\":\"probe\""));
        assert!(output.contains("<REDACTED>"));
        assert!(!output.contains("secret-value"));
        assert!(!output.contains("raw-secret-token"));
        assert!(!output.contains("user:password"));
    }

    #[test]
    fn readiness_watcher_keeps_running_after_threshold_and_recovers() {
        let manager = sleeping_manager();
        manager.start(launch()).unwrap();
        let process = manager
            .inner
            .lock()
            .unwrap()
            .process
            .as_ref()
            .cloned()
            .unwrap();
        let (url, healthy) = toggle_admin_server();
        fs::write(&process.health_url_file, url).unwrap();
        let (generation, token) = {
            let recovery = manager.lock_recovery();
            (recovery.generation, recovery.token)
        };

        healthy.store(false, Ordering::SeqCst);
        spawn_readiness_watcher(manager.clone(), process, generation, token);
        let failed = wait_for_state(&manager, TunnelState::Failed);
        assert_eq!(failed.health.process, TunnelHealthState::Ready);
        assert_eq!(failed.health.control_plane, TunnelHealthState::Unknown);

        healthy.store(true, Ordering::SeqCst);
        let recovered = wait_for_state(&manager, TunnelState::Ready);
        assert_eq!(recovered.health.process, TunnelHealthState::Ready);
        assert_eq!(recovered.health.local_mcp, TunnelHealthState::Ready);
        assert_eq!(recovered.health.control_plane, TunnelHealthState::Ready);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn process_exit_triggers_bounded_automatic_restart() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("started.marker");
        let script = directory.path().join("restart-once.cmd");
        fs::write(
            &script,
            format!(
                "@echo off\r\nif exist \"{0}\" (\r\n  ping 127.0.0.1 -n 30 > NUL\r\n) else (\r\n  echo started>\"{0}\"\r\n  exit /b 7\r\n)\r\n",
                marker.display()
            ),
        )
        .unwrap();
        let manager = TunnelProcessManager::with_test_command(TestTunnelCommand {
            executable_path: cmd_path(),
            args: vec!["/C".into(), script.display().to_string()],
        })
        .with_auto_recovery();
        let first = manager.start(launch()).unwrap();
        let first_instance = first.instance.unwrap().instance_id;

        wait_for_state(&manager, TunnelState::Failed);
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            let status = manager.status().unwrap();
            if status.state == TunnelState::Ready
                && status
                    .instance
                    .as_ref()
                    .is_some_and(|instance| instance.instance_id != first_instance)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "automatic restart did not recover"
            );
            thread::sleep(Duration::from_millis(20));
        }
        assert!(manager.stop().unwrap().accepted);
    }
}
