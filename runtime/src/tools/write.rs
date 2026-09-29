#[cfg(test)]
use super::atomic_file::AtomicFault;
use super::atomic_file::{
    atomic_mutate, AtomicCommitMode, AtomicMutationError, AtomicMutationOptions, AtomicPhase,
};
use crate::workspace::{WorkspaceError, WorkspaceGuard};
use std::fmt;
use std::fs;
use std::io;
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
    TargetNotFile {
        path: PathBuf,
    },
    UnsupportedContent {
        path: PathBuf,
    },
    TargetChanged {
        path: PathBuf,
    },
    IoFailure {
        path: PathBuf,
        kind: io::ErrorKind,
    },
    AtomicFailure {
        path: PathBuf,
        phase: AtomicPhase,
        kind: io::ErrorKind,
    },
    InvalidArguments {
        message: String,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace(error) => write!(formatter, "workspace access rejected: {error}"),
            Self::TargetNotFile { path } => {
                write!(formatter, "target is not a file: {}", path.display())
            }
            Self::UnsupportedContent { path } => {
                write!(formatter, "file is not valid UTF-8: {}", path.display())
            }
            Self::TargetChanged { path } => {
                write!(formatter, "target changed during write: {}", path.display())
            }
            Self::IoFailure { path, kind } => {
                write!(formatter, "I/O failure for {} ({kind:?})", path.display())
            }
            Self::AtomicFailure { path, phase, kind } => write!(
                formatter,
                "atomic {phase:?} failure for {} ({kind:?})",
                path.display()
            ),
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

/// Executes a full-file UTF-8 write through the shared atomic mutation path.
pub(crate) fn execute_write(
    permit: &crate::execution::ExecutionPermit,
    workspace_guard: &WorkspaceGuard,
    request: &WriteRequest,
) -> Result<WriteOutput, WriteError> {
    execute_write_with_options(
        permit,
        workspace_guard,
        request,
        AtomicMutationOptions::default(),
    )
}

fn execute_write_with_options(
    _permit: &crate::execution::ExecutionPermit,
    workspace_guard: &WorkspaceGuard,
    request: &WriteRequest,
    options: AtomicMutationOptions,
) -> Result<WriteOutput, WriteError> {
    execute_write_inner(workspace_guard, request, options, None)
}

fn execute_write_inner(
    workspace_guard: &WorkspaceGuard,
    request: &WriteRequest,
    options: AtomicMutationOptions,
    mut before_revalidation: Option<&mut dyn FnMut()>,
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
            let existing = fs::read(&target_path).map_err(|error| WriteError::IoFailure {
                path: target_path.clone(),
                kind: error.kind(),
            })?;
            String::from_utf8(existing).map_err(|_| WriteError::UnsupportedContent {
                path: target_path.clone(),
            })?;
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

    let mode = if existed {
        AtomicCommitMode::ReplaceExisting
    } else {
        AtomicCommitMode::CreateNew
    };
    let target_for_revalidation = target_path.clone();
    let requested_path = request.path.clone();
    atomic_mutate(
        &target_path,
        request.content.as_bytes(),
        mode,
        || {
            if let Some(hook) = before_revalidation.as_mut() {
                hook();
            }
            revalidate_write_target(
                workspace_guard,
                &requested_path,
                &target_for_revalidation,
                existed,
            )
        },
        options,
    )
    .map_err(|error| map_atomic_error(error, target_path.clone()))?;

    Ok(WriteOutput {
        normalized_path,
        bytes_written: request.content.len() as u64,
        created: !existed,
        overwritten: existed,
    })
}

fn revalidate_write_target(
    workspace_guard: &WorkspaceGuard,
    requested_path: &std::path::Path,
    target_path: &std::path::Path,
    existed: bool,
) -> Result<(), WriteError> {
    let approved = match workspace_guard.approve_write_target(requested_path) {
        Ok(approved) => approved,
        Err(WorkspaceError::TargetNotFound { .. }) if existed => {
            return Err(WriteError::TargetChanged {
                path: target_path.to_path_buf(),
            })
        }
        Err(error) => return Err(WriteError::Workspace(error)),
    };
    if approved.as_path() != target_path {
        return Err(WriteError::TargetChanged {
            path: target_path.to_path_buf(),
        });
    }

    match fs::metadata(target_path) {
        Ok(metadata) if existed && metadata.is_file() => Ok(()),
        Ok(_) if existed => Err(WriteError::TargetNotFile {
            path: target_path.to_path_buf(),
        }),
        Ok(_) => Err(WriteError::TargetChanged {
            path: target_path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound && !existed => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(WriteError::TargetChanged {
            path: target_path.to_path_buf(),
        }),
        Err(error) => Err(WriteError::IoFailure {
            path: target_path.to_path_buf(),
            kind: error.kind(),
        }),
    }
}

fn map_atomic_error(error: AtomicMutationError<WriteError>, path: PathBuf) -> WriteError {
    match error {
        AtomicMutationError::Revalidation { error, .. } => error,
        AtomicMutationError::Io { error, .. } => WriteError::AtomicFailure {
            path,
            phase: error.phase,
            kind: error.kind,
        },
    }
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

        fn execute_with_options(
            &self,
            request: &WriteRequest,
            options: AtomicMutationOptions,
        ) -> Result<WriteOutput, WriteError> {
            execute_write_with_options(
                &ExecutionPermit::for_executor_test(),
                &self.guard(),
                request,
                options,
            )
        }

        fn execute_with_hook(
            &self,
            request: &WriteRequest,
            hook: &mut dyn FnMut(),
        ) -> Result<WriteOutput, WriteError> {
            execute_write_inner(
                &self.guard(),
                request,
                AtomicMutationOptions::default(),
                Some(hook),
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
    fn existing_invalid_utf8_is_rejected_without_mutation() {
        let workspace = TestWorkspace::new();
        let path = workspace.root.join("binary.bin");
        fs::write(&path, [0xff, 0xfe]).expect("binary fixture");

        let result = workspace.execute(&WriteRequest::new("binary.bin", "replacement"));

        assert!(matches!(result, Err(WriteError::UnsupportedContent { .. })));
        assert_eq!(fs::read(path).unwrap(), [0xff, 0xfe]);
    }

    #[test]
    fn write_reports_utf8_bytes_and_allows_empty_content() {
        let workspace = TestWorkspace::new();
        let output = workspace
            .execute(&WriteRequest::new("unicode.txt", "你好"))
            .expect("unicode write");
        assert_eq!(output.bytes_written, 6);

        let empty = workspace
            .execute(&WriteRequest::new("empty.txt", ""))
            .expect("empty write");
        assert_eq!(empty.bytes_written, 0);
        assert_eq!(fs::read(workspace.root.join("empty.txt")).unwrap(), b"");
    }

    #[test]
    fn atomic_faults_preserve_existing_write_target_and_leave_no_temp() {
        for fault in [
            AtomicFault::TempWrite,
            AtomicFault::TempFlush,
            AtomicFault::Commit,
        ] {
            let workspace = TestWorkspace::new();
            let target = workspace.root.join("file.txt");
            fs::write(&target, "old").expect("fixture");
            let result = workspace.execute_with_options(
                &WriteRequest::new("file.txt", "new"),
                AtomicMutationOptions::with_fault(fault),
            );
            assert!(matches!(result, Err(WriteError::AtomicFailure { .. })));
            assert_eq!(fs::read_to_string(&target).unwrap(), "old");
            assert_eq!(fs::read_dir(&workspace.root).unwrap().count(), 1);
        }
    }

    #[test]
    fn target_created_after_prepare_is_not_silently_overwritten() {
        let workspace = TestWorkspace::new();
        let target = workspace.root.join("raced.txt");
        let mut hook = || fs::write(&target, "actor content").expect("actor write");

        let result = workspace
            .execute_with_hook(&WriteRequest::new("raced.txt", "agent content"), &mut hook);

        assert!(matches!(result, Err(WriteError::TargetChanged { .. })));
        assert_eq!(fs::read_to_string(target).unwrap(), "actor content");
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
