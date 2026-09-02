use super::common::{EmptyParams, PolicyDecision};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    System,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub decision: PolicyDecision,
    pub scope: String,
    pub reason: String,
    pub source: PolicySource,
    pub editable: bool,
    pub enabled: bool,
}

pub type PoliciesListParams = EmptyParams;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PoliciesListResult {
    pub items: Vec<PolicyRule>,
}
