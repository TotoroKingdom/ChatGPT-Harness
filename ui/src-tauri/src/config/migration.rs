//! Explicit, pure legacy conversion. No SecretStore access or filesystem writes.
use super::{ConfigError, CURRENT_SCHEMA_VERSION};
use harness_control_protocol::*;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LegacyAppConfigV1 {
    schema_version: u32,
    config_revision: DecimalU64,
    workspace: WorkspaceConfig,
    tunnel: TunnelConfig,
    security: SecurityConfig,
    ui: UiConfig,
    #[serde(default)]
    tools: ToolConfig,
    #[serde(default)]
    policies: PolicyConfig,
    #[serde(default)]
    bash: BashConfig,
}

fn shape(value: &Value, allowed: &[&str], required: &[&str]) -> Result<(), ConfigError> {
    let object = value.as_object().ok_or(ConfigError::Migration("shape"))?;
    if object.keys().any(|key| !allowed.contains(&key.as_str()))
        || required.iter().any(|key| !object.contains_key(*key))
    {
        return Err(ConfigError::Migration("shape"));
    }
    Ok(())
}

pub(super) fn validate_shape(raw: &Value) -> Result<(), ConfigError> {
    shape(&raw["workspace"], &["rootPath"], &["rootPath"])?;
    shape(
        &raw["tunnel"],
        &[
            "tunnelId",
            "controlPlaneHttpProxy",
            "tunnelClientPath",
            "autoDetect",
            "apiKeyRef",
        ],
        &["tunnelId", "autoDetect"],
    )?;
    if let Some(reference) = raw["tunnel"].get("apiKeyRef").filter(|v| !v.is_null()) {
        shape(reference, &["provider", "key"], &["provider", "key"])?;
    }
    shape(
        &raw["security"],
        &["mode", "developerTestMode"],
        &["mode", "developerTestMode"],
    )?;
    shape(&raw["ui"], &["theme", "language"], &[])?;
    // Defaults apply only to absent top-level V3 fields, never partial objects.
    if let Some(policies) = raw.get("policies") {
        shape(
            policies,
            &["toolDefaults", "userRules"],
            &["toolDefaults", "userRules"],
        )?;
    }
    if let Some(bash) = raw.get("bash") {
        shape(bash, &["autoDetect", "executablePath"], &["autoDetect"])?;
    }
    Ok(())
}

pub(super) fn migrate_v1_to_v2(raw: Value) -> Result<AppConfig, ConfigError> {
    validate_shape(&raw)?;
    let legacy: LegacyAppConfigV1 =
        serde_json::from_value(raw).map_err(|_| ConfigError::Migration("deserialization"))?;
    if legacy.schema_version != 1 {
        return Err(ConfigError::UnsupportedSchema);
    }
    Ok(AppConfig {
        schema_version: CURRENT_SCHEMA_VERSION,
        config_revision: legacy.config_revision,
        workspace: legacy.workspace,
        tunnel: legacy.tunnel,
        security: legacy.security,
        ui: legacy.ui,
        tools: legacy.tools,
        policies: legacy.policies,
        bash: legacy.bash,
    })
}
