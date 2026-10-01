use super::common::DecimalU64;
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLimits {
    pub max_concurrent_requests: u32,
    pub max_activity_events: u32,
    pub max_read_bytes: u32,
}
