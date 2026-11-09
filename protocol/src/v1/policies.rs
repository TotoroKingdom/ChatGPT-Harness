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

/// The matcher type discriminator remains closed and tagged.  Keeping the
/// discriminator on the wire (rather than accepting a free-form matcher
/// object) lets Runtime validate each authorization shape independently.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMatcherType {
    Tool,
    BashCommand,
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

/// A command-specific matcher persisted in a policy rule.  Its fields are
/// logical command facts, never shell source text or a regular expression.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BashCommandMatcherConfig {
    pub executable: String,
    pub argv_prefix: Vec<String>,
}

impl BashCommandMatcherConfig {
    pub fn new(executable: impl Into<String>, argv_prefix: Vec<String>) -> Self {
        Self {
            executable: executable.into(),
            argv_prefix,
        }
    }
}

/// Persisted tagged matcher.  The internally tagged representation keeps the
/// M3 wire shape (`{"type":"tool","tool":"write"}`) while adding the
/// M5 command matcher (`{"type":"bash_command",...}`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PolicyMatcherConfig {
    Tool {
        tool: String,
    },
    BashCommand {
        executable: String,
        #[serde(rename = "argvPrefix")]
        #[ts(rename = "argvPrefix")]
        argv_prefix: Vec<String>,
    },
}

impl PolicyMatcherConfig {
    pub fn tool(tool: impl Into<String>) -> Self {
        Self::Tool { tool: tool.into() }
    }

    pub fn bash_command(executable: impl Into<String>, argv_prefix: Vec<String>) -> Self {
        Self::BashCommand {
            executable: executable.into(),
            argv_prefix,
        }
    }

    pub fn as_tool_matcher(&self) -> Option<ToolMatcherConfig> {
        match self {
            Self::Tool { tool } => Some(ToolMatcherConfig::new(tool.clone())),
            Self::BashCommand { .. } => None,
        }
    }

    pub fn as_bash_command_matcher(&self) -> Option<BashCommandMatcherConfig> {
        match self {
            Self::Tool { .. } => None,
            Self::BashCommand {
                executable,
                argv_prefix,
            } => Some(BashCommandMatcherConfig::new(
                executable.clone(),
                argv_prefix.clone(),
            )),
        }
    }

    pub fn matcher_type(&self) -> PolicyMatcherType {
        match self {
            Self::Tool { .. } => PolicyMatcherType::Tool,
            Self::BashCommand { .. } => PolicyMatcherType::BashCommand,
        }
    }

    /// Validate the shape that can be safely interpreted as authorization
    /// input. Registry-specific tool identity checks remain Runtime-owned.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Tool { tool } => {
                if tool.trim().is_empty() {
                    Err("tool matcher must not be empty")
                } else {
                    Ok(())
                }
            }
            Self::BashCommand {
                executable,
                argv_prefix,
            } => {
                normalize_bash_executable(executable)
                    .ok_or("bash executable must be a normalized command token")?;
                if executable.as_bytes().len() > MAX_BASH_MATCHER_TOKEN_BYTES {
                    return Err("bash executable is too long");
                }
                if argv_prefix.len() > MAX_BASH_MATCHER_ARGV_PREFIX {
                    return Err("bash argvPrefix is too long");
                }
                if argv_prefix.iter().any(|token| {
                    token.as_bytes().len() > MAX_BASH_MATCHER_TOKEN_BYTES || token.contains('\0')
                }) {
                    return Err("bash argvPrefix contains an invalid token");
                }
                Ok(())
            }
        }
    }
}

/// These bounds protect persisted policy snapshots from unbounded matcher
/// vectors while leaving normal command prefixes comfortably representable.
pub const MAX_BASH_MATCHER_ARGV_PREFIX: usize = 64;
pub const MAX_BASH_MATCHER_TOKEN_BYTES: usize = 1024;

/// Canonical executable identity shared by policy validation and Runtime
/// analysis. Git Bash commonly exposes Windows executables with a `.exe`
/// suffix; that suffix and ASCII case are normalized deterministically.
pub fn normalize_bash_executable(value: &str) -> Option<String> {
    if value.is_empty()
        || value != value.trim()
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return None;
    }
    if value.chars().any(|character| {
        matches!(
            character,
            '&' | '|'
                | ';'
                | '<'
                | '>'
                | '$'
                | '`'
                | '('
                | ')'
                | '\''
                | '"'
                | '{'
                | '}'
                | '['
                | ']'
                | '?'
                | '*'
                | '~'
                | '#'
                | '='
        )
    }) {
        return None;
    }
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    let normalized = normalized
        .strip_suffix(".exe")
        .unwrap_or(&normalized)
        .to_owned();
    (!normalized.is_empty()).then_some(normalized)
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
    AnalyzerRiskFloor,
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
    AvailableWithApproval,
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
