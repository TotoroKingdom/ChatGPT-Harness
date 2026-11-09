use crate::tools::search::control::{ControlError, SearchBudget, SearchControl};
use crate::tools::search::filters::{fixed_directory_prefix, has_wildcard, PathFilters};
use crate::tools::search::walker::{
    self, LinkPolicy, WalkDiagnosticCode, WalkEntryKind, WalkRequest,
};
use crate::workspace::{ApprovedPath, WorkspaceError, WorkspaceGuard};
use globset::{GlobBuilder, GlobMatcher};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

/// Maximum number of matches returned by one glob execution.
pub const MAX_GLOB_RESULTS: usize = 1_000;
/// Maximum number of non-fatal traversal diagnostics returned by one glob execution.
pub const MAX_GLOB_DIAGNOSTICS: usize = 100;
/// Maximum number of entries discovered before traversal reports truncation.
pub const MAX_GLOB_DISCOVERED_ENTRIES: usize = 100_000;

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
    pub truncation_reasons: Vec<GlobTruncationReason>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobTruncationReason {
    MaxResults,
    MaxDiscoveredEntries,
    Timeout,
}

impl GlobTruncationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxResults => "max_results",
            Self::MaxDiscoveredEntries => "max_discovered_entries",
            Self::Timeout => "timeout",
        }
    }
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
    ReparseSkipped,
}

impl GlobDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceRejected => "WorkspaceRejected",
            Self::BrokenLink => "BrokenLink",
            Self::MetadataFailed => "MetadataFailed",
            Self::ReadDirectoryFailed => "ReadDirectoryFailed",
            Self::ReparseSkipped => "ReparseSkipped",
        }
    }
}

impl From<WalkDiagnosticCode> for GlobDiagnosticCode {
    fn from(code: WalkDiagnosticCode) -> Self {
        match code {
            WalkDiagnosticCode::WorkspaceRejected => Self::WorkspaceRejected,
            WalkDiagnosticCode::BrokenLink => Self::BrokenLink,
            WalkDiagnosticCode::MetadataFailed => Self::MetadataFailed,
            WalkDiagnosticCode::ReadDirectoryFailed => Self::ReadDirectoryFailed,
            WalkDiagnosticCode::ReparseSkipped => Self::ReparseSkipped,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobError {
    RuntimeNotInitialized,
    InvalidPattern { pattern: String, message: String },
    Workspace(WorkspaceError),
    TargetNotDirectory { path: String },
    IoFailure { path: String, kind: io::ErrorKind },
    SearchTimeout { partial: Option<Box<GlobOutput>> },
    Cancelled,
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
            Self::SearchTimeout { .. } => write!(formatter, "glob search timed out"),
            Self::Cancelled => write!(formatter, "glob search was cancelled"),
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

/// Executes a guarded, deterministic workspace glob with the default budget.
pub(crate) fn execute_glob(
    _permit: &crate::execution::ExecutionPermit,
    workspace_guard: &WorkspaceGuard,
    request: &GlobRequest,
) -> Result<GlobOutput, GlobError> {
    execute_glob_with_control(
        workspace_guard,
        request,
        SearchBudget::default(),
        CancellationToken::new(),
    )
}

pub(crate) fn execute_glob_with_control(
    workspace_guard: &WorkspaceGuard,
    request: &GlobRequest,
    budget: SearchBudget,
    cancel: CancellationToken,
) -> Result<GlobOutput, GlobError> {
    let plan = GlobPlan::compile(&request.pattern)?;
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

    let base_display = logical_to_string(&base_logical);
    let mut walk_logical = base_display.clone();
    let mut walk_path = base_approved;
    let mut ignore_overrides = Vec::new();
    if let Some(prefix) = &plan.fixed_prefix {
        let prefix_logical = if walk_logical.is_empty() {
            prefix.clone()
        } else {
            format!("{walk_logical}/{prefix}")
        };
        match workspace_guard.approve(Path::new(&prefix_logical)) {
            Ok(approved) => {
                if !fs::metadata(approved.as_path())
                    .map_err(|error| GlobError::IoFailure {
                        path: prefix_logical.clone(),
                        kind: error.kind(),
                    })?
                    .is_dir()
                {
                    return Ok(empty_output());
                }
                walk_logical = prefix_logical.clone();
                walk_path = approved;
                ignore_overrides.push(prefix_logical);
            }
            Err(WorkspaceError::TargetNotFound { .. }) => return Ok(empty_output()),
            Err(error) => return Err(GlobError::Workspace(error)),
        }
    }

    let control = Arc::new(Mutex::new(SearchControl::new(budget, cancel)));
    let walked = walker::walk(
        workspace_guard,
        WalkRequest {
            base_logical: walk_logical.clone(),
            base_path: walk_path.as_path().to_path_buf(),
            max_depth: plan.max_depth,
            links: LinkPolicy::FollowAuthorized,
            max_entries: MAX_GLOB_DISCOVERED_ENTRIES,
            filters: PathFilters::default(),
            ignore_overrides,
        },
        Arc::clone(&control),
    );

    let mut output = GlobOutput {
        matches: Vec::with_capacity(MAX_GLOB_RESULTS.min(walked.entries.len())),
        truncated: walked.truncated,
        truncation_reasons: if walked.truncated {
            vec![GlobTruncationReason::MaxDiscoveredEntries]
        } else {
            Vec::new()
        },
        diagnostics: walked
            .diagnostics
            .into_iter()
            .map(|diagnostic| GlobDiagnostic {
                path: diagnostic.logical,
                code: diagnostic.code.into(),
                message: diagnostic.message,
            })
            .collect(),
        diagnostics_truncated: walked.diagnostics_truncated,
    };

    for entry in walked.entries {
        if control
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .check()
            .is_err()
        {
            break;
        }
        let relative = relative_to_base(&base_display, &entry.logical);
        if relative.is_empty() {
            continue;
        }
        let name = entry
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !plan.prefilter_matches(&name) || !plan.matcher.is_match(relative) {
            continue;
        }
        if output.matches.len() == MAX_GLOB_RESULTS {
            output.truncated = true;
            if !output
                .truncation_reasons
                .contains(&GlobTruncationReason::MaxResults)
            {
                output
                    .truncation_reasons
                    .push(GlobTruncationReason::MaxResults);
            }
            break;
        }
        output.matches.push(GlobMatch {
            path: entry.logical,
            kind: match entry.kind {
                WalkEntryKind::File => GlobMatchKind::File,
                WalkEntryKind::Directory => GlobMatchKind::Directory,
            },
        });
    }

    match walked.control_error {
        Some(ControlError::Cancelled) => Err(GlobError::Cancelled),
        Some(ControlError::Timeout) => {
            output.truncated = true;
            if !output
                .truncation_reasons
                .contains(&GlobTruncationReason::Timeout)
            {
                output
                    .truncation_reasons
                    .push(GlobTruncationReason::Timeout);
            }
            if output.matches.is_empty() {
                Err(GlobError::SearchTimeout { partial: None })
            } else {
                Err(GlobError::SearchTimeout {
                    partial: Some(Box::new(output)),
                })
            }
        }
        None => Ok(output),
    }
}

fn empty_output() -> GlobOutput {
    GlobOutput {
        matches: Vec::new(),
        truncated: false,
        truncation_reasons: Vec::new(),
        diagnostics: Vec::new(),
        diagnostics_truncated: false,
    }
}

struct GlobPlan {
    matcher: GlobMatcher,
    fixed_prefix: Option<String>,
    max_depth: Option<usize>,
    name_filter: NameFilter,
}

impl GlobPlan {
    fn compile(pattern: &str) -> Result<Self, GlobError> {
        validate_pattern_form(pattern)?;
        let matcher = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map(|glob| glob.compile_matcher())
            .map_err(|error| GlobError::InvalidPattern {
                pattern: pattern.into(),
                message: error.to_string(),
            })?;
        let segments = pattern.split('/').collect::<Vec<_>>();
        let recursive = segments.iter().any(|segment| *segment == "**");
        let fixed_prefix = fixed_directory_prefix(pattern);
        let prefix_segments = fixed_prefix
            .as_deref()
            .map(|prefix| prefix.split('/').count())
            .unwrap_or(0);
        Ok(Self {
            matcher,
            fixed_prefix,
            max_depth: if recursive {
                None
            } else {
                Some(segments.len().saturating_sub(prefix_segments).max(1))
            },
            name_filter: NameFilter::from_terminal(segments.last().copied().unwrap_or("")),
        })
    }

    fn prefilter_matches(&self, name: &str) -> bool {
        match &self.name_filter {
            NameFilter::Any => true,
            NameFilter::Exact(expected) => name == expected,
            NameFilter::Extension(extension) => Path::new(name)
                .extension()
                .is_some_and(|value| value.to_string_lossy() == *extension),
        }
    }
}

enum NameFilter {
    Any,
    Exact(String),
    Extension(String),
}

impl NameFilter {
    fn from_terminal(segment: &str) -> Self {
        if !has_wildcard(segment) {
            return Self::Exact(segment.to_owned());
        }
        if let Some(extension) = segment.strip_prefix("*.") {
            if !has_wildcard(extension) && !extension.is_empty() {
                return Self::Extension(extension.to_owned());
            }
        }
        Self::Any
    }
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
    GlobPlan::compile(pattern).map(|_| ())
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
        assert_eq!(
            output.truncation_reasons,
            [GlobTruncationReason::MaxResults]
        );
        assert_eq!(output.matches.first().unwrap().path, "many/file-0000.txt");
        assert_eq!(output.matches.last().unwrap().path, "many/file-0999.txt");
    }

    #[test]
    fn gitignore_prunes_build_outputs_and_explicit_prefix_overrides_it() {
        let workspace = TestWorkspace::new();
        workspace.write(".gitignore", "target/\n*.log\n");
        workspace.write("target/debug/app.rs", "");
        workspace.write("src/main.rs", "");
        workspace.write("notes.log", "");
        let guard = workspace.guard();

        let output = execute_glob(&guard, &GlobRequest::new("**/*.rs")).unwrap();
        assert_eq!(paths(&output), ["src/main.rs"]);
        let logs = execute_glob(&guard, &GlobRequest::new("**/*.log")).unwrap();
        assert!(logs.matches.is_empty());

        let explicit = execute_glob(&guard, &GlobRequest::new("target/**/*.rs")).unwrap();
        assert_eq!(paths(&explicit), ["target/debug/app.rs"]);
    }

    #[test]
    fn star_is_shallow_but_double_star_recurses() {
        let workspace = TestWorkspace::new();
        workspace.write("top.rs", "");
        workspace.write("nested/deep.rs", "");
        let guard = workspace.guard();

        assert_eq!(
            paths(&execute_glob(&guard, &GlobRequest::new("*")).unwrap()),
            ["nested", "top.rs"]
        );
        assert_eq!(
            paths(&execute_glob(&guard, &GlobRequest::new("**/*.rs")).unwrap()),
            ["nested/deep.rs", "top.rs"]
        );
    }

    #[test]
    fn hidden_files_remain_visible_and_nested_gitignore_applies() {
        let workspace = TestWorkspace::new();
        workspace.write(".hidden.rs", "");
        workspace.write("nested/.gitignore", "secret.rs\n");
        workspace.write("nested/secret.rs", "");
        workspace.write("nested/visible.rs", "");
        let guard = workspace.guard();

        assert_eq!(
            paths(&execute_glob(&guard, &GlobRequest::new("**/*.rs")).unwrap()),
            [".hidden.rs", "nested/visible.rs"]
        );
    }

    #[test]
    fn timeout_and_cancellation_are_distinct_errors() {
        let workspace = TestWorkspace::new();
        workspace.write("a.rs", "");
        let guard = workspace.guard();
        assert!(matches!(
            execute_glob_with_control(
                &guard,
                &GlobRequest::new("**/*.rs"),
                SearchBudget {
                    timeout: std::time::Duration::ZERO
                },
                CancellationToken::new(),
            ),
            Err(GlobError::SearchTimeout { .. })
        ));
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            execute_glob_with_control(
                &guard,
                &GlobRequest::new("**/*.rs"),
                SearchBudget::default(),
                cancel,
            ),
            Err(GlobError::Cancelled)
        ));
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
