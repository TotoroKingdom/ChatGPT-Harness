use super::SupervisorError;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const DEVELOPMENT_RUNTIME_FILE_NAME: &str = if cfg!(windows) {
    "chatgpt-harness-runtime.exe"
} else {
    "chatgpt-harness-runtime"
};
pub(crate) const INSTALLED_RUNTIME_FILE_NAME: &str = "chatgpt-harness-runtime.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeDistributionMode {
    Development,
    Installed,
}

impl RuntimeDistributionMode {
    pub(crate) const fn current() -> Self {
        if cfg!(debug_assertions) {
            Self::Development
        } else {
            Self::Installed
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedRuntime {
    pub(crate) executable_path: PathBuf,
    pub(crate) mode: RuntimeDistributionMode,
    pub(crate) manifest: Option<super::manifest::DistributionManifest>,
}

impl ResolvedRuntime {
    pub(crate) fn expected_runtime_version(&self) -> &str {
        self.manifest
            .as_ref()
            .map(|manifest| manifest.runtime_version.as_str())
            .unwrap_or(env!("CARGO_PKG_VERSION"))
    }
}

pub(crate) fn resolve_runtime() -> Result<ResolvedRuntime, SupervisorError> {
    resolve_runtime_for_mode(RuntimeDistributionMode::current(), None)
}

fn resolve_runtime_for_mode(
    mode: RuntimeDistributionMode,
    current_exe_override: Option<&Path>,
) -> Result<ResolvedRuntime, SupervisorError> {
    match mode {
        RuntimeDistributionMode::Development => {
            let path = development_runtime_path();
            Ok(ResolvedRuntime {
                executable_path: validate_executable(path, None, DEVELOPMENT_RUNTIME_FILE_NAME)?,
                mode,
                manifest: None,
            })
        }
        RuntimeDistributionMode::Installed => {
            let current_exe = match current_exe_override {
                Some(path) => path.to_path_buf(),
                None => std::env::current_exe().map_err(|_| {
                    SupervisorError::distribution("Installed application location is unavailable.")
                })?,
            };
            let install_root = current_exe.parent().ok_or_else(|| {
                SupervisorError::distribution("Installed application location is invalid.")
            })?;
            let manifest_path = install_root.join("distribution.json");
            let manifest = super::manifest::read_and_validate(&manifest_path)?;
            let runtime_dir = install_root.join("runtime");
            let runtime_metadata = fs::symlink_metadata(&runtime_dir).map_err(|_| {
                SupervisorError::distribution("Installed Runtime directory is unavailable.")
            })?;
            if is_link_or_reparse_point(&runtime_metadata) {
                return Err(SupervisorError::distribution(
                    "Installed Runtime directory is invalid.",
                ));
            }
            let candidate = runtime_dir.join(INSTALLED_RUNTIME_FILE_NAME);
            let executable_path =
                validate_executable(candidate, Some(install_root), INSTALLED_RUNTIME_FILE_NAME)?;
            Ok(ResolvedRuntime {
                executable_path,
                mode,
                manifest: Some(manifest),
            })
        }
    }
}

#[cfg(windows)]
fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_type().is_symlink() || (metadata.file_attributes() & 0x400) != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn development_runtime_path() -> PathBuf {
    // One deterministic strategy: the workspace Cargo target directory selected
    // by the UI build profile. This does not consult PATH, CWD, or environment.
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join(profile)
        .join(DEVELOPMENT_RUNTIME_FILE_NAME)
}

fn validate_executable(
    path: PathBuf,
    install_root: Option<&Path>,
    expected_filename: &str,
) -> Result<PathBuf, SupervisorError> {
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| SupervisorError::distribution("Required Runtime executable is missing."))?;
    if !metadata.file_type().is_file() || is_link_or_reparse_point(&metadata) {
        return Err(SupervisorError::distribution(
            "Required Runtime executable is not a regular file.",
        ));
    }
    let canonical = fs::canonicalize(&path).map_err(|_| {
        SupervisorError::distribution("Required Runtime executable cannot be canonicalized.")
    })?;
    if canonical.file_name().and_then(|name| name.to_str()) != Some(expected_filename) {
        return Err(SupervisorError::distribution(
            "Runtime executable has an unexpected filename.",
        ));
    }
    if let Some(root) = install_root {
        let canonical_root = fs::canonicalize(root).map_err(|_| {
            SupervisorError::distribution("Installed application location cannot be canonicalized.")
        })?;
        let expected_dir = canonical_root.join("runtime");
        let canonical_dir = fs::canonicalize(&expected_dir).map_err(|_| {
            SupervisorError::distribution("Installed Runtime directory is unavailable.")
        })?;
        if canonical.parent() != Some(canonical_dir.as_path()) {
            return Err(SupervisorError::distribution(
                "Runtime executable is outside the installed Runtime directory.",
            ));
        }
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_executable(path: &Path) {
        fs::write(path, b"runtime").expect("write executable fixture");
    }

    fn manifest() -> String {
        format!(
            r#"{{"distribution_schema_version":2,"distribution_id":"chatgpt-harness","app_version":"{}","runtime_version":"{}","protocol_version":1,"runtime":{{"relative_path":"runtime/chatgpt-harness-runtime.exe"}},"tunnel":{{"version":"0.0.13","relative_path":"tunnel/tunnel-client.exe","cloudflared_relative_path":"tunnel/cloudflared.exe"}}}}"#,
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_VERSION")
        )
    }

    #[test]
    fn installed_resolution_is_deterministic_and_validates_layout() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("runtime")).expect("runtime dir");
        write_executable(&dir.path().join("chatgpt-harness-ui.exe"));
        write_executable(&dir.path().join("runtime").join(INSTALLED_RUNTIME_FILE_NAME));
        fs::write(dir.path().join("distribution.json"), manifest()).expect("manifest");
        let result = resolve_runtime_for_mode(
            RuntimeDistributionMode::Installed,
            Some(&dir.path().join("chatgpt-harness-ui.exe")),
        )
        .expect("resolve installed runtime");
        assert_eq!(result.mode, RuntimeDistributionMode::Installed);
        assert_eq!(
            result.executable_path.file_name().unwrap(),
            INSTALLED_RUNTIME_FILE_NAME
        );
    }

    #[test]
    fn installed_resolution_fails_closed_for_missing_runtime_and_manifest() {
        let dir = tempdir().expect("tempdir");
        write_executable(&dir.path().join("chatgpt-harness-ui.exe"));
        let error = resolve_runtime_for_mode(
            RuntimeDistributionMode::Installed,
            Some(&dir.path().join("chatgpt-harness-ui.exe")),
        )
        .expect_err("missing manifest must fail");
        assert_eq!(error.code, "RUNTIME_DISTRIBUTION_INVALID");
    }

    #[test]
    fn installed_resolution_rejects_wrong_filename() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("runtime")).expect("runtime dir");
        write_executable(&dir.path().join("chatgpt-harness-ui.exe"));
        write_executable(&dir.path().join("runtime").join("wrong.exe"));
        fs::write(dir.path().join("distribution.json"), manifest()).expect("manifest");
        let error = resolve_runtime_for_mode(
            RuntimeDistributionMode::Installed,
            Some(&dir.path().join("chatgpt-harness-ui.exe")),
        )
        .expect_err("wrong filename must fail");
        assert_eq!(error.code, "RUNTIME_DISTRIBUTION_INVALID");
    }
}
