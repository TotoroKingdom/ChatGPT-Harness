use super::workspace::{validate_requested_path_form, Workspace, WorkspaceError};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default)]
pub struct PathResolver;

impl PathResolver {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve(
        &self,
        workspace: &Workspace,
        requested_path: impl AsRef<Path>,
    ) -> Result<PathBuf, WorkspaceError> {
        let requested_path = requested_path.as_ref();
        validate_requested_path_form(requested_path)?;

        let candidate = if requested_path.is_absolute() {
            requested_path.to_path_buf()
        } else {
            workspace.canonical_root().join(requested_path)
        };

        fs::canonicalize(&candidate).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                WorkspaceError::TargetNotFound { path: candidate }
            } else {
                WorkspaceError::CanonicalizationFailure {
                    path: candidate,
                    kind: error.kind(),
                }
            }
        })
    }
}
