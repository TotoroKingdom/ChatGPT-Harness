use super::SupervisorError;
use harness_control_protocol::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub(crate) const DISTRIBUTION_SCHEMA_VERSION: u32 = 1;
pub(crate) const DISTRIBUTION_ID: &str = "chatgpt-harness";
pub(crate) const RUNTIME_RELATIVE_PATH: &str = "runtime/chatgpt-harness-runtime.exe";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DistributionManifest {
    pub(crate) distribution_schema_version: u32,
    pub(crate) distribution_id: String,
    pub(crate) app_version: String,
    pub(crate) runtime_version: String,
    pub(crate) protocol_version: u32,
    pub(crate) runtime: RuntimeManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeManifest {
    pub(crate) relative_path: String,
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
    Ok(manifest.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> DistributionManifest {
        DistributionManifest {
            distribution_schema_version: 1,
            distribution_id: DISTRIBUTION_ID.into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            runtime_version: env!("CARGO_PKG_VERSION").into(),
            protocol_version: PROTOCOL_VERSION,
            runtime: RuntimeManifest {
                relative_path: RUNTIME_RELATIVE_PATH.into(),
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
        value.distribution_schema_version = 2;
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
        value.protocol_version += 1;
        assert!(validate(&value).is_err());
    }
}
