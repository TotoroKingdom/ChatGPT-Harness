use super::{common::DecimalU64, tools::ToolDescriptor};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLifecycle {
    Initializing,
    Ready,
    Stopping,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeReadiness {
    NotReady,
    Initializing,
    Ready,
    Degraded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeErrorSummary {
    pub code: String,
    pub message: String,
}

/// Ephemeral observed Runtime state, distinct from persistent AppConfig.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    pub runtime_id: String,
    /// RFC3339 UTC timestamp.
    pub started_at: String,
    pub lifecycle: RuntimeLifecycle,
    pub readiness: RuntimeReadiness,
    #[ts(type = "string")]
    pub config_revision: DecimalU64,
    pub active_workspace: String,
    pub security_mode: super::config::SecurityMode,
    pub exposed_tool_catalog: Vec<ToolDescriptor>,
    pub active_requests: u32,
    pub pending_approvals: u32,
    pub running_executions: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_error: Option<RuntimeErrorSummary>,
}

pub type RuntimeStatusParams = super::common::EmptyParams;
pub type RuntimeStatusResult = RuntimeState;
pub type RuntimeStopParams = super::common::EmptyParams;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStopResult {
    pub accepted: bool,
}
