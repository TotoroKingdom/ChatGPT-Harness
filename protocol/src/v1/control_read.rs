//! M8-C local read contracts. Wire DTOs mirror the frozen safe M8-A/M8-B
//! projections; they are not Runtime authorities or MCP schemas.
use crate::{MachineErrorCode, PolicyDecision, PolicyReason, RuntimeLifecycle, RuntimeReadiness};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const DIAGNOSTICS_GET: &str = "control.diagnostics.get";
pub const ACTIVITY_SNAPSHOT: &str = "control.activity.snapshot";
pub const EFFECTIVE_TOOLS_GET: &str = "control.effective_tools.get";
pub const MAX_ACTIVITY_LIMIT: u64 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReadErrorCode {
    RuntimeUnavailable,
    RuntimeNotReady,
    InvalidRequest,
    InternalError,
}
impl ReadErrorCode {
    pub fn message(self) -> &'static str {
        match self {
            Self::RuntimeUnavailable => "Runtime is unavailable.",
            Self::RuntimeNotReady => "Runtime is not ready.",
            Self::InvalidRequest => "Invalid control read request.",
            Self::InternalError => "Control read failed.",
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeUnavailable => "RUNTIME_UNAVAILABLE",
            Self::RuntimeNotReady => "RUNTIME_NOT_READY",
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadParams {}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ActivitySnapshotParams {
    pub limit: u64,
}
impl ActivitySnapshotParams {
    pub fn validate(&self) -> Result<usize, ReadErrorCode> {
        if (1..=MAX_ACTIVITY_LIMIT).contains(&self.limit) {
            Ok(self.limit as usize)
        } else {
            Err(ReadErrorCode::InvalidRequest)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    WorkspaceRead,
    WorkspaceWrite,
    DeveloperShell,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeDiagnosticsSnapshot {
    pub schema_version: u32,
    pub captured_at: String,
    pub runtime: RuntimeDiagnostics,
    pub security: SecurityDiagnostics,
    pub tools: ToolDiagnostics,
    pub requests: RequestDiagnostics,
    pub last_error: LastErrorDiagnostics,
    pub tunnel: TunnelDiagnostics,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeDiagnostics {
    #[serde(default)]
    #[ts(type = "string")]
    pub config_revision: Option<crate::DecimalU64>,
    pub runtime_id: Option<String>,
    pub started_at: Option<String>,
    pub lifecycle: RuntimeLifecycle,
    pub readiness: RuntimeReadiness,
    pub initialized_modules: Vec<InitializedModule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum InitializedModule {
    ToolRegistry,
    PolicyEngine,
    WorkspaceGuard,
    ExecutionPipeline,
    AuditEmitter,
    EventBus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecurityDiagnostics {
    pub workspace_validation: WorkspaceValidation,
    /// Construction is observed, not rule validity or permission to execute.
    pub policy_engine_loaded: bool,
    /// Per-invocation event emission only, not durable audit storage.
    pub audit_enabled: bool,
    pub execution_pipeline: PipelineState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceValidation {
    NotInitialized,
    ValidatedAtInitialization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    NotInitialized,
    NotReady,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDiagnostics {
    pub declared_count: usize,
    /// Count of directly available entries, not the size of the declared catalog.
    pub effective_count: usize,
    pub items: Vec<ToolDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDiagnostic {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status_reason: Option<crate::ToolStatusReason>,
    /// Source-controlled registry identifier, never a request-supplied name.
    pub name: String,
    pub declared_capability: Option<ToolCapability>,
    pub availability: DiagnosticToolAvailability,
    pub decision: PolicyDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<crate::ToolReadiness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness_reason: Option<String>,
}

// Bash-only observations are additive. Existing file-tool responses retain
// their closed shape, including rejection of arbitrary reason/path fields.
impl<'de> Deserialize<'de> for ToolDiagnostic {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Raw {
            #[serde(default)]
            status_reason: Option<crate::ToolStatusReason>,
            name: String,
            declared_capability: Option<ToolCapability>,
            availability: DiagnosticToolAvailability,
            decision: PolicyDecision,
            #[serde(default)]
            reason: Option<String>,
            #[serde(default)]
            readiness: Option<crate::ToolReadiness>,
            #[serde(default)]
            readiness_reason: Option<String>,
        }
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("name").and_then(serde_json::Value::as_str) != Some("bash")
            && ["reason", "readiness", "readinessReason"]
                .iter()
                .any(|key| value.get(*key).is_some())
        {
            return Err(serde::de::Error::custom(
                "Bash observation fields are not valid on file-tool diagnostics",
            ));
        }
        let raw: Raw = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            status_reason: raw.status_reason,
            name: raw.name,
            declared_capability: raw.declared_capability,
            availability: raw.availability,
            decision: raw.decision,
            reason: raw.reason,
            readiness: raw.readiness,
            readiness_reason: raw.readiness_reason,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticToolAvailability {
    Available,
    AvailableWithApproval,
    UnavailableByMode,
    UnavailableByPolicy,
    UnavailableByRuntime,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestDiagnostics {
    pub observation: RequestObservation,
    pub active_count: Option<u32>,
    #[serde(default)]
    pub active_requests: u32,
    #[serde(default)]
    pub pending_approvals: u32,
    #[serde(default)]
    pub running_executions: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RequestObservation {
    NotInstrumented,
    Instrumented,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LastErrorDiagnostics {
    pub observation: ErrorObservation,
    /// Only an existing, allowlisted control-protocol machine code is retained.
    /// Unknown/free-form codes and all messages are discarded.
    pub code: Option<MachineErrorCode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ErrorObservation {
    NotRecorded,
    Recorded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TunnelDiagnostics {
    /// Runtime does not own Launcher config or process observations.
    pub configured: Option<bool>,
    pub process_state: TunnelProcessObservation,
    pub connection_state: TunnelConnectionObservation,
    pub blocked_reason: TunnelBlockedReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TunnelProcessObservation {
    NotObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TunnelConnectionObservation {
    NotEstablished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TunnelBlockedReason {
    M7CBlocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    RequestReceived,
    WorkspaceValidated,
    PolicyEvaluated,
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    RequestDenied,
    ApprovalRequired,
    ApprovalResolved,
    ApprovalExpired,
    ApprovalCancelled,
    SessionGrantMatched,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AuditLifecycleState {
    Received,
    Validating,
    WorkspaceValidating,
    PolicyEvaluating,
    Denied,
    AwaitingApproval,
    Executing,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    InProgress,
    Success,
    Failed,
    Denied,
    ApprovalRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCategory {
    Request,
    Workspace,
    Policy,
    Execution,
    Approval,
}

/// A safe display projection, not an audit record or a security decision.
/// Bash evidence excludes argv, raw command, environment and process output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BashActivityEvidence {
    pub mode: crate::SecurityMode,
    pub mode_eligible: bool,
    pub capability: ToolCapability,
    pub cwd: crate::BashWorkingDirectory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<crate::BashClassification>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_flags: Vec<crate::BashRiskFlag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_request_id: Option<String>,
    pub session_grant_hit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ActivityFailureCategory {
    ApprovalDenied,
    ApprovalExpired,
    ApprovalCancelled,
    ToolDisabled,
    TargetChanged,
    AtomicFailure,
    IoFailure,
    WorkspaceViolation,
    InvalidArguments,
    ExecutionFailed,
    BashUnavailable,
    BashInvalidCommand,
    BashCommandTooLong,
    BashTimeout,
    BashOutputLimit,
    BashConcurrencyLimit,
    BashSpawnFailed,
    BashCancelled,
    BashProcessFailed,
}

/// A safe display projection, not an audit record or a security decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityEntry {
    pub event_id: String,
    pub request_id: String,
    /// Normalized UTC; None when the source timestamp is invalid.
    pub timestamp: Option<String>,
    pub category: ActivityCategory,
    pub event_type: AuditEventType,
    pub lifecycle_state: AuditLifecycleState,
    /// Only a source-controlled registry identifier, otherwise None.
    pub tool_name: Option<String>,
    /// None means policy was not evaluated, never an implicit ALLOW.
    pub decision_summary: Option<PolicyDecision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_rule_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_reason: Option<PolicyReason>,
    pub outcome_summary: AuditOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_category: Option<ActivityFailureCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bash: Option<BashActivityEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub capability: Option<ToolCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sanitized_arguments: Option<crate::SanitizedArgumentsEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approval_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approval_decision: Option<crate::ApprovalDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub result_evidence: Option<crate::ExecutionResultEvidence>,
}
