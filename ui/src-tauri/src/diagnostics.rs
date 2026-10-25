use crate::{
    commands::BackendState,
    config::{ConfigError, ConfigStore},
    harness::{HarnessController, HarnessLifecycle, HarnessStatus},
    platform,
    runtime::{RuntimeProcessPhase, RuntimeSupervisor},
    tunnel::{resolve_tunnel, TunnelProcessManager, TunnelState, TunnelStatus},
};
use chrono::{SecondsFormat, Utc};
use harness_control_protocol::{
    AppConfig, DiagnosticCheck, DiagnosticEvidence, DiagnosticEvidenceKey, DiagnosticRemediation,
    DiagnosticRemediationTarget, DiagnosticRun, DiagnosticScope, DiagnosticStatus,
};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const MAX_SAFE_VALUE_CHARS: usize = 128;

/// Host-owned, read-only aggregation boundary for connection diagnostics.
///
/// This type only observes existing authorities. It never calls lifecycle
/// mutation, ConfigStore save/reset, SecretStore get/set/delete, or filesystem
/// mutation APIs.
pub(crate) struct DiagnosticsCoordinator<'a> {
    app: &'a AppHandle,
    secrets: &'a Mutex<BackendState>,
    runtime: &'a RuntimeSupervisor,
    tunnel: &'a TunnelProcessManager,
    harness: &'a HarnessController,
}

impl<'a> DiagnosticsCoordinator<'a> {
    pub(crate) fn new(
        app: &'a AppHandle,
        secrets: &'a Mutex<BackendState>,
        runtime: &'a RuntimeSupervisor,
        tunnel: &'a TunnelProcessManager,
        harness: &'a HarnessController,
    ) -> Self {
        Self {
            app,
            secrets,
            runtime,
            tunnel,
            harness,
        }
    }

    pub(crate) fn run(&self) -> DiagnosticRun {
        let started_at = timestamp();
        let config_observation = self.read_config();
        let config = config_observation
            .as_ref()
            .ok()
            .and_then(|value| value.clone());
        let config_check = config_check(&config_observation);
        let workspace = workspace_check(config.as_ref());
        let runtime_distribution = runtime_distribution_check();
        let process_status = self.runtime.process_status();
        let runtime_process = runtime_process_check(process_status.clone());
        let runtime_state = if matches!(
            process_status.phase,
            RuntimeProcessPhase::Running | RuntimeProcessPhase::Crashed
        ) {
            self.runtime.status().ok()
        } else {
            None
        };
        let runtime_readiness =
            runtime_readiness_check(process_status.phase, runtime_state.as_ref());
        let mcp_endpoint = mcp_endpoint_check(self.runtime);

        let tunnel_distribution = tunnel_distribution_check(config.as_ref());
        let tunnel_observation = self.tunnel.status();
        let tunnel_process = tunnel_process_check(&tunnel_observation);
        let tunnel_readiness = tunnel_readiness_check(&tunnel_observation);
        let control_plane = control_plane_check(&tunnel_observation);
        let mcp_target = mcp_target_check(&tunnel_observation);
        let proxy_config = proxy_config_check(config.as_ref(), config_observation.as_ref().err());
        let proxy_route = proxy_route_check(&tunnel_observation);
        let credential_presence = credential_presence_check(config.as_ref(), self.secrets);
        let credential_state = credential_state_check(&tunnel_observation);
        let lifecycle = lifecycle_check(&self.harness.status());

        let checks = vec![
            config_check,
            workspace,
            runtime_distribution,
            runtime_process,
            runtime_readiness,
            mcp_endpoint,
            tunnel_distribution,
            tunnel_process,
            tunnel_readiness,
            control_plane,
            mcp_target,
            proxy_config,
            proxy_route,
            credential_presence,
            credential_state,
            lifecycle,
        ];
        let finished_at = timestamp();
        DiagnosticRun {
            run_id: format!("diagnostic_{}", Uuid::new_v4()),
            started_at,
            finished_at,
            overall_status: overall_status(&checks),
            checks,
        }
    }

    fn read_config(&self) -> Result<Option<AppConfig>, ConfigError> {
        let directory = self.app.path().app_data_dir().map_err(|error| {
            ConfigError::Io(format!("diagnostic config path unavailable: {error}"))
        })?;
        ConfigStore::new(directory).inspect_read_only()
    }
}

fn config_check(observation: &Result<Option<AppConfig>, ConfigError>) -> DiagnosticCheck {
    match observation {
        Ok(Some(config)) => check(
            "config",
            DiagnosticScope::Config,
            DiagnosticStatus::Pass,
            "Current configuration is valid.",
            vec![
                evidence(DiagnosticEvidenceKey::Configured, "true"),
                evidence(
                    DiagnosticEvidenceKey::Version,
                    config.schema_version.to_string(),
                ),
            ],
            None,
        ),
        Ok(None) => check(
            "config",
            DiagnosticScope::Config,
            DiagnosticStatus::Fail,
            "Configuration is not available.",
            vec![evidence(DiagnosticEvidenceKey::Configured, "false")],
            Some(remediation(
                "config_open_settings",
                "Open Settings to create a valid configuration.",
                DiagnosticRemediationTarget::SettingsWorkspace,
            )),
        ),
        Err(error) => check(
            "config",
            DiagnosticScope::Config,
            DiagnosticStatus::Fail,
            "Configuration could not be validated.",
            vec![evidence(DiagnosticEvidenceKey::ErrorCode, error.code())],
            Some(remediation(
                "config_repair",
                "Review the configuration in Settings.",
                DiagnosticRemediationTarget::SettingsWorkspace,
            )),
        ),
    }
}

fn workspace_check(config: Option<&AppConfig>) -> DiagnosticCheck {
    let Some(config) = config else {
        return check(
            "workspace",
            DiagnosticScope::Workspace,
            DiagnosticStatus::Unknown,
            "Workspace cannot be checked until configuration is available.",
            Vec::new(),
            None,
        );
    };
    let exists = platform::is_valid_directory(&config.workspace.root_path);
    check(
        "workspace",
        DiagnosticScope::Workspace,
        if exists {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        if exists {
            "Workspace is accessible."
        } else {
            "Workspace is missing or inaccessible."
        },
        vec![evidence(DiagnosticEvidenceKey::Exists, exists.to_string())],
        (!exists).then(|| {
            remediation(
                "workspace_choose",
                "Choose an accessible Workspace in Settings.",
                DiagnosticRemediationTarget::SettingsWorkspace,
            )
        }),
    )
}

fn runtime_distribution_check() -> DiagnosticCheck {
    match crate::runtime::discovery::resolve_runtime() {
        Ok(resolved) => check(
            "runtime_distribution",
            DiagnosticScope::Runtime,
            DiagnosticStatus::Pass,
            "Runtime distribution is available.",
            vec![evidence(
                DiagnosticEvidenceKey::Version,
                resolved.expected_runtime_version(),
            )],
            None,
        ),
        Err(error) => check(
            "runtime_distribution",
            DiagnosticScope::Runtime,
            DiagnosticStatus::Fail,
            "Runtime distribution is unavailable or incompatible.",
            vec![evidence(DiagnosticEvidenceKey::ErrorCode, &error.code)],
            Some(remediation(
                "runtime_distribution_repair",
                "Repair or reinstall the Runtime distribution.",
                DiagnosticRemediationTarget::Overview,
            )),
        ),
    }
}

fn runtime_process_check(status: crate::runtime::RuntimeProcessStatus) -> DiagnosticCheck {
    let (phase, summary, result) = match status.phase {
        RuntimeProcessPhase::Running => (
            "running",
            "Runtime process is running.",
            DiagnosticStatus::Pass,
        ),
        RuntimeProcessPhase::Stopped => (
            "stopped",
            "Runtime process is intentionally stopped.",
            DiagnosticStatus::Warning,
        ),
        RuntimeProcessPhase::Crashed => (
            "crashed",
            "Runtime process has crashed.",
            DiagnosticStatus::Fail,
        ),
        RuntimeProcessPhase::Starting => (
            "starting",
            "Runtime process is starting.",
            DiagnosticStatus::Unknown,
        ),
        RuntimeProcessPhase::Stopping => (
            "stopping",
            "Runtime process is stopping.",
            DiagnosticStatus::Unknown,
        ),
    };
    let mut evidence_items = vec![evidence(DiagnosticEvidenceKey::ProcessPhase, phase)];
    if let Some(error) = status.last_error.as_ref() {
        evidence_items.push(evidence(DiagnosticEvidenceKey::ErrorCode, &error.code));
    }
    check(
        "runtime_process",
        DiagnosticScope::Runtime,
        result,
        summary,
        evidence_items,
        None,
    )
}

fn runtime_readiness_check(
    process: RuntimeProcessPhase,
    state: Option<&harness_control_protocol::RuntimeState>,
) -> DiagnosticCheck {
    let Some(state) = state else {
        return check(
            "runtime_readiness",
            DiagnosticScope::Runtime,
            if matches!(process, RuntimeProcessPhase::Crashed) {
                DiagnosticStatus::Fail
            } else {
                DiagnosticStatus::Unknown
            },
            "Runtime readiness is not currently observable.",
            Vec::new(),
            None,
        );
    };
    let (result, summary) = match state.readiness {
        harness_control_protocol::RuntimeReadiness::Ready => (
            DiagnosticStatus::Pass,
            "Runtime readiness is authoritative and ready.",
        ),
        harness_control_protocol::RuntimeReadiness::Degraded => (
            DiagnosticStatus::Warning,
            "Runtime is ready with degraded health.",
        ),
        harness_control_protocol::RuntimeReadiness::Initializing
        | harness_control_protocol::RuntimeReadiness::NotReady => (
            DiagnosticStatus::Unknown,
            "Runtime readiness is not yet confirmed.",
        ),
    };
    check(
        "runtime_readiness",
        DiagnosticScope::Runtime,
        result,
        summary,
        vec![evidence(
            DiagnosticEvidenceKey::Readiness,
            format!("{:?}", state.readiness).to_ascii_lowercase(),
        )],
        None,
    )
}

fn mcp_endpoint_check(runtime: &RuntimeSupervisor) -> DiagnosticCheck {
    match runtime.mcp_endpoint() {
        Ok(_) => check(
            "mcp_endpoint",
            DiagnosticScope::Mcp,
            DiagnosticStatus::Pass,
            "The current MCP endpoint is available.",
            vec![evidence(DiagnosticEvidenceKey::EndpointReachable, "true")],
            None,
        ),
        Err(_error) if runtime.process_status().phase == RuntimeProcessPhase::Stopped => check(
            "mcp_endpoint",
            DiagnosticScope::Mcp,
            DiagnosticStatus::Unknown,
            "MCP endpoint cannot be checked while Runtime is stopped.",
            Vec::new(),
            None,
        ),
        Err(error) => check(
            "mcp_endpoint",
            DiagnosticScope::Mcp,
            DiagnosticStatus::Fail,
            "Runtime is running but the MCP endpoint is unavailable.",
            vec![evidence(DiagnosticEvidenceKey::ErrorCode, &error.code)],
            Some(remediation(
                "mcp_endpoint_retry",
                "Retry diagnostics after Runtime initialization completes.",
                DiagnosticRemediationTarget::Overview,
            )),
        ),
    }
}

fn tunnel_distribution_check(config: Option<&AppConfig>) -> DiagnosticCheck {
    let Some(config) = config else {
        return check(
            "tunnel_distribution",
            DiagnosticScope::Tunnel,
            DiagnosticStatus::Unknown,
            "Tunnel distribution cannot be checked until configuration is available.",
            Vec::new(),
            None,
        );
    };
    match resolve_tunnel(config) {
        Ok(resolved) => check(
            "tunnel_distribution",
            DiagnosticScope::Tunnel,
            DiagnosticStatus::Pass,
            "Tunnel distribution is available.",
            vec![evidence(DiagnosticEvidenceKey::Version, resolved.version)],
            None,
        ),
        Err(error) => check(
            "tunnel_distribution",
            DiagnosticScope::Tunnel,
            DiagnosticStatus::Fail,
            "Tunnel distribution is unavailable or incompatible.",
            vec![evidence(DiagnosticEvidenceKey::ErrorCode, &error.code)],
            Some(remediation(
                "tunnel_distribution_repair",
                "Repair or choose a compatible Tunnel distribution.",
                DiagnosticRemediationTarget::SettingsConnection,
            )),
        ),
    }
}

fn tunnel_process_check(
    observation: &Result<TunnelStatus, crate::tunnel::TunnelProcessError>,
) -> DiagnosticCheck {
    let Ok(status) = observation else {
        let error = observation.as_ref().err().expect("error branch");
        return check(
            "tunnel_process",
            DiagnosticScope::Tunnel,
            DiagnosticStatus::Unknown,
            "Tunnel process state is unavailable.",
            vec![evidence(DiagnosticEvidenceKey::ErrorCode, &error.code)],
            None,
        );
    };
    let (result, summary, phase) = match status.state {
        TunnelState::Ready | TunnelState::Degraded => (
            DiagnosticStatus::Pass,
            "A managed Tunnel process is present.",
            "ready",
        ),
        TunnelState::Stopped => (
            DiagnosticStatus::Warning,
            "Tunnel process is intentionally stopped.",
            "stopped",
        ),
        TunnelState::Failed => (
            DiagnosticStatus::Fail,
            "Tunnel process has failed.",
            "failed",
        ),
        TunnelState::Starting => (
            DiagnosticStatus::Unknown,
            "Tunnel process is starting.",
            "starting",
        ),
        TunnelState::Stopping => (
            DiagnosticStatus::Unknown,
            "Tunnel process is stopping.",
            "stopping",
        ),
    };
    check(
        "tunnel_process",
        DiagnosticScope::Tunnel,
        result,
        summary,
        vec![evidence(DiagnosticEvidenceKey::ProcessPhase, phase)],
        None,
    )
}

fn tunnel_readiness_check(
    observation: &Result<TunnelStatus, crate::tunnel::TunnelProcessError>,
) -> DiagnosticCheck {
    let Ok(status) = observation else {
        return check(
            "tunnel_readiness",
            DiagnosticScope::Tunnel,
            DiagnosticStatus::Unknown,
            "Tunnel readiness is unavailable.",
            Vec::new(),
            None,
        );
    };
    let Some(readiness) = status.readiness.as_ref() else {
        return check(
            "tunnel_readiness",
            DiagnosticScope::Tunnel,
            if matches!(status.state, TunnelState::Failed) {
                DiagnosticStatus::Fail
            } else {
                DiagnosticStatus::Unknown
            },
            "No current Tunnel readiness evidence is available.",
            Vec::new(),
            None,
        );
    };
    let ready = readiness.control_plane_connected
        && readiness.mcp_target_reachable
        && readiness.health_check_passed;
    let age = readiness
        .last_successful_poll_age_seconds
        .map(|value| format!("{value:.0}"))
        .unwrap_or_else(|| "unknown".into());
    check(
        "tunnel_readiness",
        DiagnosticScope::Tunnel,
        if ready {
            DiagnosticStatus::Pass
        } else if matches!(status.state, TunnelState::Failed | TunnelState::Degraded) {
            DiagnosticStatus::Fail
        } else {
            DiagnosticStatus::Unknown
        },
        if ready {
            "Current Tunnel readiness evidence is healthy."
        } else {
            "Current Tunnel readiness evidence is incomplete or unhealthy."
        },
        vec![
            evidence(
                DiagnosticEvidenceKey::Readiness,
                if ready { "ready" } else { "not_ready" },
            ),
            evidence(DiagnosticEvidenceKey::LastSuccessAgeSeconds, age),
        ],
        None,
    )
}

fn control_plane_check(
    observation: &Result<TunnelStatus, crate::tunnel::TunnelProcessError>,
) -> DiagnosticCheck {
    let Ok(status) = observation else {
        return check(
            "control_plane",
            DiagnosticScope::ControlPlane,
            DiagnosticStatus::Unknown,
            "Control-plane state is unavailable.",
            Vec::new(),
            None,
        );
    };
    let Some(readiness) = status.readiness.as_ref() else {
        return check(
            "control_plane",
            DiagnosticScope::ControlPlane,
            DiagnosticStatus::Unknown,
            "No live control-plane evidence is available.",
            Vec::new(),
            None,
        );
    };
    let mut evidence_items = vec![evidence(
        DiagnosticEvidenceKey::Configured,
        readiness.control_plane_connected.to_string(),
    )];
    if let Some(route) = readiness.control_plane_route.as_ref() {
        evidence_items.push(evidence(
            DiagnosticEvidenceKey::RouteMode,
            &route.route_mode,
        ));
        evidence_items.push(evidence(
            DiagnosticEvidenceKey::ProxySource,
            &route.proxy_source,
        ));
    }
    check(
        "control_plane",
        DiagnosticScope::ControlPlane,
        if readiness.control_plane_connected {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        if readiness.control_plane_connected {
            "Control plane is connected with current evidence."
        } else {
            "Control plane is disconnected or stale."
        },
        evidence_items,
        (!readiness.control_plane_connected).then(|| {
            remediation(
                "control_plane_retry",
                "Retry the connection after checking Tunnel settings.",
                DiagnosticRemediationTarget::SettingsConnection,
            )
        }),
    )
}

fn mcp_target_check(
    observation: &Result<TunnelStatus, crate::tunnel::TunnelProcessError>,
) -> DiagnosticCheck {
    let Ok(status) = observation else {
        return check(
            "mcp_target",
            DiagnosticScope::Mcp,
            DiagnosticStatus::Unknown,
            "MCP target reachability is unavailable.",
            Vec::new(),
            None,
        );
    };
    let Some(readiness) = status.readiness.as_ref() else {
        return check(
            "mcp_target",
            DiagnosticScope::Mcp,
            DiagnosticStatus::Unknown,
            "No live Tunnel evidence is available for the MCP target.",
            Vec::new(),
            None,
        );
    };
    let result = if !readiness.control_plane_connected {
        DiagnosticStatus::Unknown
    } else if readiness.mcp_target_reachable {
        DiagnosticStatus::Pass
    } else {
        DiagnosticStatus::Fail
    };
    let summary = match result {
        DiagnosticStatus::Pass => "MCP target is reachable.",
        DiagnosticStatus::Fail => "MCP target is unreachable.",
        _ => "MCP target cannot be confirmed without a connected control plane.",
    };
    check(
        "mcp_target",
        DiagnosticScope::Mcp,
        result,
        summary,
        vec![evidence(
            DiagnosticEvidenceKey::EndpointReachable,
            readiness.mcp_target_reachable.to_string(),
        )],
        None,
    )
}

fn proxy_config_check(config: Option<&AppConfig>, error: Option<&ConfigError>) -> DiagnosticCheck {
    if let Some(error) = error {
        return check(
            "proxy_config",
            DiagnosticScope::Proxy,
            DiagnosticStatus::Unknown,
            "Proxy configuration cannot be checked while configuration is invalid.",
            vec![evidence(DiagnosticEvidenceKey::ErrorCode, error.code())],
            None,
        );
    }
    let Some(config) = config else {
        return check(
            "proxy_config",
            DiagnosticScope::Proxy,
            DiagnosticStatus::Unknown,
            "Proxy configuration is unavailable.",
            Vec::new(),
            None,
        );
    };
    check(
        "proxy_config",
        DiagnosticScope::Proxy,
        DiagnosticStatus::Pass,
        if config.tunnel.control_plane_http_proxy.is_some() {
            "A valid proxy configuration is present."
        } else {
            "Direct control-plane routing is configured."
        },
        vec![evidence(
            DiagnosticEvidenceKey::Configured,
            config.tunnel.control_plane_http_proxy.is_some().to_string(),
        )],
        None,
    )
}

fn proxy_route_check(
    observation: &Result<TunnelStatus, crate::tunnel::TunnelProcessError>,
) -> DiagnosticCheck {
    let Ok(status) = observation else {
        return check(
            "proxy_route",
            DiagnosticScope::Proxy,
            DiagnosticStatus::Unknown,
            "Observed proxy route is unavailable.",
            Vec::new(),
            None,
        );
    };
    let Some(readiness) = status.readiness.as_ref() else {
        return check(
            "proxy_route",
            DiagnosticScope::Proxy,
            DiagnosticStatus::Unknown,
            "No live route evidence is available.",
            Vec::new(),
            None,
        );
    };
    let Some(route) = readiness.control_plane_route.as_ref() else {
        return check(
            "proxy_route",
            DiagnosticScope::Proxy,
            DiagnosticStatus::Unknown,
            "Observed proxy route is not available yet.",
            Vec::new(),
            None,
        );
    };
    check(
        "proxy_route",
        DiagnosticScope::Proxy,
        if readiness.control_plane_connected {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        if readiness.control_plane_connected {
            "Observed control-plane route is available."
        } else {
            "Observed control-plane route failed readiness."
        },
        vec![
            evidence(DiagnosticEvidenceKey::RouteMode, &route.route_mode),
            evidence(DiagnosticEvidenceKey::ProxySource, &route.proxy_source),
        ],
        None,
    )
}

fn credential_presence_check(
    config: Option<&AppConfig>,
    secrets: &Mutex<BackendState>,
) -> DiagnosticCheck {
    let Some(config) = config else {
        return check(
            "credential_presence",
            DiagnosticScope::Credential,
            DiagnosticStatus::Unknown,
            "Credential presence cannot be checked until configuration is available.",
            Vec::new(),
            None,
        );
    };
    let Some(reference) = config.tunnel.api_key_ref.as_ref() else {
        return check(
            "credential_presence",
            DiagnosticScope::Credential,
            DiagnosticStatus::Fail,
            "The configured credential reference is missing.",
            vec![evidence(DiagnosticEvidenceKey::Configured, "false")],
            Some(remediation(
                "credential_save",
                "Save the connection credential in Settings.",
                DiagnosticRemediationTarget::SettingsConnection,
            )),
        );
    };
    let exists = secrets
        .lock()
        .ok()
        .and_then(|state| state.secrets.exists(&reference.key).ok());
    match exists {
        Some(true) => check(
            "credential_presence",
            DiagnosticScope::Credential,
            DiagnosticStatus::Pass,
            "The referenced credential is present in the SecretStore.",
            vec![evidence(DiagnosticEvidenceKey::Exists, "true")],
            None,
        ),
        Some(false) => check(
            "credential_presence",
            DiagnosticScope::Credential,
            DiagnosticStatus::Fail,
            "The referenced credential is missing from the SecretStore.",
            vec![evidence(DiagnosticEvidenceKey::Exists, "false")],
            Some(remediation(
                "credential_save",
                "Save the connection credential in Settings.",
                DiagnosticRemediationTarget::SettingsConnection,
            )),
        ),
        None => check(
            "credential_presence",
            DiagnosticScope::Credential,
            DiagnosticStatus::Unknown,
            "SecretStore availability could not be confirmed.",
            Vec::new(),
            None,
        ),
    }
}

fn credential_state_check(
    observation: &Result<TunnelStatus, crate::tunnel::TunnelProcessError>,
) -> DiagnosticCheck {
    let Ok(status) = observation else {
        return check(
            "credential_state",
            DiagnosticScope::Credential,
            DiagnosticStatus::Unknown,
            "Credential acceptance cannot be observed without a live Tunnel.",
            Vec::new(),
            None,
        );
    };
    let Some(readiness) = status.readiness.as_ref() else {
        return check(
            "credential_state",
            DiagnosticScope::Credential,
            DiagnosticStatus::Unknown,
            "No live credential acceptance evidence is available.",
            Vec::new(),
            None,
        );
    };
    let unauthorized = readiness
        .control_plane_error
        .as_deref()
        .map(is_unauthorized)
        .unwrap_or(false);
    if unauthorized {
        return check(
            "credential_state",
            DiagnosticScope::Credential,
            DiagnosticStatus::Fail,
            "The control plane rejected the configured credential.",
            vec![evidence(
                DiagnosticEvidenceKey::ErrorCode,
                "CONTROL_PLANE_UNAUTHORIZED",
            )],
            Some(remediation(
                "credential_replace",
                "Review or replace the connection credential in Settings.",
                DiagnosticRemediationTarget::SettingsConnection,
            )),
        );
    }
    check(
        "credential_state",
        DiagnosticScope::Credential,
        if readiness.control_plane_connected {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Unknown
        },
        if readiness.control_plane_connected {
            "Current control-plane connectivity proves credential acceptance."
        } else {
            "Credential acceptance has not been proven by a current connection."
        },
        Vec::new(),
        None,
    )
}

fn lifecycle_check(status: &HarnessStatus) -> DiagnosticCheck {
    let (result, summary) = match status.lifecycle {
        HarnessLifecycle::Ready => (DiagnosticStatus::Pass, "Harness lifecycle is Ready."),
        HarnessLifecycle::Stopped => (DiagnosticStatus::Warning, "Harness is stopped."),
        HarnessLifecycle::Starting | HarnessLifecycle::Stopping => (
            DiagnosticStatus::Warning,
            "Harness lifecycle is transitioning.",
        ),
        HarnessLifecycle::Degraded | HarnessLifecycle::Error => (
            DiagnosticStatus::Fail,
            "Harness lifecycle reports a current failure.",
        ),
    };
    let mut evidence_items = vec![evidence(
        DiagnosticEvidenceKey::Lifecycle,
        format!("{:?}", status.lifecycle).to_ascii_lowercase(),
    )];
    if let Some(error) = status.last_error.as_ref() {
        evidence_items.push(evidence(DiagnosticEvidenceKey::ErrorCode, &error.code));
    }
    check(
        "harness_lifecycle",
        DiagnosticScope::Lifecycle,
        result,
        summary,
        evidence_items,
        None,
    )
}

fn check(
    id: &str,
    scope: DiagnosticScope,
    status: DiagnosticStatus,
    summary: &str,
    evidence: Vec<DiagnosticEvidence>,
    remediation: Option<DiagnosticRemediation>,
) -> DiagnosticCheck {
    DiagnosticCheck {
        id: id.into(),
        status,
        summary: summary.into(),
        scope: Some(scope),
        evidence,
        remediation,
    }
}

fn evidence(key: DiagnosticEvidenceKey, value: impl AsRef<str>) -> DiagnosticEvidence {
    let value = value.as_ref();
    let safe = if key == DiagnosticEvidenceKey::ErrorCode {
        safe_code(value)
    } else {
        safe_value(value)
    };
    DiagnosticEvidence { key, value: safe }
}

fn remediation(
    code: &str,
    summary: &str,
    target: DiagnosticRemediationTarget,
) -> DiagnosticRemediation {
    DiagnosticRemediation {
        code: safe_value(code),
        summary: summary.into(),
        target: Some(target),
    }
}

fn safe_value(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_SAFE_VALUE_CHARS)
        .collect()
}

fn safe_code(value: &str) -> String {
    let valid = !value.is_empty()
        && value.len() <= MAX_SAFE_VALUE_CHARS
        && value.chars().all(|character| {
            character.is_ascii_uppercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.')
        });
    if valid {
        value.to_owned()
    } else {
        "UNKNOWN".into()
    }
}

pub(crate) fn overall_status(checks: &[DiagnosticCheck]) -> DiagnosticStatus {
    if checks
        .iter()
        .any(|check| check.status == DiagnosticStatus::Fail)
    {
        DiagnosticStatus::Fail
    } else if checks
        .iter()
        .any(|check| check.status == DiagnosticStatus::Warning)
    {
        DiagnosticStatus::Warning
    } else if checks
        .iter()
        .any(|check| check.status == DiagnosticStatus::Unknown)
    {
        DiagnosticStatus::Unknown
    } else {
        DiagnosticStatus::Pass
    }
}

fn is_unauthorized(value: &str) -> bool {
    let value = value.to_ascii_uppercase();
    value.contains("401") || value.contains("UNAUTHORIZED") || value.contains("FORBIDDEN")
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple(id: &str, status: DiagnosticStatus) -> DiagnosticCheck {
        check(
            id,
            DiagnosticScope::Config,
            status,
            "summary",
            Vec::new(),
            None,
        )
    }

    #[test]
    fn overall_status_uses_fail_warning_unknown_pass_precedence() {
        assert_eq!(
            overall_status(&[simple("a", DiagnosticStatus::Unknown)]),
            DiagnosticStatus::Unknown
        );
        assert_eq!(
            overall_status(&[
                simple("a", DiagnosticStatus::Pass),
                simple("b", DiagnosticStatus::Unknown),
                simple("c", DiagnosticStatus::Warning),
                simple("d", DiagnosticStatus::Fail),
            ]),
            DiagnosticStatus::Fail
        );
    }

    #[test]
    fn safe_evidence_is_bounded_and_control_free() {
        let value = evidence(
            DiagnosticEvidenceKey::ErrorCode,
            "raw-secret\nXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
        );
        assert!(value.value.len() <= MAX_SAFE_VALUE_CHARS);
        assert!(!value.value.contains('\n'));
        assert!(!value.value.contains("raw-secret"));
    }

    #[test]
    fn unauthorized_mapping_does_not_return_raw_error() {
        assert!(is_unauthorized("401 Unauthorized: bearer raw-secret"));
        let check = check(
            "credential_state",
            DiagnosticScope::Credential,
            DiagnosticStatus::Fail,
            "rejected",
            vec![evidence(
                DiagnosticEvidenceKey::ErrorCode,
                "CONTROL_PLANE_UNAUTHORIZED",
            )],
            None,
        );
        let serialized = serde_json::to_string(&check).unwrap();
        assert!(!serialized.contains("raw-secret"));
        assert!(serialized.contains("CONTROL_PLANE_UNAUTHORIZED"));
    }
}
