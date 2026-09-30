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

/// Runtime-internal input for the `edit` capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditRequest {
    pub path: PathBuf,
    pub old_text: String,
    pub new_text: String,
}

impl EditRequest {
    pub fn new(
        path: impl Into<PathBuf>,
        old_text: impl Into<String>,
        new_text: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            old_text: old_text.into(),
            new_text: new_text.into(),
        }
    }
}

/// Runtime-internal result for the `edit` capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditOutput {
    pub normalized_path: PathBuf,
    pub replacements: u64,
    pub bytes_written: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    Workspace(WorkspaceError),
    TargetNotFile {
        path: PathBuf,
    },
    UnsupportedContent {
        path: PathBuf,
    },
    MatchNotFound {
        path: PathBuf,
    },
    MatchNotUnique {
        path: PathBuf,
        matches: u64,
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

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace(error) => write!(formatter, "workspace access rejected: {error}"),
            Self::TargetNotFile { path } => {
                write!(formatter, "target is not a file: {}", path.display())
            }
            Self::UnsupportedContent { path } => {
                write!(formatter, "file is not valid UTF-8: {}", path.display())
            }
            Self::MatchNotFound { path } => {
                write!(formatter, "edit text was not found in {}", path.display())
            }
            Self::MatchNotUnique { path, matches } => write!(
                formatter,
                "edit text matched {matches} times in {}; exactly one match is required",
                path.display()
            ),
            Self::TargetChanged { path } => {
                write!(formatter, "target changed during edit: {}", path.display())
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
                write!(formatter, "invalid edit arguments: {message}")
            }
        }
    }
}

impl std::error::Error for EditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Workspace(error) => Some(error),
            _ => None,
        }
    }
}

/// Executes a deterministic exact single-match replacement through the shared
/// atomic mutation path.
pub(crate) fn execute_edit(
    permit: &crate::execution::ExecutionPermit,
    workspace_guard: &WorkspaceGuard,
    request: &EditRequest,
) -> Result<EditOutput, EditError> {
    execute_edit_with_options(
        permit,
        workspace_guard,
        request,
        AtomicMutationOptions::default(),
    )
}

fn execute_edit_with_options(
    _permit: &crate::execution::ExecutionPermit,
    workspace_guard: &WorkspaceGuard,
    request: &EditRequest,
    options: AtomicMutationOptions,
) -> Result<EditOutput, EditError> {
    execute_edit_inner(workspace_guard, request, options, None)
}

fn execute_edit_inner(
    workspace_guard: &WorkspaceGuard,
    request: &EditRequest,
    options: AtomicMutationOptions,
    mut before_revalidation: Option<&mut dyn FnMut()>,
) -> Result<EditOutput, EditError> {
    if request.path.as_os_str().is_empty() {
        return Err(EditError::InvalidArguments {
            message: "path must not be empty".into(),
        });
    }
    if request.old_text.is_empty() {
        return Err(EditError::InvalidArguments {
            message: "old_text must not be empty".into(),
        });
    }
    let approved = workspace_guard
        .approve(&request.path)
        .map_err(EditError::Workspace)?;
    let normalized_path = workspace_guard.logical_relative_path(&request.path, &approved);
    let target_path = approved.as_path().to_path_buf();
    let metadata = fs::metadata(&target_path).map_err(|error| EditError::IoFailure {
        path: target_path.clone(),
        kind: error.kind(),
    })?;
    if !metadata.is_file() {
        return Err(EditError::TargetNotFile { path: target_path });
    }

    let original_bytes = fs::read(&target_path).map_err(|error| EditError::IoFailure {
        path: target_path.clone(),
        kind: error.kind(),
    })?;
    let original =
        String::from_utf8(original_bytes.clone()).map_err(|_| EditError::UnsupportedContent {
            path: target_path.clone(),
        })?;
    let matches = original.match_indices(&request.old_text).count() as u64;
    match matches {
        0 => return Err(EditError::MatchNotFound { path: target_path }),
        1 => {}
        matches => {
            return Err(EditError::MatchNotUnique {
                path: target_path,
                matches,
            })
        }
    }

    let replacement = original.replacen(&request.old_text, &request.new_text, 1);
    let target_for_revalidation = target_path.clone();
    let requested_path = request.path.clone();
    atomic_mutate(
        &target_path,
        replacement.as_bytes(),
        AtomicCommitMode::ReplaceExisting,
        || {
            if let Some(hook) = before_revalidation.as_mut() {
                hook();
            }
            revalidate_edit_target(
                workspace_guard,
                &requested_path,
                &target_for_revalidation,
                &original_bytes,
            )
        },
        options,
    )
    .map_err(|error| map_atomic_error(error, target_path.clone()))?;

    Ok(EditOutput {
        normalized_path,
        replacements: 1,
        bytes_written: replacement.as_bytes().len() as u64,
    })
}

fn revalidate_edit_target(
    workspace_guard: &WorkspaceGuard,
    requested_path: &std::path::Path,
    target_path: &std::path::Path,
    original_bytes: &[u8],
) -> Result<(), EditError> {
    let approved = match workspace_guard.approve(requested_path) {
        Ok(approved) => approved,
        Err(WorkspaceError::TargetNotFound { .. }) => {
            return Err(EditError::TargetChanged {
                path: target_path.to_path_buf(),
            })
        }
        Err(error) => return Err(EditError::Workspace(error)),
    };
    if approved.as_path() != target_path {
        return Err(EditError::TargetChanged {
            path: target_path.to_path_buf(),
        });
    }

    let metadata = match fs::metadata(target_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(EditError::TargetChanged {
                path: target_path.to_path_buf(),
            })
        }
        Err(error) => {
            return Err(EditError::IoFailure {
                path: target_path.to_path_buf(),
                kind: error.kind(),
            })
        }
    };
    if !metadata.is_file() {
        return Err(EditError::TargetChanged {
            path: target_path.to_path_buf(),
        });
    }

    let current_bytes = fs::read(target_path).map_err(|error| EditError::IoFailure {
        path: target_path.to_path_buf(),
        kind: error.kind(),
    })?;
    if current_bytes != original_bytes {
        return Err(EditError::TargetChanged {
            path: target_path.to_path_buf(),
        });
    }
    Ok(())
}

fn map_atomic_error(error: AtomicMutationError<EditError>, path: PathBuf) -> EditError {
    match error {
        AtomicMutationError::Revalidation { error, .. } => error,
        AtomicMutationError::Io { error, .. } => EditError::AtomicFailure {
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
    use std::path::PathBuf;

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
        outside: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let base =
                std::env::temp_dir().join(format!("chatgpt-harness-edit-{}", uuid::Uuid::new_v4()));
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

        fn execute(&self, request: &EditRequest) -> Result<EditOutput, EditError> {
            execute_edit(
                &ExecutionPermit::for_executor_test(),
                &self.guard(),
                request,
            )
        }

        fn execute_with_options(
            &self,
            request: &EditRequest,
            options: AtomicMutationOptions,
        ) -> Result<EditOutput, EditError> {
            execute_edit_with_options(
                &ExecutionPermit::for_executor_test(),
                &self.guard(),
                request,
                options,
            )
        }

        fn execute_with_hook(
            &self,
            request: &EditRequest,
            hook: &mut dyn FnMut(),
        ) -> Result<EditOutput, EditError> {
            execute_edit_inner(
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
    fn replaces_exactly_one_match_and_reports_full_rewrite_bytes() {
        let workspace = TestWorkspace::new();
        fs::write(workspace.root.join("file.txt"), "hello from ChatGPT\n").expect("fixture");

        let output = workspace
            .execute(&EditRequest::new(
                "file.txt",
                "hello from ChatGPT",
                "hello from V3",
            ))
            .expect("edit file");

        assert_eq!(
            fs::read_to_string(workspace.root.join("file.txt")).unwrap(),
            "hello from V3\n"
        );
        assert_eq!(output.normalized_path, PathBuf::from("file.txt"));
        assert_eq!(output.replacements, 1);
        assert_eq!(output.bytes_written, "hello from V3\n".len() as u64);
    }

    #[test]
    fn edit_uses_utf8_byte_length_for_multibyte_replacement() {
        let workspace = TestWorkspace::new();
        fs::write(workspace.root.join("file.txt"), "hello").expect("fixture");

        let output = workspace
            .execute(&EditRequest::new("file.txt", "hello", "你好"))
            .expect("edit file");

        assert_eq!(output.bytes_written, 6);
        assert_eq!(
            fs::read_to_string(workspace.root.join("file.txt")).unwrap(),
            "你好"
        );
    }

    #[test]
    fn edit_can_grow_shrink_or_empty_the_replacement() {
        for (input, old, new, expected) in [
            ("a b c", "b", "much longer", "a much longer c"),
            ("a much longer c", "much longer", "b", "a b c"),
            ("a b c", "b", "", "a  c"),
        ] {
            let workspace = TestWorkspace::new();
            fs::write(workspace.root.join("file.txt"), input).expect("fixture");
            workspace
                .execute(&EditRequest::new("file.txt", old, new))
                .expect("edit file");
            assert_eq!(
                fs::read_to_string(workspace.root.join("file.txt")).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn concurrent_target_change_is_rejected_without_clobbering_actor_content() {
        let workspace = TestWorkspace::new();
        let target = workspace.root.join("file.txt");
        fs::write(&target, "before").expect("fixture");
        let mut hook = || fs::write(&target, "actor content").expect("actor write");

        let result = workspace.execute_with_hook(
            &EditRequest::new("file.txt", "before", "agent content"),
            &mut hook,
        );

        assert!(matches!(result, Err(EditError::TargetChanged { .. })));
        assert_eq!(fs::read_to_string(target).unwrap(), "actor content");
    }

    #[test]
    fn atomic_faults_preserve_edit_target_and_leave_no_temp() {
        for fault in [
            AtomicFault::TempWrite,
            AtomicFault::TempFlush,
            AtomicFault::Commit,
        ] {
            let workspace = TestWorkspace::new();
            let target = workspace.root.join("file.txt");
            fs::write(&target, "old").expect("fixture");
            let result = workspace.execute_with_options(
                &EditRequest::new("file.txt", "old", "new"),
                AtomicMutationOptions::with_fault(fault),
            );
            assert!(matches!(result, Err(EditError::AtomicFailure { .. })));
            assert_eq!(fs::read_to_string(&target).unwrap(), "old");
            assert_eq!(fs::read_dir(&workspace.root).unwrap().count(), 1);
        }
    }

    #[test]
    fn target_deleted_after_prepare_is_fail_closed() {
        let workspace = TestWorkspace::new();
        let target = workspace.root.join("file.txt");
        fs::write(&target, "old").expect("fixture");
        let mut hook = || fs::remove_file(&target).expect("actor delete");

        let result =
            workspace.execute_with_hook(&EditRequest::new("file.txt", "old", "new"), &mut hook);

        assert!(matches!(result, Err(EditError::TargetChanged { .. })));
        assert!(!target.exists());
    }

    #[test]
    fn target_replaced_after_prepare_is_not_silently_overwritten() {
        let workspace = TestWorkspace::new();
        let target = workspace.root.join("file.txt");
        fs::write(&target, "old").expect("fixture");
        let mut hook = || {
            fs::remove_file(&target).expect("actor delete");
            fs::write(&target, "replacement actor content").expect("actor replacement");
        };

        let result = workspace.execute_with_hook(
            &EditRequest::new("file.txt", "old", "agent content"),
            &mut hook,
        );

        assert!(matches!(result, Err(EditError::TargetChanged { .. })));
        assert_eq!(
            fs::read_to_string(target).unwrap(),
            "replacement actor content"
        );
    }

    #[cfg(windows)]
    #[test]
    fn parent_junction_change_before_commit_fails_closed_without_escape() {
        let workspace = TestWorkspace::new();
        let inside = workspace.root.join("inside");
        let outside = workspace.outside.join("outside");
        fs::create_dir_all(&inside).expect("inside directory");
        fs::create_dir_all(&outside).expect("outside directory");
        let inside_file = inside.join("file.txt");
        let outside_file = outside.join("file.txt");
        fs::write(&inside_file, "inside").expect("inside fixture");
        fs::write(&outside_file, "outside").expect("outside fixture");

        let link = workspace.root.join("link");
        if !create_junction(&link, &inside) {
            eprintln!("ENVIRONMENT_SKIP: mklink /J unavailable for dynamic junction test");
            return;
        }

        let mut hook = || {
            fs::remove_dir(&link).expect("remove original junction");
            assert!(create_junction(&link, &outside));
        };
        let result = workspace.execute_with_hook(
            &EditRequest::new("link/file.txt", "inside", "agent replacement"),
            &mut hook,
        );

        assert!(matches!(
            result,
            Err(EditError::Workspace(WorkspaceError::WorkspaceEscape { .. }))
        ));
        assert_eq!(fs::read(&inside_file).unwrap(), b"inside");
        assert_eq!(fs::read(&outside_file).unwrap(), b"outside");
        assert!(fs::read_dir(&inside).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".chatgpt-harness-")));
        let _ = fs::remove_dir(&link);
    }

    #[cfg(windows)]
    fn create_junction(link: &std::path::Path, target: &std::path::Path) -> bool {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[test]
    fn zero_and_multiple_matches_fail_without_modifying_the_file() {
        let workspace = TestWorkspace::new();
        let path = workspace.root.join("file.txt");
        fs::write(&path, "one two one").expect("fixture");

        let no_match = workspace.execute(&EditRequest::new("file.txt", "missing", "new"));
        assert!(matches!(no_match, Err(EditError::MatchNotFound { .. })));
        assert_eq!(fs::read_to_string(&path).unwrap(), "one two one");

        let multiple = workspace.execute(&EditRequest::new("file.txt", "one", "new"));
        assert!(matches!(
            multiple,
            Err(EditError::MatchNotUnique { matches: 2, .. })
        ));
        assert_eq!(fs::read_to_string(path).unwrap(), "one two one");
    }

    #[test]
    fn missing_outside_and_directory_targets_fail() {
        let workspace = TestWorkspace::new();
        let missing = workspace.execute(&EditRequest::new("missing.txt", "old", "new"));
        assert!(matches!(
            missing,
            Err(EditError::Workspace(WorkspaceError::TargetNotFound { .. }))
        ));

        let outside = workspace.outside.join("outside.txt");
        fs::write(&outside, "old").expect("outside fixture");
        for path in [PathBuf::from("../outside/outside.txt"), outside] {
            assert!(matches!(
                workspace.execute(&EditRequest::new(path, "old", "new")),
                Err(EditError::Workspace(WorkspaceError::WorkspaceEscape { .. }))
            ));
        }

        fs::create_dir(workspace.root.join("directory")).expect("directory fixture");
        assert!(matches!(
            workspace.execute(&EditRequest::new("directory", "old", "new")),
            Err(EditError::TargetNotFile { .. })
        ));
    }

    #[test]
    fn invalid_utf8_and_empty_old_text_fail_without_modifying_the_file() {
        let workspace = TestWorkspace::new();
        let binary = workspace.root.join("binary.bin");
        fs::write(&binary, [0xff, 0xfe]).expect("binary fixture");
        assert!(matches!(
            workspace.execute(&EditRequest::new("binary.bin", "old", "new")),
            Err(EditError::UnsupportedContent { .. })
        ));
        assert_eq!(fs::read(&binary).unwrap(), [0xff, 0xfe]);

        let text = workspace.root.join("text.txt");
        fs::write(&text, "content").expect("text fixture");
        let request = EditRequest::new("text.txt", "", "new");
        assert!(matches!(
            workspace.execute(&request),
            Err(EditError::InvalidArguments { .. })
        ));
        assert_eq!(fs::read_to_string(text).unwrap(), "content");
    }
}
