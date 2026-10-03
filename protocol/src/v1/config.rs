use super::common::{DecimalU64, PolicyDecision};
use super::policies::UserPolicyRuleConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    #[ts(type = "string")]
    pub config_revision: DecimalU64,
    pub workspace: WorkspaceConfig,
    pub tunnel: TunnelConfig,
    pub security: SecurityConfig,
    pub ui: UiConfig,
    #[serde(default)]
    pub tools: ToolConfig,
    // Desired Git Bash configuration. Runtime validates the executable again
    // for every generation.
    #[serde(default)]
    pub bash: BashConfig,
    #[serde(default)]
    pub policies: PolicyConfig,
}

// Desired Git-for-Windows Bash configuration. `executable_path` is only a
// user preference and does not imply that Runtime has accepted or launched it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BashConfig {
    #[serde(default = "default_true")]
    pub auto_detect: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub executable_path: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for BashConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            executable_path: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPolicyDefaults {
    pub read_file: PolicyDecision,
    pub glob: PolicyDecision,
    pub write: PolicyDecision,
    pub edit: PolicyDecision,
    pub bash: PolicyDecision,
}

impl Default for ToolPolicyDefaults {
    fn default() -> Self {
        Self {
            read_file: PolicyDecision::Allow,
            glob: PolicyDecision::Allow,
            write: PolicyDecision::Allow,
            edit: PolicyDecision::Allow,
            bash: PolicyDecision::Ask,
        }
    }
}

impl ToolPolicyDefaults {
    pub fn decision_for(&self, tool: &str) -> Option<PolicyDecision> {
        match tool {
            "read_file" => Some(self.read_file.clone()),
            "glob" => Some(self.glob.clone()),
            "write" => Some(self.write.clone()),
            "edit" => Some(self.edit.clone()),
            "bash" => Some(self.bash.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyConfig {
    #[serde(default)]
    pub tool_defaults: ToolPolicyDefaults,
    #[serde(default)]
    pub user_rules: Vec<UserPolicyRuleConfig>,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            tool_defaults: ToolPolicyDefaults::default(),
            user_rules: Vec::new(),
        }
    }
}

impl PolicyConfig {
    /// Validation used by the persistence/UI boundary. Runtime compilation
    /// repeats the authoritative registry check so hand-edited config cannot
    /// weaken authorization.
    pub fn validate_for_save(&self) -> Result<(), String> {
        let mut ids = std::collections::BTreeSet::new();
        for rule in &self.user_rules {
            if rule.id.trim().is_empty() {
                return Err("User policy rule ID must not be empty.".into());
            }
            if rule.name.trim().is_empty() {
                return Err(format!("User policy rule '{}' needs a name.", rule.id));
            }
            if rule.reason.trim().is_empty() {
                return Err(format!("User policy rule '{}' needs a reason.", rule.id));
            }
            if !ids.insert(rule.id.as_str()) {
                return Err(format!("Duplicate user policy rule ID '{}'.", rule.id));
            }
            rule.matcher.validate().map_err(|error| {
                format!(
                    "User policy rule '{}' has an invalid matcher: {error}.",
                    rule.id
                )
            })?;
            match &rule.matcher {
                super::policies::PolicyMatcherConfig::Tool { tool }
                    if matches!(
                        tool.as_str(),
                        "read_file" | "glob" | "write" | "edit" | "bash"
                    ) => {}
                super::policies::PolicyMatcherConfig::Tool { .. } => {
                    return Err(format!(
                        "User policy rule '{}' targets an unknown or unavailable tool.",
                        rule.id
                    ));
                }
                // BashCommand implicitly targets the registered `bash` tool;
                // Runtime repeats the authoritative capability check.
                super::policies::PolicyMatcherConfig::BashCommand { .. } => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolEnabledConfig {
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolConfig {
    pub read_file: ToolEnabledConfig,
    pub glob: ToolEnabledConfig,
    pub write: ToolEnabledConfig,
    pub edit: ToolEnabledConfig,
    pub bash: ToolEnabledConfig,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            read_file: ToolEnabledConfig { enabled: true },
            glob: ToolEnabledConfig { enabled: true },
            write: ToolEnabledConfig { enabled: true },
            edit: ToolEnabledConfig { enabled: true },
            bash: ToolEnabledConfig { enabled: false },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub root_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct TunnelConfig {
    pub tunnel_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub control_plane_http_proxy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tunnel_client_path: Option<String>,
    pub auto_detect: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub api_key_ref: Option<SecretReference>,
}

/// An opaque SecretStore lookup reference; it never contains a raw secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SecretReference {
    pub provider: String,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    Normal,
    DeveloperTest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    pub mode: SecurityMode,
    pub developer_test_mode: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UiConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub language: Option<UiLanguage>,
}

/// Supported React presentation languages.
///
/// This preference is persisted by the host/UI boundary and deliberately does
/// not appear in `RuntimeConfigSnapshot` or any Runtime-facing contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub enum UiLanguage {
    #[serde(rename = "en")]
    #[ts(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    #[ts(rename = "zh-CN")]
    ZhCn,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum StartEligibilityStatus {
    SetupRequired,
    ConfigurationInvalid,
    RuntimeDistributionInvalid,
    TunnelIdRequired,
    ApiKeyRequired,
    TunnelDistributionInvalid,
    ReadyToStart,
    RuntimeAlreadyRunning,
    HarnessAlreadyRunning,
    LifecycleConflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartEligibility {
    pub status: StartEligibilityStatus,
    pub can_start: bool,
    pub message: String,
}

/// The non-secret, immutable-at-startup configuration consumed by a Runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfigSnapshot {
    #[ts(type = "string")]
    pub config_revision: DecimalU64,
    pub workspace: WorkspaceConfig,
    pub security: SecurityConfig,
    pub runtime_limits: RuntimeLimits,
    pub tools: ToolConfig,
    #[serde(default)]
    pub bash: BashConfig,
    #[serde(default)]
    pub policies: PolicyConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLimits {
    pub max_concurrent_requests: u32,
    pub max_activity_events: u32,
    pub max_read_bytes: u32,
}

#[cfg(test)]
mod tests {
    use super::{UiConfig, UiLanguage};

    #[test]
    fn ui_language_wire_values_are_strict_and_backward_compatible() {
        assert_eq!(serde_json::to_string(&UiLanguage::En).unwrap(), "\"en\"");
        assert_eq!(
            serde_json::to_string(&UiLanguage::ZhCn).unwrap(),
            "\"zh-CN\""
        );
        assert_eq!(
            serde_json::from_str::<UiLanguage>("\"en\"").unwrap(),
            UiLanguage::En
        );
        assert_eq!(
            serde_json::from_str::<UiLanguage>("\"zh-CN\"").unwrap(),
            UiLanguage::ZhCn
        );
        for unsupported in ["zh", "en-US", "fr", "zh-TW"] {
            assert!(
                serde_json::from_str::<UiLanguage>(&format!("\"{unsupported}\"")).is_err(),
                "accepted unsupported UI language: {unsupported}"
            );
        }
        assert_eq!(
            serde_json::from_str::<UiConfig>("{}").unwrap(),
            UiConfig::default()
        );
    }
}
