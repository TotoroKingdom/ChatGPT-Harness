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
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub id: String,
    pub status: DiagnosticStatus,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub remediation: Option<String>,
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
