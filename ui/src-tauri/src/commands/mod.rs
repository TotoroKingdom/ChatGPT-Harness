mod control_read;
pub use control_read::*;

use crate::{
    config::{ConfigError, ConfigMetadata, ConfigStore, CURRENT_SCHEMA_VERSION},
    harness::{HarnessController, HarnessStatus},
    runtime::{RuntimeProcessStatus, RuntimeSupervisor, SupervisorError},
    secrets::{api_key_ref, SecretStore},
    tunnel::TunnelProcessError,
};
use chrono::{SecondsFormat, Utc};
use harness_control_protocol::{
    ActivityListParams, ActivityListResult, ActivitySubscribeParams, ActivitySubscribeResult,
    AppConfig, ApprovalsListParams, ApprovalsListResult, ApprovalsResolveParams,
    ApprovalsResolveResult, DiagnosticCheck, DiagnosticRun, DiagnosticStatus, PoliciesListResult,
    RuntimeState, RuntimeStopResult, SecurityMode, StartEligibility, StartEligibilityStatus,
    ToolsListResult,
};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Serialize)]
pub struct ControlError {
    pub code: String,
    pub message: String,
}

impl From<SupervisorError> for ControlError {
    fn from(error: SupervisorError) -> Self {
        Self {
            code: error.code,
            message: error.message,
        }
    }
}
impl From<ConfigError> for ControlError {
    fn from(error: ConfigError) -> Self {
        Self {
            code: error.code().into(),
            message: error.message(),
        }
    }
}

impl From<TunnelProcessError> for ControlError {
    fn from(error: TunnelProcessError) -> Self {
        Self {
            code: error.code,
            message: error.message,
        }
    }
}

fn store(app: &AppHandle) -> Result<ConfigStore, ControlError> {
    let directory = app.path().app_data_dir().map_err(|error| ControlError {
        code: "PATH_ERROR".into(),
        message: error.to_string(),
    })?;
    Ok(ConfigStore::new(directory))
}

fn secret_error(message: impl Into<String>) -> ControlError {
    ControlError {
        code: "SECRET_STORE_FAILED".into(),
        message: message.into(),
    }
}

pub struct BackendState {
    pub secrets: Box<dyn SecretStore>,
}

#[tauri::command]
pub fn config_get(app: AppHandle) -> Result<Option<AppConfig>, ControlError> {
    store(&app)?
        .load_or_create_defaults()
        .map(Some)
        .map_err(Into::into)
}

#[tauri::command]
pub fn config_metadata(app: AppHandle) -> Result<ConfigMetadata, ControlError> {
    Ok(store(&app)?.metadata())
}

#[tauri::command]
pub fn config_save(
    app: AppHandle,
    state: State<'_, Mutex<BackendState>>,
    mut config: AppConfig,
    api_key: Option<String>,
) -> Result<AppConfig, ControlError> {
    if config.schema_version == 0 {
        config.schema_version = CURRENT_SCHEMA_VERSION;
    }
    if let Some(path) = config.tunnel.tunnel_client_path.as_mut() {
        let normalized = crate::platform::normalize_tunnel_client_directory(path);
        *path = normalized.display().to_string();
    }
    crate::config::normalize_and_validate_control_plane_http_proxy(
        &mut config.tunnel.control_plane_http_proxy,
    )
    .map_err(ControlError::from)?;
    // Validate the existing on-disk configuration before validating or persisting
    // a replacement. A corrupt config must remain untouched and must not cause
    // a partial save that changes the machine-local credential state.
    let config_store = store(&app)?;
    config_store.load().map_err(ControlError::from)?;
    validate_config_for_save(&config)?;
    if let Some(secret) = api_key.filter(|value| !value.trim().is_empty()) {
        let reference = api_key_ref();
        state
            .lock()
            .map_err(|_| secret_error("Secret store unavailable."))?
            .secrets
            .set(&reference.key, &secret)
            .map_err(secret_error)?;
        config.tunnel.api_key_ref = Some(reference);
    }
    config_store.save(config).map_err(Into::into)
}

#[tauri::command]
pub fn config_reset(
    app: AppHandle,
    state: State<'_, Mutex<BackendState>>,
) -> Result<(), ControlError> {
    let config_store = store(&app)?;
    let api_key = match config_store.load() {
        Ok(Some(config)) => config.tunnel.api_key_ref.map(|reference| reference.key),
        Ok(None) => None,
        Err(ConfigError::Invalid(_)) => Some(api_key_ref().key),
        Err(error) => return Err(error.into()),
    };
    if let Some(key) = api_key {
        state
            .lock()
            .map_err(|_| secret_error("Secret store unavailable."))?
            .secrets
            .delete(&key)
            .map_err(secret_error)?;
    }
    config_store.reset().map_err(Into::into)
}

#[tauri::command]
pub fn secret_api_key_status(
    app: AppHandle,
    state: State<'_, Mutex<BackendState>>,
) -> Result<bool, ControlError> {
    let config = store(&app)?.load().map_err(ControlError::from)?;
    let key = config
        .and_then(|value| value.tunnel.api_key_ref)
        .unwrap_or_else(api_key_ref)
        .key;
    state
        .lock()
        .map_err(|_| secret_error("Secret store unavailable."))?
        .secrets
        .exists(&key)
        .map_err(secret_error)
}

#[tauri::command]
pub fn secret_api_key_delete(
    app: AppHandle,
    state: State<'_, Mutex<BackendState>>,
) -> Result<(), ControlError> {
    let key = store(&app)?
        .load()
        .ok()
        .flatten()
        .and_then(|value| value.tunnel.api_key_ref)
        .unwrap_or_else(api_key_ref)
        .key;
    state
        .lock()
        .map_err(|_| secret_error("Secret store unavailable."))?
        .secrets
        .delete(&key)
        .map_err(secret_error)
}

#[tauri::command]
pub fn workspace_choose(app: AppHandle) -> Result<Option<String>, ControlError> {
    Ok(app
        .dialog()
        .file()
        .set_title("Choose workspace folder")
        .blocking_pick_folder()
        .and_then(|path| path.into_path().ok())
        .map(|path| path.display().to_string()))
}

#[tauri::command]
pub fn tunnel_client_choose(app: AppHandle) -> Result<Option<String>, ControlError> {
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Choose tunnel-client folder")
        .blocking_pick_folder()
        .and_then(|path| path.into_path().ok())
    else {
        return Ok(None);
    };
    crate::platform::resolve_tunnel_client_executable(&path).map_err(|message| ControlError {
        code: "TUNNEL_CLIENT_NOT_FOUND".into(),
        message: message.into(),
    })?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub fn tunnel_client_detect() -> Option<String> {
    crate::platform::find_tunnel_client()
}

#[tauri::command]
pub fn validate_path(path: String, file: bool) -> bool {
    if file {
        crate::platform::is_valid_file(&path)
    } else {
        crate::platform::is_valid_directory(&path)
    }
}

#[tauri::command]
pub fn runtime_start(
    app: AppHandle,
    runtime: State<'_, RuntimeSupervisor>,
) -> Result<RuntimeState, ControlError> {
    let config = store(&app)?.load()?.ok_or(ControlError {
        code: "CONFIG_NOT_FOUND".into(),
        message: "Save configuration before starting the Runtime.".into(),
    })?;
    validate_config_for_local_start(&config)?;
    runtime.start(&app, &config).map_err(Into::into)
}

#[tauri::command]
pub fn runtime_start_eligibility(
    app: AppHandle,
    runtime: State<'_, RuntimeSupervisor>,
) -> Result<StartEligibility, ControlError> {
    let process = runtime.process_status();
    if !matches!(
        process.phase,
        crate::runtime::RuntimeProcessPhase::Stopped | crate::runtime::RuntimeProcessPhase::Crashed
    ) {
        return Ok(StartEligibility {
            status: StartEligibilityStatus::RuntimeAlreadyRunning,
            can_start: false,
            message: "Local Runtime is already running or changing state.".into(),
        });
    }
    let config = match store(&app)?.load() {
        Ok(Some(config)) => config,
        Ok(None) => {
            return Ok(StartEligibility {
                status: StartEligibilityStatus::SetupRequired,
                can_start: false,
                message: "Choose a Workspace to finish local setup.".into(),
            })
        }
        Err(ConfigError::Invalid(error)) => {
            return Ok(StartEligibility {
                status: StartEligibilityStatus::ConfigurationInvalid,
                can_start: false,
                message: format!("Configuration needs attention: {error}"),
            })
        }
        Err(error) => return Err(error.into()),
    };
    if let Err(error) = validate_config_for_local_start(&config) {
        let status = if error.code == "WORKSPACE_INVALID" {
            StartEligibilityStatus::SetupRequired
        } else {
            StartEligibilityStatus::ConfigurationInvalid
        };
        return Ok(StartEligibility {
            status,
            can_start: false,
            message: error.message,
        });
    }
    if let Err(error) = crate::runtime::discovery::resolve_runtime() {
        return Ok(StartEligibility {
            status: StartEligibilityStatus::RuntimeDistributionInvalid,
            can_start: false,
            message: error.message,
        });
    }
    Ok(StartEligibility {
        status: StartEligibilityStatus::ReadyToStart,
        can_start: true,
        message: "Local Runtime is ready to start.".into(),
    })
}

#[tauri::command]
pub fn runtime_status(runtime: State<'_, RuntimeSupervisor>) -> Result<RuntimeState, ControlError> {
    runtime.status().map_err(Into::into)
}

#[tauri::command]
pub fn runtime_process_status(runtime: State<'_, RuntimeSupervisor>) -> RuntimeProcessStatus {
    runtime.process_status()
}

#[tauri::command]
pub fn runtime_stop(
    app: AppHandle,
    runtime: State<'_, RuntimeSupervisor>,
) -> Result<RuntimeStopResult, ControlError> {
    runtime.stop(&app).map_err(Into::into)
}

#[tauri::command]
pub fn runtime_tools_list(
    runtime: State<'_, RuntimeSupervisor>,
) -> Result<ToolsListResult, ControlError> {
    runtime.tools_list().map_err(Into::into)
}

#[tauri::command]
pub fn runtime_policies_list(
    runtime: State<'_, RuntimeSupervisor>,
) -> Result<PoliciesListResult, ControlError> {
    runtime.policies_list().map_err(Into::into)
}

#[tauri::command]
pub fn runtime_activity_list(
    runtime: State<'_, RuntimeSupervisor>,
    params: Option<ActivityListParams>,
) -> Result<ActivityListResult, ControlError> {
    runtime
        .activity_list(params.unwrap_or_default())
        .map_err(Into::into)
}

#[tauri::command]
pub fn runtime_activity_subscribe(
    runtime: State<'_, RuntimeSupervisor>,
    params: Option<ActivitySubscribeParams>,
) -> Result<ActivitySubscribeResult, ControlError> {
    runtime
        .activity_subscribe(params.unwrap_or_default())
        .map_err(Into::into)
}

#[tauri::command]
pub fn runtime_approvals_list(
    runtime: State<'_, RuntimeSupervisor>,
    params: Option<ApprovalsListParams>,
) -> Result<ApprovalsListResult, ControlError> {
    runtime
        .approvals_list(params.unwrap_or_default())
        .map_err(Into::into)
}

#[tauri::command]
pub fn runtime_approvals_resolve(
    runtime: State<'_, RuntimeSupervisor>,
    params: ApprovalsResolveParams,
) -> Result<ApprovalsResolveResult, ControlError> {
    runtime.approvals_resolve(params).map_err(Into::into)
}

#[tauri::command]
pub fn runtime_diagnostics_run(
    runtime: State<'_, RuntimeSupervisor>,
) -> Result<DiagnosticRun, ControlError> {
    runtime.diagnostics().map_err(Into::into)
}

#[tauri::command]
pub fn local_diagnostics_run(
    app: AppHandle,
    state: State<'_, Mutex<BackendState>>,
) -> Result<DiagnosticRun, ControlError> {
    let config = store(&app)?
        .load()
        .map_err(ControlError::from)?
        .ok_or(ControlError {
            code: "CONFIG_NOT_FOUND".into(),
            message: "Save configuration before running diagnostics.".into(),
        })?;
    let mut checks = Vec::new();
    let workspace_ok = crate::platform::is_valid_directory(&config.workspace.root_path);
    checks.push(DiagnosticCheck {
        id: "workspace".into(),
        status: if workspace_ok {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        summary: if workspace_ok {
            "Workspace folder is accessible.".into()
        } else {
            "Workspace folder is missing or inaccessible.".into()
        },
        remediation: (!workspace_ok).then(|| "Choose an accessible folder in Settings.".into()),
    });
    let tunnel_ok = !config.tunnel.tunnel_id.trim().is_empty();
    checks.push(DiagnosticCheck {
        id: "tunnel_id".into(),
        status: if tunnel_ok {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Warning
        },
        summary: if tunnel_ok {
            "Tunnel ID is configured.".into()
        } else {
            "External connection is not configured (optional for Local Runtime).".into()
        },
        remediation: (!tunnel_ok)
            .then(|| "Configure an external connection in Settings when needed.".into()),
    });
    let client_ok = config
        .tunnel
        .tunnel_client_path
        .as_deref()
        .map(std::path::Path::new)
        .and_then(|path| crate::platform::resolve_tunnel_client_executable(path).ok())
        .is_some();
    checks.push(DiagnosticCheck {
        id: "tunnel_client".into(),
        status: if client_ok {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Warning
        },
        summary: if client_ok {
            "External tunnel client found.".into()
        } else {
            "External tunnel client is not configured (optional for Local Runtime).".into()
        },
        remediation: (!client_ok).then(|| {
            "Choose a tunnel-client package folder when external connection is needed.".into()
        }),
    });
    let api_key_ok = if let Some(reference) = config.tunnel.api_key_ref {
        state
            .lock()
            .map_err(|_| secret_error("Secret store unavailable."))?
            .secrets
            .exists(&reference.key)
            .unwrap_or(false)
    } else {
        false
    };
    checks.push(DiagnosticCheck {
        id: "api_key".into(),
        status: if api_key_ok {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Warning
        },
        summary: if api_key_ok {
            "API Key is configured in the OS credential store.".into()
        } else {
            "API Key is not configured (optional for Local Runtime).".into()
        },
        remediation: (!api_key_ok)
            .then(|| "Save an API Key when external connection is needed.".into()),
    });
    let security_ok = (matches!(config.security.mode, SecurityMode::Normal)
        && !config.security.developer_test_mode)
        || (matches!(config.security.mode, SecurityMode::DeveloperTest)
            && config.security.developer_test_mode);
    checks.push(DiagnosticCheck {
        id: "security".into(),
        status: if security_ok {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        summary: if security_ok {
            "Security settings are consistent.".into()
        } else {
            "Security mode fields are inconsistent.".into()
        },
        remediation: None,
    });
    let overall_status = if checks
        .iter()
        .any(|check| matches!(check.status, DiagnosticStatus::Fail))
    {
        DiagnosticStatus::Fail
    } else if checks
        .iter()
        .any(|check| matches!(check.status, DiagnosticStatus::Warning))
    {
        DiagnosticStatus::Warning
    } else {
        DiagnosticStatus::Pass
    };
    let timestamp = current_timestamp();
    Ok(DiagnosticRun {
        run_id: "diagnostic_local".into(),
        started_at: timestamp.clone(),
        finished_at: timestamp,
        overall_status,
        checks,
    })
}

fn validate_config_for_save(config: &AppConfig) -> Result<(), ControlError> {
    if config.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(ControlError {
            code: "CONFIG_INVALID".into(),
            message: "Unsupported configuration schema version.".into(),
        });
    }
    if config.workspace.root_path.trim().is_empty()
        || !crate::platform::is_valid_directory(&config.workspace.root_path)
    {
        return Err(ControlError {
            code: "WORKSPACE_INVALID".into(),
            message: "Choose an accessible Workspace folder.".into(),
        });
    }
    validate_security(config)
}

fn validate_config_for_local_start(config: &AppConfig) -> Result<(), ControlError> {
    validate_config_for_save(config)
}

fn validate_security(config: &AppConfig) -> Result<(), ControlError> {
    let security_consistent = (matches!(config.security.mode, SecurityMode::Normal)
        && !config.security.developer_test_mode)
        || (matches!(config.security.mode, SecurityMode::DeveloperTest)
            && config.security.developer_test_mode);
    if !security_consistent {
        return Err(ControlError {
            code: "CONFIG_INVALID".into(),
            message: "Security mode fields must be consistent.".into(),
        });
    }
    Ok(())
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[tauri::command]
pub fn harness_start_eligibility(
    app: AppHandle,
    controller: State<'_, HarnessController>,
    state: State<'_, Mutex<BackendState>>,
) -> Result<StartEligibility, ControlError> {
    controller.start_eligibility(&app, &state)
}

#[tauri::command]
pub fn harness_start(
    app: AppHandle,
    controller: State<'_, HarnessController>,
    state: State<'_, Mutex<BackendState>>,
) -> Result<HarnessStatus, ControlError> {
    controller.start(&app, &state)
}

#[tauri::command]
pub fn harness_status(controller: State<'_, HarnessController>) -> HarnessStatus {
    controller.status()
}

#[tauri::command]
pub fn harness_stop(
    app: AppHandle,
    controller: State<'_, HarnessController>,
) -> Result<HarnessStatus, ControlError> {
    controller.stop(&app)
}

#[tauri::command]
pub fn harness_restart(
    app: AppHandle,
    controller: State<'_, HarnessController>,
    state: State<'_, Mutex<BackendState>>,
) -> Result<HarnessStatus, ControlError> {
    controller.restart(&app, &state)
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use harness_control_protocol::{
        SecurityConfig, ToolConfig, TunnelConfig, UiConfig, WorkspaceConfig,
    };

    fn valid_config(directory: &std::path::Path) -> AppConfig {
        AppConfig {
            schema_version: CURRENT_SCHEMA_VERSION,
            config_revision: harness_control_protocol::DecimalU64::ZERO,
            workspace: WorkspaceConfig {
                root_path: directory.display().to_string(),
            },
            tunnel: TunnelConfig {
                tunnel_id: "tunnel".into(),
                control_plane_http_proxy: None,
                tunnel_client_path: Some(directory.display().to_string()),
                auto_detect: false,
                api_key_ref: None,
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            ui: UiConfig::default(),
            tools: ToolConfig::default(),
        }
    }

    #[test]
    fn local_start_requires_workspace_and_consistent_security_but_not_external_connection() {
        let directory = tempfile::tempdir().unwrap();
        let client = directory.path().join("tunnel-client.exe");
        std::fs::write(&client, b"mock").unwrap();
        let config = valid_config(directory.path());
        assert!(validate_config_for_local_start(&config).is_ok());

        let mut empty_tunnel = config.clone();
        empty_tunnel.tunnel.tunnel_id.clear();
        assert!(validate_config_for_local_start(&empty_tunnel).is_ok());

        let mut inconsistent_security = config.clone();
        inconsistent_security.security.developer_test_mode = true;
        assert_eq!(
            validate_config_for_local_start(&inconsistent_security)
                .unwrap_err()
                .code,
            "CONFIG_INVALID"
        );

        let mut missing_workspace = config;
        missing_workspace.workspace.root_path =
            directory.path().join("missing").display().to_string();
        assert_eq!(
            validate_config_for_local_start(&missing_workspace)
                .unwrap_err()
                .code,
            "WORKSPACE_INVALID"
        );
    }
}
