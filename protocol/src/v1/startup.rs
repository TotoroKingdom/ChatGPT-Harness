use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The startup cohort is assigned by the Host process. `cold` means the first
/// accepted start in the current desktop process; `warm` means a later start
/// in the same process after a previous lifecycle attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum StartupCohort {
    Cold,
    Warm,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum StartupOutcome {
    Ready,
    Failed,
    Cancelled,
    Incomplete,
}

/// Frozen Host-owned startup phase identifiers. Keep this enum additive and
/// ordered; phase order is part of the measurement contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum StartupPhase {
    EligibilityPrepare,
    RuntimeSpawn,
    RuntimeInitialize,
    McpEndpointReady,
    TunnelResolve,
    TunnelLaunch,
    ControlPlaneConnected,
    McpTargetReady,
    HarnessReady,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum StartupPhaseStatus {
    Completed,
    Failed,
    Skipped,
    Incomplete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartupPhaseTiming {
    pub phase: StartupPhase,
    pub started_offset_ms: u32,
    pub duration_ms: u32,
    pub status: StartupPhaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub safe_error_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartupAttempt {
    pub attempt_id: String,
    pub harness_session_id: String,
    pub cohort: StartupCohort,
    /// RFC3339 UTC wall-clock timestamp for display and correlation only.
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub finished_at: Option<String>,
    pub outcome: StartupOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub total_duration_ms: Option<u32>,
    pub phases: Vec<StartupPhaseTiming>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failure_code: Option<String>,
    pub app_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub runtime_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tunnel_version: Option<String>,
}
