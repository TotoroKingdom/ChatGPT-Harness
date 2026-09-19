use super::TunnelProcessError;
use crate::runtime::manifest;
use harness_control_protocol::AppConfig;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const DEVELOPMENT_TUNNEL_PACKAGE: &str = "tunnel-client-v0.0.13-windows-amd64";
const TUNNEL_CLIENT_FILE_NAME: &str = "tunnel-client.exe";
const CLOUDFLARED_FILE_NAME: &str = "cloudflared.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TunnelDistributionMode {
    Development,
    Installed,
}

impl TunnelDistributionMode {
    pub(crate) const fn current() -> Self {
        if cfg!(debug_assertions) {
            Self::Development
        } else {
            Self::Installed
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedTunnel {
    pub(crate) client_path: PathBuf,
    pub(crate) cloudflared_path: PathBuf,
    pub(crate) package_dir: PathBuf,
    pub(crate) mode: TunnelDistributionMode,
    pub(crate) version: String,
}

pub(crate) fn resolve_tunnel(config: &AppConfig) -> Result<ResolvedTunnel, TunnelProcessError> {
    resolve_tunnel_for_mode(
        TunnelDistributionMode::current(),
        config.tunnel.tunnel_client_path.as_deref().map(Path::new),
        None,
    )
}

pub(crate) fn resolve_tunnel_for_mode(
    mode: TunnelDistributionMode,
    developer_override: Option<&Path>,
    current_exe_override: Option<&Path>,
) -> Result<ResolvedTunnel, TunnelProcessError> {
    match mode {
        TunnelDistributionMode::Development => {
            let package_dir = developer_override
                .map(Path::to_path_buf)
                .unwrap_or_else(development_tunnel_package_path);
            resolve_package(
                package_dir,
                mode,
                manifest::TUNNEL_CLIENT_VERSION.to_owned(),
            )
        }
        TunnelDistributionMode::Installed => {
            let current_exe = current_exe_override
                .map(Path::to_path_buf)
                .unwrap_or_else(|| std::env::current_exe().unwrap_or_default());
            let install_root = current_exe.parent().ok_or_else(|| {
                TunnelProcessError::distribution("Installed application location is invalid.")
            })?;
            let distribution = manifest::read_and_validate(&install_root.join("distribution.json"))
                .map_err(|error| TunnelProcessError::distribution(error.message))?;
            let package_dir = install_root.join("tunnel");
            let tunnel_root =
                canonical_directory(&package_dir, "Installed Tunnel directory is invalid.")?;
            let client = install_root.join(&distribution.tunnel.relative_path);
            let cloudflared = install_root.join(&distribution.tunnel.cloudflared_relative_path);
            let client_path = validate_payload_file(
                client,
                &tunnel_root,
                TUNNEL_CLIENT_FILE_NAME,
                "Installed tunnel-client executable is invalid.",
            )?;
            let cloudflared_path = validate_payload_file(
                cloudflared,
                &tunnel_root,
                CLOUDFLARED_FILE_NAME,
                "Installed cloudflared executable is invalid.",
            )?;
            Ok(ResolvedTunnel {
                client_path,
                cloudflared_path,
                package_dir: tunnel_root,
                mode,
                version: distribution.tunnel.version,
            })
        }
    }
}

fn development_tunnel_package_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(DEVELOPMENT_TUNNEL_PACKAGE)
}

fn resolve_package(
    package_dir: PathBuf,
    mode: TunnelDistributionMode,
    version: String,
) -> Result<ResolvedTunnel, TunnelProcessError> {
    let package_dir = canonical_directory(&package_dir, "Tunnel package directory is invalid.")?;
    let client_path = validate_payload_file(
        package_dir.join(TUNNEL_CLIENT_FILE_NAME),
        &package_dir,
        TUNNEL_CLIENT_FILE_NAME,
        "Tunnel client executable is invalid.",
    )?;
    let cloudflared_path = validate_payload_file(
        package_dir.join(CLOUDFLARED_FILE_NAME),
        &package_dir,
        CLOUDFLARED_FILE_NAME,
        "Cloudflared companion executable is invalid.",
    )?;
    Ok(ResolvedTunnel {
        client_path,
        cloudflared_path,
        package_dir,
        mode,
        version,
    })
}

fn canonical_directory(path: &Path, message: &str) -> Result<PathBuf, TunnelProcessError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| TunnelProcessError::distribution(message))?;
    if !metadata.is_dir() || is_link_or_reparse_point(&metadata) {
        return Err(TunnelProcessError::distribution(message));
    }
    fs::canonicalize(path).map_err(|_| TunnelProcessError::distribution(message))
}

fn validate_payload_file(
    path: PathBuf,
    package_dir: &Path,
    expected_name: &str,
    message: &str,
) -> Result<PathBuf, TunnelProcessError> {
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| TunnelProcessError::distribution(message))?;
    if !metadata.is_file() || is_link_or_reparse_point(&metadata) {
        return Err(TunnelProcessError::distribution(message));
    }
    let canonical =
        fs::canonicalize(&path).map_err(|_| TunnelProcessError::distribution(message))?;
    if canonical.file_name().and_then(|name| name.to_str()) != Some(expected_name)
        || canonical.parent() != Some(package_dir)
    {
        return Err(TunnelProcessError::distribution(message));
    }
    Ok(canonical)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn package() -> tempfile::TempDir {
        let dir = tempdir().expect("package tempdir");
        fs::write(dir.path().join(TUNNEL_CLIENT_FILE_NAME), b"client").expect("client");
        fs::write(dir.path().join(CLOUDFLARED_FILE_NAME), b"cloudflared").expect("cloudflared");
        dir
    }

    #[test]
    fn development_resolution_prefers_explicit_override_and_requires_companion() {
        let dir = package();
        let resolved =
            resolve_tunnel_for_mode(TunnelDistributionMode::Development, Some(dir.path()), None)
                .expect("resolve development package");
        assert_eq!(resolved.mode, TunnelDistributionMode::Development);
        assert_eq!(
            resolved.client_path,
            fs::canonicalize(dir.path().join(TUNNEL_CLIENT_FILE_NAME)).unwrap()
        );
        assert_eq!(resolved.version, manifest::TUNNEL_CLIENT_VERSION);

        fs::remove_file(dir.path().join(CLOUDFLARED_FILE_NAME)).expect("remove companion");
        assert!(resolve_tunnel_for_mode(
            TunnelDistributionMode::Development,
            Some(dir.path()),
            None,
        )
        .is_err());
    }

    #[test]
    fn installed_resolution_rejects_missing_or_escaped_payload() {
        let dir = tempdir().expect("install tempdir");
        fs::create_dir(dir.path().join("tunnel")).expect("tunnel dir");
        fs::write(dir.path().join("chatgpt-harness-ui.exe"), b"ui").expect("ui");
        fs::write(
            dir.path().join("distribution.json"),
            format!(
                r#"{{"distribution_schema_version":2,"distribution_id":"chatgpt-harness","app_version":"{}","runtime_version":"{}","protocol_version":1,"runtime":{{"relative_path":"runtime/chatgpt-harness-runtime.exe"}},"tunnel":{{"version":"0.0.13","relative_path":"tunnel/tunnel-client.exe","cloudflared_relative_path":"tunnel/cloudflared.exe"}}}}"#,
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_VERSION")
            ),
        )
        .expect("manifest");
        assert!(resolve_tunnel_for_mode(
            TunnelDistributionMode::Installed,
            None,
            Some(&dir.path().join("chatgpt-harness-ui.exe")),
        )
        .is_err());
    }
}
