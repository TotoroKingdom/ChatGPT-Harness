use crate::workspace::{ApprovedPath, WorkspaceError, WorkspaceGuard};
use globset::{GlobBuilder, GlobMatcher};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Maximum number of matches returned by one glob execution.
pub const MAX_GLOB_RESULTS: usize = 1_000;
/// Maximum number of non-fatal traversal diagnostics returned by one glob execution.
pub const MAX_GLOB_DIAGNOSTICS: usize = 100;

/// Runtime-internal input for the `glob` capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobRequest {
    pub pattern: String,
    pub base_path: Option<PathBuf>,
}

impl GlobRequest {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            base_path: None,
        }
    }

    pub fn with_base_path(mut self, base_path: impl Into<PathBuf>) -> Self {
        self.base_path = Some(base_path.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobOutput {
    pub matches: Vec<GlobMatch>,
    pub truncated: bool,
    pub diagnostics: Vec<GlobDiagnostic>,
    pub diagnostics_truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobMatch {
    /// Normalized logical path relative to the workspace root, using `/`.
    pub path: String,
    pub kind: GlobMatchKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobMatchKind {
    File,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobDiagnostic {
    /// Normalized logical path relative to the workspace root, using `/`.
    pub path: String,
    pub code: GlobDiagnosticCode,
    /// Sanitized I/O context; canonical filesystem targets are never exposed.
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobDiagnosticCode {
    WorkspaceRejected,
    BrokenLink,
    MetadataFailed,
    ReadDirectoryFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobError {
    RuntimeNotInitialized,
    InvalidPattern { pattern: String, message: String },
    Workspace(WorkspaceError),
    TargetNotDirectory { path: String },
    IoFailure { path: String, kind: io::ErrorKind },
}

impl fmt::Display for GlobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeNotInitialized => write!(formatter, "runtime is not initialized"),
            Self::InvalidPattern { pattern, message } => {
                write!(formatter, "invalid glob pattern {pattern:?}: {message}")
            }
            Self::Workspace(error) => write!(formatter, "workspace access rejected: {error}"),
            Self::TargetNotDirectory { path } => {
                write!(formatter, "base path is not a directory: {path}")
            }
            Self::IoFailure { path, kind } => {
                write!(formatter, "I/O failure for {path} ({kind:?})")
            }
        }
    }
}

impl std::error::Error for GlobError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Workspace(error) => Some(error),
            _ => None,
        }
    }
}

/// Executes a guarded, deterministic workspace glob.
///
/// The `WorkspaceGuard` remains the only filesystem boundary authority. Every
/// candidate is approved before metadata inspection or directory traversal;
/// logical paths are retained separately for pattern matching and output.
pub(crate) fn execute_glob(
    _permit: &crate::execution::ExecutionPermit,
    workspace_guard: &WorkspaceGuard,
    request: &GlobRequest,
) -> Result<GlobOutput, GlobError> {
    let matcher = compile_matcher(&request.pattern)?;
    let (base_logical, base_approved) = approve_base_path(workspace_guard, request)?;

    let base_metadata =
        fs::metadata(base_approved.as_path()).map_err(|error| GlobError::IoFailure {
            path: logical_to_string(&base_logical),
            kind: error.kind(),
        })?;
    if !base_metadata.is_dir() {
        return Err(GlobError::TargetNotDirectory {
            path: logical_to_string(&base_logical),
        });
    }

    let mut pending = BinaryHeap::new();
    let base_display = logical_to_string(&base_logical);
    pending.push(PendingPath::new(base_display.clone(), base_approved));

    let mut visited_directories = HashSet::new();
    let mut matches = Vec::with_capacity(MAX_GLOB_RESULTS);
    let mut diagnostics = Diagnostics::default();
    let mut truncated = false;

    while let Some(current) = pending.pop() {
        let metadata = match fs::metadata(current.approved.as_path()) {
            Ok(metadata) => metadata,
            Err(error) => {
                diagnostics.push(
                    current.logical_path,
                    GlobDiagnosticCode::MetadataFailed,
                    Some(sanitized_io_message(error.kind())),
                );
                continue;
            }
        };

        let kind = if metadata.is_dir() {
            GlobMatchKind::Directory
        } else {
            GlobMatchKind::File
        };
        if !current.logical_path.is_empty()
            && matcher.is_match(relative_to_base(&base_display, &current.logical_path))
        {
            if matches.len() == MAX_GLOB_RESULTS {
                truncated = true;
                break;
            }
            matches.push(GlobMatch {
                path: current.logical_path.clone(),
                kind,
            });
        }

        if !metadata.is_dir() {
            continue;
        }
        if !visited_directories.insert(current.approved.as_path().to_path_buf()) {
            continue;
        }

        let entries = match read_sorted_entries(current.approved.as_path()) {
            Ok(entries) => entries,
            Err(error) => {
                diagnostics.push(
                    current.logical_path,
                    GlobDiagnosticCode::ReadDirectoryFailed,
                    Some(sanitized_io_message(error.kind())),
                );
                continue;
            }
        };

        for entry in entries {
            let candidate_logical = join_logical(&current.logical_path, &entry.file_name());
            let candidate_path = PathBuf::from(&candidate_logical);
            match workspace_guard.approve(&candidate_path) {
                Ok(approved) => pending.push(PendingPath::new(candidate_logical, approved)),
                Err(error) => diagnostics.push(
                    candidate_logical,
                    diagnostic_code_for_workspace_error(&error),
                    None,
                ),
            }
        }
    }

    Ok(GlobOutput {
        matches,
        truncated,
        diagnostics: diagnostics.items,
        diagnostics_truncated: diagnostics.truncated,
    })
}

fn approve_base_path(
    workspace_guard: &WorkspaceGuard,
    request: &GlobRequest,
) -> Result<(PathBuf, ApprovedPath), GlobError> {
    let requested = request
        .base_path
        .as_deref()
        .unwrap_or_else(|| Path::new("."));
    let approved = workspace_guard
        .approve(requested)
        .map_err(GlobError::Workspace)?;
    let logical = workspace_guard.logical_relative_path(requested, &approved);
    Ok((logical, approved))
}

/// Reuses the executor's pure pattern validator before filesystem enforcement.
pub(crate) fn validate_glob_pattern(pattern: &str) -> Result<(), GlobError> {
    compile_matcher(pattern).map(|_| ())
}

fn compile_matcher(pattern: &str) -> Result<GlobMatcher, GlobError> {
    validate_pattern_form(pattern)?;
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|error| GlobError::InvalidPattern {
            pattern: pattern.into(),
            message: error.to_string(),
        })
}

fn validate_pattern_form(pattern: &str) -> Result<(), GlobError> {
    if pattern.is_empty() {
        return Err(invalid_pattern(pattern, "pattern must not be empty"));
    }
    if pattern.contains('\\') {
        return Err(invalid_pattern(
            pattern,
            "patterns must use '/' as the path separator",
        ));
    }
    if pattern.starts_with('/') || pattern.starts_with("//") || is_windows_drive_pattern(pattern) {
        return Err(invalid_pattern(
            pattern,
            "absolute or rooted patterns are not allowed",
        ));
    }
    if pattern.split('/').any(|segment| segment == "..") {
        return Err(invalid_pattern(
            pattern,
            "parent-directory segments are not allowed",
        ));
    }
    Ok(())
}

fn invalid_pattern(pattern: &str, message: &str) -> GlobError {
    GlobError::InvalidPattern {
        pattern: pattern.into(),
        message: message.into(),
    }
}

fn is_windows_drive_pattern(pattern: &str) -> bool {
    let bytes = pattern.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn read_sorted_entries(path: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        entries.push(entry?);
    }
    entries.sort_by(|left, right| normalized_file_name(left).cmp(&normalized_file_name(right)));
    Ok(entries)
}

fn normalized_file_name(entry: &fs::DirEntry) -> String {
    entry.file_name().to_string_lossy().replace('\\', "/")
}

fn join_logical(parent: &str, name: &std::ffi::OsStr) -> String {
    let name = name.to_string_lossy().replace('\\', "/");
    if parent.is_empty() {
        name
    } else {
        format!("{parent}/{name}")
    }
}

fn logical_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn relative_to_base<'a>(base: &str, logical_path: &'a str) -> &'a str {
    if base.is_empty() {
        logical_path
    } else {
        logical_path
            .strip_prefix(base)
            .and_then(|path| path.strip_prefix('/'))
            .unwrap_or(logical_path)
    }
}

fn diagnostic_code_for_workspace_error(error: &WorkspaceError) -> GlobDiagnosticCode {
    match error {
        WorkspaceError::TargetNotFound { .. } => GlobDiagnosticCode::BrokenLink,
        _ => GlobDiagnosticCode::WorkspaceRejected,
    }
}

fn sanitized_io_message(kind: io::ErrorKind) -> String {
    format!("filesystem error: {kind:?}")
}

#[derive(Debug)]
struct PendingPath {
    logical_path: String,
    approved: ApprovedPath,
}

impl PendingPath {
    fn new(logical_path: String, approved: ApprovedPath) -> Self {
        Self {
            logical_path,
            approved,
        }
    }
}

impl PartialEq for PendingPath {
    fn eq(&self, other: &Self) -> bool {
        self.logical_path == other.logical_path
    }
}

impl Eq for PendingPath {}

impl PartialOrd for PendingPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingPath {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse lexical order because `BinaryHeap` is a max-heap.
        other.logical_path.cmp(&self.logical_path)
    }
}

#[derive(Default)]
struct Diagnostics {
    items: Vec<GlobDiagnostic>,
    truncated: bool,
}

impl Diagnostics {
    fn push(&mut self, path: String, code: GlobDiagnosticCode, message: Option<String>) {
        if self.items.len() < MAX_GLOB_DIAGNOSTICS {
            self.items.push(GlobDiagnostic {
                path,
                code,
                message,
            });
        } else {
            self.truncated = true;
        }
    }
}

#[cfg(test)]
mod tests {
    // These M5 unit tests target the low-level algorithm, not production entrypoints.
    fn execute_glob(
        guard: &crate::workspace::WorkspaceGuard,
        request: &super::GlobRequest,
    ) -> Result<super::GlobOutput, super::GlobError> {
        super::execute_glob(
            &crate::execution::ExecutionPermit::for_executor_test(),
            guard,
            request,
        )
    }
    use super::*;
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
                std::env::temp_dir().join(format!("chatgpt-harness-glob-{}", uuid::Uuid::new_v4()));
            let root = base.join("workspace");
            let outside = base.join("outside");
            fs::create_dir_all(&root).unwrap();
            fs::create_dir_all(&outside).unwrap();
            Self {
                base,
                root,
                outside,
            }
        }

        fn guard(&self) -> WorkspaceGuard {
            WorkspaceGuard::new(&Workspace::new(&self.root).unwrap())
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn paths(output: &GlobOutput) -> Vec<&str> {
        output
            .matches
            .iter()
            .map(|item| item.path.as_str())
            .collect()
    }

    #[test]
    fn matches_files_directories_and_supported_patterns() {
        let workspace = TestWorkspace::new();
        workspace.write("lib.rs", "");
        workspace.write("src/main.rs", "");
        workspace.write("src/nested/mod.rs", "");
        workspace.write("docs/readme.md", "");
        fs::create_dir_all(workspace.root.join("empty-dir")).unwrap();
        let guard = workspace.guard();

        assert_eq!(
            paths(&execute_glob(&guard, &GlobRequest::new("*.rs")).unwrap()),
            ["lib.rs"]
        );
        assert_eq!(
            paths(&execute_glob(&guard, &GlobRequest::new("src/*.rs")).unwrap()),
            ["src/main.rs"]
        );
        assert_eq!(
            paths(&execute_glob(&guard, &GlobRequest::new("src/**/*.rs")).unwrap()),
            ["src/main.rs", "src/nested/mod.rs"]
        );
        assert_eq!(
            paths(&execute_glob(&guard, &GlobRequest::new("**/*.md")).unwrap()),
            ["docs/readme.md"]
        );
        assert_eq!(
            paths(&execute_glob(&guard, &GlobRequest::new("src/?.rs")).unwrap()),
            Vec::<&str>::new()
        );

        let directories = execute_glob(&guard, &GlobRequest::new("empty-*"))
            .unwrap()
            .matches;
        assert_eq!(
            directories,
            vec![GlobMatch {
                path: "empty-dir".into(),
                kind: GlobMatchKind::Directory
            }]
        );
        assert!(execute_glob(&guard, &GlobRequest::new("none/*"))
            .unwrap()
            .matches
            .is_empty());
    }

    #[test]
    fn base_path_is_guarded_and_outputs_workspace_relative_paths() {
        let workspace = TestWorkspace::new();
        workspace.write("src/main.rs", "");
        workspace.write("src/nested/lib.rs", "");
        workspace.write("plain.txt", "");
        let guard = workspace.guard();

        let output =
            execute_glob(&guard, &GlobRequest::new("**/*.rs").with_base_path("src")).unwrap();
        assert_eq!(paths(&output), ["src/main.rs", "src/nested/lib.rs"]);

        assert!(matches!(
            execute_glob(&guard, &GlobRequest::new("*").with_base_path("plain.txt")),
            Err(GlobError::TargetNotDirectory { .. })
        ));
        assert!(matches!(
            execute_glob(&guard, &GlobRequest::new("*").with_base_path("missing")),
            Err(GlobError::Workspace(WorkspaceError::TargetNotFound { .. }))
        ));
        assert!(matches!(
            execute_glob(&guard, &GlobRequest::new("*").with_base_path("../outside")),
            Err(GlobError::Workspace(WorkspaceError::WorkspaceEscape { .. }))
        ));
        assert!(matches!(
            execute_glob(
                &guard,
                &GlobRequest::new("*").with_base_path(&workspace.outside)
            ),
            Err(GlobError::Workspace(WorkspaceError::WorkspaceEscape { .. }))
        ));
    }

    #[test]
    fn rejects_absolute_parent_and_malformed_patterns() {
        let workspace = TestWorkspace::new();
        let guard = workspace.guard();
        for pattern in ["/etc/*", "../*.rs", "C:/temp/*", "src\\*.rs", "["] {
            assert!(matches!(
                execute_glob(&guard, &GlobRequest::new(pattern)),
                Err(GlobError::InvalidPattern { .. })
            ));
        }
    }

    #[test]
    fn results_are_lexically_ordered_and_bounded_without_global_collection() {
        let workspace = TestWorkspace::new();
        workspace.write("a/z.rs", "");
        workspace.write("a-file.rs", "");
        workspace.write("a/a.rs", "");
        workspace.write("z.rs", "");
        let guard = workspace.guard();

        let first = execute_glob(&guard, &GlobRequest::new("**/*.rs")).unwrap();
        let second = execute_glob(&guard, &GlobRequest::new("**/*.rs")).unwrap();
        assert_eq!(first.matches, second.matches);
        assert_eq!(paths(&first), ["a-file.rs", "a/a.rs", "a/z.rs", "z.rs"]);

        for index in 0..=MAX_GLOB_RESULTS {
            workspace.write(&format!("many/file-{index:04}.txt"), "");
        }
        let output = execute_glob(&guard, &GlobRequest::new("many/*.txt")).unwrap();
        assert_eq!(output.matches.len(), MAX_GLOB_RESULTS);
        assert!(output.truncated);
        assert_eq!(output.matches.first().unwrap().path, "many/file-0000.txt");
        assert_eq!(output.matches.last().unwrap().path, "many/file-0999.txt");
    }

    #[test]
    fn diagnostics_are_bounded() {
        let mut diagnostics = Diagnostics::default();
        for index in 0..=MAX_GLOB_DIAGNOSTICS {
            diagnostics.push(index.to_string(), GlobDiagnosticCode::BrokenLink, None);
        }
        assert_eq!(diagnostics.items.len(), MAX_GLOB_DIAGNOSTICS);
        assert!(diagnostics.truncated);
    }

    #[cfg(windows)]
    #[test]
    fn symlinks_and_junctions_cannot_escape_and_cycles_do_not_recurse_forever() {
        use std::os::windows::fs::symlink_dir;

        let workspace = TestWorkspace::new();
        workspace.write("inside-target/inside.md", "inside");
        fs::create_dir_all(workspace.outside.join("outside-target")).unwrap();
        fs::write(workspace.outside.join("outside-target/secret.md"), "secret").unwrap();
        let inside_link = workspace.root.join("inside-link");
        let outside_link = workspace.root.join("outside-link");
        let loop_link = workspace.root.join("loop");
        if symlink_dir(workspace.root.join("inside-target"), &inside_link).is_err()
            || symlink_dir(workspace.outside.join("outside-target"), &outside_link).is_err()
            || symlink_dir(&workspace.root, &loop_link).is_err()
        {
            eprintln!("SKIP symlink glob test: symlink creation is unavailable");
            return;
        }
        let guard = workspace.guard();
        let output = execute_glob(&guard, &GlobRequest::new("**/*.md")).unwrap();
        assert!(paths(&output).contains(&"inside-link/inside.md"));
        assert!(!paths(&output).iter().any(|path| path.contains("secret")));
        assert!(output
            .diagnostics
            .iter()
            .any(|item| item.path == "outside-link"));

        let junction = workspace.root.join("inside-junction");
        if create_junction(&junction, &workspace.root.join("inside-target")) {
            let junction_output =
                execute_glob(&guard, &GlobRequest::new("inside-junction/**/*.md")).unwrap();
            assert_eq!(paths(&junction_output), ["inside-junction/inside.md"]);
        } else {
            eprintln!("SKIP junction glob test: mklink /J is unavailable");
        }
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
