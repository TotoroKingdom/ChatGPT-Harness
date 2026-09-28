use crate::workspace::{WorkspaceError, WorkspaceGuard};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

/// Runtime-internal input for the `write` capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteRequest {
    pub path: PathBuf,
    pub content: String,
}

impl WriteRequest {
    pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }
}

/// Runtime-internal result for the `write` capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteOutput {
    pub normalized_path: PathBuf,
    pub bytes_written: u64,
    pub created: bool,
    pub overwritten: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    Workspace(WorkspaceError),
    TargetNotFile { path: PathBuf },
    IoFailure { path: PathBuf, kind: io::ErrorKind },
    InvalidArguments { message: String },
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace(error) => write!(formatter, "workspace access rejected: {error}"),
            Self::TargetNotFile { path } => {
                write!(formatter, "target is not a file: {}", path.display())
            }
            Self::IoFailure { path, kind } => {
                write!(formatter, "I/O failure for {} ({kind:?})", path.display())
            }
            Self::InvalidArguments { message } => {
                write!(formatter, "invalid write arguments: {message}")
            }
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Workspace(error) => Some(error),
            _ => None,
        }
    }
}

/// Executes a non-atomic full-file write after revalidating the write target.
/// Atomic replacement and production hardening remain outside M0.
pub(crate) fn execute_write(
    _permit: &crate::execution::ExecutionPermit,
    workspace_guard: &WorkspaceGuard,
    request: &WriteRequest,
) -> Result<WriteOutput, WriteError> {
    if request.path.as_os_str().is_empty() {
        return Err(WriteError::InvalidArguments {
            message: "path must not be empty".into(),
        });
    }
    let approved = workspace_guard
        .approve_write_target(&request.path)
        .map_err(WriteError::Workspace)?;
    let normalized_path = workspace_guard.logical_relative_write_path(&request.path, &approved);
    let target_path = approved.as_path().to_path_buf();

    let existed = match fs::metadata(&target_path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(WriteError::TargetNotFile { path: target_path });
            }
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(WriteError::IoFailure {
                path: target_path,
                kind: error.kind(),
            });
        }
    };

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&target_path)
        .map_err(|error| WriteError::IoFailure {
            path: target_path.clone(),
            kind: error.kind(),
        })?;
    file.write_all(request.content.as_bytes())
        .map_err(|error| WriteError::IoFailure {
            path: target_path,
            kind: error.kind(),
        })?;

    Ok(WriteOutput {
        normalized_path,
        bytes_written: request.content.len() as u64,
        created: !existed,
        overwritten: existed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::ExecutionPermit;
    use crate::workspace::Workspace;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
        outside: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let base = std::env::temp_dir()
                .join(format!("chatgpt-harness-write-{}", uuid::Uuid::new_v4()));
            let root = base.join("workspace");
            let outside = base.join("outside");
            fs::create_dir_all(&root).expect("create workspace");
            fs::create_dir_all(&outside).expect("create outside");
            Self {
                base,
                root,
                outside,
            }
        }

        fn guard(&self) -> WorkspaceGuard {
            WorkspaceGuard::new(&Workspace::new(&self.root).expect("workspace"))
        }

        fn execute(&self, request: &WriteRequest) -> Result<WriteOutput, WriteError> {
            execute_write(
                &ExecutionPermit::for_executor_test(),
                &self.guard(),
                request,
            )
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn creates_nested_file_with_exact_content_and_flags() {
        let workspace = TestWorkspace::new();
        fs::create_dir_all(workspace.root.join("src")).expect("parent");

        let output = workspace
            .execute(&WriteRequest::new("src/main.rs", "fn main() {}\n"))
            .expect("create file");

        assert_eq!(output.normalized_path, PathBuf::from("src/main.rs"));
        assert_eq!(output.bytes_written, "fn main() {}\n".len() as u64);
        assert!(output.created);
        assert!(!output.overwritten);
        assert_eq!(
            fs::read_to_string(workspace.root.join("src/main.rs")).unwrap(),
            "fn main() {}\n"
        );
    }

    #[test]
    fn overwrites_existing_regular_file_and_reports_flags() {
        let workspace = TestWorkspace::new();
        fs::write(workspace.root.join("file.txt"), "old").expect("fixture");

        let output = workspace
            .execute(&WriteRequest::new("file.txt", "new content"))
            .expect("overwrite file");

        assert_eq!(
            fs::read_to_string(workspace.root.join("file.txt")).unwrap(),
            "new content"
        );
        assert_eq!(output.bytes_written, "new content".len() as u64);
        assert!(!output.created);
        assert!(output.overwritten);
    }

    #[test]
    fn missing_parent_and_non_file_targets_fail_without_mutation() {
        let workspace = TestWorkspace::new();
        let missing = workspace.execute(&WriteRequest::new("missing/file.txt", "content"));
        assert!(matches!(
            missing,
            Err(WriteError::Workspace(WorkspaceError::TargetNotFound { .. }))
        ));

        fs::create_dir(workspace.root.join("directory")).expect("directory fixture");
        let directory = workspace.execute(&WriteRequest::new("directory", "content"));
        assert!(matches!(directory, Err(WriteError::TargetNotFile { .. })));
    }

    #[test]
    fn relative_and_absolute_outside_targets_are_rejected() {
        let workspace = TestWorkspace::new();
        let outside = workspace.outside.join("secret.txt");
        fs::write(&outside, "secret").expect("outside fixture");

        for path in [PathBuf::from("../outside/new.txt"), outside] {
            assert!(matches!(
                workspace.execute(&WriteRequest::new(path, "blocked")),
                Err(WriteError::Workspace(
                    WorkspaceError::WorkspaceEscape { .. }
                ))
            ));
        }
        assert!(!workspace.root.join("new.txt").exists());
    }

    #[test]
    fn empty_path_is_rejected_as_invalid_arguments_by_the_executor_contract() {
        let workspace = TestWorkspace::new();
        let request = WriteRequest::new(Path::new(""), "content");
        assert!(matches!(
            workspace.execute(&request),
            Err(WriteError::InvalidArguments { .. })
        ));
        assert!(workspace.root.is_dir());
    }
}
