use crate::{
    config::{ConfigStore, CURRENT_SCHEMA_VERSION},
    runtime::{RuntimeEventSink, RuntimeProcessPhase, RuntimeSupervisor, SupervisorError},
    tunnel::{
        resolve_tunnel, TunnelEventSink, TunnelLaunch, TunnelProcessError, TunnelProcessManager,
        TunnelReadinessEvidence, TunnelState, TunnelStatus,
    },
};
use harness_control_protocol::{AppConfig, SecurityMode, StartEligibility, StartEligibilityStatus};
use serde::Serialize;
use std::sync::{Arc, Mutex};
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
    pub tunnel_readiness: Option<TunnelReadinessEvidence>,
}

struct HarnessInner {
    status: HarnessStatus,
    runtime_generation: Option<Uuid>,
    tunnel_instance: Option<String>,
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
}

impl Clone for HarnessController {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            runtime: self.runtime.clone(),
            tunnel: self.tunnel.clone(),
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
                    tunnel_instance: None,
                }),
                operation: Mutex::new(()),
                status_sink: Mutex::new(None),
            }),
            runtime,
            tunnel,
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
        self.shared
            .inner
            .lock()
            .map(|inner| inner.status.clone())
            .unwrap_or_else(|_| stopped_status())
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
        let _operation = self
            .shared
            .operation
            .lock()
            .map_err(|_| lifecycle_error("Harness lifecycle state is unavailable."))?;
        self.attach_app(app);
        self.start_locked(app, secrets)
    }

    pub fn stop(&self, app: &AppHandle) -> Result<HarnessStatus, crate::commands::ControlError> {
        let _operation = self
            .shared
            .operation
            .lock()
            .map_err(|_| lifecycle_error("Harness lifecycle state is unavailable."))?;
        self.attach_app(app);
        self.stop_locked(app)
    }

    pub fn restart(
        &self,
        app: &AppHandle,
        secrets: &Mutex<crate::commands::BackendState>,
    ) -> Result<HarnessStatus, crate::commands::ControlError> {
        let _operation = self
            .shared
            .operation
            .lock()
            .map_err(|_| lifecycle_error("Harness lifecycle state is unavailable."))?;
        self.attach_app(app);
        let stopped = self.stop_locked(app)?;
        if stopped.lifecycle != HarnessLifecycle::Stopped {
            return Err(lifecycle_error(
                "Harness could not complete shutdown before restart.",
            ));
        }
        self.start_locked(app, secrets)
    }

    fn start_locked(
        &self,
        app: &AppHandle,
        secrets: &Mutex<crate::commands::BackendState>,
    ) -> Result<HarnessStatus, crate::commands::ControlError> {
        let eligibility = self.start_eligibility(app, secrets)?;
        if !eligibility.can_start {
            return Err(lifecycle_error(eligibility.message));
        }
        let config = load_config(app)?
            .ok_or_else(|| lifecycle_error("Save configuration before starting Harness."))?;
        let session_id = Uuid::new_v4().to_string();
        self.update(|inner| {
            inner.status = HarnessStatus {
                lifecycle: HarnessLifecycle::Starting,
                runtime: HarnessComponentStatus::Starting,
                mcp: HarnessComponentStatus::Starting,
                tunnel: HarnessComponentStatus::Starting,
                active_session_id: Some(session_id.clone()),
                config_revision: Some(config.config_revision.get().to_string()),
                last_error: None,
                tunnel_readiness: None,
            };
            inner.runtime_generation = None;
            inner.tunnel_instance = None;
        });

        let runtime_state = match self.runtime.start(app, &config) {
            Ok(state) => state,
            Err(error) => return Err(self.fail_start(app, error.into(), false)),
        };
        let runtime_generation = self.runtime.active_generation();
        let endpoint = match self.runtime.mcp_endpoint() {
            Ok(endpoint) => endpoint,
            Err(error) => return Err(self.fail_start(app, error.into(), true)),
        };
        self.update(|inner| {
            inner.runtime_generation = runtime_generation;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
        });

        let resolved = match resolve_tunnel(&config) {
            Ok(value) => value,
            Err(error) => return Err(self.fail_start(app, control_error_from_tunnel(error), true)),
        };
        let reference = config
            .tunnel
            .api_key_ref
            .as_ref()
            .ok_or_else(|| lifecycle_error("Tunnel API Key is unavailable."));
        let reference = match reference {
            Ok(reference) => reference,
            Err(error) => return Err(self.fail_start(app, error, true)),
        };
        let api_key = match secrets
            .lock()
            .map_err(|_| secret_error("Secret store unavailable."))?
            .secrets
            .get(&reference.key)
            .map_err(secret_error)?
        {
            Some(value) => value,
            None => {
                return Err(self.fail_start(
                    app,
                    lifecycle_error("Tunnel API Key is unavailable."),
                    true,
                ))
            }
        };
        let launch = TunnelLaunch::new(resolved.client_path, endpoint)
            .with_tunnel_id(config.tunnel.tunnel_id.clone())
            .with_control_plane_http_proxy(config.tunnel.control_plane_http_proxy.clone())
            .with_cloudflared_path(resolved.cloudflared_path)
            .with_api_key(api_key);
        let tunnel_status = match self.tunnel.start(launch) {
            Ok(status) => status,
            Err(error) => return Err(self.fail_start(app, control_error_from_tunnel(error), true)),
        };
        let Some(instance) = tunnel_status.instance.as_ref() else {
            return Err(self.fail_start(
                app,
                lifecycle_error("Tunnel did not return a managed instance."),
                true,
            ));
        };
        let instance_id = instance.instance_id.clone();
        self.update(|inner| {
            inner.tunnel_instance = Some(instance_id.clone());
        });
        let tunnel_status = self.tunnel.status().unwrap_or(tunnel_status);
        self.update(|inner| {
            apply_tunnel_start_snapshot_if_pending(inner, &instance_id, &tunnel_status);
        });
        let status = self.status();
        let _ = runtime_state;
        self.emit_status(&status);
        Ok(status)
    }

    fn stop_locked(&self, app: &AppHandle) -> Result<HarnessStatus, crate::commands::ControlError> {
        let current = self.status();
        if current.lifecycle == HarnessLifecycle::Stopped && current.active_session_id.is_none() {
            return Ok(current);
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
            inner.tunnel_instance = None;
            inner.status.active_session_id = None;
            inner.status.config_revision = None;
            if let Some(error) = &error {
                inner.status.lifecycle = HarnessLifecycle::Error;
                inner.status.runtime = HarnessComponentStatus::Error;
                inner.status.mcp = HarnessComponentStatus::Error;
                inner.status.tunnel = HarnessComponentStatus::Error;
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
        let _ = self.tunnel.stop();
        if runtime_started {
            let _ = self.runtime.stop(app);
        }
        self.update(|inner| {
            inner.runtime_generation = None;
            inner.tunnel_instance = None;
            inner.status.lifecycle = HarnessLifecycle::Error;
            inner.status.runtime = HarnessComponentStatus::Stopped;
            inner.status.mcp = HarnessComponentStatus::Stopped;
            inner.status.tunnel = HarnessComponentStatus::Stopped;
            inner.status.last_error = Some(error_summary(&error));
        });
        let status = self.status();
        self.emit_status(&status);
        error
    }

    fn update(&self, update: impl FnOnce(&mut HarnessInner)) {
        if let Ok(mut inner) = self.shared.inner.lock() {
            update(&mut inner);
        }
    }

    fn emit_status(&self, status: &HarnessStatus) {
        if let Ok(sink) = self.shared.status_sink.lock() {
            if let Some(sink) = sink.as_ref() {
                sink(status);
            }
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

    fn on_tunnel_event(
        &self,
        instance_id: Option<&str>,
        state: TunnelState,
        readiness: Option<&TunnelReadinessEvidence>,
        error: Option<&TunnelProcessError>,
    ) {
        let mut changed = false;
        if let Ok(mut inner) = self.shared.inner.lock() {
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
            changed = harness_status_event_changed(&previous, &inner.status);
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

impl TunnelEventSink for HarnessController {
    fn tunnel_event(
        &self,
        instance_id: Option<&str>,
        state: TunnelState,
        readiness: Option<&TunnelReadinessEvidence>,
        error: Option<&TunnelProcessError>,
    ) {
        self.on_tunnel_event(instance_id, state, readiness, error);
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
        tunnel_readiness: None,
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
    inner.status.tunnel = match tunnel_status.state {
        TunnelState::Ready => HarnessComponentStatus::Ready,
        TunnelState::Degraded => HarnessComponentStatus::Degraded,
        TunnelState::Failed => HarnessComponentStatus::Error,
        _ => HarnessComponentStatus::Starting,
    };
    inner.status.tunnel_readiness = tunnel_status.readiness.clone();
    inner.status.last_error = tunnel_status
        .last_error
        .as_ref()
        .map(error_summary_from_tunnel);
    inner.status.lifecycle = if inner.status.tunnel == HarnessComponentStatus::Ready {
        HarnessLifecycle::Ready
    } else {
        HarnessLifecycle::Degraded
    };
}

fn harness_status_event_changed(previous: &HarnessStatus, next: &HarnessStatus) -> bool {
    previous.lifecycle != next.lifecycle
        || previous.runtime != next.runtime
        || previous.mcp != next.mcp
        || previous.tunnel != next.tunnel
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn controller() -> HarnessController {
        HarnessController::new(RuntimeSupervisor::new(), TunnelProcessManager::new())
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
    fn stale_tunnel_instance_cannot_change_a_new_session() {
        let controller = controller();
        controller.update(|inner| {
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Ready;
            inner.status.tunnel = HarnessComponentStatus::Ready;
        });

        controller.on_tunnel_event(Some("stale-instance"), TunnelState::Failed, None, None);

        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Ready);
        assert_eq!(controller.status().tunnel, HarnessComponentStatus::Ready);
    }

    #[test]
    fn harness_ready_requires_runtime_mcp_and_tunnel_ready() {
        let controller = controller();
        controller.update(|inner| {
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Starting;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Starting;
        });
        controller.on_tunnel_event(Some("active-instance"), TunnelState::Ready, None, None);

        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Ready);
    }

    #[test]
    fn active_tunnel_recovers_harness_from_degraded_to_ready() {
        let controller = controller();
        controller.update(|inner| {
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
        controller.on_tunnel_event(
            Some("active-instance"),
            TunnelState::Degraded,
            Some(&stale),
            None,
        );
        assert_eq!(controller.status().lifecycle, HarnessLifecycle::Degraded);

        let fresh = TunnelReadinessEvidence {
            control_plane_connected: true,
            mcp_target_reachable: true,
            health_check_passed: true,
            control_plane_route: None,
            control_plane_error: None,
            last_successful_poll_timestamp: Some(1_799_999_990.0),
            last_successful_poll_age_seconds: Some(10.0),
        };
        controller.on_tunnel_event(
            Some("active-instance"),
            TunnelState::Ready,
            Some(&fresh),
            None,
        );

        let status = controller.status();
        assert_eq!(status.lifecycle, HarnessLifecycle::Ready);
        assert_eq!(status.tunnel, HarnessComponentStatus::Ready);
        assert_eq!(status.tunnel_readiness, Some(fresh));
    }

    #[test]
    fn age_only_readiness_updates_do_not_emit_status() {
        let controller = controller();
        let initial = readiness(true, 1_800_000_000.0, 10.0, None);
        controller.update(|inner| {
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Ready;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Ready;
            inner.status.tunnel_readiness = Some(initial);
        });
        let emitted = Arc::new(AtomicUsize::new(0));
        let emitted_by_sink = emitted.clone();
        controller.set_status_sink(Arc::new(move |_| {
            emitted_by_sink.fetch_add(1, Ordering::SeqCst);
        }));

        let aged = readiness(true, 1_800_000_000.0, 10.5, None);
        controller.on_tunnel_event(
            Some("active-instance"),
            TunnelState::Ready,
            Some(&aged),
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
        controller.update(|inner| {
            inner.tunnel_instance = Some("active-instance".into());
            inner.status.lifecycle = HarnessLifecycle::Degraded;
            inner.status.runtime = HarnessComponentStatus::Ready;
            inner.status.mcp = HarnessComponentStatus::Ready;
            inner.status.tunnel = HarnessComponentStatus::Degraded;
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
            Some("active-instance"),
            TunnelState::Degraded,
            Some(&aged),
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
                tunnel_readiness: Some(readiness(true, 1_800_000_010.0, 1.0, None)),
            },
            runtime_generation: None,
            tunnel_instance: Some("active-instance".into()),
        };
        let stale_snapshot = TunnelStatus {
            state: TunnelState::Degraded,
            instance: None,
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
