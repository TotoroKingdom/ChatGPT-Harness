use crate::{
    config::{ConfigStore, CURRENT_SCHEMA_VERSION},
    runtime::{RuntimeEventSink, RuntimeProcessPhase, RuntimeSupervisor, SupervisorError},
    startup::StartupPhaseRecorder,
    tunnel::{
        resolve_tunnel, TunnelEventSink, TunnelHealth, TunnelLaunch, TunnelProcessError,
        TunnelProcessManager, TunnelReadinessEvidence, TunnelRecoveryStatus, TunnelState,
        TunnelStatus,
    },
};
use harness_control_protocol::{
    AppConfig, SecurityMode, StartEligibility, StartEligibilityStatus, StartupCohort, StartupPhase,
};
use serde::Serialize;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessLifecycle {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Error,
    Stopping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessComponentStatus {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Error,
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessErrorSummary {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStatus {
    pub lifecycle: HarnessLifecycle,
    pub runtime: HarnessComponentStatus,
    pub mcp: HarnessComponentStatus,
    pub tunnel: HarnessComponentStatus,
    pub active_session_id: Option<String>,
    pub config_revision: Option<String>,
    pub last_error: Option<HarnessErrorSummary>,
    pub tunnel_health: TunnelHealth,
    pub tunnel_readiness: Option<TunnelReadinessEvidence>,
    pub tunnel_recovery: TunnelRecoveryStatus,
}

struct HarnessInner {
    status: HarnessStatus,
    runtime_generation: Option<Uuid>,
    tunnel_generation: Option<u64>,
    tunnel_instance: Option<String>,
    startup_seen: bool,
}

struct HarnessShared {
    inner: Mutex<HarnessInner>,
    operation: Mutex<()>,
    status_sink: Mutex<Option<Arc<dyn Fn(&HarnessStatus) + Send + Sync>>>,
}

pub struct HarnessController {
    shared: Arc<HarnessShared>,
    runtime: RuntimeSupervisor,
    tunnel: TunnelProcessManager,
    startup: StartupPhaseRecorder,
}

impl Clone for HarnessController {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            runtime: self.runtime.clone(),
            tunnel: self.tunnel.clone(),
            startup: self.startup.clone(),
        }
    }
}

impl HarnessController {
    pub fn new(runtime: RuntimeSupervisor, tunnel: TunnelProcessManager) -> Self {
        Self {
            shared: Arc::new(HarnessShared {
                inner: Mutex::new(HarnessInner {
                    status: stopped_status(),
                    runtime_generation: None,
                    tunnel_generation: None,
                    tunnel_instance: None,
                    startup_seen: false,
                }),
                operation: Mutex::new(()),
                status_sink: Mutex::new(None),
            }),
            runtime,
            tunnel,
            startup: StartupPhaseRecorder::new(),
        }
    }

    pub(crate) fn attach_app(&self, app: &AppHandle) {
        let _ = app;
    }

    pub(crate) fn set_status_sink(&self, sink: Arc<dyn Fn(&HarnessStatus) + Send + Sync>) {
        if let Ok(mut current) = self.shared.status_sink.lock() {
            *current = Some(sink);
        }
    }

    pub fn status(&self) -> HarnessStatus {
        self.reconcile_tunnel_projection();
        let status = self
            .shared
            .inner
            .lock()
            .map(|inner| inner.status.clone())
            .unwrap_or_else(|_| stopped_status());
        status
    }

    fn reconcile_tunnel_projection(&self) -> bool {
        let Ok(tunnel_status) = self.tunnel.status() else {
            return false;
        };
        let Ok(mut inner) = self.shared.inner.lock() else {
            return false;
        };
        if inner.status.active_session_id.is_none() {
            return false;
        }
        let previous = inner.status.clone();
        if let Some(instance) = tunnel_status.instance.as_ref() {
            inner.tunnel_instance = Some(instance.instance_id.clone());
        }
        inner.tunnel_generation = Some(tunnel_status.generation);
        inner.status.tunnel = match tunnel_status.state {
            TunnelState::Ready => HarnessComponentStatus::Ready,
            TunnelState::Degraded => HarnessComponentStatus::Degraded,
            TunnelState::Failed => HarnessComponentStatus::Error,
            TunnelState::Stopping => HarnessComponentStatus::Stopping,
            TunnelState::Stopped => HarnessComponentStatus::Stopped,
            TunnelState::Starting => HarnessComponentStatus::Starting,
        };
        inner.status.tunnel_readiness = tunnel_status.readiness.clone();
        inner.status.tunnel_health = tunnel_status.health;
        inner.status.tunnel_recovery = tunnel_status.recovery.clone();
        if inner.status.runtime == HarnessComponentStatus::Ready
            && inner.status.mcp == HarnessComponentStatus::Ready
        {
            match tunnel_status.state {
                TunnelState::Ready => {
                    inner.status.lifecycle = HarnessLifecycle::Ready;
                    inner.status.last_error = None;
                }
                TunnelState::Degraded if inner.status.lifecycle == HarnessLifecycle::Ready => {
                    inner.status.lifecycle = HarnessLifecycle::Degraded;
                }
                TunnelState::Failed if inner.status.lifecycle != HarnessLifecycle::Stopping => {
                    inner.status.lifecycle = HarnessLifecycle::Error;
                    inner.status.last_error = tunnel_status
                        .last_error
                        .as_ref()
                        .map(error_summary_from_tunnel);
                }
                _ => {}
            }
        }
        let session_id = inner.status.active_session_id.clone();
        let status = inner.status.clone();
        let changed = harness_status_event_changed(&previous, &status);
        drop(inner);
        if let Some(session_id) = session_id {
            self.observe_startup_readiness(&session_id, &status);
        }
        changed
    }

    pub(crate) fn latest_startup_attempt(
        &self,
    ) -> Option<harness_control_protocol::StartupAttempt> {
        self.startup.latest()
    }

    pub fn start_eligibility(
        &self,
        app: &AppHandle,
        secrets: &Mutex<crate::commands::BackendState>,
    ) -> Result<StartEligibility, crate::commands::ControlError> {
        self.attach_app(app);
        let status = self.status();
        if matches!(
            status.lifecycle,
            HarnessLifecycle::Starting
                | HarnessLifecycle::Ready
                | HarnessLifecycle::Degraded
                | HarnessLifecycle::Stopping
        ) {
            return Ok(StartEligibility {
                status: if matches!(
                    status.lifecycle,
                    HarnessLifecycle::Ready | HarnessLifecycle::Degraded
                ) {
                    StartEligibilityStatus::HarnessAlreadyRunning
                } else {
                    StartEligibilityStatus::LifecycleConflict
                },
                can_start: false,
                message: "Harness is already running or changing state.".into(),
            });
        }
        let config = match load_config(app)? {
            Some(config) => config,
            None => {
                return Ok(eligibility(
                    StartEligibilityStatus::SetupRequired,
                    "Save a Workspace before starting Harness.",
                ))
            }
        };
        if let Err(error) = validate_harness_config(&config) {
            return Ok(eligibility(
                StartEligibilityStatus::ConfigurationInvalid,
                error,
            ));
        }
        if !matches!(
            self.runtime.process_status().phase,
            RuntimeProcessPhase::Stopped | RuntimeProcessPhase::Crashed
        ) {
            return Ok(eligibility(
                StartEligibilityStatus::RuntimeAlreadyRunning,
                "Runtime is already running outside the active Harness session.",
            ));
        }
        if !config.tunnel.tunnel_id.trim().is_empty() {
            // Keep the specific Tunnel ID check below separate so the UI can
            // explain the missing connection input.
        } else {
            return Ok(eligibility(
                StartEligibilityStatus::TunnelIdRequired,
                "Configure a Tunnel ID before starting Harness.",
            ));
        }
        let Some(reference) = config.tunnel.api_key_ref.as_ref() else {
            return Ok(eligibility(
                StartEligibilityStatus::ApiKeyRequired,
                "Save a Tunnel API Key before starting Harness.",
            ));
        };
        let secret_available = secrets
            .lock()
            .map_err(|_| secret_error("Secret store unavailable."))?
            .secrets
            .get(&reference.key)
            .map_err(secret_error)?
            .is_some();
        if !secret_available {
            return Ok(eligibility(
                StartEligibilityStatus::ApiKeyRequired,
                "The saved Tunnel API Key is unavailable; save it again.",
            ));
        }
        crate::runtime::discovery::resolve_runtime().map_err(|error| {
            crate::commands::ControlError {
                code: error.code,
                message: error.message,
            }
        })?;
        resolve_tunnel(&config).map_err(control_error_from_tunnel)?;
        Ok(eligibility(
            StartEligibilityStatus::ReadyToStart,
            "Harness is ready to start Runtime, MCP, and Tunnel.",
        ))
    }

    pub fn start(
        &self,
        app: &AppHandle,
        secrets: &Mutex<crate::commands::BackendState>,
    ) -> Result<HarnessStatus, crate::commands::ControlError> {
        let _operation = self.lock_operation()?;
        self.attach_app(app);
        self.start_locked(app, secrets)
    }

    pub fn stop(&self, app: &AppHandle) -> Result<HarnessStatus, crate::commands::ControlError> {
        let _operation = self.lock_operation()?;
        self.attach_app(app);
        self.stop_locked(app)
    }

    pub fn restart(
        &self,
        app: &AppHandle,
        secrets: &Mutex<crate::commands::BackendState>,
    ) -> Result<HarnessStatus, crate::commands::ControlError> {
        let _operation = self.lock_operation()?;
        self.attach_app(app);
        let stopped = self.stop_locked(app)?;
        if stopped.lifecycle != HarnessLifecycle::Stopped {
            return Err(lifecycle_error(
                "Harness could not complete shutdown before restart.",
            ));
        }
        self.start_locked(app, secrets)
    }

    fn lock_operation(&self) -> Result<MutexGuard<'_, ()>, crate::commands::ControlError> {
        self.shared.operation.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => crate::commands::ControlError {
                code: "LIFECYCLE_CONFLICT".into(),
                message: "Another Harness operation is in progress. Wait for it to finish, then retry.".into(),
            },
            TryLockError::Poisoned(_) => lifecycle_error("Harness lifecycle state is unavailable."),
        })
    }

    fn start_locked(
        &self,
        app: &AppHandle,
        secrets: &Mutex<crate::commands::BackendState>,
    ) -> Result<HarnessStatus, crate::commands::ControlError> {
        if app
            .try_state::<crate::desktop::DesktopLifecycleCoordinator>()
            .is_some_and(|desktop| desktop.snapshot().state.app_quitting)
        {
            return Err(crate::commands::ControlError {
                code: "LIFECYCLE_CONFLICT".into(),
                message: "Application shutdown is in progress; Harness cannot start.".into(),
            });
        }
        let session_id = Uuid::new_v4().to_string();
        let cohort = self
            .shared
            .inner
            .lock()
            .map(|mut inner| {
                let cohort = if inner.startup_seen {
                    StartupCohort::Warm
                } else {
                    StartupCohort::Cold
                };
                inner.startup_seen = true;
                cohort
            })
            .unwrap_or(StartupCohort::Warm);
        self.startup.begin(session_id.clone(), cohort);
        self.startup
            .start_phase(&session_id, StartupPhase::EligibilityPrepare);
        let eligibility = match self.start_eligibility(app, secrets) {
            Ok(value) => value,
            Err(error) => {
                self.startup.fail_active(&session_id, &error.code);
                return Err(error);
            }
        };
        if !eligibility.can_start {
            self.startup.fail_phase(
                &session_id,
                StartupPhase::EligibilityPrepare,
                "ELIGIBILITY_REJECTED",
            );
            self.startup
                .fail_active(&session_id, "ELIGIBILITY_REJECTED");
            return Err(lifecycle_error(eligibility.message));
        }
        self.startup
            .complete_phase(&session_id, StartupPhase::EligibilityPrepare);
        let config = match load_config(app) {
            Ok(Some(config)) => config,
            Ok(None) => {
                self.startup.fail_active(&session_id, "CONFIG_NOT_FOUND");
                return Err(lifecycle_error(
                    "Save configuration before starting Harness.",
                ));
            }
            Err(error) => {
                self.startup.fail_active(&session_id, &error.code);
                return Err(error);
            }
        };
        self.update(|inner| {
            inner.status = HarnessStatus {
                lifecycle: HarnessLifecycle::Starting,
                runtime: HarnessComponentStatus::Starting,
                mcp: HarnessComponentStatus::Starting,
                tunnel: HarnessComponentStatus::Starting,
                active_session_id: Some(session_id.clone()),
                config_revision: Some(config.config_revision.get().to_string()),
                last_error: None,
                tunnel_health: TunnelHealth::starting(),
                tunnel_readiness: None,
                tunnel_recovery: TunnelRecoveryStatus::default(),
            };
            inner.runtime_generation = None;
            inner.tunnel_generation = None;
            inner.tunnel_instance = None;
        });

        self.startup
            .start_phase(&session_id, StartupPhase::RuntimeSpawn);
        crate::history::prepare_before_runtime_start(app);
        let runtime_state =
            match self
                .runtime
                .start_with_host_session(app, &config, Some(session_id.as_str()))
            {
                Ok(state) => state,
                Err(error) => return Err(self.fail_start(app, error.into(), false)),
            };
        // RuntimeSupervisor emits finer-grained hooks in production. These
        // idempotent completions keep direct/unit construction deterministic.
        self.startup
            .complete_phase(&session_id, StartupPhase::RuntimeSpawn);
        self.startup
            .start_phase(&session_id, StartupPhase::RuntimeInitialize);
        self.startup
            .complete_phase(&session_id, StartupPhase::RuntimeInitialize);
        self.startup
            .start_phase(&session_id, StartupPhase::McpEndpointReady);
        let runtime_generation = self.runtime.active_generation();
        let endpoint = match self.runtime.mcp_endpoint() {
            Ok(endpoint) => endpoint,
            Err(error) => return Err(self.fail_start(app, error.into(), true)),
        };
        self.startup
            .complete_phase(&session_id, StartupPhase::McpEndpointReady);
        if let Some(identity) = self.runtime.identity() {
            self.startup
                .set_runtime_version(&session_id, identity.runtime_version);
        }
        self.update(|inner| {
            inner.runtime_generation = runtime_generation;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
        });

        self.startup
            .start_phase(&session_id, StartupPhase::TunnelResolve);
        let resolved = match resolve_tunnel(&config) {
            Ok(value) => value,
            Err(error) => return Err(self.fail_start(app, control_error_from_tunnel(error), true)),
        };
        self.startup
            .complete_phase(&session_id, StartupPhase::TunnelResolve);
        self.startup
            .set_tunnel_version(&session_id, resolved.version.clone());
        let reference = config
            .tunnel
            .api_key_ref
            .as_ref()
            .ok_or_else(|| lifecycle_error("Tunnel API Key is unavailable."));
        let reference = match reference {
            Ok(reference) => reference,
            Err(error) => return Err(self.fail_start(app, error, true)),
        };
        let secret = match secrets.lock() {
            Ok(backend) => match backend.secrets.get(&reference.key) {
                Ok(value) => value,
                Err(error) => return Err(self.fail_start(app, secret_error(error), true)),
            },
            Err(_) => {
                return Err(self.fail_start(app, secret_error("Secret store unavailable."), true))
            }
        };
        let api_key = match secret {
            Some(value) => value,
            None => {
                return Err(self.fail_start(
                    app,
                    lifecycle_error("Tunnel API Key is unavailable."),
                    true,
                ))
            }
        };
        let mut launch = TunnelLaunch::new(resolved.client_path, endpoint)
            .with_tunnel_id(config.tunnel.tunnel_id.clone())
            .with_control_plane_http_proxy(config.tunnel.control_plane_http_proxy.clone())
            .with_cloudflared_path(resolved.cloudflared_path)
            .with_api_key(api_key);
        if let Ok(directory) = app.path().app_data_dir() {
            launch = launch.with_log_file(directory.join("logs").join("tunnel-supervisor.ndjson"));
        }
        self.startup
            .start_phase(&session_id, StartupPhase::TunnelLaunch);
        let tunnel_status = match self.tunnel.start(launch) {
            Ok(status) => status,
            Err(error) => return Err(self.fail_start(app, control_error_from_tunnel(error), true)),
        };
        self.startup
            .complete_phase(&session_id, StartupPhase::TunnelLaunch);
        // The managed Tunnel start returns before control-plane readiness is
        // necessarily proven. Keep the fixed tail phases sequential because
        // the recorder intentionally tracks one open phase at a time.
        self.startup
            .start_phase(&session_id, StartupPhase::ControlPlaneConnected);
        let Some(instance) = tunnel_status.instance.as_ref() else {
            return Err(self.fail_start(
                app,
                lifecycle_error("Tunnel did not return a managed instance."),
                true,
            ));
        };
        let instance_id = instance.instance_id.clone();
        let generation = tunnel_status.generation;
        self.update(|inner| {
            inner.tunnel_instance = Some(instance_id.clone());
            inner.tunnel_generation = Some(generation);
        });
        let tunnel_status = self.tunnel.status().unwrap_or(tunnel_status);
        self.update(|inner| {
            apply_tunnel_start_snapshot_if_pending(inner, &instance_id, &tunnel_status);
        });
        let status = self.status();
        self.observe_startup_readiness(&session_id, &status);
        let _ = runtime_state;
        self.emit_status(&status);
        Ok(status)
    }

    fn stop_locked(&self, app: &AppHandle) -> Result<HarnessStatus, crate::commands::ControlError> {
        let current = self.status();
        if current.lifecycle == HarnessLifecycle::Stopped && current.active_session_id.is_none() {
            return Ok(current);
        }
        if let Some(session_id) = current.active_session_id.as_deref() {
            self.startup.cancel(session_id);
        }
        self.update(|inner| {
            inner.status.lifecycle = HarnessLifecycle::Stopping;
            inner.status.runtime = HarnessComponentStatus::Stopping;
            inner.status.mcp = HarnessComponentStatus::Stopping;
            inner.status.tunnel = HarnessComponentStatus::Stopping;
        });
        let tunnel_error = self.tunnel.stop().err().map(control_error_from_tunnel);
        let runtime_error = self
            .runtime
            .stop(app)
            .err()
            .map(crate::commands::ControlError::from);
        let error = tunnel_error.or(runtime_error);
        self.update(|inner| {
            inner.runtime_generation = None;
            inner.tunnel_generation = None;
            inner.tunnel_instance = None;
            inner.status.active_session_id = None;
            inner.status.config_revision = None;
            if let Some(error) = &error {
                inner.status.lifecycle = HarnessLifecycle::Error;
                inner.status.runtime = HarnessComponentStatus::Error;
                inner.status.mcp = HarnessComponentStatus::Error;
                inner.status.tunnel = HarnessComponentStatus::Error;
                inner.status.tunnel_health = TunnelHealth::failed();
                inner.status.last_error = Some(error_summary(error));
            } else {
                inner.status = stopped_status();
            }
        });
        let status = self.status();
        self.emit_status(&status);
        match error {
            Some(error) => Err(error),
            None => Ok(status),
        }
    }

    fn fail_start(
        &self,
        app: &AppHandle,
        error: crate::commands::ControlError,
        runtime_started: bool,
    ) -> crate::commands::ControlError {
        let session_id = self.status().active_session_id;
        let _ = self.tunnel.stop();
        if runtime_started {
            let _ = self.runtime.stop(app);
        }
        self.update(|inner| {
            inner.runtime_generation = None;
            inner.tunnel_generation = None;
            inner.tunnel_instance = None;
            inner.status.lifecycle = HarnessLifecycle::Error;
            inner.status.runtime = HarnessComponentStatus::Stopped;
            inner.status.mcp = HarnessComponentStatus::Stopped;
            inner.status.tunnel = HarnessComponentStatus::Stopped;
            inner.status.tunnel_health = TunnelHealth::stopped();
            inner.status.last_error = Some(error_summary(&error));
        });
        let status = self.status();
        if let Some(session_id) = session_id.as_deref() {
            self.startup.fail_active(session_id, &error.code);
        }
        self.emit_status(&status);
        error
    }

    fn update(&self, update: impl FnOnce(&mut HarnessInner)) {
        if let Ok(mut inner) = self.shared.inner.lock() {
            update(&mut inner);
        }
    }

    fn emit_status(&self, status: &HarnessStatus) {
        // Status sinks may synchronously query Runtime/Tunnel to refresh
        // authoritative projections. Clone the callback while protected, but
        // never hold status_sink across that external call.
        let sink = self
            .shared
            .status_sink
            .lock()
            .ok()
            .and_then(|sink| sink.clone());
        if let Some(sink) = sink {
            sink(status);
        }
    }

    fn on_runtime_event(
        &self,
        generation: Uuid,
        phase: RuntimeProcessPhase,
        error: Option<&SupervisorError>,
    ) {
        let mut stop_tunnel = false;
        let mut changed = false;
        if let Ok(mut inner) = self.shared.inner.lock() {
            if inner.runtime_generation != Some(generation) {
                return;
            }
            match phase {
                RuntimeProcessPhase::Crashed => {
                    inner.status.lifecycle = HarnessLifecycle::Error;
                    inner.status.runtime = HarnessComponentStatus::Error;
                    inner.status.mcp = HarnessComponentStatus::Error;
                    inner.status.tunnel = HarnessComponentStatus::Error;
                    inner.status.last_error = Some(error_summary_from_runtime(error));
                    stop_tunnel = true;
                    changed = true;
                }
                RuntimeProcessPhase::Stopped => {
                    if inner.status.lifecycle != HarnessLifecycle::Stopping {
                        inner.status.lifecycle = HarnessLifecycle::Error;
                        inner.status.runtime = HarnessComponentStatus::Error;
                        inner.status.mcp = HarnessComponentStatus::Error;
                        inner.status.last_error = Some(error_summary_from_runtime(error));
                        stop_tunnel = true;
                    }
                    changed = true;
                }
                RuntimeProcessPhase::Stopping => {
                    inner.status.runtime = HarnessComponentStatus::Stopping;
                    inner.status.mcp = HarnessComponentStatus::Stopping;
                    changed = true;
                }
                RuntimeProcessPhase::Running => {
                    inner.status.runtime = HarnessComponentStatus::Ready;
                    inner.status.mcp = HarnessComponentStatus::Ready;
                    changed = true;
                }
                RuntimeProcessPhase::Starting => {
                    inner.status.runtime = HarnessComponentStatus::Starting;
                    changed = true;
                }
            }
        }
        if stop_tunnel {
            let _ = self.tunnel.stop();
            self.update(|inner| {
                if inner.status.lifecycle != HarnessLifecycle::Stopping {
                    inner.status.tunnel = HarnessComponentStatus::Error;
                }
            });
        }
        if changed {
            let status = self.status();
            self.emit_status(&status);
        }
    }

    fn on_runtime_startup(
        &self,
        generation: Uuid,
        phase: crate::runtime::RuntimeStartupPhase,
        error: Option<&SupervisorError>,
    ) {
        let session_id = self.shared.inner.lock().ok().and_then(|mut inner| {
            if inner.status.lifecycle != HarnessLifecycle::Starting {
                return None;
            }
            if inner.runtime_generation.is_none() {
                inner.runtime_generation = Some(generation);
            }
            (inner.runtime_generation == Some(generation))
                .then(|| inner.status.active_session_id.clone())
                .flatten()
        });
        let Some(session_id) = session_id else {
            return;
        };
        if let Some(error) = error {
            self.startup.fail_active(&session_id, &error.code);
            return;
        }
        match phase {
            crate::runtime::RuntimeStartupPhase::Spawned => {
                self.startup
                    .complete_phase(&session_id, StartupPhase::RuntimeSpawn);
                self.startup
                    .start_phase(&session_id, StartupPhase::RuntimeInitialize);
            }
            crate::runtime::RuntimeStartupPhase::Initialized => {
                self.startup
                    .complete_phase(&session_id, StartupPhase::RuntimeInitialize);
                self.startup
                    .start_phase(&session_id, StartupPhase::McpEndpointReady);
            }
            crate::runtime::RuntimeStartupPhase::McpEndpointReady => {
                self.startup
                    .complete_phase(&session_id, StartupPhase::McpEndpointReady);
            }
        }
    }

    fn observe_startup_readiness(&self, session_id: &str, status: &HarnessStatus) {
        let Some(readiness) = status.tunnel_readiness.as_ref() else {
            return;
        };
        if readiness.control_plane_connected {
            self.startup
                .complete_phase(session_id, StartupPhase::ControlPlaneConnected);
            self.startup
                .start_phase(session_id, StartupPhase::McpTargetReady);
            if readiness.mcp_target_reachable {
                self.startup
                    .complete_phase(session_id, StartupPhase::McpTargetReady);
            }
        } else {
            self.startup
                .start_phase(session_id, StartupPhase::ControlPlaneConnected);
        }
        if status.lifecycle == HarnessLifecycle::Ready
            && readiness.control_plane_connected
            && readiness.mcp_target_reachable
        {
            self.startup
                .start_phase(session_id, StartupPhase::HarnessReady);
            self.startup.ready(session_id);
        }
    }

    fn on_tunnel_event(
        &self,
        generation: u64,
        instance_id: Option<&str>,
        state: TunnelState,
        health: TunnelHealth,
        readiness: Option<&TunnelReadinessEvidence>,
        recovery: &TunnelRecoveryStatus,
        error: Option<&TunnelProcessError>,
    ) {
        let mut changed = false;
        let mut accepted_session = None;
        if let Ok(mut inner) = self.shared.inner.lock() {
            if inner.tunnel_generation != Some(generation) {
                return;
            }
            if state == TunnelState::Starting {
                if let Some(instance_id) = instance_id {
                    inner.tunnel_instance = Some(instance_id.to_owned());
                }
            }
            // A failed spawn has no process instance. Only the current generation's
            // pending start may relinquish its provisional instance identity.
            if instance_id.is_none()
                && state == TunnelState::Failed
                && inner.status.tunnel == HarnessComponentStatus::Starting
            {
                inner.tunnel_instance = None;
            }
            if inner.tunnel_instance.as_deref() != instance_id {
                return;
            }
            let previous = inner.status.clone();
            match state {
                TunnelState::Ready => {
                    inner.status.tunnel = HarnessComponentStatus::Ready;
                    if inner.status.runtime == HarnessComponentStatus::Ready
                        && inner.status.mcp == HarnessComponentStatus::Ready
                    {
                        inner.status.lifecycle = HarnessLifecycle::Ready;
                    }
                    inner.status.last_error = None;
                }
                TunnelState::Degraded => {
                    inner.status.tunnel = HarnessComponentStatus::Degraded;
                    if inner.status.lifecycle == HarnessLifecycle::Ready {
                        inner.status.lifecycle = HarnessLifecycle::Degraded;
                    }
                    inner.status.last_error = error.map(error_summary_from_tunnel);
                }
                TunnelState::Failed => {
                    inner.status.tunnel = HarnessComponentStatus::Error;
                    inner.status.lifecycle = HarnessLifecycle::Error;
                    inner.status.last_error = error.map(error_summary_from_tunnel);
                }
                TunnelState::Stopping => inner.status.tunnel = HarnessComponentStatus::Stopping,
                TunnelState::Stopped => inner.status.tunnel = HarnessComponentStatus::Stopped,
                TunnelState::Starting => inner.status.tunnel = HarnessComponentStatus::Starting,
            }
            inner.status.tunnel_readiness = readiness.cloned();
            inner.status.tunnel_health = health;
            inner.status.tunnel_recovery = recovery.clone();
            accepted_session = inner.status.active_session_id.clone();
            changed = harness_status_event_changed(&previous, &inner.status);
        }
        if let Some(session_id) = accepted_session.as_deref() {
            if state == TunnelState::Failed {
                if let Some(error) = error {
                    self.startup.fail_active(session_id, &error.code);
                } else {
                    self.startup.fail_active(session_id, "TUNNEL_FAILED");
                }
            }
            let status = self.status();
            self.observe_startup_readiness(session_id, &status);
        }
        if changed {
            let status = self.status();
            self.emit_status(&status);
        }
    }
}

impl RuntimeEventSink for HarnessController {
    fn runtime_event(
        &self,
        generation: Uuid,
        phase: RuntimeProcessPhase,
        error: Option<&SupervisorError>,
    ) {
        self.on_runtime_event(generation, phase, error);
    }
}

impl crate::runtime::RuntimeStartupSink for HarnessController {
    fn runtime_startup_event(
        &self,
        generation: Uuid,
        phase: crate::runtime::RuntimeStartupPhase,
        error: Option<&SupervisorError>,
    ) {
        self.on_runtime_startup(generation, phase, error);
    }
}

impl TunnelEventSink for HarnessController {
    fn tunnel_event(
        &self,
        generation: u64,
        instance_id: Option<&str>,
        state: TunnelState,
        health: TunnelHealth,
        readiness: Option<&TunnelReadinessEvidence>,
        recovery: &TunnelRecoveryStatus,
        error: Option<&TunnelProcessError>,
    ) {
        self.on_tunnel_event(
            generation,
            instance_id,
            state,
            health,
            readiness,
            recovery,
            error,
        );
    }
}

fn stopped_status() -> HarnessStatus {
    HarnessStatus {
        lifecycle: HarnessLifecycle::Stopped,
        runtime: HarnessComponentStatus::Stopped,
        mcp: HarnessComponentStatus::Stopped,
        tunnel: HarnessComponentStatus::Stopped,
        active_session_id: None,
        config_revision: None,
        last_error: None,
        tunnel_health: TunnelHealth::stopped(),
        tunnel_readiness: None,
        tunnel_recovery: TunnelRecoveryStatus::default(),
    }
}

fn apply_tunnel_start_snapshot_if_pending(
    inner: &mut HarnessInner,
    instance_id: &str,
    tunnel_status: &TunnelStatus,
) {
    if inner.tunnel_instance.as_deref() != Some(instance_id)
        || inner.status.tunnel != HarnessComponentStatus::Starting
    {
        return;
    }
    inner.tunnel_generation = Some(tunnel_status.generation);
    inner.status.tunnel = match tunnel_status.state {
        TunnelState::Ready => HarnessComponentStatus::Ready,
        TunnelState::Degraded => HarnessComponentStatus::Degraded,
        TunnelState::Failed => HarnessComponentStatus::Error,
        _ => HarnessComponentStatus::Starting,
    };
    inner.status.tunnel_readiness = tunnel_status.readiness.clone();
    inner.status.tunnel_health = tunnel_status.health;
    inner.status.tunnel_recovery = tunnel_status.recovery.clone();
    inner.status.last_error = tunnel_status
        .last_error
        .as_ref()
        .map(error_summary_from_tunnel);
    inner.status.lifecycle = match inner.status.tunnel {
        HarnessComponentStatus::Ready => HarnessLifecycle::Ready,
        HarnessComponentStatus::Degraded => HarnessLifecycle::Degraded,
        HarnessComponentStatus::Error => HarnessLifecycle::Error,
        _ => HarnessLifecycle::Starting,
    };
}

fn harness_status_event_changed(previous: &HarnessStatus, next: &HarnessStatus) -> bool {
    previous.lifecycle != next.lifecycle
        || previous.runtime != next.runtime
        || previous.mcp != next.mcp
        || previous.tunnel != next.tunnel
        || previous.tunnel_health != next.tunnel_health
        || previous.tunnel_recovery != next.tunnel_recovery
        || previous.active_session_id != next.active_session_id
        || previous.config_revision != next.config_revision
        || !harness_error_event_equivalent(previous.last_error.as_ref(), next.last_error.as_ref())
        || !readiness_event_equivalent(
            previous.tunnel_readiness.as_ref(),
            next.tunnel_readiness.as_ref(),
        )
}

fn harness_error_event_equivalent(
    previous: Option<&HarnessErrorSummary>,
    next: Option<&HarnessErrorSummary>,
) -> bool {
    match (previous, next) {
        (None, None) => true,
        (Some(previous), Some(next)) => {
            previous.code == next.code
                && control_plane_error_event_equivalent(
                    Some(previous.message.as_str()),
                    Some(next.message.as_str()),
                )
        }
        _ => false,
    }
}

fn readiness_event_equivalent(
    previous: Option<&TunnelReadinessEvidence>,
    next: Option<&TunnelReadinessEvidence>,
) -> bool {
    match (previous, next) {
        (None, None) => true,
        (Some(previous), Some(next)) => {
            previous.control_plane_connected == next.control_plane_connected
                && previous.mcp_target_reachable == next.mcp_target_reachable
                && previous.health_check_passed == next.health_check_passed
                && previous.control_plane_route == next.control_plane_route
                && control_plane_error_event_equivalent(
                    previous.control_plane_error.as_deref(),
                    next.control_plane_error.as_deref(),
                )
                && previous.last_successful_poll_timestamp == next.last_successful_poll_timestamp
        }
        _ => false,
    }
}

fn control_plane_error_event_equivalent(previous: Option<&str>, next: Option<&str>) -> bool {
    const STALE_PREFIX: &str = "Control-plane poll has been stale for ";
    match (previous, next) {
        (Some(previous), Some(next))
            if previous.starts_with(STALE_PREFIX) && next.starts_with(STALE_PREFIX) =>
        {
            true
        }
        _ => previous == next,
    }
}

fn eligibility(status: StartEligibilityStatus, message: impl Into<String>) -> StartEligibility {
    let can_start = matches!(status, StartEligibilityStatus::ReadyToStart);
    StartEligibility {
        status,
        can_start,
        message: message.into(),
    }
}

fn validate_harness_config(config: &AppConfig) -> Result<(), String> {
    if config.schema_version != CURRENT_SCHEMA_VERSION {
        return Err("Unsupported configuration schema version.".into());
    }
    if config.workspace.root_path.trim().is_empty()
        || !crate::platform::is_valid_directory(&config.workspace.root_path)
    {
        return Err("Choose an accessible Workspace folder.".into());
    }
    config
        .history
        .validate()
        .map_err(|error| format!("History configuration needs attention: {error}"))?;
    let security_consistent = (matches!(config.security.mode, SecurityMode::Normal)
        && !config.security.developer_test_mode)
        || (matches!(config.security.mode, SecurityMode::DeveloperTest)
            && config.security.developer_test_mode);
    if !security_consistent {
        return Err("Security mode fields must be consistent.".into());
    }
    Ok(())
}

fn load_config(app: &AppHandle) -> Result<Option<AppConfig>, crate::commands::ControlError> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| crate::commands::ControlError {
            code: "PATH_ERROR".into(),
            message: error.to_string(),
        })?;
    ConfigStore::new(directory)
        .load()
        .map_err(|error| crate::commands::ControlError {
            code: error.code().into(),
            message: error.message(),
        })
}

fn lifecycle_error(message: impl Into<String>) -> crate::commands::ControlError {
    crate::commands::ControlError {
        code: "HARNESS_LIFECYCLE_FAILED".into(),
        message: message.into(),
    }
}

fn secret_error(message: impl Into<String>) -> crate::commands::ControlError {
    crate::commands::ControlError {
        code: "SECRET_STORE_FAILED".into(),
        message: message.into(),
    }
}

fn control_error_from_tunnel(error: TunnelProcessError) -> crate::commands::ControlError {
    crate::commands::ControlError {
        code: error.code,
        message: error.message,
    }
}

fn error_summary(error: &crate::commands::ControlError) -> HarnessErrorSummary {
    HarnessErrorSummary {
        code: error.code.clone(),
        message: error.message.clone(),
    }
}

fn error_summary_from_runtime(error: Option<&SupervisorError>) -> HarnessErrorSummary {
    error
        .map(|error| HarnessErrorSummary {
            code: error.code.clone(),
            message: error.message.clone(),
        })
        .unwrap_or_else(|| HarnessErrorSummary {
            code: "RUNTIME_CRASHED".into(),
            message: "Runtime exited unexpectedly.".into(),
        })
}

fn error_summary_from_tunnel(error: &TunnelProcessError) -> HarnessErrorSummary {
    HarnessErrorSummary {
        code: error.code.clone(),
        message: error.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::TunnelHealthState;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn controller() -> HarnessController {
        HarnessController::new(RuntimeSupervisor::new(), TunnelProcessManager::new())
    }

    #[test]
    fn concurrent_lifecycle_operation_returns_conflict_and_can_retry() {
        let controller = controller();
        let held = controller.lock_operation().expect("first operation");
        assert_eq!(
            controller.lock_operation().unwrap_err().code,
            "LIFECYCLE_CONFLICT"
        );
        drop(held);
        assert!(
            controller.lock_operation().is_ok(),
            "retry after operation completes"
        );
    }

    #[test]
    fn status_sink_can_reenter_controller_without_deadlock() {
        let controller = controller();
        let nested = controller.clone();
        controller.set_status_sink(Arc::new(move |_| {
            // Replacing a sink is a harmless Host-side reconfiguration, but
            // it must not self-deadlock when emit_status is dispatching.
            nested.set_status_sink(Arc::new(|_| {}));
        }));
        let status = controller.status();
        let dispatch = controller.clone();
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        let dispatch_thread = std::thread::spawn(move || {
            dispatch.emit_status(&status);
            let _ = done_sender.send(());
        });
        assert!(
            done_receiver
                .recv_timeout(std::time::Duration::from_millis(250))
                .is_ok(),
            "Harness status sink callback deadlocked on status_sink"
        );
        dispatch_thread.join().expect("status dispatch thread");
    }

    fn readiness(
        control_plane_connected: bool,
        timestamp: f64,
        age_seconds: f64,
        error: Option<&str>,
    ) -> TunnelReadinessEvidence {
        TunnelReadinessEvidence {
            control_plane_connected,
            mcp_target_reachable: true,
            health_check_passed: true,
            control_plane_route: None,
            control_plane_error: error.map(str::to_owned),
            last_successful_poll_timestamp: Some(timestamp),
            last_successful_poll_age_seconds: Some(age_seconds),
        }
    }

    #[test]
    fn stale_runtime_generation_cannot_change_a_new_session() {
        let controller = controller();
        let active_generation = Uuid::new_v4();
        let stale_generation = Uuid::new_v4();
        controller.update(|inner| {
            inner.runtime_generation = Some(active_generation);
            inner.status.lifecycle = HarnessLifecycle::Ready;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Ready;
        });

        controller.on_runtime_event(stale_generation, RuntimeProcessPhase::Crashed, None);

        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Ready);
        assert_eq!(controller.status().runtime, HarnessComponentStatus::Ready);
    }

    #[test]
    fn failed_recovery_spawn_without_instance_exposes_blocked_status() {
        use crate::tunnel::TunnelRecoveryPhase;
        let controller = controller();
        controller.update(|inner| {
            inner.tunnel_generation = Some(7);
            inner.tunnel_instance = Some("old-instance".into());
        });
        controller.on_tunnel_event(
            7,
            Some("pending-instance"),
            TunnelState::Starting,
            TunnelHealth::starting(),
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );
        let blocked = TunnelRecoveryStatus {
            phase: TunnelRecoveryPhase::Blocked,
            reason: Some(TunnelProcessError {
                code: "TUNNEL_EXECUTABLE_NOT_FOUND".into(),
                message: "Executable unavailable".into(),
            }),
            next_attempt_at: None,
            cooldown_until: None,
        };
        // A failed spawn must never leave Harness stuck at Starting.
        controller.on_tunnel_event(
            7,
            None,
            TunnelState::Failed,
            TunnelHealth::failed(),
            None,
            &blocked,
            blocked.reason.as_ref(),
        );
        assert_eq!(controller.status().tunnel, HarnessComponentStatus::Error);
        assert_eq!(controller.status().tunnel_recovery, blocked);
        controller.on_tunnel_event(
            7,
            Some("old-instance"),
            TunnelState::Ready,
            TunnelHealth::starting(),
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );
        assert_eq!(controller.status().tunnel_recovery, blocked);
    }

    #[test]
    fn recovery_only_events_expose_cooldown_blocked_and_next_attempt() {
        use crate::tunnel::TunnelRecoveryPhase;
        let controller = controller();
        controller.update(|inner| {
            inner.tunnel_generation = Some(7);
            inner.tunnel_instance = Some("exited-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Error;
            inner.status.tunnel = HarnessComponentStatus::Error;
            inner.status.tunnel_health = TunnelHealth::failed();
        });
        let emitted = Arc::new(AtomicUsize::new(0));
        let count = emitted.clone();
        controller.set_status_sink(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        }));
        let mut recovery = TunnelRecoveryStatus {
            phase: TunnelRecoveryPhase::Cooldown,
            reason: Some(TunnelProcessError {
                code: "TUNNEL_RECOVERY_COOLDOWN".into(),
                message: "Cooling down".into(),
            }),
            next_attempt_at: Some("2026-10-23T09:10:00Z".into()),
            cooldown_until: Some("2026-10-23T09:10:00Z".into()),
        };
        controller.on_tunnel_event(
            7,
            Some("exited-instance"),
            TunnelState::Failed,
            TunnelHealth::failed(),
            None,
            &recovery,
            None,
        );
        assert_eq!(controller.status().tunnel_recovery, recovery);
        assert_eq!(emitted.load(Ordering::SeqCst), 1);
        let json = serde_json::to_value(controller.status()).unwrap();
        assert_eq!(json["tunnelRecovery"]["phase"], "cooldown");
        assert_eq!(
            json["tunnelRecovery"]["nextAttemptAt"],
            "2026-10-23T09:10:00Z"
        );
        recovery.phase = TunnelRecoveryPhase::HalfOpen;
        controller.on_tunnel_event(
            7,
            Some("exited-instance"),
            TunnelState::Failed,
            TunnelHealth::failed(),
            None,
            &recovery,
            None,
        );
        assert_eq!(emitted.load(Ordering::SeqCst), 2);
        recovery.phase = TunnelRecoveryPhase::Blocked;
        recovery.reason = Some(TunnelProcessError {
            code: "CONTROL_PLANE_UNAUTHORIZED".into(),
            message: "401 Unauthorized".into(),
        });
        recovery.next_attempt_at = None;
        recovery.cooldown_until = None;
        controller.on_tunnel_event(
            7,
            Some("exited-instance"),
            TunnelState::Failed,
            TunnelHealth::failed(),
            None,
            &recovery,
            None,
        );
        assert_eq!(controller.status().tunnel_recovery, recovery);
        assert_eq!(emitted.load(Ordering::SeqCst), 3);
        controller.on_tunnel_event(
            6,
            Some("exited-instance"),
            TunnelState::Failed,
            TunnelHealth::failed(),
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );
        controller.on_tunnel_event(
            7,
            Some("stale-instance"),
            TunnelState::Failed,
            TunnelHealth::failed(),
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );
        assert_eq!(controller.status().tunnel_recovery, recovery);
        assert_eq!(emitted.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn stale_tunnel_instance_cannot_change_a_new_session() {
        let controller = controller();
        controller.update(|inner| {
            inner.tunnel_generation = Some(1);
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Ready;
            inner.status.tunnel = HarnessComponentStatus::Ready;
        });

        controller.on_tunnel_event(
            1,
            Some("stale-instance"),
            TunnelState::Failed,
            TunnelHealth::starting(),
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );

        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Ready);
        assert_eq!(controller.status().tunnel, HarnessComponentStatus::Ready);
    }

    #[test]
    fn supervised_tunnel_generation_adopts_new_instance_and_rejects_old_events() {
        let controller = controller();
        controller.update(|inner| {
            inner.tunnel_generation = Some(7);
            inner.tunnel_instance = Some("old-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Ready;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Ready;
        });

        controller.on_tunnel_event(
            7,
            Some("new-instance"),
            TunnelState::Starting,
            TunnelHealth::starting(),
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );
        assert_eq!(controller.status().tunnel, HarnessComponentStatus::Starting);
        assert_eq!(
            controller
                .shared
                .inner
                .lock()
                .unwrap()
                .tunnel_instance
                .as_deref(),
            Some("new-instance")
        );

        let ready_health = TunnelHealth {
            process: TunnelHealthState::Ready,
            local_mcp: TunnelHealthState::Ready,
            control_plane: TunnelHealthState::Ready,
        };
        controller.on_tunnel_event(
            7,
            Some("new-instance"),
            TunnelState::Ready,
            ready_health,
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );
        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Ready);

        controller.on_tunnel_event(
            7,
            Some("old-instance"),
            TunnelState::Failed,
            TunnelHealth::failed(),
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );
        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Ready);
    }

    #[test]
    fn harness_ready_requires_runtime_mcp_and_tunnel_ready() {
        let controller = controller();
        controller.update(|inner| {
            inner.tunnel_generation = Some(1);
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Starting;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Starting;
        });
        controller.on_tunnel_event(
            1,
            Some("active-instance"),
            TunnelState::Ready,
            TunnelHealth::starting(),
            None,
            &TunnelRecoveryStatus::default(),
            None,
        );

        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Ready);
    }

    #[test]
    fn active_tunnel_recovers_harness_from_degraded_to_ready() {
        let controller = controller();
        controller.update(|inner| {
            inner.tunnel_generation = Some(1);
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Degraded;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Degraded;
        });

        let stale = TunnelReadinessEvidence {
            control_plane_connected: false,
            mcp_target_reachable: true,
            health_check_passed: true,
            control_plane_route: None,
            control_plane_error: Some("Control-plane poll has been stale for 91 seconds.".into()),
            last_successful_poll_timestamp: Some(1_799_999_909.0),
            last_successful_poll_age_seconds: Some(91.0),
        };
        let degraded_health = TunnelHealth {
            process: TunnelHealthState::Ready,
            local_mcp: TunnelHealthState::Ready,
            control_plane: TunnelHealthState::Degraded,
        };
        controller.on_tunnel_event(
            1,
            Some("active-instance"),
            TunnelState::Degraded,
            degraded_health,
            Some(&stale),
            &TunnelRecoveryStatus::default(),
            None,
        );
        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Degraded);
        assert_eq!(controller.status().tunnel_health, degraded_health);

        let fresh = TunnelReadinessEvidence {
            control_plane_connected: true,
            mcp_target_reachable: true,
            health_check_passed: true,
            control_plane_route: None,
            control_plane_error: None,
            last_successful_poll_timestamp: Some(1_799_999_990.0),
            last_successful_poll_age_seconds: Some(10.0),
        };
        let ready_health = TunnelHealth {
            process: TunnelHealthState::Ready,
            local_mcp: TunnelHealthState::Ready,
            control_plane: TunnelHealthState::Ready,
        };
        controller.on_tunnel_event(
            1,
            Some("active-instance"),
            TunnelState::Ready,
            ready_health,
            Some(&fresh),
            &TunnelRecoveryStatus::default(),
            None,
        );

        let status = controller.status();
        assert_eq!(status.lifecycle, HarnessLifecycle::Ready);
        assert_eq!(status.tunnel, HarnessComponentStatus::Ready);
        assert_eq!(status.tunnel_health, ready_health);
        assert_eq!(status.tunnel_readiness, Some(fresh));
    }

    #[test]
    fn age_only_readiness_updates_do_not_emit_status() {
        let controller = controller();
        let initial = readiness(true, 1_800_000_000.0, 10.0, None);
        let health = TunnelHealth {
            process: TunnelHealthState::Ready,
            local_mcp: TunnelHealthState::Ready,
            control_plane: TunnelHealthState::Ready,
        };
        controller.update(|inner| {
            inner.tunnel_generation = Some(1);
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Ready;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Ready;
            inner.status.tunnel_health = health;
            inner.status.tunnel_readiness = Some(initial);
        });
        let emitted = Arc::new(AtomicUsize::new(0));
        let emitted_by_sink = emitted.clone();
        controller.set_status_sink(Arc::new(move |_| {
            emitted_by_sink.fetch_add(1, Ordering::SeqCst);
        }));

        let aged = readiness(true, 1_800_000_000.0, 10.5, None);
        controller.on_tunnel_event(
            1,
            Some("active-instance"),
            TunnelState::Ready,
            health,
            Some(&aged),
            &TunnelRecoveryStatus::default(),
            None,
        );

        assert_eq!(emitted.load(Ordering::SeqCst), 0);
        assert_eq!(
            controller
                .status()
                .tunnel_readiness
                .unwrap()
                .last_successful_poll_age_seconds,
            Some(10.5)
        );
    }

    #[test]
    fn stale_age_message_updates_do_not_emit_status() {
        let controller = controller();
        let initial = readiness(
            false,
            1_800_000_000.0,
            91.0,
            Some("Control-plane poll has been stale for 91 seconds."),
        );
        let health = TunnelHealth {
            process: TunnelHealthState::Ready,
            local_mcp: TunnelHealthState::Ready,
            control_plane: TunnelHealthState::Degraded,
        };
        controller.update(|inner| {
            inner.tunnel_generation = Some(1);
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Degraded;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Degraded;
            inner.status.tunnel_health = health;
            inner.status.tunnel_readiness = Some(initial);
            inner.status.last_error = Some(HarnessErrorSummary {
                code: "CONTROL_PLANE_UNAVAILABLE".into(),
                message: "Control-plane poll has been stale for 91 seconds.".into(),
            });
        });
        let emitted = Arc::new(AtomicUsize::new(0));
        let emitted_by_sink = emitted.clone();
        controller.set_status_sink(Arc::new(move |_| {
            emitted_by_sink.fetch_add(1, Ordering::SeqCst);
        }));
        let error = TunnelProcessError {
            code: "CONTROL_PLANE_UNAVAILABLE".into(),
            message: "Control-plane poll has been stale for 92 seconds.".into(),
        };
        let aged = readiness(
            false,
            1_800_000_000.0,
            92.0,
            Some("Control-plane poll has been stale for 92 seconds."),
        );

        controller.on_tunnel_event(
            1,
            Some("active-instance"),
            TunnelState::Degraded,
            health,
            Some(&aged),
            &TunnelRecoveryStatus::default(),
            Some(&error),
        );

        assert_eq!(emitted.load(Ordering::SeqCst), 0);
        assert_eq!(
            controller
                .status()
                .tunnel_readiness
                .unwrap()
                .last_successful_poll_age_seconds,
            Some(92.0)
        );
    }

    #[test]
    fn startup_snapshot_does_not_overwrite_a_newer_tunnel_event() {
        let mut inner = HarnessInner {
            status: HarnessStatus {
                lifecycle: HarnessLifecycle::Ready,
                runtime: HarnessComponentStatus::Ready,
                mcp: HarnessComponentStatus::Ready,
                tunnel: HarnessComponentStatus::Ready,
                active_session_id: Some("session".into()),
                config_revision: Some("1".into()),
                last_error: None,
                tunnel_health: TunnelHealth {
                    process: TunnelHealthState::Ready,
                    local_mcp: TunnelHealthState::Ready,
                    control_plane: TunnelHealthState::Ready,
                },
                tunnel_readiness: Some(readiness(true, 1_800_000_010.0, 1.0, None)),
                tunnel_recovery: TunnelRecoveryStatus::default(),
            },
            runtime_generation: None,
            tunnel_generation: Some(1),
            tunnel_instance: Some("active-instance".into()),
            startup_seen: true,
        };
        let stale_snapshot = TunnelStatus {
            recovery: TunnelRecoveryStatus::default(),
            state: TunnelState::Degraded,
            instance: None,
            generation: 1,
            health: TunnelHealth {
                process: TunnelHealthState::Ready,
                local_mcp: TunnelHealthState::Ready,
                control_plane: TunnelHealthState::Degraded,
            },
            readiness: Some(readiness(
                false,
                1_799_999_900.0,
                100.0,
                Some("Control-plane poll has been stale for 100 seconds."),
            )),
            last_exit: None,
            last_error: Some(TunnelProcessError {
                code: "CONTROL_PLANE_UNAVAILABLE".into(),
                message: "Control-plane poll has been stale for 100 seconds.".into(),
            }),
        };

        apply_tunnel_start_snapshot_if_pending(&mut inner, "active-instance", &stale_snapshot);

        assert_eq!(inner.status.lifecycle, HarnessLifecycle::Ready);
        assert_eq!(inner.status.tunnel, HarnessComponentStatus::Ready);
        assert!(inner
            .status
            .tunnel_readiness
            .as_ref()
            .is_some_and(|evidence| evidence.control_plane_connected));
    }
}
