use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Workspace {
    configured_root: PathBuf,
    canonical_root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceError {
    InvalidWorkspaceRoot {
        path: PathBuf,
    },
    WorkspaceNotFound {
        path: PathBuf,
    },
    WorkspaceNotDirectory {
        path: PathBuf,
    },
    UnsupportedPathForm {
        path: PathBuf,
    },
    TargetNotFound {
        path: PathBuf,
    },
    CanonicalizationFailure {
        path: PathBuf,
        kind: io::ErrorKind,
    },
    WorkspaceEscape {
        workspace_root: PathBuf,
        resolved_path: PathBuf,
    },
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWorkspaceRoot { path } => {
                write!(formatter, "invalid workspace root: {}", path.display())
            }
            Self::WorkspaceNotFound { path } => {
                write!(formatter, "workspace root not found: {}", path.display())
            }
            Self::WorkspaceNotDirectory { path } => {
                write!(
                    formatter,
                    "workspace root is not a directory: {}",
                    path.display()
                )
            }
            Self::UnsupportedPathForm { path } => {
                write!(formatter, "unsupported path form: {}", path.display())
            }
            Self::TargetNotFound { path } => {
                write!(formatter, "target not found: {}", path.display())
            }
            Self::CanonicalizationFailure { path, kind } => write!(
                formatter,
                "failed to canonicalize {} ({kind:?})",
                path.display()
            ),
            Self::WorkspaceEscape {
                workspace_root,
                resolved_path,
            } => write!(
                formatter,
                "resolved path {} escapes workspace {}",
                resolved_path.display(),
                workspace_root.display()
            ),
        }
    }
}

impl std::error::Error for WorkspaceError {}

impl Workspace {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let configured_root = root.as_ref().to_path_buf();
        validate_workspace_root_form(&configured_root)?;

        let metadata = match fs::metadata(&configured_root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(WorkspaceError::WorkspaceNotFound {
                    path: configured_root,
                });
            }
            Err(_) => {
                return Err(WorkspaceError::InvalidWorkspaceRoot {
                    path: configured_root,
                });
            }
        };
        if !metadata.is_dir() {
            return Err(WorkspaceError::WorkspaceNotDirectory {
                path: configured_root,
            });
        }

        let canonical_root = fs::canonicalize(&configured_root).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                WorkspaceError::WorkspaceNotFound {
                    path: configured_root.clone(),
                }
            } else {
                WorkspaceError::CanonicalizationFailure {
                    path: configured_root.clone(),
                    kind: error.kind(),
                }
            }
        })?;
        validate_canonical_workspace_root(&canonical_root)?;

        Ok(Self {
            configured_root,
            canonical_root,
        })
    }

    pub fn configured_root(&self) -> &Path {
        &self.configured_root
    }

    pub(super) fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }
}

pub(crate) fn validate_requested_path_form(path: &Path) -> Result<(), WorkspaceError> {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};

        if path.has_root() && !path.is_absolute() {
            return Err(WorkspaceError::UnsupportedPathForm {
                path: path.to_path_buf(),
            });
        }

        for component in path.components() {
            let Component::Prefix(prefix) = component else {
                continue;
            };
            match prefix.kind() {
                Prefix::Disk(_) if !path.has_root() => {
                    return Err(WorkspaceError::UnsupportedPathForm {
                        path: path.to_path_buf(),
                    });
                }
                Prefix::UNC(..)
                | Prefix::Verbatim(_)
                | Prefix::VerbatimUNC(..)
                | Prefix::VerbatimDisk(_)
                | Prefix::DeviceNS(_) => {
                    return Err(WorkspaceError::UnsupportedPathForm {
                        path: path.to_path_buf(),
                    });
                }
                Prefix::Disk(_) => {}
            }
        }
    }

    Ok(())
}

fn validate_canonical_workspace_root(path: &Path) -> Result<(), WorkspaceError> {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};

        let is_local_disk = path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(prefix)
                    if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
            )
        });

        if !path.has_root() || !is_local_disk {
            return Err(WorkspaceError::UnsupportedPathForm {
                path: path.to_path_buf(),
            });
        }

        for component in path.components() {
            let Component::Prefix(prefix) = component else {
                continue;
            };
            if !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) {
                return Err(WorkspaceError::UnsupportedPathForm {
                    path: path.to_path_buf(),
                });
            }
        }
    }

    Ok(())
}

fn validate_workspace_root_form(path: &Path) -> Result<(), WorkspaceError> {
    validate_requested_path_form(path)?;
    if !path.is_absolute() {
        return Err(WorkspaceError::InvalidWorkspaceRoot {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn canonical_workspace_root_accepts_local_disk_and_verbatim_disk() {
        assert!(validate_canonical_workspace_root(Path::new(r"C:\project")).is_ok());
        assert!(validate_canonical_workspace_root(Path::new(r"\\?\C:\project")).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn canonical_workspace_root_rejects_unc_and_device_namespaces() {
        for path in [
            Path::new(r"\\server\share\project"),
            Path::new(r"\\?\UNC\server\share\project"),
            Path::new(r"\\.\PhysicalDrive0"),
            Path::new(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1"),
        ] {
            assert!(matches!(
                validate_canonical_workspace_root(path),
                Err(WorkspaceError::UnsupportedPathForm { .. })
            ));
        }
    }
}
