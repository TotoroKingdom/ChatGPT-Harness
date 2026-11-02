use crate::workspace::{WorkspaceError, WorkspaceGuard};
use globset::{GlobBuilder, GlobMatcher};
use harness_control_protocol::GrepMode;
use regex::{Regex, RegexBuilder};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub const MAX_QUERY_BYTES: usize = 4096;
pub const MAX_FILTER_PATTERNS_PER_KIND: usize = 32;
pub const MAX_FILTER_PATTERN_BYTES: usize = 512;
pub const DEFAULT_MAX_RESULTS: u32 = 100;
pub const MAX_MAX_RESULTS: u32 = 1000;
pub const DEFAULT_MAX_OUTPUT_BYTES: u32 = 65_536;
pub const MAX_MAX_OUTPUT_BYTES: u32 = 262_144;
pub const MAX_PREVIEW_BYTES: usize = 4096;
pub const MAX_DISCOVERED_ENTRIES: usize = 50_000;
pub const MAX_SCANNED_FILES: usize = 10_000;
pub const MAX_SCANNED_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_SINGLE_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_DIAGNOSTICS: usize = 100;
pub const SEARCH_WALL_TIMEOUT: Duration = Duration::from_secs(5);
pub const REGEX_COMPILED_PROGRAM_LIMIT: usize = 2 * 1024 * 1024;
pub const MAX_SCOPE_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrepRequest {
    pub query: String,
    pub mode: GrepMode,
    pub scope: PathBuf,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub max_results: u32,
    pub max_output_bytes: u32,
}

impl GrepRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            mode: GrepMode::Literal,
            scope: PathBuf::from("."),
            include: Vec::new(),
            exclude: Vec::new(),
            max_results: DEFAULT_MAX_RESULTS,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    pub fn with_mode(mut self, mode: GrepMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_scope(mut self, scope: impl Into<PathBuf>) -> Self {
        self.scope = scope.into();
        self
    }

    pub fn with_include(mut self, include: Vec<String>) -> Self {
        self.include = include;
        self
    }

    pub fn with_exclude(mut self, exclude: Vec<String>) -> Self {
        self.exclude = exclude;
        self
    }

    pub fn with_limits(mut self, max_results: u32, max_output_bytes: u32) -> Self {
        self.max_results = max_results;
        self.max_output_bytes = max_output_bytes;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GrepOutput {
    pub matches: Vec<GrepMatch>,
    pub scanned_files: u64,
    pub scanned_bytes: u64,
    pub skipped_files: u64,
    pub truncated: bool,
    pub truncation_reasons: Vec<GrepTruncationReason>,
    pub diagnostics: Vec<GrepDiagnostic>,
    pub diagnostics_truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GrepMatch {
    pub path: String,
    pub line_number: u64,
    pub preview: String,
    pub preview_start_byte: u64,
    pub match_start_byte: u64,
    pub match_end_byte: u64,
    pub preview_truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrepTruncationReason {
    MaxResults,
    MaxOutputBytes,
    MaxDiscoveredEntries,
    MaxScannedFiles,
    MaxScannedBytes,
}

impl GrepTruncationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxResults => "max_results",
            Self::MaxOutputBytes => "max_output_bytes",
            Self::MaxDiscoveredEntries => "max_discovered_entries",
            Self::MaxScannedFiles => "max_scanned_files",
            Self::MaxScannedBytes => "max_scanned_bytes",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GrepDiagnostic {
    pub path: String,
    pub code: GrepDiagnosticCode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrepDiagnosticCode {
    BinarySkipped,
    InvalidUtf8Skipped,
    FileTooLarge,
    Unreadable,
    ReparseSkipped,
    TargetChanged,
    UnsupportedFile,
}

impl GrepDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BinarySkipped => "binary_skipped",
            Self::InvalidUtf8Skipped => "invalid_utf8_skipped",
            Self::FileTooLarge => "file_too_large",
            Self::Unreadable => "unreadable",
            Self::ReparseSkipped => "reparse_skipped",
            Self::TargetChanged => "target_changed",
            Self::UnsupportedFile => "unsupported_file",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrepError {
    InvalidArguments { message: String },
    InvalidRegex { message: String },
    InvalidFilterPattern { message: String },
    InvalidScope { message: String },
    Workspace(WorkspaceError),
    IoFailure { path: String, kind: io::ErrorKind },
    SearchTimeout,
    Cancelled,
}

impl fmt::Display for GrepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments { message } => {
                write!(formatter, "invalid grep arguments: {message}")
            }
            Self::InvalidRegex { message } => write!(formatter, "invalid grep regex: {message}"),
            Self::InvalidFilterPattern { message } => {
                write!(formatter, "invalid grep filter pattern: {message}")
            }
            Self::InvalidScope { message } => write!(formatter, "invalid grep scope: {message}"),
            Self::Workspace(error) => write!(formatter, "grep workspace access rejected: {error}"),
            Self::IoFailure { path, kind } => {
                write!(formatter, "grep I/O failure for {path}: {kind:?}")
            }
            Self::SearchTimeout => write!(formatter, "grep search timed out"),
            Self::Cancelled => write!(formatter, "grep search was cancelled"),
        }
    }
}

impl std::error::Error for GrepError {}

pub(crate) fn validate_grep_request(request: &GrepRequest) -> Result<(), GrepError> {
    if request.query.is_empty() {
        return Err(GrepError::InvalidArguments {
            message: "query must not be empty".into(),
        });
    }
    if request.query.len() > MAX_QUERY_BYTES {
        return Err(GrepError::InvalidArguments {
            message: "query exceeds 4096 UTF-8 bytes".into(),
        });
    }
    if !(1..=MAX_MAX_RESULTS).contains(&request.max_results) {
        return Err(GrepError::InvalidArguments {
            message: "maxResults must be between 1 and 1000".into(),
        });
    }
    if !(1024..=MAX_MAX_OUTPUT_BYTES).contains(&request.max_output_bytes) {
        return Err(GrepError::InvalidArguments {
            message: "maxOutputBytes must be between 1024 and 262144".into(),
        });
    }
    validate_scope_form(&request.scope)?;
    validate_filter_patterns(&request.include, "include")?;
    validate_filter_patterns(&request.exclude, "exclude")?;
    Ok(())
}

/// Executes a grep request only after the ExecutionPipeline has granted the
/// private permit. The synchronous form remains available to non-async
/// callers and tests; the MCP path uses the blocking-worker variant below.
pub(crate) fn execute_grep(
    _permit: &crate::execution::ExecutionPermit,
    guard: &WorkspaceGuard,
    request: &GrepRequest,
) -> Result<GrepOutput, GrepError> {
    execute_grep_inner(guard, request, CancellationToken::new())
}

pub(crate) fn execute_grep_cancellable(
    guard: &WorkspaceGuard,
    request: &GrepRequest,
    cancel: CancellationToken,
) -> Result<GrepOutput, GrepError> {
    execute_grep_inner(guard, request, cancel)
}

fn execute_grep_inner(
    guard: &WorkspaceGuard,
    request: &GrepRequest,
    cancel: CancellationToken,
) -> Result<GrepOutput, GrepError> {
    validate_grep_request(request)?;
    let matcher = compile_matcher(request)?;
    let filters = Filters::compile(request)?;
    let scope = approve_scope(guard, &request.scope)?;
    let started = Instant::now();
    check_control(&cancel, started)?;

    let mut pending = BinaryHeap::new();
    pending.push(PendingEntry::directory(scope.logical.clone(), scope.path));
    let mut state = SearchState::default();

    while let Some(entry) = pending.pop() {
        check_control(&cancel, started)?;
        if entry.kind == EntryKind::File {
            match scan_file(
                guard, &entry, request, &matcher, &filters, &mut state, &cancel, started,
            )? {
                FileOutcome::Continue => {}
                FileOutcome::Stop(reason) => {
                    state.stop(reason);
                    break;
                }
            }
            continue;
        }

        if state.discovered_entries >= MAX_DISCOVERED_ENTRIES {
            state.stop(GrepTruncationReason::MaxDiscoveredEntries);
            break;
        }
        let remaining = MAX_DISCOVERED_ENTRIES - state.discovered_entries;
        let entries = match read_sorted_entries(&entry.path, remaining) {
            Ok(entries) => entries,
            Err(error) => {
                state.diagnostics.push(
                    entry.logical.clone(),
                    GrepDiagnosticCode::Unreadable,
                    Some(error.kind()),
                );
                continue;
            }
        };
        if entries.overflow {
            state.stop(GrepTruncationReason::MaxDiscoveredEntries);
            break;
        }
        for (name, path) in entries.items {
            state.discovered_entries += 1;
            let logical = join_logical(&entry.logical, &name);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    state.diagnostics.push(
                        logical,
                        if error.kind() == io::ErrorKind::NotFound {
                            GrepDiagnosticCode::TargetChanged
                        } else {
                            GrepDiagnosticCode::Unreadable
                        },
                        Some(error.kind()),
                    );
                    continue;
                }
            };
            if is_reparse_or_symlink(&metadata) {
                state.skipped_files += 1;
                state
                    .diagnostics
                    .push(logical, GrepDiagnosticCode::ReparseSkipped, None);
                continue;
            }
            if metadata.is_dir() {
                pending.push(PendingEntry::directory(logical, path));
            } else if metadata.is_file() {
                pending.push(PendingEntry::file(logical, path));
            } else {
                state.skipped_files += 1;
                state
                    .diagnostics
                    .push(logical, GrepDiagnosticCode::UnsupportedFile, None);
            }
        }
    }

    Ok(state.finish())
}

fn scan_file(
    guard: &WorkspaceGuard,
    entry: &PendingEntry,
    request: &GrepRequest,
    matcher: &LineMatcher,
    filters: &Filters,
    state: &mut SearchState,
    cancel: &CancellationToken,
    started: Instant,
) -> Result<FileOutcome, GrepError> {
    if !filters.allows(&entry.logical) {
        return Ok(FileOutcome::Continue);
    }
    let metadata = match fs::symlink_metadata(&entry.path) {
        Ok(metadata) => metadata,
        Err(error) => {
            state.skipped_files += 1;
            state.diagnostics.push(
                entry.logical.clone(),
                if error.kind() == io::ErrorKind::NotFound {
                    GrepDiagnosticCode::TargetChanged
                } else {
                    GrepDiagnosticCode::Unreadable
                },
                Some(error.kind()),
            );
            return Ok(FileOutcome::Continue);
        }
    };
    if is_reparse_or_symlink(&metadata) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::ReparseSkipped,
            None,
        );
        return Ok(FileOutcome::Continue);
    }
    if metadata.is_dir() {
        return Ok(FileOutcome::Continue);
    }
    if !metadata.is_file() {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::UnsupportedFile,
            None,
        );
        return Ok(FileOutcome::Continue);
    }
    if metadata.len() > MAX_SINGLE_FILE_BYTES {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::FileTooLarge,
            None,
        );
        return Ok(FileOutcome::Continue);
    }
    if state.scanned_files >= MAX_SCANNED_FILES as u64 {
        return Ok(FileOutcome::Stop(GrepTruncationReason::MaxScannedFiles));
    }
    if state.scanned_bytes.saturating_add(metadata.len()) > MAX_SCANNED_BYTES {
        return Ok(FileOutcome::Stop(GrepTruncationReason::MaxScannedBytes));
    }

    run_toctou_hook(&entry.path);
    let file = match File::open(&entry.path) {
        Ok(file) => file,
        Err(error) => {
            state.skipped_files += 1;
            state.diagnostics.push(
                entry.logical.clone(),
                if error.kind() == io::ErrorKind::NotFound {
                    GrepDiagnosticCode::TargetChanged
                } else {
                    GrepDiagnosticCode::Unreadable
                },
                Some(error.kind()),
            );
            return Ok(FileOutcome::Continue);
        }
    };

    let final_path = opened_final_path(&file).map_err(|error| GrepError::IoFailure {
        path: entry.logical.clone(),
        kind: error.kind(),
    })?;
    if !guard.contains_canonical_path(&final_path) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::TargetChanged,
            None,
        );
        return Ok(FileOutcome::Continue);
    }
    let opened_metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            state.skipped_files += 1;
            state.diagnostics.push(
                entry.logical.clone(),
                GrepDiagnosticCode::Unreadable,
                Some(error.kind()),
            );
            return Ok(FileOutcome::Continue);
        }
    };
    if !opened_metadata.is_file() || is_reparse_or_symlink(&opened_metadata) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::TargetChanged,
            None,
        );
        return Ok(FileOutcome::Continue);
    }
    if opened_metadata.len() > MAX_SINGLE_FILE_BYTES {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::FileTooLarge,
            None,
        );
        return Ok(FileOutcome::Continue);
    }
    if state.scanned_bytes.saturating_add(opened_metadata.len()) > MAX_SCANNED_BYTES {
        return Ok(FileOutcome::Stop(GrepTruncationReason::MaxScannedBytes));
    }

    state.scanned_files += 1;
    let read_limit = opened_metadata
        .len()
        .min(MAX_SINGLE_FILE_BYTES)
        .saturating_add(1);
    let mut bytes = Vec::new();
    if let Err(error) = file.take(read_limit).read_to_end(&mut bytes) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::Unreadable,
            Some(error.kind()),
        );
        return Ok(FileOutcome::Continue);
    }
    state.scanned_bytes = state.scanned_bytes.saturating_add(bytes.len() as u64);
    if bytes.len() as u64 > MAX_SINGLE_FILE_BYTES {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::FileTooLarge,
            None,
        );
        return Ok(FileOutcome::Continue);
    }
    if bytes.contains(&0) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::BinarySkipped,
            None,
        );
        return Ok(FileOutcome::Continue);
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            state.skipped_files += 1;
            state.diagnostics.push(
                entry.logical.clone(),
                GrepDiagnosticCode::InvalidUtf8Skipped,
                None,
            );
            return Ok(FileOutcome::Continue);
        }
    };
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    if let Some(reason) = scan_lines(
        &entry.logical,
        text,
        request,
        matcher,
        state,
        cancel,
        started,
    )? {
        return Ok(FileOutcome::Stop(reason));
    }
    Ok(FileOutcome::Continue)
}

fn scan_lines(
    logical_path: &str,
    text: &str,
    request: &GrepRequest,
    matcher: &LineMatcher,
    state: &mut SearchState,
    cancel: &CancellationToken,
    started: Instant,
) -> Result<Option<GrepTruncationReason>, GrepError> {
    let mut line_start = 0;
    let mut line_number = 1_u64;
    while line_start <= text.len() {
        check_control(cancel, started)?;
        let (line_end, next_start) = match text[line_start..].find('\n') {
            Some(offset) => (line_start + offset, line_start + offset + 1),
            None => (text.len(), text.len() + 1),
        };
        let mut trimmed_end = line_end;
        if trimmed_end > line_start && text.as_bytes()[trimmed_end - 1] == b'\r' {
            trimmed_end -= 1;
        }
        let line = &text[line_start..trimmed_end];
        if let Some(range) = matcher.find(line) {
            if state.matches.len() >= request.max_results as usize {
                return Ok(Some(GrepTruncationReason::MaxResults));
            }
            let (preview, preview_start) = bounded_preview(line, range.start, range.end);
            let cost = logical_path.len().saturating_add(preview.len()) as u64;
            if state.output_bytes.saturating_add(cost) > request.max_output_bytes as u64 {
                return Ok(Some(GrepTruncationReason::MaxOutputBytes));
            }
            let preview_truncated =
                preview_start != 0 || preview_start.saturating_add(preview.len()) < line.len();
            state.output_bytes = state.output_bytes.saturating_add(cost);
            state.matches.push(GrepMatch {
                path: logical_path.to_owned(),
                line_number,
                preview,
                preview_start_byte: preview_start as u64,
                match_start_byte: range.start as u64,
                match_end_byte: range.end as u64,
                preview_truncated,
            });
        }
        if next_start > text.len() {
            break;
        }
        line_start = next_start;
        line_number += 1;
    }
    Ok(None)
}

fn bounded_preview(line: &str, match_start: usize, match_end: usize) -> (String, usize) {
    if line.len() <= MAX_PREVIEW_BYTES {
        return (line.to_owned(), 0);
    }
    let match_len = match_end.saturating_sub(match_start);
    if match_len >= MAX_PREVIEW_BYTES {
        let end = previous_char_boundary(line, (match_start + MAX_PREVIEW_BYTES).min(line.len()));
        return (line[match_start..end].to_owned(), match_start);
    }
    let slack = MAX_PREVIEW_BYTES - match_len;
    let mut start = match_start.saturating_sub(slack / 2);
    while start < line.len() && !line.is_char_boundary(start) {
        start += 1;
    }
    if start > match_start {
        start = match_start;
    }
    let mut end = (start + MAX_PREVIEW_BYTES).min(line.len());
    while end > start && !line.is_char_boundary(end) {
        end -= 1;
    }
    if end < match_end {
        start = match_start.saturating_sub(MAX_PREVIEW_BYTES - match_len);
        while start < line.len() && !line.is_char_boundary(start) {
            start += 1;
        }
        end = (start + MAX_PREVIEW_BYTES).min(line.len());
        while end > start && !line.is_char_boundary(end) {
            end -= 1;
        }
    }
    (line[start..end].to_owned(), start)
}

fn previous_char_boundary(value: &str, mut index: usize) -> usize {
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn compile_matcher(request: &GrepRequest) -> Result<LineMatcher, GrepError> {
    match request.mode {
        GrepMode::Literal => Ok(LineMatcher::Literal(request.query.clone())),
        GrepMode::Regex => RegexBuilder::new(&request.query)
            .size_limit(REGEX_COMPILED_PROGRAM_LIMIT)
            .build()
            .map(LineMatcher::Regex)
            .map_err(|error| GrepError::InvalidRegex {
                message: error.to_string(),
            }),
    }
}

enum LineMatcher {
    Literal(String),
    Regex(Regex),
}

impl LineMatcher {
    fn find(&self, line: &str) -> Option<std::ops::Range<usize>> {
        match self {
            Self::Literal(query) => line.find(query).map(|start| start..start + query.len()),
            Self::Regex(regex) => regex.find(line).map(|found| found.range()),
        }
    }
}

struct Filters {
    include: Vec<GlobMatcher>,
    exclude: Vec<GlobMatcher>,
}

impl Filters {
    fn compile(request: &GrepRequest) -> Result<Self, GrepError> {
        Ok(Self {
            include: compile_patterns(&request.include)?,
            exclude: compile_patterns(&request.exclude)?,
        })
    }

    fn allows(&self, path: &str) -> bool {
        let included =
            self.include.is_empty() || self.include.iter().any(|glob| glob.is_match(path));
        included && !self.exclude.iter().any(|glob| glob.is_match(path))
    }
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<GlobMatcher>, GrepError> {
    patterns
        .iter()
        .map(|pattern| {
            GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map(|glob| glob.compile_matcher())
                .map_err(|error| GrepError::InvalidFilterPattern {
                    message: error.to_string(),
                })
        })
        .collect()
}

fn validate_filter_patterns(patterns: &[String], kind: &str) -> Result<(), GrepError> {
    if patterns.len() > MAX_FILTER_PATTERNS_PER_KIND {
        return Err(GrepError::InvalidArguments {
            message: format!("{kind} exceeds 32 patterns"),
        });
    }
    for pattern in patterns {
        if pattern.is_empty() || pattern.len() > MAX_FILTER_PATTERN_BYTES {
            return Err(GrepError::InvalidFilterPattern {
                message: format!("{kind} pattern must be 1..=512 UTF-8 bytes"),
            });
        }
        if pattern.starts_with('/')
            || pattern.starts_with("//")
            || pattern.contains('\\')
            || pattern.split('/').any(|segment| segment == "..")
            || is_windows_drive_pattern(pattern)
        {
            return Err(GrepError::InvalidFilterPattern {
                message: format!("{kind} pattern must be Workspace-relative with '/' separators"),
            });
        }
    }
    Ok(())
}

fn validate_scope_form(scope: &Path) -> Result<(), GrepError> {
    let display = scope.to_string_lossy();
    if display.len() > MAX_SCOPE_BYTES {
        return Err(GrepError::InvalidArguments {
            message: "scope exceeds 1024 UTF-8 bytes".into(),
        });
    }
    if display.is_empty() {
        return Err(GrepError::InvalidScope {
            message: "scope must not be empty".into(),
        });
    }
    if scope.is_absolute() {
        return Err(GrepError::InvalidScope {
            message: "absolute scope is not allowed".into(),
        });
    }
    for component in scope.components() {
        match component {
            Component::ParentDir => {
                return Err(GrepError::InvalidScope {
                    message: "parent traversal is not allowed".into(),
                })
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(GrepError::InvalidScope {
                    message: "rooted scope is not allowed".into(),
                })
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

struct ApprovedScope {
    logical: String,
    path: PathBuf,
}

fn approve_scope(guard: &WorkspaceGuard, requested: &Path) -> Result<ApprovedScope, GrepError> {
    validate_scope_form(requested)?;
    let root = guard.canonical_workspace_root();
    let mut candidate = root.to_path_buf();
    for component in requested.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        candidate.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&candidate) {
            if is_reparse_or_symlink(&metadata) {
                return Err(GrepError::InvalidScope {
                    message: "scope must not traverse a symlink or reparse point".into(),
                });
            }
        }
    }
    let approved = guard.approve(requested).map_err(GrepError::Workspace)?;
    if !fs::metadata(approved.as_path())
        .map_err(|error| GrepError::IoFailure {
            path: ".".into(),
            kind: error.kind(),
        })?
        .is_dir()
    {
        return Err(GrepError::InvalidScope {
            message: "scope must resolve to a directory".into(),
        });
    }
    let logical = guard.logical_relative_path(requested, &approved);
    Ok(ApprovedScope {
        logical: logical_to_string(&logical),
        path: approved.as_path().to_path_buf(),
    })
}

pub(crate) fn validate_grep_scope(
    guard: &WorkspaceGuard,
    requested: &Path,
) -> Result<String, GrepError> {
    approve_scope(guard, requested).map(|scope| {
        if scope.logical.is_empty() {
            ".".into()
        } else {
            scope.logical
        }
    })
}

#[derive(Default)]
struct SearchState {
    matches: Vec<GrepMatch>,
    scanned_files: u64,
    scanned_bytes: u64,
    skipped_files: u64,
    discovered_entries: usize,
    output_bytes: u64,
    truncation_reasons: Vec<GrepTruncationReason>,
    diagnostics: Diagnostics,
}

impl SearchState {
    fn stop(&mut self, reason: GrepTruncationReason) {
        if !self.truncation_reasons.contains(&reason) {
            self.truncation_reasons.push(reason);
        }
    }

    fn finish(self) -> GrepOutput {
        GrepOutput {
            truncated: !self.truncation_reasons.is_empty(),
            matches: self.matches,
            scanned_files: self.scanned_files,
            scanned_bytes: self.scanned_bytes,
            skipped_files: self.skipped_files,
            truncation_reasons: self.truncation_reasons,
            diagnostics: self.diagnostics.items,
            diagnostics_truncated: self.diagnostics.truncated,
        }
    }
}

#[derive(Default)]
struct Diagnostics {
    items: Vec<GrepDiagnostic>,
    truncated: bool,
}

impl Diagnostics {
    fn push(&mut self, path: String, code: GrepDiagnosticCode, _kind: Option<io::ErrorKind>) {
        if self.items.len() < MAX_DIAGNOSTICS {
            self.items.push(GrepDiagnostic { path, code });
        } else {
            self.truncated = true;
        }
    }
}

enum FileOutcome {
    Continue,
    Stop(GrepTruncationReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryKind {
    Directory,
    File,
}

struct PendingEntry {
    logical: String,
    path: PathBuf,
    kind: EntryKind,
}

impl PendingEntry {
    fn directory(logical: String, path: PathBuf) -> Self {
        Self {
            logical,
            path,
            kind: EntryKind::Directory,
        }
    }

    fn file(logical: String, path: PathBuf) -> Self {
        Self {
            logical,
            path,
            kind: EntryKind::File,
        }
    }
}

impl PartialEq for PendingEntry {
    fn eq(&self, other: &Self) -> bool {
        self.logical == other.logical
    }
}

impl Eq for PendingEntry {}

impl PartialOrd for PendingEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.logical.cmp(&self.logical)
    }
}

struct SortedEntries {
    items: Vec<(String, PathBuf)>,
    overflow: bool,
}

fn read_sorted_entries(path: &Path, remaining: usize) -> io::Result<SortedEntries> {
    let mut names = BTreeMap::<String, PathBuf>::new();
    let mut overflow = false;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            overflow = true;
            continue;
        };
        if names.len() < remaining {
            names.insert(name, entry.path());
        } else {
            overflow = true;
            if let Some(last) = names.keys().next_back().cloned() {
                if name < last {
                    names.remove(&last);
                    names.insert(name, entry.path());
                }
            }
        }
    }
    Ok(SortedEntries {
        items: names.into_iter().collect(),
        overflow,
    })
}

fn join_logical(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn logical_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn check_control(cancel: &CancellationToken, started: Instant) -> Result<(), GrepError> {
    if cancel.is_cancelled() {
        return Err(GrepError::Cancelled);
    }
    if started.elapsed() >= active_search_timeout() {
        return Err(GrepError::SearchTimeout);
    }
    Ok(())
}

#[cfg(not(test))]
fn active_search_timeout() -> Duration {
    SEARCH_WALL_TIMEOUT
}

#[cfg(test)]
thread_local! {
    static SEARCH_TIMEOUT_OVERRIDE: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn active_search_timeout() -> Duration {
    SEARCH_TIMEOUT_OVERRIDE
        .with(std::cell::Cell::get)
        .unwrap_or(SEARCH_WALL_TIMEOUT)
}

#[cfg(test)]
struct TimeoutOverride;

#[cfg(test)]
impl Drop for TimeoutOverride {
    fn drop(&mut self) {
        SEARCH_TIMEOUT_OVERRIDE.with(|value| value.set(None));
    }
}

#[cfg(test)]
fn override_search_timeout(timeout: Duration) -> TimeoutOverride {
    SEARCH_TIMEOUT_OVERRIDE.with(|value| value.set(Some(timeout)));
    TimeoutOverride
}

fn is_windows_drive_pattern(pattern: &str) -> bool {
    let bytes = pattern.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn opened_final_path(file: &File) -> io::Result<PathBuf> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;

        let handle = file.as_raw_handle();
        let mut buffer = vec![0_u16; 512];
        loop {
            let length = unsafe {
                GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0)
            };
            if length == 0 {
                return Err(io::Error::last_os_error());
            }
            let length = length as usize;
            if length < buffer.len() {
                buffer.truncate(length);
                return Ok(PathBuf::from(std::ffi::OsString::from_wide(&buffer)));
            }
            buffer.resize(length + 1, 0);
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        let link = PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
        return fs::read_link(link);
    }
    #[cfg(all(not(windows), not(target_os = "linux")))]
    {
        let _ = file;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "opened-handle final path is Windows-only",
        ))
    }
}

#[cfg(test)]
thread_local! {
    static TOCTOU_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce(&Path)>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
pub(crate) fn install_toctou_hook(hook: impl FnOnce(&Path) + 'static) {
    TOCTOU_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

#[cfg(test)]
fn run_toctou_hook(path: &Path) {
    TOCTOU_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook(path);
        }
    });
}

#[cfg(not(test))]
fn run_toctou_hook(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
        outside: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let base =
                std::env::temp_dir().join(format!("chatgpt-harness-grep-{}", uuid::Uuid::new_v4()));
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

        fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
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

    fn grep(workspace: &TestWorkspace, request: &GrepRequest) -> Result<GrepOutput, GrepError> {
        execute_grep(
            &crate::execution::ExecutionPermit::for_executor_test(),
            &workspace.guard(),
            request,
        )
    }

    #[test]
    fn literal_regex_bom_crlf_offsets_and_one_result_per_line() {
        let workspace = TestWorkspace::new();
        workspace.write("a/one.txt", "\u{FEFF}Alpha alpha ALPHA\r\nsecond alpha\r\n");
        workspace.write("b/two.txt", "alpha once\nnone\n");
        let literal = grep(&workspace, &GrepRequest::new("alpha")).unwrap();
        assert_eq!(
            literal
                .matches
                .iter()
                .map(|item| (item.path.as_str(), item.line_number))
                .collect::<Vec<_>>(),
            [("a/one.txt", 1), ("a/one.txt", 2), ("b/two.txt", 1)]
        );
        assert_eq!(literal.matches[0].match_start_byte, 6);
        assert_eq!(literal.matches[0].match_end_byte, 11);

        let regex = grep(
            &workspace,
            &GrepRequest::new("(?i)ALPHA").with_mode(GrepMode::Regex),
        )
        .unwrap();
        assert_eq!(regex.matches.len(), 3);
        assert_eq!(regex.matches[0].line_number, 1);
        assert!(!regex.matches[0].preview.contains('\r'));

        let none = grep(&workspace, &GrepRequest::new("missing")).unwrap();
        assert!(none.matches.is_empty());
        assert!(!none.truncated);
    }

    #[test]
    fn scope_filters_ordering_and_invalid_patterns_are_deterministic() {
        let workspace = TestWorkspace::new();
        workspace.write("z.txt", "needle\n");
        workspace.write("docs/b.md", "needle\n");
        workspace.write("docs/a.md", "needle\n");
        workspace.write("docs/skip.md", "needle\n");
        workspace.write("src/main.rs", "needle\n");
        workspace.write("a-file.txt", "needle\n");
        workspace.write("a/nested.txt", "needle\n");

        let output = grep(
            &workspace,
            &GrepRequest::new("needle")
                .with_scope("docs")
                .with_include(vec!["docs/*.md".into()])
                .with_exclude(vec!["docs/skip.*".into()]),
        )
        .unwrap();
        assert_eq!(
            output
                .matches
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            ["docs/a.md", "docs/b.md"]
        );
        for _ in 0..3 {
            assert_eq!(
                grep(&workspace, &GrepRequest::new("needle"))
                    .unwrap()
                    .matches,
                grep(&workspace, &GrepRequest::new("needle"))
                    .unwrap()
                    .matches
            );
        }
        assert_eq!(
            grep(&workspace, &GrepRequest::new("needle"))
                .unwrap()
                .matches
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            [
                "a-file.txt",
                "a/nested.txt",
                "docs/a.md",
                "docs/b.md",
                "docs/skip.md",
                "src/main.rs",
                "z.txt"
            ]
        );
        assert!(matches!(
            grep(
                &workspace,
                &GrepRequest::new("needle").with_include(vec!["[".into()])
            ),
            Err(GrepError::InvalidFilterPattern { .. })
        ));
        assert!(matches!(
            grep(
                &workspace,
                &GrepRequest::new("needle").with_scope("../outside")
            ),
            Err(GrepError::InvalidScope { .. })
        ));
        assert!(matches!(
            grep(
                &workspace,
                &GrepRequest::new("(").with_mode(GrepMode::Regex)
            ),
            Err(GrepError::InvalidRegex { .. })
        ));
    }

    #[test]
    fn limits_truncate_successfully_and_preview_is_bounded_utf8() {
        let workspace = TestWorkspace::new();
        let long = format!("{}needle{}", "界".repeat(3000), "x".repeat(3000));
        workspace.write("long.txt", format!("{long}\nneedle2\nneedle3\n"));
        let output = grep(
            &workspace,
            &GrepRequest::new("needle").with_limits(1, 65_536),
        )
        .unwrap();
        assert_eq!(output.matches.len(), 1);
        assert!(output.truncated);
        assert_eq!(
            output.truncation_reasons,
            [GrepTruncationReason::MaxResults]
        );
        assert!(output.matches[0].preview.len() <= MAX_PREVIEW_BYTES);
        assert!(output.matches[0].preview.contains("needle"));
        assert!(output.matches[0].preview_truncated);

        let budget_workspace = TestWorkspace::new();
        budget_workspace.write("many.txt", (0..100).map(|_| "needle\n").collect::<String>());
        let output = grep(
            &budget_workspace,
            &GrepRequest::new("needle").with_limits(100, 1024),
        )
        .unwrap();
        assert!(output.truncated);
        assert_eq!(
            output.truncation_reasons,
            [GrepTruncationReason::MaxOutputBytes]
        );
        assert!(!output.matches.is_empty());
        assert!(
            output
                .matches
                .iter()
                .map(|item| item.path.len() + item.preview.len())
                .sum::<usize>()
                <= 1024
        );
    }

    #[test]
    fn file_eligibility_produces_bounded_safe_diagnostics() {
        let workspace = TestWorkspace::new();
        workspace.write("ok.txt", "needle\n");
        workspace.write("binary.dat", b"needle\0SECRET");
        workspace.write("invalid.txt", [0xff, 0xfe, b'n']);
        let output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(output.matches.len(), 1);
        assert_eq!(output.skipped_files, 2);
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|item| item.code)
                .collect::<Vec<_>>(),
            [
                GrepDiagnosticCode::BinarySkipped,
                GrepDiagnosticCode::InvalidUtf8Skipped
            ]
        );
        let serialized = serde_json::to_string(&output).unwrap();
        assert!(!serialized.contains("SECRET"));
        assert!(!serialized.contains(&*workspace.root.to_string_lossy()));
    }

    #[test]
    fn cancellation_and_timeout_are_errors_without_partial_success() {
        let workspace = TestWorkspace::new();
        workspace.write("a.txt", "needle\n");
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            execute_grep_cancellable(&workspace.guard(), &GrepRequest::new("needle"), cancel),
            Err(GrepError::Cancelled)
        );
        let _timeout = override_search_timeout(Duration::ZERO);
        assert_eq!(
            grep(&workspace, &GrepRequest::new("needle")),
            Err(GrepError::SearchTimeout)
        );
    }

    #[test]
    fn request_boundaries_accept_exact_limits_and_reject_one_over() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "needle\n");
        let exact_query = "a".repeat(MAX_QUERY_BYTES);
        assert!(grep(&workspace, &GrepRequest::new(exact_query.clone())).is_ok());
        assert_eq!(
            grep(
                &workspace,
                &GrepRequest::new("a".repeat(MAX_QUERY_BYTES + 1))
            ),
            Err(GrepError::InvalidArguments {
                message: "query exceeds 4096 UTF-8 bytes".into()
            })
        );
        let exact_pattern = "a".repeat(MAX_FILTER_PATTERN_BYTES);
        assert!(grep(
            &workspace,
            &GrepRequest::new("needle").with_include(vec![exact_pattern])
        )
        .is_ok());
        assert!(matches!(
            grep(
                &workspace,
                &GrepRequest::new("needle")
                    .with_include(vec!["a".repeat(MAX_FILTER_PATTERN_BYTES + 1)])
            ),
            Err(GrepError::InvalidFilterPattern { .. })
        ));
        assert!(grep(
            &workspace,
            &GrepRequest::new("needle")
                .with_include(vec!["*".into(); MAX_FILTER_PATTERNS_PER_KIND])
        )
        .is_ok());
        assert!(matches!(
            grep(
                &workspace,
                &GrepRequest::new("needle")
                    .with_include(vec!["*".into(); MAX_FILTER_PATTERNS_PER_KIND + 1])
            ),
            Err(GrepError::InvalidArguments { .. })
        ));
        for (results, output, valid) in [
            (1, 1024, true),
            (MAX_MAX_RESULTS, MAX_MAX_OUTPUT_BYTES, true),
            (0, 1024, false),
            (MAX_MAX_RESULTS + 1, 1024, false),
            (1, 1023, false),
            (1, MAX_MAX_OUTPUT_BYTES + 1, false),
        ] {
            let result = grep(
                &workspace,
                &GrepRequest::new("needle").with_limits(results, output),
            );
            assert_eq!(result.is_ok(), valid, "results={results} output={output}");
        }
    }

    #[test]
    fn scanned_file_and_byte_limits_are_deterministic_and_do_not_read_past_the_boundary() {
        let workspace = TestWorkspace::new();
        let many = workspace.root.join("many");
        fs::create_dir_all(&many).unwrap();
        for index in 0..=MAX_SCANNED_FILES {
            File::create(many.join(format!("{index:05}.txt"))).unwrap();
        }
        let output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(output.scanned_files, MAX_SCANNED_FILES as u64);
        assert!(output.truncated);
        assert_eq!(
            output.truncation_reasons,
            [GrepTruncationReason::MaxScannedFiles]
        );

        let bytes_workspace = TestWorkspace::new();
        let large = bytes_workspace.root.join("large");
        fs::create_dir_all(&large).unwrap();
        for index in 0..=8 {
            let file = File::create(large.join(format!("{index}.bin"))).unwrap();
            file.set_len(MAX_SINGLE_FILE_BYTES).unwrap();
        }
        let output = grep(&bytes_workspace, &GrepRequest::new("needle")).unwrap();
        assert!(output.truncated);
        assert_eq!(
            output.truncation_reasons,
            [GrepTruncationReason::MaxScannedBytes]
        );
        assert!(output.scanned_bytes <= MAX_SCANNED_BYTES);
    }

    #[test]
    fn oversized_files_and_diagnostic_caps_are_typed_and_bounded() {
        let workspace = TestWorkspace::new();
        let oversized = File::create(workspace.root.join("oversized.txt")).unwrap();
        oversized.set_len(MAX_SINGLE_FILE_BYTES + 1).unwrap();
        let output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(output.skipped_files, 1);
        assert_eq!(output.diagnostics[0].code, GrepDiagnosticCode::FileTooLarge);

        let diagnostics_workspace = TestWorkspace::new();
        for index in 0..=MAX_DIAGNOSTICS {
            fs::write(
                diagnostics_workspace.root.join(format!("{index:03}.bin")),
                [0_u8, 0xff],
            )
            .unwrap();
        }
        let output = grep(&diagnostics_workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(output.diagnostics.len(), MAX_DIAGNOSTICS);
        assert!(output.diagnostics_truncated);
        assert_eq!(output.skipped_files, (MAX_DIAGNOSTICS + 1) as u64);
    }

    #[test]
    fn discovered_entry_selection_is_sorted_and_reports_overflow_without_reading_more() {
        let workspace = TestWorkspace::new();
        let directory = workspace.root.join("entries");
        fs::create_dir_all(&directory).unwrap();
        for name in ["c.txt", "a.txt", "b.txt"] {
            fs::write(directory.join(name), "").unwrap();
        }
        let selected = read_sorted_entries(&directory, 2).unwrap();
        assert!(selected.overflow);
        assert_eq!(
            selected
                .items
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["a.txt", "b.txt"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn reparse_candidates_and_scope_are_skipped_or_rejected_without_outside_bytes() {
        let workspace = TestWorkspace::new();
        workspace.write("inside.txt", "needle-inside\n");
        fs::write(workspace.outside.join("secret.txt"), "needle-outside\n").unwrap();
        let junction = workspace.root.join("outside-junction");
        if !create_junction(&junction, &workspace.outside) {
            eprintln!("SKIP junction grep test: mklink /J is unavailable");
            return;
        }
        let output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(output.matches.len(), 1);
        assert!(output
            .diagnostics
            .iter()
            .any(|item| item.code == GrepDiagnosticCode::ReparseSkipped));
        assert!(!serde_json::to_string(&output)
            .unwrap()
            .contains("needle-outside"));
        assert!(matches!(
            grep(
                &workspace,
                &GrepRequest::new("needle").with_scope("outside-junction")
            ),
            Err(GrepError::InvalidScope { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn opened_handle_final_path_rejects_toctou_junction_swap() {
        let workspace = TestWorkspace::new();
        workspace.write("swap/candidate.txt", "needle-inside\n");
        fs::write(workspace.outside.join("candidate.txt"), "needle-outside\n").unwrap();
        let swap_dir = workspace.root.join("swap");
        let outside = workspace.outside.clone();
        install_toctou_hook(move |path| {
            if path.file_name().and_then(|name| name.to_str()) == Some("candidate.txt") {
                let _ = fs::remove_dir_all(&swap_dir);
                let _ = create_junction(&swap_dir, &outside);
            }
        });
        let output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert!(output.matches.is_empty());
        assert_eq!(output.skipped_files, 1);
        assert_eq!(
            output.diagnostics[0].code,
            GrepDiagnosticCode::TargetChanged
        );
        assert!(!serde_json::to_string(&output)
            .unwrap()
            .contains("needle-outside"));
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
