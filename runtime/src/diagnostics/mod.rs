//! M8-A: safe, read-only observations, never execution or authorization inputs.
//! The transport and UI adapter are deliberately deferred; see docs/DIAGNOSTICS.md.

use crate::state::{Phase, RuntimeCore};
use crate::tools::{EffectiveTool, ToolAvailability, ToolCapability};
use harness_control_protocol::{
    MachineErrorCode, PolicyDecision, RuntimeLifecycle, RuntimeReadiness,
};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeDiagnosticsSnapshot {
    pub schema_version: u32,
    pub captured_at: String,
    pub runtime: RuntimeDiagnostics,
    pub security: SecurityDiagnostics,
    pub tools: ToolDiagnostics,
    pub requests: RequestDiagnostics,
    pub last_error: LastErrorDiagnostics,
    pub tunnel: TunnelDiagnostics,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeDiagnostics {
    pub config_revision: Option<harness_control_protocol::DecimalU64>,
    pub runtime_id: Option<uuid::Uuid>,
    pub started_at: Option<String>,
    pub lifecycle: RuntimeLifecycle,
    pub readiness: RuntimeReadiness,
    pub initialized_modules: Vec<InitializedModule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InitializedModule {
    ToolRegistry,
    PolicyEngine,
    WorkspaceGuard,
    ExecutionPipeline,
    AuditEmitter,
    EventBus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecurityDiagnostics {
    pub workspace_validation: WorkspaceValidation,
    /// Construction is observed, not rule validity or permission to execute.
    pub policy_engine_loaded: bool,
    /// Per-invocation event emission only, not durable audit storage.
    pub audit_enabled: bool,
    pub execution_pipeline: PipelineState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkspaceValidation {
    NotInitialized,
    ValidatedAtInitialization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PipelineState {
    NotInitialized,
    NotReady,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolDiagnostics {
    pub declared_count: usize,
    /// Count of directly available entries, not the size of the declared catalog.
    pub effective_count: usize,
    pub items: Vec<ToolDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolDiagnostic {
    pub status_reason: harness_control_protocol::ToolStatusReason,
    /// Source-controlled registry identifier, never a request-supplied name.
    pub name: String,
    pub declared_capability: Option<ToolCapability>,
    pub availability: DiagnosticToolAvailability,
    pub decision: PolicyDecision,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<harness_control_protocol::ToolReadiness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiagnosticToolAvailability {
    Available,
    AvailableWithApproval,
    UnavailableByMode,
    UnavailableByPolicy,
    UnavailableByRuntime,
    Disabled,
}

impl From<EffectiveTool> for ToolDiagnostic {
    fn from(tool: EffectiveTool) -> Self {
        let reason = (tool.name == "bash").then(|| tool.reason.clone());
        Self {
            status_reason: tool.status_reason,
            name: tool.name,
            declared_capability: tool.declared_capability,
            availability: match tool.availability {
                ToolAvailability::Available => DiagnosticToolAvailability::Available,
                ToolAvailability::AvailableWithApproval => {
                    DiagnosticToolAvailability::AvailableWithApproval
                }
                ToolAvailability::UnavailableByMode => {
                    DiagnosticToolAvailability::UnavailableByMode
                }
                ToolAvailability::UnavailableByPolicy => {
                    DiagnosticToolAvailability::UnavailableByPolicy
                }
                ToolAvailability::UnavailableByRuntime => {
                    DiagnosticToolAvailability::UnavailableByRuntime
                }
                ToolAvailability::UnavailableDisabled => DiagnosticToolAvailability::Disabled,
            },
            decision: tool.decision,
            reason,
            readiness: tool.readiness,
            readiness_reason: tool.readiness_reason,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestDiagnostics {
    pub observation: RequestObservation,
    pub active_count: Option<u32>,
    pub active_requests: u32,
    pub pending_approvals: u32,
    pub running_executions: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RequestObservation {
    Instrumented,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LastErrorDiagnostics {
    pub observation: ErrorObservation,
    /// Only an existing, allowlisted control-protocol machine code is retained.
    /// Unknown/free-form codes and all messages are discarded.
    pub code: Option<MachineErrorCode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorObservation {
    NotRecorded,
    Recorded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TunnelDiagnostics {
    /// Runtime does not own Launcher config or process observations.
    pub configured: Option<bool>,
    pub process_state: TunnelProcessObservation,
    pub connection_state: TunnelConnectionObservation,
    pub blocked_reason: TunnelBlockedReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TunnelProcessObservation {
    NotObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TunnelConnectionObservation {
    NotEstablished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TunnelBlockedReason {
    M7CBlocked,
}

impl RuntimeDiagnosticsSnapshot {
    /// Called only with facts from the owning RuntimeCore. No filesystem probes,
    /// event consumption, policy implementation, secret/config export or network I/O.
    pub(super) fn capture(core: &RuntimeCore, pipeline_initialized: bool, ready: bool) -> Self {
        let workspace_initialized = core.workspace_guard().is_some();
        let (lifecycle, readiness) = match core.phase {
            Phase::WaitingInitialize => {
                (RuntimeLifecycle::Initializing, RuntimeReadiness::NotReady)
            }
            Phase::InitializedPendingAck => (
                RuntimeLifecycle::Initializing,
                RuntimeReadiness::Initializing,
            ),
            Phase::Ready => (
                RuntimeLifecycle::Ready,
                if ready {
                    RuntimeReadiness::Ready
                } else {
                    RuntimeReadiness::NotReady
                },
            ),
            Phase::Stopping => (RuntimeLifecycle::Stopping, RuntimeReadiness::NotReady),
        };
        // These two non-optional authorities are constructed by RuntimeCore::new.
        let mut initialized_modules = vec![
            InitializedModule::ToolRegistry,
            InitializedModule::PolicyEngine,
        ];
        if workspace_initialized {
            initialized_modules.push(InitializedModule::WorkspaceGuard);
        }
        if pipeline_initialized {
            initialized_modules.extend([
                InitializedModule::ExecutionPipeline,
                InitializedModule::AuditEmitter,
            ]);
        }
        if core.events.is_some() {
            initialized_modules.push(InitializedModule::EventBus);
        }
        let items: Vec<ToolDiagnostic> = core
            .effective_tools()
            .into_iter()
            .map(ToolDiagnostic::from)
            .collect();
        Self {
            schema_version: 1,
            captured_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            runtime: RuntimeDiagnostics {
                config_revision: core.config.as_ref().map(|c| c.config_revision),
                // Canonicalization prevents public String fields becoming an export escape hatch.
                runtime_id: uuid::Uuid::parse_str(&core.runtime_id).ok(),
                started_at: chrono::DateTime::parse_from_rfc3339(&core.started_at)
                    .ok()
                    .map(|time| {
                        time.with_timezone(&chrono::Utc)
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                    }),
                lifecycle,
                readiness,
                initialized_modules,
            },
            security: SecurityDiagnostics {
                workspace_validation: if workspace_initialized {
                    WorkspaceValidation::ValidatedAtInitialization
                } else {
                    WorkspaceValidation::NotInitialized
                },
                policy_engine_loaded: true,
                audit_enabled: pipeline_initialized,
                execution_pipeline: if !pipeline_initialized {
                    PipelineState::NotInitialized
                } else if ready {
                    PipelineState::Ready
                } else {
                    PipelineState::NotReady
                },
            },
            tools: ToolDiagnostics {
                declared_count: items.len(),
                effective_count: items
                    .iter()
                    .filter(|tool| {
                        matches!(
                            tool.availability,
                            DiagnosticToolAvailability::Available
                                | DiagnosticToolAvailability::AvailableWithApproval
                        )
                    })
                    .count(),
                items,
            },
            requests: RequestDiagnostics {
                observation: RequestObservation::Instrumented,
                active_count: Some(core.request_counts().0),
                active_requests: core.request_counts().0,
                pending_approvals: core.approval_manager().pending_count() as u32,
                running_executions: core.request_counts().1,
            },
            last_error: LastErrorDiagnostics {
                observation: if core.last_error.is_some() {
                    ErrorObservation::Recorded
                } else {
                    ErrorObservation::NotRecorded
                },
                code: core.last_error.as_ref().and_then(|error| {
                    serde_json::from_value(serde_json::Value::String(error.code.clone())).ok()
                }),
            },
            tunnel: TunnelDiagnostics {
                configured: None,
                process_state: TunnelProcessObservation::NotObserved,
                connection_state: TunnelConnectionObservation::NotEstablished,
                blocked_reason: TunnelBlockedReason::M7CBlocked,
            },
        }
    }
}

#[cfg(test)]
mod tests;
