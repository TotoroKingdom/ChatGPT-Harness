use super::common::{EmptyParams, PolicyDecision};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Content-search mode shared by MCP arguments and typed evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum GrepMode {
    Literal,
    Regex,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToolReadiness {
    Ready,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub implemented: bool,
    pub enabled: bool,
    pub exposed: bool,
    pub effective_policy: PolicyDecision,
    pub scope: String,
    pub readiness: ToolReadiness,
    pub risk_level: RiskLevel,
}

pub type ToolsListParams = EmptyParams;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolsListResult {
    pub items: Vec<ToolDescriptor>,
}

/// Machine meaning for blocked/available tools; UI never parses prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatusReason {
    Available,
    ApprovalRequired,
    ToolDisabled,
    DeveloperModeRequired,
    RuntimeNotReady,
    CapabilityUnavailable,
    BashExecutableUnavailable,
    PolicyDenied,
    InvalidPolicySnapshot,
    InvalidPolicyContext,
    UnsupportedCapability,
    MetadataInvalid,
}
