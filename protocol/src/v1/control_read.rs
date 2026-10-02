//! M8-C local read contracts. Wire DTOs mirror the frozen safe M8-A/M8-B
//! projections; they are not Runtime authorities or MCP schemas.
use crate::{MachineErrorCode, PolicyDecision, PolicyReason, RuntimeLifecycle, RuntimeReadiness};
use serde::{Deserialize, Serialize};

pub const DIAGNOSTICS_GET: &str = "control.diagnostics.get";
pub const ACTIVITY_SNAPSHOT: &str = "control.activity.snapshot";
pub const EFFECTIVE_TOOLS_GET: &str = "control.effective_tools.get";
pub const MAX_ACTIVITY_LIMIT: u64 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    WorkspaceRead,
    WorkspaceWrite,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeDiagnostics {
    pub runtime_id: Option<String>,
    pub started_at: Option<String>,
    pub lifecycle: RuntimeLifecycle,
    pub readiness: RuntimeReadiness,
    pub initialized_modules: Vec<InitializedModule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitializedModule {
    ToolRegistry,
    PolicyEngine,
    WorkspaceGuard,
    ExecutionPipeline,
    AuditEmitter,
    EventBus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecurityDiagnostics {
    pub workspace_validation: WorkspaceValidation,
    /// Construction is observed, not rule validity or permission to execute.
    pub policy_engine_loaded: bool,
    /// Per-invocation event emission only, not durable audit storage.
    pub audit_enabled: bool,
    pub execution_pipeline: PipelineState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceValidation {
    NotInitialized,
    ValidatedAtInitialization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    NotInitialized,
    NotReady,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDiagnostics {
    pub declared_count: usize,
    /// Count of directly available entries, not the size of the declared catalog.
    pub effective_count: usize,
    pub items: Vec<ToolDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDiagnostic {
    /// Source-controlled registry identifier, never a request-supplied name.
    pub name: String,
    pub declared_capability: Option<ToolCapability>,
    pub availability: DiagnosticToolAvailability,
    pub decision: PolicyDecision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticToolAvailability {
    Available,
    UnavailableByPolicy,
    UnavailableByRuntime,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestDiagnostics {
    pub observation: RequestObservation,
    pub active_count: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestObservation {
    NotInstrumented,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LastErrorDiagnostics {
    pub observation: ErrorObservation,
    /// Only an existing, allowlisted control-protocol machine code is retained.
    /// Unknown/free-form codes and all messages are discarded.
    pub code: Option<MachineErrorCode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorObservation {
    NotRecorded,
    Recorded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TunnelDiagnostics {
    /// Runtime does not own Launcher config or process observations.
    pub configured: Option<bool>,
    pub process_state: TunnelProcessObservation,
    pub connection_state: TunnelConnectionObservation,
    pub blocked_reason: TunnelBlockedReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelProcessObservation {
    NotObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelConnectionObservation {
    NotEstablished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelBlockedReason {
    M7CBlocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    InProgress,
    Success,
    Failed,
    Denied,
    ApprovalRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCategory {
    Request,
    Workspace,
    Policy,
    Execution,
    Approval,
}

/// A safe display projection, not an audit record or a security decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}
