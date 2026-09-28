use super::path_resolver::PathResolver;
use super::workspace::validate_requested_path_form;
use super::{Workspace, WorkspaceError};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovedPath {
    canonical_path: PathBuf,
}

impl ApprovedPath {
    pub fn as_path(&self) -> &Path {
        &self.canonical_path
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovedWriteTarget {
    canonical_path: PathBuf,
}

impl ApprovedWriteTarget {
    pub fn as_path(&self) -> &Path {
        &self.canonical_path
    }
}

pub struct WorkspaceGuard {
    workspace: Workspace,
    resolver: PathResolver,
}

impl WorkspaceGuard {
    pub fn new(workspace: &Workspace) -> Self {
        Self {
            workspace: workspace.clone(),
            resolver: PathResolver::new(),
        }
    }

    pub fn approve(
        &self,
        requested_path: impl AsRef<Path>,
    ) -> Result<ApprovedPath, WorkspaceError> {
        let canonical_path = self.resolver.resolve(&self.workspace, requested_path)?;
        if is_within_workspace(self.workspace.canonical_root(), &canonical_path) {
            Ok(ApprovedPath { canonical_path })
        } else {
            Err(WorkspaceError::WorkspaceEscape {
                workspace_root: self.workspace.canonical_root().to_path_buf(),
                resolved_path: canonical_path,
            })
        }
    }

    /// Approves an existing or missing regular-file target without weakening
    /// `approve`'s existing-target-only contract. A missing target is safe
    /// only when its already-existing canonical parent remains in the
    /// workspace and its final component is a single normal path component.
    pub fn approve_write_target(
        &self,
        requested_path: impl AsRef<Path>,
    ) -> Result<ApprovedWriteTarget, WorkspaceError> {
        let requested_path = requested_path.as_ref();
        validate_requested_path_form(requested_path)?;
        let candidate = if requested_path.is_absolute() {
            requested_path.to_path_buf()
        } else {
            self.workspace.canonical_root().join(requested_path)
        };

        match fs::symlink_metadata(&candidate) {
            Ok(_) => {
                let canonical_target = fs::canonicalize(&candidate)
                    .map_err(|error| workspace_io_error(candidate.clone(), error))?;
                if is_within_workspace(self.workspace.canonical_root(), &canonical_target) {
                    Ok(ApprovedWriteTarget {
                        canonical_path: canonical_target,
                    })
                } else {
                    Err(WorkspaceError::WorkspaceEscape {
                        workspace_root: self.workspace.canonical_root().to_path_buf(),
                        resolved_path: canonical_target,
                    })
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let final_component = candidate
                    .file_name()
                    .map(std::ffi::OsStr::to_owned)
                    .filter(|component| is_safe_final_component(component))
                    .ok_or_else(|| WorkspaceError::UnsupportedPathForm {
                        path: requested_path.to_path_buf(),
                    })?;
                let parent =
                    candidate
                        .parent()
                        .ok_or_else(|| WorkspaceError::UnsupportedPathForm {
                            path: requested_path.to_path_buf(),
                        })?;
                let canonical_parent = fs::canonicalize(parent)
                    .map_err(|error| workspace_io_error(parent.to_path_buf(), error))?;
                if !fs::metadata(&canonical_parent)
                    .map_err(|error| workspace_io_error(canonical_parent.clone(), error))?
                    .is_dir()
                {
                    return Err(WorkspaceError::TargetNotFound {
                        path: parent.to_path_buf(),
                    });
                }
                if !is_within_workspace(self.workspace.canonical_root(), &canonical_parent) {
                    return Err(WorkspaceError::WorkspaceEscape {
                        workspace_root: self.workspace.canonical_root().to_path_buf(),
                        resolved_path: canonical_parent,
                    });
                }
                Ok(ApprovedWriteTarget {
                    canonical_path: canonical_parent.join(final_component),
                })
            }
            Err(error) => Err(workspace_io_error(candidate, error)),
        }
    }

    pub fn resolve(
        &self,
        requested_path: impl AsRef<Path>,
    ) -> Result<ApprovedPath, WorkspaceError> {
        self.approve(requested_path)
    }
    /// Returns a normalized workspace-relative logical path without exposing
    /// the workspace's canonical root. Callers must first obtain `approved`
    /// through this guard; this helper is representation only, not a second
    /// boundary check.
    pub fn logical_relative_path(
        &self,
        requested_path: impl AsRef<Path>,
        approved: &ApprovedPath,
    ) -> PathBuf {
        self.logical_relative(requested_path.as_ref(), approved.as_path())
    }

    /// Returns a normalized workspace-relative logical path for a write
    /// target. This is representation only; authorization remains in the
    /// preceding `approve_write_target` call.
    pub fn logical_relative_write_path(
        &self,
        requested_path: impl AsRef<Path>,
        approved: &ApprovedWriteTarget,
    ) -> PathBuf {
        self.logical_relative(requested_path.as_ref(), approved.as_path())
    }

    fn logical_relative(&self, requested_path: &Path, approved_path: &Path) -> PathBuf {
        if !requested_path.is_absolute() {
            return normalize_relative(requested_path);
        }

        if let Ok(relative) = requested_path.strip_prefix(self.workspace.configured_root()) {
            return normalize_relative(relative);
        }

        approved_path
            .strip_prefix(self.workspace.canonical_root())
            .map(normalize_relative)
            .unwrap_or_default()
    }
}

fn is_safe_final_component(component: &std::ffi::OsStr) -> bool {
    let mut components = Path::new(component).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn workspace_io_error(path: PathBuf, error: io::Error) -> WorkspaceError {
    match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => {
            WorkspaceError::TargetNotFound { path }
        }
        kind => WorkspaceError::CanonicalizationFailure { path, kind },
    }
}

fn normalize_relative(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    normalized
}

fn is_within_workspace(workspace_root: &Path, resolved_path: &Path) -> bool {
    resolved_path == workspace_root || resolved_path.strip_prefix(workspace_root).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::{Component, Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
        outside: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
            let base = std::env::temp_dir().join(format!(
                "chatgpt-harness-workspace-{id}-{}",
                std::process::id()
            ));
            let root = base.join("project");
            let outside = base.join("outside");
            fs::create_dir_all(&root).expect("create workspace root");
            fs::create_dir_all(&outside).expect("create outside directory");
            Self {
                base,
                root,
                outside,
            }
        }

        fn write(path: &Path, contents: &str) {
            let mut file = File::create(path).expect("create test file");
            file.write_all(contents.as_bytes())
                .expect("write test file");
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn workspace_for(test_workspace: &TestWorkspace) -> Workspace {
        Workspace::new(&test_workspace.root).expect("valid workspace")
    }

    #[test]
    fn existing_local_directory_is_a_valid_workspace() {
        let test_workspace = TestWorkspace::new();
        let workspace = Workspace::new(&test_workspace.root).expect("valid workspace");

        assert_eq!(workspace.configured_root(), test_workspace.root);
        assert_eq!(
            workspace.canonical_root(),
            fs::canonicalize(&test_workspace.root).expect("canonical root")
        );
    }

    #[test]
    fn missing_workspace_is_rejected() {
        let test_workspace = TestWorkspace::new();
        let missing = test_workspace.base.join("missing");

        assert!(matches!(
            Workspace::new(&missing),
            Err(WorkspaceError::WorkspaceNotFound { .. })
        ));
    }

    #[test]
    fn workspace_file_is_rejected() {
        let test_workspace = TestWorkspace::new();
        let file = test_workspace.base.join("not-a-directory");
        TestWorkspace::write(&file, "file");

        assert!(matches!(
            Workspace::new(&file),
            Err(WorkspaceError::WorkspaceNotDirectory { .. })
        ));
    }

    #[test]
    fn relative_file_is_allowed() {
        let test_workspace = TestWorkspace::new();
        TestWorkspace::write(&test_workspace.root.join("README.md"), "read me");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        let approved = guard.approve("README.md").expect("inside path");
        assert_eq!(
            approved.as_path(),
            fs::canonicalize(test_workspace.root.join("README.md")).unwrap()
        );
    }

    #[test]
    fn nested_relative_file_is_allowed() {
        let test_workspace = TestWorkspace::new();
        fs::create_dir_all(test_workspace.root.join("src").join("nested"))
            .expect("nested directory");
        TestWorkspace::write(
            &test_workspace
                .root
                .join("src")
                .join("nested")
                .join("main.rs"),
            "fn main() {}",
        );
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(guard
            .approve(Path::new("src").join("nested").join("main.rs"))
            .is_ok());
    }

    #[test]
    fn missing_write_target_uses_the_canonical_existing_parent() {
        let test_workspace = TestWorkspace::new();
        fs::create_dir_all(test_workspace.root.join("src")).expect("src directory");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        let approved = guard
            .approve_write_target("src/new.txt")
            .expect("missing target with existing parent");
        assert_eq!(
            approved.as_path(),
            fs::canonicalize(&test_workspace.root)
                .unwrap()
                .join("src")
                .join("new.txt")
        );
        assert_eq!(
            guard.logical_relative_write_path("src/./new.txt", &approved),
            PathBuf::from("src/new.txt")
        );
    }

    #[test]
    fn existing_write_target_is_approved_and_missing_parent_is_rejected() {
        let test_workspace = TestWorkspace::new();
        TestWorkspace::write(&test_workspace.root.join("existing.txt"), "existing");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(guard.approve_write_target("existing.txt").is_ok());
        assert!(matches!(
            guard.approve_write_target("missing/new.txt"),
            Err(WorkspaceError::TargetNotFound { .. })
        ));
    }

    #[test]
    fn write_target_rejects_relative_absolute_and_sibling_prefix_escapes() {
        let test_workspace = TestWorkspace::new();
        let outside_file = test_workspace.outside.join("outside.txt");
        TestWorkspace::write(&outside_file, "outside");
        let sibling = test_workspace.root.parent().unwrap().join(format!(
            "{}-evil",
            test_workspace.root.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir_all(&sibling).expect("sibling directory");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        for path in [
            PathBuf::from("../outside/new.txt"),
            outside_file.clone(),
            sibling.join("new.txt"),
        ] {
            assert!(matches!(
                guard.approve_write_target(path),
                Err(WorkspaceError::WorkspaceEscape { .. })
            ));
        }
        let _ = fs::remove_dir_all(sibling);
    }

    #[test]
    fn absolute_file_inside_workspace_is_allowed() {
        let test_workspace = TestWorkspace::new();
        let file = test_workspace.root.join("README.md");
        TestWorkspace::write(&file, "read me");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(guard.approve(&file).is_ok());
    }

    #[test]
    fn parent_traversal_that_ends_inside_workspace_is_allowed() {
        let test_workspace = TestWorkspace::new();
        TestWorkspace::write(&test_workspace.root.join("README.md"), "read me");
        fs::create_dir_all(test_workspace.root.join("src")).expect("src directory");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        let request = Path::new("src").join("..").join("README.md");
        assert!(guard.approve(request).is_ok());
    }

    #[test]
    fn parent_traversal_outside_workspace_is_rejected() {
        let test_workspace = TestWorkspace::new();
        let outside_file = test_workspace.outside.join("outside.txt");
        TestWorkspace::write(&outside_file, "outside");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        let result = guard.approve(Path::new("..").join("outside").join("outside.txt"));
        assert!(matches!(
            result,
            Err(WorkspaceError::WorkspaceEscape { .. })
        ));
    }

    #[test]
    fn absolute_outside_path_is_rejected() {
        let test_workspace = TestWorkspace::new();
        let outside_file = test_workspace.outside.join("outside.txt");
        TestWorkspace::write(&outside_file, "outside");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(matches!(
            guard.approve(&outside_file),
            Err(WorkspaceError::WorkspaceEscape { .. })
        ));
    }

    #[test]
    fn sibling_prefix_attack_is_rejected() {
        let test_workspace = TestWorkspace::new();
        let sibling = test_workspace.root.parent().unwrap().join(format!(
            "{}-evil",
            test_workspace.root.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir_all(&sibling).expect("sibling directory");
        let sibling_file = sibling.join("file.txt");
        TestWorkspace::write(&sibling_file, "evil");
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        let result = guard.approve(&sibling_file);
        assert!(matches!(
            result,
            Err(WorkspaceError::WorkspaceEscape { .. })
        ));
        let _ = fs::remove_dir_all(sibling);
    }

    #[test]
    fn missing_target_fails_closed() {
        let test_workspace = TestWorkspace::new();
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(matches!(
            guard.approve("missing.txt"),
            Err(WorkspaceError::TargetNotFound { .. })
        ));
    }

    #[test]
    fn existing_approval_remains_missing_target_fail_closed() {
        let test_workspace = TestWorkspace::new();
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(matches!(
            guard.approve("new.txt"),
            Err(WorkspaceError::TargetNotFound { .. })
        ));
        assert!(guard.approve_write_target("new.txt").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn windows_special_path_forms_are_rejected_before_resolution() {
        let test_workspace = TestWorkspace::new();
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);
        for path in [
            PathBuf::from(r"C:foo"),
            PathBuf::from(r"\\server\share\foo"),
            PathBuf::from(r"\\?\C:\foo"),
            PathBuf::from(r"\\.\PhysicalDrive0"),
        ] {
            assert!(matches!(
                guard.approve(&path),
                Err(WorkspaceError::UnsupportedPathForm { .. })
            ));
            assert!(matches!(
                guard.approve_write_target(&path),
                Err(WorkspaceError::UnsupportedPathForm { .. })
            ));
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_root_relative_paths_are_rejected_before_resolution() {
        let test_workspace = TestWorkspace::new();
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        for path in [PathBuf::from(r"\foo"), PathBuf::from(r"\project\file")] {
            assert!(matches!(
                guard.approve(&path),
                Err(WorkspaceError::UnsupportedPathForm { .. })
            ));
            assert!(matches!(
                guard.approve_write_target(&path),
                Err(WorkspaceError::UnsupportedPathForm { .. })
            ));
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_special_workspace_roots_are_rejected() {
        for path in [
            PathBuf::from(r"C:foo"),
            PathBuf::from(r"\\server\share\foo"),
            PathBuf::from(r"\\?\C:\foo"),
            PathBuf::from(r"\\.\PhysicalDrive0"),
        ] {
            assert!(matches!(
                Workspace::new(path),
                Err(WorkspaceError::UnsupportedPathForm { .. })
            ));
        }
    }

    #[cfg(windows)]
    #[test]
    fn directory_symlinks_are_allowed_only_when_the_target_stays_inside() {
        use std::os::windows::fs::symlink_dir;

        let test_workspace = TestWorkspace::new();
        let inside_target = test_workspace.root.join("inside-target");
        let outside_target = test_workspace.outside.join("outside-target");
        fs::create_dir_all(&inside_target).expect("inside target");
        fs::create_dir_all(&outside_target).expect("outside target");
        TestWorkspace::write(&inside_target.join("inside.txt"), "inside");
        TestWorkspace::write(&outside_target.join("outside.txt"), "outside");
        let inside_link = test_workspace.root.join("inside-link");
        let outside_link = test_workspace.root.join("outside-link");
        if symlink_dir(&inside_target, &inside_link).is_err()
            || symlink_dir(&outside_target, &outside_link).is_err()
        {
            eprintln!("SKIP symlink tests: directory symlink creation is unavailable");
            return;
        }
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(guard.approve(inside_link.join("inside.txt")).is_ok());
        assert!(matches!(
            guard.approve(outside_link.join("outside.txt")),
            Err(WorkspaceError::WorkspaceEscape { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn write_target_directory_symlinks_cannot_escape() {
        use std::os::windows::fs::symlink_dir;

        let test_workspace = TestWorkspace::new();
        let inside_target = test_workspace.root.join("inside-target");
        let outside_target = test_workspace.outside.join("outside-target");
        fs::create_dir_all(&inside_target).expect("inside target");
        fs::create_dir_all(&outside_target).expect("outside target");
        let inside_link = test_workspace.root.join("inside-link");
        let outside_link = test_workspace.root.join("outside-link");
        if symlink_dir(&inside_target, &inside_link).is_err()
            || symlink_dir(&outside_target, &outside_link).is_err()
        {
            eprintln!("SKIP write-target symlink test: directory symlink creation is unavailable");
            return;
        }
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(guard.approve_write_target("inside-link/new.txt").is_ok());
        assert!(matches!(
            guard.approve_write_target("outside-link/new.txt"),
            Err(WorkspaceError::WorkspaceEscape { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_allowed_only_when_the_target_stays_inside() {
        let test_workspace = TestWorkspace::new();
        let inside_target = test_workspace.root.join("inside-target");
        let outside_target = test_workspace.outside.join("outside-target");
        fs::create_dir_all(&inside_target).expect("inside target");
        fs::create_dir_all(&outside_target).expect("outside target");
        TestWorkspace::write(&inside_target.join("inside.txt"), "inside");
        TestWorkspace::write(&outside_target.join("outside.txt"), "outside");
        let inside_link = test_workspace.root.join("inside-junction");
        let outside_link = test_workspace.root.join("outside-junction");
        if !create_junction(&inside_link, &inside_target)
            || !create_junction(&outside_link, &outside_target)
        {
            eprintln!("SKIP junction tests: mklink /J is unavailable");
            return;
        }
        let workspace = workspace_for(&test_workspace);
        let guard = WorkspaceGuard::new(&workspace);

        assert!(guard.approve(inside_link.join("inside.txt")).is_ok());
        assert!(matches!(
            guard.approve(outside_link.join("outside.txt")),
            Err(WorkspaceError::WorkspaceEscape { .. })
        ));
    }

    #[cfg(windows)]
    fn create_junction(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}
