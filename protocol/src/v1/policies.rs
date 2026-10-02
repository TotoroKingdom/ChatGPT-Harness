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

/// The matcher type discriminator is intentionally closed in M3. Future
/// matcher kinds can be added as new tagged variants without making the
/// persisted user rule shape free-form again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMatcherType {
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolMatcherConfig {
    pub tool: String,
}

impl ToolMatcherConfig {
    pub fn new(tool: impl Into<String>) -> Self {
        Self { tool: tool.into() }
    }
}

/// Persisted typed matcher. Its wire shape is `{ "type": "tool", "tool":
/// "write" }`, while the closed discriminator prevents arbitrary matcher
/// expressions from becoming authorization input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyMatcherConfig {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub matcher_type: PolicyMatcherType,
    pub tool: String,
}

impl PolicyMatcherConfig {
    pub fn tool(tool: impl Into<String>) -> Self {
        Self {
            matcher_type: PolicyMatcherType::Tool,
            tool: tool.into(),
        }
    }

    pub fn as_tool_matcher(&self) -> Option<ToolMatcherConfig> {
        matches!(self.matcher_type, PolicyMatcherType::Tool)
            .then(|| ToolMatcherConfig::new(self.tool.clone()))
    }
}

/// Only user-owned fields are persisted. Runtime projections add `source` and
/// `editable`, so callers cannot manufacture a system rule in AppConfig.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPolicyRuleConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub decision: PolicyDecision,
    pub matcher: PolicyMatcherConfig,
    pub reason: String,
}

impl UserPolicyRuleConfig {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        decision: PolicyDecision,
        tool: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            enabled: true,
            decision,
            matcher: PolicyMatcherConfig::tool(tool),
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PolicySnapshotStatus {
    Valid,
    Invalid,
}

/// Reasons are typed because policy evidence is consumed by Runtime, control
/// reads, activity, and the UI. They are not display-only log strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReason {
    SystemGuardrail,
    MatchedUserRules,
    ToolDefault,
    InvalidPolicySnapshot,
    InvalidContext,
    UnsupportedCapability,
    ToolDisabled,
}

/// Runtime-owned projection of either a fixed system rule or a compiled user
/// rule. User projections retain the typed matcher; system rules have none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivePolicyRule {
    pub id: String,
    pub name: String,
    pub decision: PolicyDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub matcher: Option<PolicyMatcherConfig>,
    pub reason: String,
    pub source: PolicySource,
    pub editable: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PolicyToolAvailability {
    Available,
    UnavailableByPolicy,
    UnavailableByRuntime,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivePolicyTool {
    pub tool: String,
    pub decision: PolicyDecision,
    pub matched_rule_ids: Vec<String>,
    pub reason: PolicyReason,
    pub availability: PolicyToolAvailability,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyEffectiveSnapshot {
    pub valid: bool,
    pub status: PolicySnapshotStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status_reason: Option<PolicyReason>,
    pub system_rules: Vec<EffectivePolicyRule>,
    pub user_rules: Vec<EffectivePolicyRule>,
    pub tool_defaults: super::config::ToolPolicyDefaults,
    pub per_tool: Vec<EffectivePolicyTool>,
}

pub type PoliciesEffectiveParams = EmptyParams;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PoliciesEffectiveResult {
    pub snapshot: PolicyEffectiveSnapshot,
}

pub type PoliciesListParams = EmptyParams;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PoliciesListResult {
    pub items: Vec<PolicyRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effective: Option<PolicyEffectiveSnapshot>,
}
