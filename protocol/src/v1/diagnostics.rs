use super::common::EmptyParams;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Pass,
    Warning,
    Fail,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticScope {
    Config,
    Workspace,
    Runtime,
    Mcp,
    Tunnel,
    ControlPlane,
    Proxy,
    Credential,
    Lifecycle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticEvidenceKey {
    Configured,
    Exists,
    Version,
    ProcessPhase,
    Lifecycle,
    Readiness,
    RouteMode,
    ProxySource,
    LastSuccessAgeSeconds,
    ErrorCode,
    EndpointReachable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvidence {
    pub key: DiagnosticEvidenceKey,
    /// Values are bounded and sanitized by the Host coordinator.
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticRemediationTarget {
    Overview,
    SettingsWorkspace,
    SettingsConnection,
    SettingsSecurity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRemediation {
    pub code: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target: Option<DiagnosticRemediationTarget>,
}

fn default_diagnostic_scope() -> Option<DiagnosticScope> {
    None
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub id: String,
    pub status: DiagnosticStatus,
    pub summary: String,
    #[serde(default = "default_diagnostic_scope")]
    #[ts(optional)]
    pub scope: Option<DiagnosticScope>,
    #[serde(default)]
    pub evidence: Vec<DiagnosticEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub remediation: Option<DiagnosticRemediation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRun {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub overall_status: DiagnosticStatus,
    pub checks: Vec<DiagnosticCheck>,
}
pub type DiagnosticsRunParams = EmptyParams;
pub type DiagnosticsRunResult = DiagnosticRun;
