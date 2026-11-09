use harness_control_protocol::{
    AppConfig, BashConfig, DecimalU64, DesktopConfig, PolicyConfig, SearchConfig, SecurityConfig,
    SecurityMode, ToolConfig, TunnelConfig, UiConfig, UiLanguage, WorkspaceConfig,
};
use serde::Deserialize;
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

// ConfigStore is constructed at several Host entry points, including the Tray.
// Serialize their read/modify/write transactions within the app process.
static CONFIG_TRANSACTION: Mutex<()> = Mutex::new(());

fn config_transaction() -> Result<MutexGuard<'static, ()>, ConfigError> {
    CONFIG_TRANSACTION
        .lock()
        .map_err(|_| ConfigError::Io("configuration transaction unavailable".into()))
}

mod migration;
pub use harness_control_protocol::CURRENT_SCHEMA_VERSION;

#[cfg(test)]
mod m7_qa;

#[cfg(test)]
mod m00_contract;

#[cfg(test)]
mod m01_contract;

#[cfg(test)]
mod m07_contract;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigMetadata {
    pub config_directory: String,
    pub config_file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopPreferencesPatch {
    pub approval_notifications: Option<bool>,
    pub connection_notifications: Option<bool>,
    pub background_notice_acknowledged: Option<bool>,
    pub auto_start_harness_on_app_launch: Option<bool>,
}

#[derive(Debug)]
pub enum ConfigError {
    Invalid(String),
    Io(String),
    UnsupportedSchema,
    Migration(&'static str),
}
impl ConfigError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "CONFIG_INVALID",
            Self::Io(_) => "CONFIG_WRITE_FAILED",
            Self::UnsupportedSchema => "CONFIG_SCHEMA_UNSUPPORTED",
            Self::Migration(_) => "CONFIG_MIGRATION_FAILED",
        }
    }
    pub fn message(&self) -> String {
        match self {
            Self::Invalid(e) => format!("Configuration needs attention: {e}"),
            Self::Io(e) => format!("Could not save configuration: {e}"),
            Self::UnsupportedSchema => {
                "Configuration schema is not supported; original file unchanged.".into()
            }
            Self::Migration(stage) => {
                format!("Configuration migration failed at {stage}; original file unchanged.")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    directory: PathBuf,
    file: PathBuf,
}
impl ConfigStore {
    pub fn new(directory: PathBuf) -> Self {
        let file = directory.join("config.json");
        Self { directory, file }
    }
    pub fn metadata(&self) -> ConfigMetadata {
        ConfigMetadata {
            config_directory: self.directory.display().to_string(),
            config_file: self.file.display().to_string(),
        }
    }
    pub fn load(&self) -> Result<Option<AppConfig>, ConfigError> {
        let _transaction = config_transaction()?;
        self.load_locked()
    }

    fn load_locked(&self) -> Result<Option<AppConfig>, ConfigError> {
        if !self.file.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&self.file).map_err(|e| ConfigError::Io(e.to_string()))?;
        let raw: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| ConfigError::Invalid("invalid JSON".into()))?;
        let schema_version = raw.get("schemaVersion").and_then(|v| v.as_u64());
        let migrated = match schema_version {
            Some(1) => true,
            Some(2) => true,
            Some(3) => false,
            _ => return Err(ConfigError::UnsupportedSchema),
        };
        let additive_m05_normalization = schema_version == Some(3)
            && (raw
                .get("tools")
                .and_then(|tools| tools.get("grep"))
                .is_none()
                || raw
                    .get("policies")
                    .and_then(|policies| policies.get("toolDefaults"))
                    .and_then(|defaults| defaults.get("grep"))
                    .is_none());
        let mut config = match schema_version {
            Some(1) => migration::migrate_v1_to_v3(raw)?,
            Some(2) => migration::migrate_v2_to_v3(raw)?,
            Some(3) => {
                migration::validate_v3_shape(&raw)
                    .map_err(|_| ConfigError::Invalid("invalid v3 shape".into()))?;
                serde_json::from_value(raw)
                    .map_err(|_| ConfigError::Invalid("invalid config shape".into()))?
            }
            _ => return Err(ConfigError::UnsupportedSchema),
        };
        validate_current(&mut config).map_err(|error| {
            if migrated {
                let _ = error;
                ConfigError::Migration("validation")
            } else {
                error
            }
        })?;
        if migrated || additive_m05_normalization {
            let bytes = serde_json::to_vec_pretty(&config)
                .map_err(|_| ConfigError::Migration("serialization"))?;
            atomic_write(&self.file, &bytes).map_err(|_| {
                ConfigError::Io("atomic migration persist failed; original file unchanged".into())
            })?;
        }
        Ok(Some(config))
    }

    /// Read and validate the current configuration without migrating or
    /// writing anything. Diagnostics must use this boundary so a missing or
    /// legacy config cannot be changed as a side effect of observation.
    pub(crate) fn inspect_read_only(&self) -> Result<Option<AppConfig>, ConfigError> {
        if !self.file.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&self.file).map_err(|e| ConfigError::Io(e.to_string()))?;
        let raw: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| ConfigError::Invalid("invalid JSON".into()))?;
        let mut config = match raw.get("schemaVersion").and_then(|v| v.as_u64()) {
            Some(1) => migration::migrate_v1_to_v3(raw)?,
            Some(2) => migration::migrate_v2_to_v3(raw)?,
            Some(3) => {
                migration::validate_v3_shape(&raw)
                    .map_err(|_| ConfigError::Invalid("invalid v3 shape".into()))?;
                serde_json::from_value(raw)
                    .map_err(|_| ConfigError::Invalid("invalid config shape".into()))?
            }
            _ => return Err(ConfigError::UnsupportedSchema),
        };
        validate_current(&mut config)?;
        Ok(Some(config))
    }
    pub fn load_or_create_defaults(&self) -> Result<AppConfig, ConfigError> {
        let _transaction = config_transaction()?;
        match self.load_locked()? {
            Some(config) => Ok(config),
            None => self.save_locked(Self::safe_defaults()),
        }
    }

    pub fn safe_defaults() -> AppConfig {
        AppConfig {
            schema_version: CURRENT_SCHEMA_VERSION,
            config_revision: DecimalU64::ZERO,
            workspace: WorkspaceConfig {
                root_path: String::new(),
            },
            tunnel: TunnelConfig {
                tunnel_id: String::new(),
                control_plane_http_proxy: None,
                tunnel_client_path: None,
                auto_detect: true,
                api_key_ref: None,
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            ui: UiConfig::default(),
            tools: ToolConfig::default(),
            bash: BashConfig::default(),
            policies: PolicyConfig::default(),
            desktop: harness_control_protocol::DesktopConfig::default(),
            history: harness_control_protocol::HistoryConfig::default(),
            search: SearchConfig::default(),
        }
    }

    pub fn save(&self, config: AppConfig) -> Result<AppConfig, ConfigError> {
        let _transaction = config_transaction()?;
        self.save_locked(config)
    }

    fn save_locked(&self, mut config: AppConfig) -> Result<AppConfig, ConfigError> {
        if config.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(ConfigError::Invalid("unsupported schemaVersion".into()));
        }
        validate_current(&mut config)?;
        let previous = self.load_locked()?;
        config.config_revision = match previous {
            None => DecimalU64::new(1),
            Some(old) if same_restart_semantics(&old, &config) => old.config_revision,
            Some(old) => old
                .config_revision
                .checked_add_one()
                .ok_or_else(|| ConfigError::Invalid("configRevision overflow".into()))?,
        };
        let bytes =
            serde_json::to_vec_pretty(&config).map_err(|e| ConfigError::Io(e.to_string()))?;
        fs::create_dir_all(&self.directory).map_err(|e| ConfigError::Io(e.to_string()))?;
        atomic_write(&self.file, &bytes).map_err(|e| ConfigError::Io(e.to_string()))?;
        Ok(config)
    }

    /// Persist only the presentation language preference.
    ///
    /// Read the latest persisted configuration first so this focused mutation
    /// cannot commit an unrelated Settings draft. When no config exists, use
    /// the same safe defaults as the normal config bootstrap path.
    pub fn set_ui_language(&self, language: Option<UiLanguage>) -> Result<AppConfig, ConfigError> {
        let _transaction = config_transaction()?;
        let mut config = match self.load_locked()? {
            Some(config) => config,
            None => Self::safe_defaults(),
        };
        config.ui.language = language;
        self.save_locked(config)
    }

    /// Persist only host-owned desktop preferences. The latest configuration
    /// is read first so a Settings draft cannot overwrite unrelated fields.
    pub fn set_desktop_preferences(
        &self,
        patch: DesktopPreferencesPatch,
    ) -> Result<AppConfig, ConfigError> {
        let _transaction = config_transaction()?;
        let mut config = match self.load_locked()? {
            Some(config) => config,
            None => Self::safe_defaults(),
        };
        if let Some(value) = patch.approval_notifications {
            config.desktop.approval_notifications = value;
        }
        if let Some(value) = patch.connection_notifications {
            config.desktop.connection_notifications = value;
        }
        if let Some(value) = patch.background_notice_acknowledged {
            config.desktop.background_notice_acknowledged = value;
        }
        if let Some(value) = patch.auto_start_harness_on_app_launch {
            config.desktop.auto_start_harness_on_app_launch = value;
        }
        self.save_locked(config)
    }

    pub fn reset(&self) -> Result<(), ConfigError> {
        let _transaction = config_transaction()?;
        if self.file.exists() {
            fs::remove_file(&self.file).map_err(|e| ConfigError::Io(e.to_string()))?;
        }
        let tmp = self.file.with_extension("json.tmp");
        if tmp.exists() {
            fs::remove_file(tmp).map_err(|e| ConfigError::Io(e.to_string()))?;
        }
        Ok(())
    }
}

fn validate_current(config: &mut AppConfig) -> Result<(), ConfigError> {
    if config.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(ConfigError::UnsupportedSchema);
    }
    config.security.developer_test_mode = config.security.mode == SecurityMode::DeveloperTest;
    config
        .policies
        .validate_for_save()
        .map_err(|_| ConfigError::Invalid("invalid policy configuration".into()))?;
    config
        .history
        .validate()
        .map_err(|error| ConfigError::Invalid(error.into()))?;
    config
        .search
        .validate()
        .map_err(|error| ConfigError::Invalid(error.into()))?;
    if let Some(path) = config.tunnel.tunnel_client_path.as_mut() {
        *path = crate::platform::normalize_tunnel_client_directory(path)
            .display()
            .to_string();
    }
    normalize_and_validate_control_plane_http_proxy(&mut config.tunnel.control_plane_http_proxy)
}

pub(crate) fn normalize_and_validate_control_plane_http_proxy(
    proxy: &mut Option<String>,
) -> Result<(), ConfigError> {
    let Some(value) = proxy.as_mut() else {
        return Ok(());
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        *proxy = None;
        return Ok(());
    }
    let parsed = url::Url::parse(trimmed).map_err(|_| {
        ConfigError::Invalid("controlPlaneHttpProxy must be a valid HTTP or HTTPS proxy URL".into())
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ConfigError::Invalid(
            "controlPlaneHttpProxy must use http:// or https://".into(),
        ));
    }
    if parsed.host_str().is_none() {
        return Err(ConfigError::Invalid(
            "controlPlaneHttpProxy must include a host".into(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(ConfigError::Invalid(
            "controlPlaneHttpProxy must not contain credentials".into(),
        ));
    }
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(ConfigError::Invalid(
            "controlPlaneHttpProxy must not contain a path, query, or fragment".into(),
        ));
    }
    *value = trimmed.to_owned();
    Ok(())
}

/// Compare only configuration that affects Harness/Runtime restart semantics.
///
/// `UiConfig` is presentation state. It is persisted in the same local config
/// file, but changing it must not create Runtime restart debt or advance the
/// Runtime-facing revision. Keep every non-presentation field in this
/// comparison so a UI-only save cannot hide an unrelated desired change.
fn same_restart_semantics(a: &AppConfig, b: &AppConfig) -> bool {
    let mut left = a.clone();
    let mut right = b.clone();
    left.config_revision = DecimalU64::ZERO;
    right.config_revision = DecimalU64::ZERO;
    left.ui = UiConfig::default();
    right.ui = UiConfig::default();
    left.desktop = DesktopConfig::default();
    right.desktop = DesktopConfig::default();
    left == right
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&tmp, path)
    })();
    if result.is_err() && tmp.is_file() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(tmp: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(tmp, target)
}

#[cfg(windows)]
fn replace_file(tmp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let mut source: Vec<u16> = tmp.as_os_str().encode_wide().collect();
    let mut destination: Vec<u16> = target.as_os_str().encode_wide().collect();
    source.push(0);
    destination.push(0);
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        let error = std::io::Error::last_os_error();
        let _ = fs::remove_file(tmp);
        Err(error)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_control_protocol::{
        AppConfig, BashConfig, PolicyConfig, PolicyDecision, PolicyMatcherConfig,
        RuntimeConfigSnapshot, RuntimeLimits, SecurityConfig, SecurityMode, TunnelConfig, UiConfig,
        UiLanguage, UserPolicyRuleConfig, WorkspaceConfig,
    };
    fn c() -> AppConfig {
        AppConfig {
            schema_version: CURRENT_SCHEMA_VERSION,
            config_revision: DecimalU64::ZERO,
            workspace: WorkspaceConfig {
                root_path: std::env::temp_dir().display().to_string(),
            },
            tunnel: TunnelConfig {
                tunnel_id: "t".into(),
                control_plane_http_proxy: None,
                tunnel_client_path: None,
                auto_detect: true,
                api_key_ref: None,
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            ui: UiConfig::default(),
            tools: ToolConfig::default(),
            bash: BashConfig::default(),
            policies: PolicyConfig::default(),
            desktop: harness_control_protocol::DesktopConfig::default(),
            history: harness_control_protocol::HistoryConfig::default(),
            search: harness_control_protocol::SearchConfig::default(),
        }
    }
    #[test]
    fn missing_creates_safe_defaults() {
        let d = tempfile::tempdir().unwrap();
        let config = ConfigStore::new(d.path().into())
            .load_or_create_defaults()
            .unwrap();
        assert!(config.workspace.root_path.is_empty());
        assert!(d.path().join("config.json").exists());
    }

    #[test]
    fn inspect_read_only_never_creates_or_migrates_config() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().into());
        assert_eq!(store.inspect_read_only().unwrap(), None);
        assert!(!store.file.exists());

        fs::write(
            &store.file,
            include_bytes!("../../../../fixtures/config/v1-v2-era.json"),
        )
        .unwrap();
        let before = fs::read(&store.file).unwrap();
        let inspected = store.inspect_read_only().unwrap().unwrap();
        assert_eq!(inspected.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(fs::read(&store.file).unwrap(), before);
        assert!(!store.file.with_extension("json.tmp").exists());
    }

    #[test]
    fn safe_defaults_enable_m2_tools_and_keep_bash_disabled() {
        let tools = ConfigStore::safe_defaults().tools;

        assert!(tools.read_file.enabled);
        assert!(tools.glob.enabled);
        assert!(tools.grep.enabled);
        assert!(tools.write.enabled);
        assert!(tools.edit.enabled);
        assert!(!tools.bash.enabled);
        assert_eq!(
            ConfigStore::safe_defaults().policies.tool_defaults.grep,
            PolicyDecision::Allow
        );
    }

    #[test]
    fn legacy_schema_v1_without_tools_loads_safe_tool_defaults() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["schemaVersion"] = serde_json::json!(1);
        value.as_object_mut().unwrap().remove("tools");
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        let tools = ConfigStore::new(d.path().into())
            .load()
            .unwrap()
            .unwrap()
            .tools;
        let mut expected = ToolConfig::default();
        expected.grep.enabled = false;
        assert_eq!(tools, expected);
    }

    #[test]
    fn m05_pre_upgrade_v1_v2_v3_configs_gain_explicit_disabled_grep_without_revision_drift() {
        for schema_version in [1, 2, 3] {
            let directory = tempfile::tempdir().unwrap();
            let store = ConfigStore::new(directory.path().into());
            let mut value = serde_json::to_value(c()).unwrap();
            value["schemaVersion"] = serde_json::json!(schema_version);
            value["configRevision"] = serde_json::json!("47");
            if let Some(tools) = value
                .get_mut("tools")
                .and_then(|value| value.as_object_mut())
            {
                tools.remove("grep");
            }
            if let Some(defaults) = value
                .get_mut("policies")
                .and_then(|value| value.get_mut("toolDefaults"))
                .and_then(|value| value.as_object_mut())
            {
                defaults.remove("grep");
            }
            if schema_version < 3 {
                value.as_object_mut().unwrap().remove("desktop");
                value.as_object_mut().unwrap().remove("history");
            }
            let source = serde_json::to_vec_pretty(&value).unwrap();
            fs::write(&store.file, &source).unwrap();

            let loaded = store.load().unwrap().unwrap();
            assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
            assert_eq!(loaded.config_revision, DecimalU64::new(47));
            assert!(!loaded.tools.grep.enabled);
            assert_eq!(loaded.policies.tool_defaults.grep, PolicyDecision::Allow);
            let persisted: serde_json::Value =
                serde_json::from_slice(&fs::read(&store.file).unwrap()).unwrap();
            assert_eq!(persisted["schemaVersion"], CURRENT_SCHEMA_VERSION);
            assert_eq!(persisted["tools"]["grep"]["enabled"], false);
            assert_eq!(persisted["policies"]["toolDefaults"]["grep"], "allow");

            let before = fs::read(&store.file).unwrap();
            let inspected = store.inspect_read_only().unwrap().unwrap();
            assert!(!inspected.tools.grep.enabled);
            assert_eq!(fs::read(&store.file).unwrap(), before);
            assert!(!store.file.with_extension("json.tmp").exists());
        }
    }

    #[test]
    fn m05_explicit_grep_enable_advances_revision_and_preserves_new_install_default() {
        assert!(ToolConfig::default().grep.enabled);
        assert_eq!(
            PolicyConfig::default().tool_defaults.grep,
            PolicyDecision::Allow
        );

        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().into());
        let mut value = serde_json::to_value(c()).unwrap();
        value["configRevision"] = serde_json::json!("47");
        value["tools"].as_object_mut().unwrap().remove("grep");
        value["policies"]["toolDefaults"]
            .as_object_mut()
            .unwrap()
            .remove("grep");
        fs::write(&store.file, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let mut loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.config_revision, DecimalU64::new(47));
        assert!(!loaded.tools.grep.enabled);
        loaded.tools.grep.enabled = true;
        let saved = store.save(loaded).unwrap();
        assert_eq!(saved.config_revision, DecimalU64::new(48));
        assert!(saved.tools.grep.enabled);
        assert_eq!(
            store.load().unwrap().unwrap().config_revision,
            DecimalU64::new(48)
        );
    }

    #[test]
    fn search_defaults_load_and_valid_changes_advance_revision() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().into());
        let mut value = serde_json::to_value(c()).unwrap();
        value["configRevision"] = serde_json::json!("47");
        value.as_object_mut().unwrap().remove("search");
        fs::write(&store.file, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let mut loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.config_revision, DecimalU64::new(47));
        assert_eq!(
            loaded.search,
            harness_control_protocol::SearchConfig::default()
        );

        loaded.search.glob_timeout_seconds = 45;
        let saved = store.save(loaded.clone()).unwrap();
        assert_eq!(saved.config_revision, DecimalU64::new(48));
        assert_eq!(saved.search.glob_timeout_seconds, 45);

        loaded.search.glob_timeout_seconds = 4;
        assert!(store.save(loaded).is_err());
    }

    #[test]
    fn m05_failed_additive_persist_preserves_the_original_v3_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().into());
        let mut value = serde_json::to_value(c()).unwrap();
        value["configRevision"] = serde_json::json!("47");
        value["tools"].as_object_mut().unwrap().remove("grep");
        let source = serde_json::to_vec_pretty(&value).unwrap();
        fs::write(&store.file, &source).unwrap();
        fs::create_dir(store.file.with_extension("json.tmp")).unwrap();

        assert_eq!(store.load().unwrap_err().code(), "CONFIG_WRITE_FAILED");
        assert_eq!(fs::read(&store.file).unwrap(), source);
    }

    #[test]
    fn legacy_schema_v1_without_policies_loads_safe_policy_defaults() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["schemaVersion"] = serde_json::json!(1);
        value.as_object_mut().unwrap().remove("policies");
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        let policies = ConfigStore::new(d.path().into())
            .load()
            .unwrap()
            .unwrap()
            .policies;
        assert_eq!(policies, PolicyConfig::default());
        assert_eq!(policies.tool_defaults.read_file, PolicyDecision::Allow);
        assert_eq!(policies.tool_defaults.bash, PolicyDecision::Ask);
        assert!(policies.user_rules.is_empty());
    }

    #[test]
    fn legacy_schema_v1_without_bash_loads_safe_bash_defaults() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["schemaVersion"] = serde_json::json!(1);
        value.as_object_mut().unwrap().remove("bash");
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        let bash = ConfigStore::new(d.path().into())
            .load()
            .unwrap()
            .unwrap()
            .bash;
        assert_eq!(bash, BashConfig::default());
    }

    #[test]
    fn legacy_schema_v1_without_language_loads_with_system_follow_default() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["schemaVersion"] = serde_json::json!(1);
        value["ui"].as_object_mut().unwrap().remove("language");
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        let loaded = ConfigStore::new(d.path().into()).load().unwrap().unwrap();
        assert_eq!(loaded.ui.language, None);
    }

    #[test]
    fn ui_language_values_round_trip_and_clear_to_follow_system() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let first = store.save(c()).unwrap();

        let mut english = first.clone();
        english.ui.language = Some(UiLanguage::En);
        let english = store.save(english).unwrap();
        assert_eq!(english.ui.language, Some(UiLanguage::En));
        assert_eq!(english.config_revision, DecimalU64::new(1));

        let mut chinese = english.clone();
        chinese.ui.language = Some(UiLanguage::ZhCn);
        let chinese = store.save(chinese).unwrap();
        assert_eq!(chinese.ui.language, Some(UiLanguage::ZhCn));
        assert_eq!(chinese.config_revision, DecimalU64::new(1));

        let mut follow_system = chinese.clone();
        follow_system.ui.language = None;
        let follow_system = store.save(follow_system).unwrap();
        assert_eq!(follow_system.ui.language, None);
        assert_eq!(follow_system.config_revision, DecimalU64::new(1));
        assert_eq!(store.load().unwrap().unwrap().ui.language, None);
    }

    #[test]
    fn invalid_serialized_ui_language_fails_closed() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["ui"]["language"] = serde_json::json!("fr");
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            ConfigStore::new(d.path().into()).load(),
            Err(ConfigError::Invalid(_))
        ));
    }

    #[test]
    fn ui_only_theme_and_language_saves_keep_revision_stable() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let first = store.save(c()).unwrap();

        let mut themed = first.clone();
        themed.ui.theme = Some("dark".into());
        let themed = store.save(themed).unwrap();
        assert_eq!(themed.config_revision, DecimalU64::new(1));

        let mut localized = themed.clone();
        localized.ui.language = Some(UiLanguage::ZhCn);
        let localized = store.save(localized).unwrap();
        assert_eq!(localized.config_revision, DecimalU64::new(1));
    }

    #[test]
    fn language_combined_with_each_runtime_change_increments_once() {
        let runtime_changes = [
            "tool",
            "security",
            "tunnel",
            "workspace",
            "policy",
            "bash",
            "history",
        ];
        for change in runtime_changes {
            let d = tempfile::tempdir().unwrap();
            let store = ConfigStore::new(d.path().into());
            let first = store.save(c()).unwrap();
            let mut changed = first.clone();
            changed.ui.language = Some(UiLanguage::En);
            match change {
                "tool" => changed.tools.write.enabled = false,
                "security" => {
                    changed.security.mode = SecurityMode::DeveloperTest;
                    changed.security.developer_test_mode = true;
                }
                "tunnel" => changed.tunnel.tunnel_id = "next".into(),
                "workspace" => changed.workspace.root_path = r"D:\next".into(),
                "policy" => changed.policies.tool_defaults.write = PolicyDecision::Deny,
                "bash" => changed.bash.auto_detect = false,
                "history" => changed.history.retention_days = 31,
                _ => unreachable!(),
            }
            let changed = store.save(changed).unwrap();
            assert_eq!(changed.config_revision, DecimalU64::new(2), "{change}");
        }
    }

    #[test]
    fn history_defaults_and_ranges_are_generation_effective() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let first = store.save(c()).unwrap();
        assert!(first.history.enabled);
        assert_eq!(first.history.retention_days, 30);
        assert_eq!(first.history.max_storage_mib, 256);

        let mut expected_revision = first.config_revision;
        for (retention_days, max_storage_mib) in [(1, 32), (3650, 4096), (31, 256)] {
            let mut changed = first.clone();
            changed.history.retention_days = retention_days;
            changed.history.max_storage_mib = max_storage_mib;
            let saved = store.save(changed).unwrap();
            expected_revision = expected_revision.checked_add_one().unwrap();
            assert_eq!(saved.config_revision, expected_revision);
        }

        for (retention_days, max_storage_mib) in [(0, 256), (3651, 256), (30, 31), (30, 4097)] {
            let mut invalid = first.clone();
            invalid.history.retention_days = retention_days;
            invalid.history.max_storage_mib = max_storage_mib;
            assert!(matches!(store.save(invalid), Err(ConfigError::Invalid(_))));
        }
    }

    #[test]
    fn isolated_language_update_reads_current_config_and_preserves_unrelated_fields() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let mut original = c();
        original.workspace.root_path = "D:\\persisted".into();
        original.tunnel.tunnel_id = "persisted-tunnel".into();
        original.tools.write.enabled = false;
        let original = store.save(original).unwrap();

        let saved = store.set_ui_language(Some(UiLanguage::ZhCn)).unwrap();
        assert_eq!(saved.ui.language, Some(UiLanguage::ZhCn));
        assert_eq!(saved.workspace, original.workspace);
        assert_eq!(saved.tunnel, original.tunnel);
        assert_eq!(saved.tools, original.tools);
        assert_eq!(saved.config_revision, original.config_revision);

        let cleared = store.set_ui_language(None).unwrap();
        assert_eq!(cleared.ui.language, None);
        assert_eq!(cleared.workspace, original.workspace);
        assert_eq!(cleared.tunnel, original.tunnel);
        assert_eq!(cleared.tools, original.tools);
        assert_eq!(cleared.config_revision, original.config_revision);
    }

    #[test]
    fn isolated_language_update_uses_safe_defaults_when_config_is_missing() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());

        let saved = store.set_ui_language(Some(UiLanguage::En)).unwrap();
        assert_eq!(saved.ui.language, Some(UiLanguage::En));
        assert_eq!(saved.config_revision, DecimalU64::new(1));
        assert!(saved.workspace.root_path.is_empty());
        assert!(saved.tunnel.tunnel_id.is_empty());
        assert!(saved.tunnel.api_key_ref.is_none());

        let cleared = store.set_ui_language(None).unwrap();
        assert_eq!(cleared.ui.language, None);
        assert_eq!(cleared.config_revision, DecimalU64::new(1));
        assert!(cleared.workspace.root_path.is_empty());
        assert!(cleared.tunnel.tunnel_id.is_empty());
    }

    #[test]
    fn isolated_language_update_rejects_corrupt_config_without_overwriting_it() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("config.json");
        fs::write(&target, b"{not-json").unwrap();

        let result = ConfigStore::new(d.path().into()).set_ui_language(Some(UiLanguage::ZhCn));
        assert!(matches!(result, Err(ConfigError::Invalid(_))));
        assert_eq!(fs::read(target).unwrap(), b"{not-json");
    }

    #[test]
    fn isolated_language_atomic_save_failure_preserves_previous_config_and_revision() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let original = store.save(c()).unwrap();
        let target = d.path().join("config.json");
        let bytes = fs::read(&target).unwrap();
        // Deterministically prevent creating the temporary file; do not rely
        // on platform-specific ACLs or a privileged test account.
        fs::create_dir(d.path().join("config.json.tmp")).unwrap();

        let error = store.set_ui_language(Some(UiLanguage::ZhCn)).unwrap_err();
        assert_eq!(error.code(), "CONFIG_WRITE_FAILED");
        assert_eq!(fs::read(target).unwrap(), bytes);
        assert_eq!(store.load().unwrap().unwrap(), original);
    }

    #[test]
    fn runtime_snapshot_serialization_excludes_ui_language() {
        let mut config = c();
        config.ui.language = Some(UiLanguage::ZhCn);
        let snapshot = RuntimeConfigSnapshot {
            config_revision: config.config_revision,
            workspace: config.workspace,
            security: config.security,
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: 1,
                max_activity_events: 2,
                max_read_bytes: 3,
            },
            tools: config.tools,
            bash: config.bash,
            policies: config.policies,
            search: config.search,
        };
        let encoded = serde_json::to_value(snapshot).unwrap();
        assert!(encoded.get("ui").is_none());
        assert!(serde_json::to_string(&encoded)
            .unwrap()
            .contains("runtimeLimits"));
        assert!(!serde_json::to_string(&encoded).unwrap().contains("zh-CN"));
    }

    #[test]
    fn bash_config_round_trips_and_semantic_revision_is_stable() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let first = store.save(c()).unwrap();
        assert_eq!(first.config_revision, DecimalU64::new(1));
        assert_eq!(
            store.save(first.clone()).unwrap().config_revision,
            DecimalU64::new(1)
        );

        let mut changed = first.clone();
        changed.bash.auto_detect = false;
        changed.bash.executable_path = Some(r"C:\Git\bin\bash.exe".into());
        let changed = store.save(changed).unwrap();
        assert_eq!(changed.config_revision, DecimalU64::new(2));
        assert_eq!(store.load().unwrap().unwrap().bash, changed.bash);
        assert_eq!(
            store.save(changed).unwrap().config_revision,
            DecimalU64::new(2)
        );
    }

    #[test]
    fn save_and_reload_persists_policy_defaults_and_user_rule_crud() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let mut config = c();
        config.policies.tool_defaults.write = PolicyDecision::Ask;
        config.policies.user_rules.push(UserPolicyRuleConfig {
            id: "rule-write".into(),
            name: "Review writes".into(),
            enabled: true,
            decision: PolicyDecision::Ask,
            matcher: PolicyMatcherConfig::tool("write"),
            reason: "Writes need review.".into(),
        });
        let saved = store.save(config).unwrap();
        assert_eq!(saved.config_revision, DecimalU64::new(1));
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.policies, saved.policies);

        let mut edited = loaded.clone();
        edited.policies.user_rules[0].name = "Review changed writes".into();
        edited.policies.user_rules[0].enabled = false;
        edited.policies.user_rules[0].decision = PolicyDecision::Deny;
        let edited = store.save(edited).unwrap();
        assert_eq!(edited.config_revision, DecimalU64::new(2));

        let mut deleted = edited.clone();
        deleted.policies.user_rules.clear();
        let deleted = store.save(deleted).unwrap();
        assert_eq!(deleted.config_revision, DecimalU64::new(3));
        assert!(store
            .load()
            .unwrap()
            .unwrap()
            .policies
            .user_rules
            .is_empty());
    }

    #[test]
    fn identical_policy_save_keeps_revision_and_invalid_save_preserves_old_config() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let first = store.save(c()).unwrap();
        assert_eq!(
            store.save(first.clone()).unwrap().config_revision,
            DecimalU64::new(1)
        );

        let mut invalid = first.clone();
        invalid.policies.user_rules.push(UserPolicyRuleConfig {
            id: "duplicate".into(),
            name: "first".into(),
            enabled: true,
            decision: PolicyDecision::Allow,
            matcher: PolicyMatcherConfig::tool("write"),
            reason: "valid".into(),
        });
        invalid.policies.user_rules.push(UserPolicyRuleConfig {
            id: "duplicate".into(),
            name: "second".into(),
            enabled: true,
            decision: PolicyDecision::Deny,
            matcher: PolicyMatcherConfig::tool("write"),
            reason: "invalid duplicate".into(),
        });
        assert!(matches!(store.save(invalid), Err(ConfigError::Invalid(_))));
        assert_eq!(store.load().unwrap().unwrap(), first);
    }

    #[test]
    fn unsupported_or_unknown_policy_matchers_are_rejected_before_save() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let mut config = c();
        config.policies.user_rules.push(UserPolicyRuleConfig {
            id: "bad-tool".into(),
            name: "Bad tool".into(),
            enabled: true,
            decision: PolicyDecision::Allow,
            matcher: PolicyMatcherConfig::tool("not_a_tool"),
            reason: "unknown tool must remain rejected".into(),
        });
        assert!(matches!(store.save(config), Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn persisted_user_rule_cannot_claim_system_authority() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["policies"]["userRules"] = serde_json::json!([{
            "id": "forged",
            "name": "Forged system rule",
            "enabled": true,
            "decision": "allow",
            "matcher": { "type": "tool", "tool": "write" },
            "reason": "invalid",
            "source": "system",
            "editable": false
        }]);
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            ConfigStore::new(d.path().into()).load(),
            Err(ConfigError::Invalid(_))
        ));
    }

    #[test]
    fn save_and_reload_preserves_each_tool_toggle() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let mut config = c();
        config.tools.read_file.enabled = false;
        config.tools.glob.enabled = false;
        config.tools.write.enabled = false;
        config.tools.edit.enabled = false;
        config.tools.bash.enabled = true;

        let saved = store.save(config).unwrap();
        assert_eq!(saved.config_revision, DecimalU64::new(1));
        assert_eq!(store.load().unwrap().unwrap().tools, saved.tools);
    }

    #[test]
    fn tool_semantic_change_increments_revision_once_and_identical_state_does_not() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let first = store.save(c()).unwrap();
        assert_eq!(
            store.save(first.clone()).unwrap().config_revision,
            DecimalU64::new(1)
        );

        let mut changed = first.clone();
        changed.tools.write.enabled = false;
        let changed = store.save(changed).unwrap();
        assert_eq!(changed.config_revision, DecimalU64::new(2));
        assert_eq!(
            store.save(changed).unwrap().config_revision,
            DecimalU64::new(2)
        );
    }

    #[test]
    fn malformed_tool_state_fails_closed() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["tools"]["write"]["enabled"] = serde_json::json!("false");
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            ConfigStore::new(d.path().into()).load(),
            Err(ConfigError::Invalid(_))
        ));
    }

    #[test]
    fn malformed_tool_object_shape_fails_closed_without_overwriting_config() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("config.json");
        let mut value = serde_json::to_value(c()).unwrap();
        value["tools"]["write"] = serde_json::json!(true);
        let original = serde_json::to_vec_pretty(&value).unwrap();
        fs::write(&target, &original).unwrap();

        assert!(matches!(
            ConfigStore::new(d.path().into()).load(),
            Err(ConfigError::Invalid(_))
        ));
        assert_eq!(fs::read(target).unwrap(), original);
    }

    #[test]
    fn unknown_tool_state_field_fails_closed() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["tools"]["write"]["unknown"] = serde_json::json!(true);
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            ConfigStore::new(d.path().into()).load(),
            Err(ConfigError::Invalid(_))
        ));
    }
    #[test]
    fn revisions_are_semantic() {
        let d = tempfile::tempdir().unwrap();
        let s = ConfigStore::new(d.path().into());
        let a = s.save(c()).unwrap();
        assert_eq!(a.config_revision, DecimalU64::new(1));
        assert_eq!(
            s.save(a.clone()).unwrap().config_revision,
            DecimalU64::new(1)
        );
        let mut b = a;
        b.tunnel.tunnel_id = "next".into();
        assert_eq!(s.save(b).unwrap().config_revision, DecimalU64::new(2));
    }

    #[test]
    fn proxy_is_optional_normalized_and_semantic() {
        let d = tempfile::tempdir().unwrap();
        let s = ConfigStore::new(d.path().into());
        let mut config = c();
        config.tunnel.control_plane_http_proxy = Some("  http://127.0.0.1:7897  ".into());
        let saved = s.save(config).unwrap();
        assert_eq!(
            saved.tunnel.control_plane_http_proxy.as_deref(),
            Some("http://127.0.0.1:7897")
        );
        let mut changed = saved.clone();
        changed.tunnel.control_plane_http_proxy = None;
        assert_eq!(s.save(changed).unwrap().config_revision, DecimalU64::new(2));
    }

    #[test]
    fn legacy_config_without_proxy_field_still_loads() {
        let d = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(c()).unwrap();
        value["tunnel"]
            .as_object_mut()
            .unwrap()
            .remove("controlPlaneHttpProxy");
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
        let loaded = ConfigStore::new(d.path().into()).load().unwrap().unwrap();
        assert!(loaded.tunnel.control_plane_http_proxy.is_none());
    }

    #[test]
    fn proxy_validation_rejects_unsupported_or_sensitive_urls() {
        for invalid in [
            "socks5://127.0.0.1:7897",
            "http://user:secret@127.0.0.1:7897",
            "http://127.0.0.1:7897/path",
            "http://127.0.0.1:7897?mode=test",
            "http://127.0.0.1:7897#fragment",
            "http://",
        ] {
            let mut value = Some(invalid.to_owned());
            assert!(normalize_and_validate_control_plane_http_proxy(&mut value).is_err());
        }
        let mut blank = Some("   ".into());
        normalize_and_validate_control_plane_http_proxy(&mut blank).unwrap();
        assert!(blank.is_none());
    }
    #[test]
    fn load_normalizes_legacy_executable_path_and_save_persists_directory() {
        let d = tempfile::tempdir().unwrap();
        let package_dir = d.path().join("tunnel-client-v0.0.13-windows-amd64");
        fs::create_dir_all(&package_dir).unwrap();
        let executable = package_dir.join("tunnel-client.exe");
        fs::write(&executable, b"mock").unwrap();

        let mut legacy = c();
        legacy.tunnel.tunnel_client_path = Some(executable.display().to_string());
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec_pretty(&legacy).unwrap(),
        )
        .unwrap();

        let store = ConfigStore::new(d.path().into());
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(
            loaded.tunnel.tunnel_client_path.as_deref(),
            Some(package_dir.to_str().unwrap())
        );

        let saved = store.save(loaded).unwrap();
        assert_eq!(
            saved.tunnel.tunnel_client_path.as_deref(),
            Some(package_dir.to_str().unwrap())
        );
        let persisted: serde_json::Value =
            serde_json::from_slice(&fs::read(d.path().join("config.json")).unwrap()).unwrap();
        assert_eq!(
            persisted["tunnel"]["tunnelClientPath"].as_str(),
            Some(package_dir.to_str().unwrap())
        );
    }
    #[test]
    fn invalid_json_is_error() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path()).unwrap();
        fs::write(d.path().join("config.json"), b"{").unwrap();
        assert!(matches!(
            ConfigStore::new(d.path().into()).load(),
            Err(ConfigError::Invalid(_))
        ));
    }
    #[test]
    fn atomic_write_replaces_target_and_removes_temporary_file() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("config.json");
        fs::write(&target, b"old").unwrap();
        atomic_write(&target, b"new").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert!(!d.path().join("config.json.tmp").exists());
    }

    #[test]
    fn atomic_write_failure_keeps_existing_target_unchanged() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("config.json");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("sentinel"), b"old").unwrap();

        assert!(atomic_write(&target, b"new").is_err());
        assert!(target.is_dir());
        assert_eq!(fs::read(target.join("sentinel")).unwrap(), b"old");
        assert!(!d.path().join("config.json.tmp").exists());
    }

    #[test]
    fn safe_defaults_do_not_guess_workspace_or_external_connection() {
        let config = ConfigStore::safe_defaults();
        assert!(config.workspace.root_path.is_empty());
        assert!(config.tunnel.tunnel_id.is_empty());
        assert!(config.tunnel.tunnel_client_path.is_none());
        assert!(config.tunnel.api_key_ref.is_none());
    }

    #[test]
    fn unsupported_schema_fails_closed() {
        let d = tempfile::tempdir().unwrap();
        let mut config = ConfigStore::safe_defaults();
        config.schema_version = CURRENT_SCHEMA_VERSION + 1;
        fs::write(
            d.path().join("config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            ConfigStore::new(d.path().into()).load(),
            Err(ConfigError::UnsupportedSchema)
        ));
    }

    #[test]
    fn save_and_reload_preserves_explicit_workspace() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let workspace = d.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let mut config = c();
        config.workspace.root_path = workspace.display().to_string();
        store.save(config).unwrap();
        assert_eq!(
            store.load().unwrap().unwrap().workspace.root_path,
            workspace.display().to_string()
        );
    }

    #[test]
    fn secret_values_are_not_serialized() {
        let mut config = c();
        config.tunnel.api_key_ref = Some(harness_control_protocol::SecretReference {
            provider: "windows-credential-manager".into(),
            key: "tunnel-api-key".into(),
        });
        let encoded = serde_json::to_string(&config).unwrap();
        assert!(!encoded.contains("raw-api-key"));
        assert!(!encoded.contains("secret-value"));
        assert!(encoded.contains("tunnel-api-key"));
    }

    #[test]
    fn corrupt_config_cannot_be_overwritten_by_save() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("config.json");
        fs::write(&target, b"{not-json").unwrap();
        let error = ConfigStore::new(d.path().into()).save(c()).unwrap_err();
        assert!(matches!(error, ConfigError::Invalid(_)));
        assert_eq!(fs::read(&target).unwrap(), b"{not-json");
    }
    #[test]
    fn reset_removes_invalid_config() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        fs::write(d.path().join("config.json"), b"{").unwrap();
        store.reset().unwrap();
        assert!(!d.path().join("config.json").exists());
    }
    #[test]
    fn m7_dual_source_preservation_and_idempotency() {
        for transitional in [false, true] {
            let d = tempfile::tempdir().unwrap();
            let store = ConfigStore::new(d.path().into());
            let mut original = c();
            original.schema_version = 1;
            original.config_revision = DecimalU64::new(47);
            original.tunnel.api_key_ref = Some(harness_control_protocol::SecretReference {
                provider: "fake-provider".into(),
                key: "opaque-reference".into(),
            });
            original.tunnel.control_plane_http_proxy = Some("http://127.0.0.1:8080".into());
            original.security.mode = SecurityMode::DeveloperTest;
            original.security.developer_test_mode = false;
            original.tools.write.enabled = false;
            original.bash.auto_detect = false;
            original.bash.executable_path = Some(r"C:\fake\bash.exe".into());
            original.ui.language = Some(UiLanguage::ZhCn);
            original.ui.theme = Some("dark".into());
            original.policies.user_rules.push(UserPolicyRuleConfig {
                id: "deny-write".into(),
                name: "deny".into(),
                enabled: true,
                decision: PolicyDecision::Deny,
                matcher: PolicyMatcherConfig::tool("write"),
                reason: "test".into(),
            });
            let mut raw = serde_json::to_value(&original).unwrap();
            if !transitional {
                for field in ["tools", "policies", "bash"] {
                    raw.as_object_mut().unwrap().remove(field);
                }
                raw["ui"].as_object_mut().unwrap().remove("language");
            }
            fs::write(&store.file, serde_json::to_vec(&raw).unwrap()).unwrap();
            let migrated = store.load().unwrap().unwrap();
            assert_eq!(migrated.schema_version, CURRENT_SCHEMA_VERSION);
            assert_eq!(migrated.config_revision, original.config_revision);
            assert_eq!(migrated.workspace, original.workspace);
            assert_eq!(migrated.tunnel, original.tunnel);
            assert_eq!(migrated.ui.theme, original.ui.theme);
            assert_eq!(migrated.security.mode, original.security.mode);
            assert!(migrated.security.developer_test_mode);
            if transitional {
                let mut expected_tools = original.tools.clone();
                expected_tools.grep.enabled = false;
                assert_eq!(migrated.tools, expected_tools);
                assert_eq!(migrated.policies, original.policies);
                assert_eq!(migrated.bash, original.bash);
                assert_eq!(migrated.ui.language, original.ui.language);
            } else {
                let mut expected_tools = ToolConfig::default();
                expected_tools.grep.enabled = false;
                assert_eq!(migrated.tools, expected_tools);
                assert_eq!(migrated.policies, PolicyConfig::default());
                assert_eq!(migrated.bash, BashConfig::default());
                assert_eq!(migrated.ui.language, None);
            }
            let bytes = fs::read(&store.file).unwrap();
            let modified = fs::metadata(&store.file).unwrap().modified().unwrap();
            assert_eq!(store.load().unwrap().unwrap(), migrated);
            assert_eq!(fs::read(&store.file).unwrap(), bytes);
            assert_eq!(
                fs::metadata(&store.file).unwrap().modified().unwrap(),
                modified
            );
            assert_eq!(
                store
                    .set_ui_language(Some(UiLanguage::En))
                    .unwrap()
                    .config_revision,
                original.config_revision
            );
        }
    }

    #[test]
    fn m7_malformed_sources_preserve_exact_bytes_and_safe_errors() {
        let mut base = serde_json::to_value(c()).unwrap();
        base["schemaVersion"] = serde_json::json!(1);
        let mutations = [
            ("tools", serde_json::json!({"write":{"enabled":false}})),
            ("policies", serde_json::json!({})),
            ("bash", serde_json::json!({"autoDetect":"SECRET_SENTINEL"})),
            ("ui", serde_json::json!({"language":"SECRET_SENTINEL"})),
            (
                "security",
                serde_json::json!({"mode":"SECRET_SENTINEL", "developerTestMode":false}),
            ),
            ("unexpectedSecret", serde_json::json!("SECRET_SENTINEL")),
        ];
        for (key, value) in mutations {
            let d = tempfile::tempdir().unwrap();
            let store = ConfigStore::new(d.path().into());
            let mut raw = base.clone();
            raw[key] = value;
            let bytes = serde_json::to_vec(&raw).unwrap();
            fs::write(&store.file, &bytes).unwrap();
            let error = store.load().unwrap_err();
            assert_eq!(error.code(), "CONFIG_MIGRATION_FAILED", "{key}");
            assert!(!error.message().contains("SECRET_SENTINEL"));
            assert_eq!(fs::read(&store.file).unwrap(), bytes);
            assert!(!store.file.with_extension("json.tmp").exists());
        }
        for version in [
            serde_json::Value::Null,
            serde_json::json!(0),
            serde_json::json!(4),
            serde_json::json!("1"),
            serde_json::json!(-1),
        ] {
            let d = tempfile::tempdir().unwrap();
            let store = ConfigStore::new(d.path().into());
            let mut raw = base.clone();
            raw["schemaVersion"] = version;
            let bytes = serde_json::to_vec(&raw).unwrap();
            fs::write(&store.file, &bytes).unwrap();
            assert_eq!(
                store.load().unwrap_err().code(),
                "CONFIG_SCHEMA_UNSUPPORTED"
            );
            assert_eq!(fs::read(&store.file).unwrap(), bytes);
        }
    }

    #[test]
    fn m7_migration_persist_failure_preserves_source() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let mut raw = serde_json::to_value(c()).unwrap();
        raw["schemaVersion"] = serde_json::json!(1);
        let bytes = serde_json::to_vec(&raw).unwrap();
        fs::write(&store.file, &bytes).unwrap();
        // A reserved temporary pathname deterministically prevents temp creation.
        fs::create_dir(store.file.with_extension("json.tmp")).unwrap();
        assert_eq!(store.load().unwrap_err().code(), "CONFIG_WRITE_FAILED");
        assert_eq!(fs::read(&store.file).unwrap(), bytes);
    }

    #[test]
    fn m7_invalid_policy_proxy_and_missing_version_preserve_source() {
        let mut base = serde_json::to_value(c()).unwrap();
        base["schemaVersion"] = serde_json::json!(1);
        for case in ["policy", "proxy", "missing-version", "raw-secret"] {
            let d = tempfile::tempdir().unwrap();
            let store = ConfigStore::new(d.path().into());
            let mut raw = base.clone();
            match case {
                "policy" => {
                    raw["policies"]["userRules"] = serde_json::json!([{
                        "id":"bad", "name":"bad", "enabled":true, "decision":"allow",
                        "matcher":{"type":"tool","tool":"unknown-tool"}, "reason":"SECRET_SENTINEL"
                    }])
                }
                "proxy" => {
                    raw["tunnel"]["controlPlaneHttpProxy"] =
                        serde_json::json!("http://user:SECRET_SENTINEL@127.0.0.1")
                }
                "raw-secret" => raw["tunnel"]["apiKey"] = serde_json::json!("SECRET_SENTINEL"),
                _ => {
                    raw.as_object_mut().unwrap().remove("schemaVersion");
                }
            }
            let bytes = serde_json::to_vec(&raw).unwrap();
            fs::write(&store.file, &bytes).unwrap();
            let error = store.load().unwrap_err();
            assert_eq!(
                error.code(),
                if case == "missing-version" {
                    "CONFIG_SCHEMA_UNSUPPORTED"
                } else {
                    "CONFIG_MIGRATION_FAILED"
                }
            );
            assert!(!error.message().contains("SECRET_SENTINEL"));
            assert_eq!(fs::read(&store.file).unwrap(), bytes);
            assert!(!store.file.with_extension("json.tmp").exists());
        }
    }
    #[cfg(windows)]
    #[test]
    fn m7_failed_atomic_replace_cleans_created_temp_and_keeps_original() {
        let d = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(d.path().into());
        let mut raw = serde_json::to_value(c()).unwrap();
        raw["schemaVersion"] = serde_json::json!(1);
        let bytes = serde_json::to_vec(&raw).unwrap();
        fs::write(&store.file, &bytes).unwrap();
        let mut permissions = fs::metadata(&store.file).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&store.file, permissions.clone()).unwrap();
        assert_eq!(store.load().unwrap_err().code(), "CONFIG_WRITE_FAILED");
        assert_eq!(fs::read(&store.file).unwrap(), bytes);
        assert!(!store.file.with_extension("json.tmp").exists());
        permissions.set_readonly(false);
        fs::set_permissions(&store.file, permissions).unwrap();
    }

    #[test]
    fn m7_current_schema_rejects_partial_and_null_fields_without_defaulting() {
        for field in ["tools", "policies", "bash"] {
            for value in [serde_json::Value::Null, serde_json::json!({})] {
                let d = tempfile::tempdir().unwrap();
                let store = ConfigStore::new(d.path().into());
                let mut raw = serde_json::to_value(c()).unwrap();
                raw[field] = value;
                let bytes = serde_json::to_vec(&raw).unwrap();
                fs::write(&store.file, &bytes).unwrap();
                assert_eq!(store.load().unwrap_err().code(), "CONFIG_INVALID");
                assert_eq!(fs::read(&store.file).unwrap(), bytes);
            }
        }
    }

    #[test]
    fn m7_historical_fixture_regression_matrix() {
        let fixtures = [
            (
                "v1-v2-era",
                include_str!("../../../../fixtures/config/v1-v2-era.json"),
                true,
            ),
            (
                "v1-transitional-m6",
                include_str!("../../../../fixtures/config/v1-transitional-m6.json"),
                true,
            ),
            (
                "v1-invalid-policy",
                include_str!("../../../../fixtures/config/v1-invalid-policy.json"),
                false,
            ),
            (
                "v1-invalid-proxy",
                include_str!("../../../../fixtures/config/v1-invalid-proxy.json"),
                false,
            ),
            (
                "v1-unsupported-field",
                include_str!("../../../../fixtures/config/v1-unsupported-field.json"),
                false,
            ),
            (
                "v2-current",
                include_str!("../../../../fixtures/config/v2-current.json"),
                true,
            ),
        ];
        for (name, source, valid) in fixtures {
            let directory = tempfile::tempdir().unwrap();
            let store = ConfigStore::new(directory.path().into());
            fs::write(&store.file, source.as_bytes()).unwrap();
            let before = fs::read(&store.file).unwrap();
            let raw: serde_json::Value = serde_json::from_slice(&before).unwrap();
            let original: AppConfig = serde_json::from_value(raw.clone())
                .unwrap_or_else(|_| ConfigStore::safe_defaults());
            let result = store.load();
            if !valid {
                let error = result.unwrap_err();
                assert_eq!(error.code(), "CONFIG_MIGRATION_FAILED", "{name}");
                assert_eq!(fs::read(&store.file).unwrap(), before, "{name}");
                assert!(!error.message().contains("FAKE_SECRET_SENTINEL"));
            } else {
                let config = result.unwrap().unwrap();
                assert_eq!(config.schema_version, CURRENT_SCHEMA_VERSION);
                assert_eq!(config.config_revision, DecimalU64::new(41));
                assert_eq!(config.workspace, original.workspace);
                assert_eq!(config.tunnel, original.tunnel);
                assert_eq!(config.security.mode, original.security.mode);
                assert!(config.security.developer_test_mode);
                assert_eq!(config.ui, original.ui);
                let mut expected_tools = original.tools.clone();
                expected_tools.grep.enabled = false;
                assert_eq!(config.tools, expected_tools);
                assert_eq!(config.policies, original.policies);
                assert_eq!(config.bash, original.bash);
                let after = fs::read(&store.file).unwrap();
                assert!(!String::from_utf8_lossy(&after).contains("FAKE_SECRET_SENTINEL"));
                if name == "v2-current" {
                    assert_ne!(after, before);
                    assert_eq!(
                        serde_json::from_slice::<serde_json::Value>(&after).unwrap()
                            ["schemaVersion"],
                        CURRENT_SCHEMA_VERSION
                    );
                }
                let modified = fs::metadata(&store.file).unwrap().modified().unwrap();
                assert_eq!(store.load().unwrap().unwrap(), config);
                assert_eq!(fs::read(&store.file).unwrap(), after);
                assert_eq!(
                    fs::metadata(&store.file).unwrap().modified().unwrap(),
                    modified
                );
            }
            assert!(!store.file.with_extension("json.tmp").exists());
        }
    }

    #[test]
    fn m7_migration_preserves_opaque_reference_without_materializing_secret_store_value() {
        use crate::secrets::SecretStore;
        let secrets = crate::secrets::MemorySecretStore::default();
        secrets
            .set("opaque-fake-reference", "API_SECRET_CONTENT_SENTINEL")
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().into());
        fs::write(
            &store.file,
            include_bytes!("../../../../fixtures/config/v1-v2-era.json"),
        )
        .unwrap();
        let config = store.load().unwrap().unwrap();
        assert_eq!(
            config.tunnel.api_key_ref.unwrap().key,
            "opaque-fake-reference"
        );
        let persisted = fs::read_to_string(&store.file).unwrap();
        assert!(!persisted.contains("API_SECRET_CONTENT_SENTINEL"));
        assert_eq!(
            secrets.get("opaque-fake-reference").unwrap().as_deref(),
            Some("API_SECRET_CONTENT_SENTINEL")
        );
    }
}
