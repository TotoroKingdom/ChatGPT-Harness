use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread,
    time::Duration,
};

const STOP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TunnelInstance {
    pub instance_id: String,
    pub pid: u32,
    pub tunnel_id: Option<String>,
    pub runtime_endpoint_ref: String,
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
pub struct TunnelStatus {
    pub state: TunnelState,
    pub instance: Option<TunnelInstance>,
    pub last_exit: Option<TunnelExit>,
    pub last_error: Option<TunnelProcessError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TunnelStopResult {
    pub accepted: bool,
    pub forced: bool,
}

#[derive(Debug, Clone)]
pub struct TunnelLaunch {
    pub executable_path: PathBuf,
    pub tunnel_id: Option<String>,
    pub runtime_endpoint_ref: String,
}

impl TunnelLaunch {
    pub fn new(executable_path: PathBuf, runtime_endpoint_ref: impl Into<String>) -> Self {
        Self {
            executable_path,
            tunnel_id: None,
            runtime_endpoint_ref: runtime_endpoint_ref.into(),
        }
    }

    pub fn with_tunnel_id(mut self, tunnel_id: impl Into<String>) -> Self {
        self.tunnel_id = Some(tunnel_id.into());
        self
    }
}

#[derive(Clone)]
struct ManagedTunnelProcess {
    instance: TunnelInstance,
    child: Arc<Mutex<Child>>,
    exited: Arc<(Mutex<bool>, Condvar)>,
}

struct TunnelInner {
    process: Option<ManagedTunnelProcess>,
    status: TunnelStatus,
}

pub struct TunnelProcessManager {
    inner: Arc<Mutex<TunnelInner>>,
    #[cfg(test)]
    test_command: Option<TestTunnelCommand>,
    #[cfg(test)]
    stop_timeout: Duration,
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
                    last_exit: None,
                    last_error: None,
                },
            })),
            #[cfg(test)]
            test_command: None,
            #[cfg(test)]
            stop_timeout: STOP_TIMEOUT,
        }
    }

    pub fn start(&self, launch: TunnelLaunch) -> Result<TunnelStatus, TunnelProcessError> {
        let executable_path = self.resolve_executable(&launch)?;

        let instance_id = {
            let mut inner = self.lock_inner()?;
            match inner.status.state {
                TunnelState::Stopped | TunnelState::Failed => {}
                TunnelState::Starting => {
                    return Err(TunnelProcessError::conflict(
                        "Tunnel client is already starting.",
                    ))
                }
                TunnelState::Running => {
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
                last_exit: None,
                last_error: None,
            };
            uuid::Uuid::new_v4().to_string()
        };

        let child = match self.spawn_process(&executable_path) {
            Ok(child) => child,
            Err(error) => {
                let mut inner = self.lock_inner()?;
                inner.status = TunnelStatus {
                    state: TunnelState::Failed,
                    instance: None,
                    last_exit: None,
                    last_error: Some(error.clone()),
                };
                return Err(error);
            }
        };

        let instance = TunnelInstance {
            instance_id,
            pid: child.id(),
            tunnel_id: launch.tunnel_id.filter(|value| !value.trim().is_empty()),
            runtime_endpoint_ref: launch.runtime_endpoint_ref,
            started_at: timestamp(),
            executable_path,
        };
        let process = ManagedTunnelProcess {
            instance: instance.clone(),
            child: Arc::new(Mutex::new(child)),
            exited: Arc::new((Mutex::new(false), Condvar::new())),
        };

        {
            let mut inner = self.lock_inner()?;
            if inner.status.state != TunnelState::Starting {
                return Err(TunnelProcessError::conflict(
                    "Tunnel lifecycle changed while starting.",
                ));
            }
            inner.process = Some(process.clone());
            inner.status = TunnelStatus {
                state: TunnelState::Running,
                instance: Some(instance),
                last_exit: None,
                last_error: None,
            };
        }
        spawn_exit_watcher(self.inner.clone(), process);
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
                TunnelState::Stopped | TunnelState::Failed => {
                    return Ok(TunnelStopResult {
                        accepted: false,
                        forced: false,
                    })
                }
                TunnelState::Starting => {
                    return Err(TunnelProcessError::conflict(
                        "Tunnel client is still starting.",
                    ))
                }
                TunnelState::Stopping => {
                    return Err(TunnelProcessError::conflict(
                        "Tunnel client is already stopping.",
                    ))
                }
                TunnelState::Running => {}
            }
            let process = inner.process.clone().ok_or_else(|| {
                TunnelProcessError::state_unavailable("Managed tunnel process is missing.")
            })?;
            if process.instance.instance_id != expected_instance_id {
                return Err(TunnelProcessError::instance_mismatch());
            }
            inner.status.state = TunnelState::Stopping;
            inner.status.last_error = None;
            process
        };

        let _ = request_graceful_termination(process.instance.pid);
        if wait_for_exit(&process.exited, self.stop_timeout()) {
            return Ok(TunnelStopResult {
                accepted: true,
                forced: false,
            });
        }

        let force_result = process
            .child
            .lock()
            .map_err(|_| {
                TunnelProcessError::state_unavailable("Tunnel child handle is unavailable.")
            })?
            .kill();
        if let Err(error) = force_result {
            if !wait_for_exit(&process.exited, Duration::from_millis(50)) {
                return Err(TunnelProcessError::stop_failed(format!(
                    "Could not force-stop owned tunnel process: {error}"
                )));
            }
        }
        if !wait_for_exit(&process.exited, self.stop_timeout()) {
            return Err(TunnelProcessError::stop_failed(
                "Owned tunnel process did not exit after force termination.",
            ));
        }
        Ok(TunnelStopResult {
            accepted: true,
            forced: true,
        })
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

        if !candidate.is_file() {
            return Err(TunnelProcessError::executable_not_found(candidate));
        }
        candidate.canonicalize().map_err(|error| {
            TunnelProcessError::executable_not_found_with_reason(candidate, error.to_string())
        })
    }

    fn spawn_process(&self, executable_path: &PathBuf) -> Result<Child, TunnelProcessError> {
        let mut command = Command::new(executable_path);
        #[cfg(test)]
        if let Some(test_command) = &self.test_command {
            command.args(&test_command.args);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
        }
        command.spawn().map_err(|error| {
            TunnelProcessError::spawn(format!(
                "Could not start {}: {error}",
                executable_path.display()
            ))
        })
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
                    last_exit: None,
                    last_error: None,
                },
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

impl TunnelProcessError {
    fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: "LIFECYCLE_CONFLICT".into(),
            message: message.into(),
        }
    }

    fn executable_not_found(path: PathBuf) -> Self {
        Self {
            code: "TUNNEL_EXECUTABLE_NOT_FOUND".into(),
            message: format!("Tunnel executable does not exist: {}.", path.display()),
        }
    }

    fn executable_not_found_with_reason(path: PathBuf, reason: String) -> Self {
        Self {
            code: "TUNNEL_EXECUTABLE_NOT_FOUND".into(),
            message: format!(
                "Tunnel executable is unavailable at {}: {reason}",
                path.display()
            ),
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
                    last_exit: Some(exit),
                    last_error: if stopped_by_manager {
                        None
                    } else {
                        Some(TunnelProcessError::crashed(exit_code))
                    },
                };
            }
        }

        let (lock, wake) = &*process.exited;
        if let Ok(mut exited) = lock.lock() {
            *exited = true;
            wake.notify_all();
        }
    });
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
        "graceful tunnel termination is only implemented for Windows",
    ))
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        TunnelLaunch::new(cmd_path(), "runtime-instance:current").with_tunnel_id("tunnel-a")
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
    fn start_creates_a_running_owned_instance() {
        let manager = sleeping_manager();
        let status = manager.start(launch()).unwrap();
        let instance = status.instance.clone().unwrap();

        assert_eq!(status.state, TunnelState::Running);
        assert!(!instance.instance_id.is_empty());
        assert!(instance.pid > 0);
        assert_eq!(instance.tunnel_id.as_deref(), Some("tunnel-a"));
        assert_eq!(instance.runtime_endpoint_ref, "runtime-instance:current");
        assert!(manager.stop().unwrap().accepted);
        assert_eq!(
            wait_for_state(&manager, TunnelState::Stopped).state,
            TunnelState::Stopped
        );
    }

    #[test]
    fn stop_stops_the_owned_running_instance() {
        let manager = sleeping_manager();
        manager.start(launch()).unwrap();

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
        assert_eq!(manager.status().unwrap().state, TunnelState::Running);
        assert_ne!(old_instance_id, new_instance_id);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn unexpected_exit_transitions_running_to_failed() {
        let manager = crashing_manager();
        manager.start(launch()).unwrap();

        let status = wait_for_state(&manager, TunnelState::Failed);
        assert_eq!(status.last_exit.unwrap().exit_code, Some(7));
        assert_eq!(status.last_error.unwrap().code, "TUNNEL_PROCESS_EXITED");
    }

    #[test]
    fn double_start_is_rejected() {
        let manager = sleeping_manager();
        manager.start(launch()).unwrap();

        let error = manager.start(launch()).unwrap_err();
        assert_eq!(error.code, "LIFECYCLE_CONFLICT");
        assert_eq!(manager.status().unwrap().state, TunnelState::Running);
        assert!(manager.stop().unwrap().accepted);
    }

    #[test]
    fn restart_stops_the_old_instance_before_starting_a_new_one() {
        let manager = sleeping_manager();
        let old_instance_id = manager
            .start(launch())
            .unwrap()
            .instance
            .unwrap()
            .instance_id;

        let restarted = manager.restart(launch()).unwrap();
        let new_instance_id = restarted.instance.unwrap().instance_id;
        assert_eq!(restarted.state, TunnelState::Running);
        assert_ne!(old_instance_id, new_instance_id);
        assert_eq!(
            manager.stop_instance(&old_instance_id).unwrap_err().code,
            "TUNNEL_INSTANCE_MISMATCH"
        );
        assert!(manager.stop().unwrap().accepted);
    }
}
