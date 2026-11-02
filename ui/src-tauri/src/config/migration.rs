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
    #[serde(default)]
    desktop: Option<DesktopConfig>,
    #[serde(default)]
    history: Option<HistoryConfig>,
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

pub(super) fn validate_v2_shape(raw: &Value) -> Result<(), ConfigError> {
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

pub(super) fn validate_v3_shape(raw: &Value) -> Result<(), ConfigError> {
    shape(
        raw,
        &[
            "schemaVersion",
            "configRevision",
            "workspace",
            "tunnel",
            "security",
            "ui",
            "tools",
            "bash",
            "policies",
            "desktop",
            "history",
        ],
        &[
            "schemaVersion",
            "configRevision",
            "workspace",
            "tunnel",
            "security",
            "ui",
            "tools",
            "bash",
            "policies",
            "desktop",
            "history",
        ],
    )?;
    validate_v2_shape(raw)?;
    shape(
        &raw["desktop"],
        &[
            "approvalNotifications",
            "connectionNotifications",
            "backgroundNoticeAcknowledged",
            "autoStartHarnessOnAppLaunch",
        ],
        &[
            "approvalNotifications",
            "connectionNotifications",
            "backgroundNoticeAcknowledged",
            "autoStartHarnessOnAppLaunch",
        ],
    )?;
    shape(
        &raw["history"],
        &["enabled", "retentionDays", "maxStorageMiB"],
        &["enabled", "retentionDays", "maxStorageMiB"],
    )?;
    Ok(())
}

pub(super) fn migrate_v1_to_v3(raw: Value) -> Result<AppConfig, ConfigError> {
    validate_v2_shape(&raw)?;
    let legacy: LegacyAppConfigV1 =
        serde_json::from_value(raw).map_err(|_| ConfigError::Migration("deserialization"))?;
    if legacy.schema_version != 1 {
        return Err(ConfigError::UnsupportedSchema);
    }
    let mut config = AppConfig {
        schema_version: CURRENT_SCHEMA_VERSION,
        config_revision: legacy.config_revision,
        workspace: legacy.workspace,
        tunnel: legacy.tunnel,
        security: legacy.security,
        ui: legacy.ui,
        tools: legacy.tools,
        policies: legacy.policies,
        bash: legacy.bash,
        desktop: legacy.desktop.unwrap_or_default(),
        history: legacy.history.unwrap_or_default(),
    };
    // A pre-M05 Tool surface did not contain grep. Preserve that exposure
    // even when the legacy file omitted the entire tools object.
    config.tools.grep.enabled = false;
    Ok(config)
}

pub(super) fn migrate_v2_to_v3(mut raw: Value) -> Result<AppConfig, ConfigError> {
    validate_v2_shape(&raw)?;
    let schema_version = raw
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or(ConfigError::Migration("schemaVersion"))?;
    if schema_version != 2 {
        return Err(ConfigError::UnsupportedSchema);
    }
    let object = raw.as_object_mut().ok_or(ConfigError::Migration("shape"))?;
    object.insert("schemaVersion".into(), Value::from(3));
    object.insert(
        "desktop".into(),
        serde_json::to_value(DesktopConfig::default())
            .map_err(|_| ConfigError::Migration("desktop defaults"))?,
    );
    object.insert(
        "history".into(),
        serde_json::to_value(HistoryConfig::default())
            .map_err(|_| ConfigError::Migration("history defaults"))?,
    );
    validate_v3_shape(&raw)?;
    let mut config: AppConfig =
        serde_json::from_value(raw).map_err(|_| ConfigError::Migration("deserialization"))?;
    config.tools.grep.enabled = false;
    Ok(config)
}
