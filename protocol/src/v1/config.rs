use super::common::{DecimalU64, PolicyDecision};
use super::policies::UserPolicyRuleConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Persistent host configuration version, independent of wire protocol v1.
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    /// Host-only desktop presentation and lifecycle preferences. These fields
    /// are deliberately excluded from `RuntimeConfigSnapshot`.
    #[serde(default)]
    pub desktop: DesktopConfig,
    /// Local History persistence preferences. Changes are generation-effective
    /// and never part of `RuntimeConfigSnapshot` or authorization truth.
    #[serde(default)]
    pub history: HistoryConfig,
    /// Runtime search budgets. Optional so existing v3 files keep loading.
    #[serde(default)]
    pub search: SearchConfig,
}

/// Independent wall-clock budgets for the two search tools.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchConfig {
    #[serde(default = "default_search_timeout_seconds")]
    pub glob_timeout_seconds: u32,
    #[serde(default = "default_search_timeout_seconds")]
    pub grep_timeout_seconds: u32,
}

pub const SEARCH_TIMEOUT_MIN_SECONDS: u32 = 5;
pub const SEARCH_TIMEOUT_MAX_SECONDS: u32 = 120;

fn default_search_timeout_seconds() -> u32 {
    30
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            glob_timeout_seconds: 30,
            grep_timeout_seconds: 30,
        }
    }
}

impl SearchConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(SEARCH_TIMEOUT_MIN_SECONDS..=SEARCH_TIMEOUT_MAX_SECONDS)
            .contains(&self.glob_timeout_seconds)
        {
            return Err("globTimeoutSeconds must be between 5 and 120");
        }
        if !(SEARCH_TIMEOUT_MIN_SECONDS..=SEARCH_TIMEOUT_MAX_SECONDS)
            .contains(&self.grep_timeout_seconds)
        {
            return Err("grepTimeoutSeconds must be between 5 and 120");
        }
        Ok(())
    }

    /// Runtime-side fail-safe for hand-written snapshots. Persistence rejects
    /// out-of-range values; the Runtime never adopts them silently as valid.
    pub fn clamped_or_default(&self) -> Self {
        if self.validate().is_ok() {
            self.clone()
        } else {
            Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopConfig {
    #[serde(default = "default_true")]
    pub approval_notifications: bool,
    #[serde(default = "default_true")]
    pub connection_notifications: bool,
    #[serde(default)]
    pub background_notice_acknowledged: bool,
    #[serde(default)]
    pub auto_start_harness_on_app_launch: bool,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            approval_notifications: true,
            connection_notifications: true,
            background_notice_acknowledged: false,
            auto_start_harness_on_app_launch: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_history_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_history_max_storage_mib")]
    #[serde(rename = "maxStorageMiB")]
    #[ts(rename = "maxStorageMiB")]
    pub max_storage_mib: u32,
}

fn default_history_retention_days() -> u32 {
    30
}

fn default_history_max_storage_mib() -> u32 {
    256
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 30,
            max_storage_mib: 256,
        }
    }
}

impl HistoryConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=3650).contains(&self.retention_days) {
            return Err("retentionDays must be between 1 and 3650");
        }
        if !(32..=4096).contains(&self.max_storage_mib) {
            return Err("maxStorageMiB must be between 32 and 4096");
        }
        Ok(())
    }
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
    #[serde(default = "default_allow")]
    pub grep: PolicyDecision,
    pub write: PolicyDecision,
    pub edit: PolicyDecision,
    pub bash: PolicyDecision,
}

fn default_allow() -> PolicyDecision {
    PolicyDecision::Allow
}

impl Default for ToolPolicyDefaults {
    fn default() -> Self {
        Self {
            read_file: PolicyDecision::Allow,
            glob: PolicyDecision::Allow,
            grep: PolicyDecision::Allow,
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
            "grep" => Some(self.grep.clone()),
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
                        "read_file" | "glob" | "grep" | "write" | "edit" | "bash"
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
    #[serde(default = "default_false")]
    pub grep: ToolEnabledConfig,
    pub write: ToolEnabledConfig,
    pub edit: ToolEnabledConfig,
    pub bash: ToolEnabledConfig,
}

fn default_false() -> ToolEnabledConfig {
    ToolEnabledConfig { enabled: false }
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            read_file: ToolEnabledConfig { enabled: true },
            glob: ToolEnabledConfig { enabled: true },
            grep: ToolEnabledConfig { enabled: true },
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    #[serde(default)]
    pub search: SearchConfig,
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
    use super::{SearchConfig, UiConfig, UiLanguage};

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

    #[test]
    fn search_defaults_load_without_breaking_existing_v3_shape() {
        let config: super::AppConfig = serde_json::from_value(serde_json::json!({
            "schemaVersion": 3,
            "configRevision": "9",
            "workspace": { "rootPath": "C:/workspace" },
            "tunnel": {
                "tunnelId": "tunnel",
                "autoDetect": true
            },
            "security": { "mode": "normal", "developerTestMode": false },
            "ui": {},
            "tools": {
                "readFile": { "enabled": true },
                "glob": { "enabled": true },
                "grep": { "enabled": true },
                "write": { "enabled": true },
                "edit": { "enabled": true },
                "bash": { "enabled": false }
            },
            "bash": { "autoDetect": true },
            "policies": {},
            "desktop": {},
            "history": {}
        }))
        .expect("existing v3 config without search");
        assert_eq!(config.search, SearchConfig::default());
        assert_eq!(config.search.glob_timeout_seconds, 30);
        assert_eq!(config.search.grep_timeout_seconds, 30);
    }

    #[test]
    fn search_timeout_bounds_are_enforced() {
        for (glob, grep, valid) in [
            (5, 5, true),
            (120, 120, true),
            (4, 30, false),
            (30, 121, false),
            (0, 0, false),
        ] {
            let config = SearchConfig {
                glob_timeout_seconds: glob,
                grep_timeout_seconds: grep,
            };
            assert_eq!(config.validate().is_ok(), valid);
        }
    }
}
