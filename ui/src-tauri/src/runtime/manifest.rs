use super::SupervisorError;
use harness_control_protocol::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub(crate) const DISTRIBUTION_SCHEMA_VERSION: u32 = 2;
pub(crate) const DISTRIBUTION_ID: &str = "chatgpt-harness";
pub(crate) const RUNTIME_RELATIVE_PATH: &str = "runtime/chatgpt-harness-runtime.exe";
pub(crate) const TUNNEL_CLIENT_RELATIVE_PATH: &str = "tunnel/tunnel-client.exe";
pub(crate) const CLOUDFLARED_RELATIVE_PATH: &str = "tunnel/cloudflared.exe";
pub(crate) const TUNNEL_CLIENT_VERSION: &str = "0.0.13";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DistributionManifest {
    pub(crate) distribution_schema_version: u32,
    pub(crate) distribution_id: String,
    pub(crate) app_version: String,
    pub(crate) runtime_version: String,
    #[serde(default)]
    pub(crate) build_sha: Option<String>,
    #[serde(default)]
    pub(crate) source_dirty: bool,
    #[serde(default)]
    pub(crate) source_state_sha256: Option<String>,
    #[serde(default)]
    pub(crate) ui_sha256: Option<String>,
    #[serde(default)]
    pub(crate) runtime_sha256: Option<String>,
    #[serde(default)]
    pub(crate) tunnel_client_sha256: Option<String>,
    #[serde(default)]
    pub(crate) cloudflared_sha256: Option<String>,
    pub(crate) protocol_version: u32,
    pub(crate) runtime: RuntimeManifest,
    pub(crate) tunnel: TunnelManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeManifest {
    pub(crate) relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TunnelManifest {
    pub(crate) version: String,
    pub(crate) relative_path: String,
    pub(crate) cloudflared_relative_path: String,
}

pub(crate) fn read_and_validate(path: &Path) -> Result<DistributionManifest, SupervisorError> {
    let text = fs::read_to_string(path).map_err(|_| {
        SupervisorError::distribution("Installed distribution manifest is missing.")
    })?;
    let manifest: DistributionManifest = serde_json::from_str(&text).map_err(|_| {
        SupervisorError::distribution("Installed distribution manifest is malformed.")
    })?;
    validate(&manifest)
}

pub(crate) fn validate(
    manifest: &DistributionManifest,
) -> Result<DistributionManifest, SupervisorError> {
    if manifest.distribution_schema_version != DISTRIBUTION_SCHEMA_VERSION {
        return Err(SupervisorError::distribution(
            "Installed distribution manifest schema is unsupported.",
        ));
    }
    if manifest.distribution_id != DISTRIBUTION_ID {
        return Err(SupervisorError::distribution(
            "Installed distribution identity does not match.",
        ));
    }
    if manifest.runtime.relative_path != RUNTIME_RELATIVE_PATH
        || Path::new(&manifest.runtime.relative_path).is_absolute()
        || manifest.runtime.relative_path.contains("..")
    {
        return Err(SupervisorError::distribution(
            "Installed distribution Runtime path is invalid.",
        ));
    }
    if manifest.tunnel.version != TUNNEL_CLIENT_VERSION
        || manifest.tunnel.relative_path != TUNNEL_CLIENT_RELATIVE_PATH
        || manifest.tunnel.cloudflared_relative_path != CLOUDFLARED_RELATIVE_PATH
        || !valid_relative_path(&manifest.tunnel.relative_path)
        || !valid_relative_path(&manifest.tunnel.cloudflared_relative_path)
    {
        return Err(SupervisorError::distribution(
            "Installed distribution Tunnel payload is invalid.",
        ));
    }
    if manifest.protocol_version != PROTOCOL_VERSION {
        return Err(SupervisorError::distribution(
            "Installed distribution protocol version is incompatible.",
        ));
    }
    if manifest.app_version != env!("CARGO_PKG_VERSION") {
        return Err(SupervisorError::distribution(
            "Installed distribution application version is incompatible.",
        ));
    }
    if manifest.runtime_version != manifest.app_version {
        return Err(SupervisorError::distribution(
            "Installed distribution Runtime version is incompatible.",
        ));
    }
    for hash in [
        manifest.ui_sha256.as_deref(),
        manifest.runtime_sha256.as_deref(),
        manifest.tunnel_client_sha256.as_deref(),
        manifest.cloudflared_sha256.as_deref(),
        manifest.source_state_sha256.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if hash.len() != 64 || !hash.chars().all(|value| value.is_ascii_hexdigit()) {
            return Err(SupervisorError::distribution(
                "Installed distribution provenance hash is invalid.",
            ));
        }
    }
    Ok(manifest.clone())
}

fn valid_relative_path(path: &str) -> bool {
    !Path::new(path).is_absolute() && !path.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> DistributionManifest {
        DistributionManifest {
            distribution_schema_version: 2,
            distribution_id: DISTRIBUTION_ID.into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            runtime_version: env!("CARGO_PKG_VERSION").into(),
            build_sha: None,
            source_dirty: false,
            source_state_sha256: None,
            ui_sha256: None,
            runtime_sha256: None,
            tunnel_client_sha256: None,
            cloudflared_sha256: None,
            protocol_version: PROTOCOL_VERSION,
            runtime: RuntimeManifest {
                relative_path: RUNTIME_RELATIVE_PATH.into(),
            },
            tunnel: TunnelManifest {
                version: TUNNEL_CLIENT_VERSION.into(),
                relative_path: TUNNEL_CLIENT_RELATIVE_PATH.into(),
                cloudflared_relative_path: CLOUDFLARED_RELATIVE_PATH.into(),
            },
        }
    }

    #[test]
    fn accepts_valid_manifest() {
        assert!(validate(&valid()).is_ok());
    }

    #[test]
    fn rejects_schema_identity_path_and_versions() {
        let mut value = valid();
        value.distribution_schema_version = 1;
        assert!(validate(&value).is_err());
        let mut value = valid();
        value.tunnel.relative_path = "../outside.exe".into();
        assert!(validate(&value).is_err());
        let mut value = valid();
        value.tunnel.cloudflared_relative_path = "cloudflared.exe".into();
        assert!(validate(&value).is_err());
        let mut value = valid();
        value.distribution_id = "other".into();
        assert!(validate(&value).is_err());
        let mut value = valid();
        value.runtime.relative_path = "../outside.exe".into();
        assert!(validate(&value).is_err());
        let mut value = valid();
        value.app_version = "0.0.0".into();
        assert!(validate(&value).is_err());
        let mut value = valid();
        value.runtime_version = "0.0.0".into();
        assert!(validate(&value).is_err());
        let mut value = valid();
        value.source_state_sha256 = Some("not-a-sha".into());
        assert!(validate(&value).is_err());
        let mut value = valid();
        value.protocol_version += 1;
        assert!(validate(&value).is_err());
    }
}
