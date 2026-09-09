use harness_control_protocol::{
    AppConfig, DecimalU64, SecurityConfig, SecurityMode, TunnelConfig, UiConfig, WorkspaceConfig,
};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigMetadata {
    pub config_directory: String,
    pub config_file: String,
}

#[derive(Debug)]
pub enum ConfigError {
    Invalid(String),
    Io(String),
}
impl ConfigError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "CONFIG_INVALID",
            Self::Io(_) => "CONFIG_WRITE_FAILED",
        }
    }
    pub fn message(&self) -> String {
        match self {
            Self::Invalid(e) => format!("Configuration needs attention: {e}"),
            Self::Io(e) => format!("Could not save configuration: {e}"),
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
        if !self.file.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&self.file).map_err(|e| ConfigError::Io(e.to_string()))?;
        let mut config: AppConfig =
            serde_json::from_str(&text).map_err(|e| ConfigError::Invalid(e.to_string()))?;
        if config.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(ConfigError::Invalid(format!(
                "unsupported schemaVersion {}",
                config.schema_version
            )));
        }
        if let Some(path) = config.tunnel.tunnel_client_path.as_mut() {
            let normalized = crate::platform::normalize_tunnel_client_directory(path);
            *path = normalized.display().to_string();
        }
        Ok(Some(config))
    }
    pub fn load_or_create_defaults(&self) -> Result<AppConfig, ConfigError> {
        match self.load()? {
            Some(config) => Ok(config),
            None => self.save(Self::safe_defaults()),
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
                tunnel_client_path: None,
                auto_detect: true,
                api_key_ref: None,
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            ui: UiConfig::default(),
        }
    }

    pub fn save(&self, mut config: AppConfig) -> Result<AppConfig, ConfigError> {
        if config.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(ConfigError::Invalid("unsupported schemaVersion".into()));
        }
        if let Some(path) = config.tunnel.tunnel_client_path.as_mut() {
            let normalized = crate::platform::normalize_tunnel_client_directory(path);
            *path = normalized.display().to_string();
        }
        let previous = self.load()?;
        config.config_revision = match previous {
            None => DecimalU64::new(1),
            Some(old) if same_semantics(&old, &config) => old.config_revision,
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
    pub fn reset(&self) -> Result<(), ConfigError> {
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
fn same_semantics(a: &AppConfig, b: &AppConfig) -> bool {
    let mut left = a.clone();
    let mut right = b.clone();
    left.config_revision = DecimalU64::ZERO;
    right.config_revision = DecimalU64::ZERO;
    left == right
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    replace_file(&tmp, path)
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
        AppConfig, SecurityConfig, SecurityMode, TunnelConfig, UiConfig, WorkspaceConfig,
    };
    fn c() -> AppConfig {
        AppConfig {
            schema_version: 1,
            config_revision: DecimalU64::ZERO,
            workspace: WorkspaceConfig {
                root_path: std::env::temp_dir().display().to_string(),
            },
            tunnel: TunnelConfig {
                tunnel_id: "t".into(),
                tunnel_client_path: None,
                auto_detect: true,
                api_key_ref: None,
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            ui: UiConfig::default(),
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
            Err(ConfigError::Invalid(message)) if message.contains("unsupported schemaVersion")
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
}
