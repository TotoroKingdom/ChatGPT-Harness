use crate::tools::search::control::{ControlError, SearchBudget, SearchControl};
use crate::tools::search::filters::{fixed_directory_prefix, PathFilters};
use crate::tools::search::walker::{
    self, LinkPolicy, WalkDiagnosticCode, WalkEntry, WalkEntryKind, WalkRequest,
};
use crate::workspace::{WorkspaceError, WorkspaceGuard};
use grep_matcher::Matcher as _;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{BinaryDetection, SearcherBuilder, Sink, SinkMatch};
use harness_control_protocol::GrepMode;
use serde::Serialize;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
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
/// Historical name retained for callers that imported the old constant.
/// The effective budget now comes from Runtime configuration.
pub const SEARCH_WALL_TIMEOUT: Duration = Duration::from_secs(30);
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
    Timeout,
}

impl GrepTruncationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxResults => "max_results",
            Self::MaxOutputBytes => "max_output_bytes",
            Self::MaxDiscoveredEntries => "max_discovered_entries",
            Self::MaxScannedFiles => "max_scanned_files",
            Self::MaxScannedBytes => "max_scanned_bytes",
            Self::Timeout => "timeout",
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
    SearchTimeout { partial: Option<Box<GrepOutput>> },
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
            Self::SearchTimeout { .. } => write!(formatter, "grep search timed out"),
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
/// private permit. The synchronous form uses the default configured budget.
pub(crate) fn execute_grep(
    _permit: &crate::execution::ExecutionPermit,
    guard: &WorkspaceGuard,
    request: &GrepRequest,
) -> Result<GrepOutput, GrepError> {
    execute_grep_with_control(
        guard,
        request,
        active_search_budget(),
        CancellationToken::new(),
    )
}

pub(crate) fn execute_grep_cancellable(
    guard: &WorkspaceGuard,
    request: &GrepRequest,
    cancel: CancellationToken,
) -> Result<GrepOutput, GrepError> {
    execute_grep_with_control(guard, request, active_search_budget(), cancel)
}

pub(crate) fn execute_grep_with_control(
    guard: &WorkspaceGuard,
    request: &GrepRequest,
    budget: SearchBudget,
    cancel: CancellationToken,
) -> Result<GrepOutput, GrepError> {
    let budget = effective_search_budget(budget);
    validate_grep_request(request)?;
    let matcher = GrepMatcher::compile(request)?;
    let filters = PathFilters::compile(&request.include, &request.exclude).map_err(|error| {
        GrepError::InvalidFilterPattern {
            message: error.to_string(),
        }
    })?;
    let scope = approve_scope(guard, &request.scope)?;
    let control = Arc::new(Mutex::new(SearchControl::new(budget, cancel)));

    let mut ignore_overrides = Vec::new();
    if !scope.logical.is_empty() {
        ignore_overrides.push(scope.logical.clone());
    }
    for pattern in &request.include {
        if let Some(prefix) = fixed_directory_prefix(pattern) {
            ignore_overrides.push(prefix);
        }
    }

    let walked = walker::walk(
        guard,
        WalkRequest {
            base_logical: scope.logical,
            base_path: scope.path,
            max_depth: None,
            links: LinkPolicy::Skip,
            max_entries: MAX_DISCOVERED_ENTRIES,
            filters: filters.clone(),
            ignore_overrides,
        },
        Arc::clone(&control),
    );

    let mut state = SearchState::default();
    state.discovered_entries = walked.entries.len();
    if walked.truncated {
        state.stop(GrepTruncationReason::MaxDiscoveredEntries);
    }
    for diagnostic in walked.diagnostics {
        state.diagnostics.push(
            diagnostic.logical,
            grep_diagnostic_code(diagnostic.code),
            None,
        );
    }
    state.diagnostics.truncated |= walked.diagnostics_truncated;

    let mut control_error = walked.control_error;
    if control_error.is_none() {
        for entry in walked.entries {
            if entry.kind != WalkEntryKind::File {
                continue;
            }
            if !filters.allows(&entry.logical) {
                continue;
            }
            match scan_file(
                guard,
                &entry,
                request,
                &matcher,
                &mut state,
                Arc::clone(&control),
            ) {
                FileOutcome::Continue => {}
                FileOutcome::Stop(reason) => {
                    state.stop(reason);
                    break;
                }
                FileOutcome::Control(error) => {
                    control_error = Some(error);
                    break;
                }
            }
        }
    }

    match control_error {
        Some(ControlError::Cancelled) => Err(GrepError::Cancelled),
        Some(ControlError::Timeout) => {
            state.stop(GrepTruncationReason::Timeout);
            let output = state.finish();
            if output.matches.is_empty() {
                Err(GrepError::SearchTimeout { partial: None })
            } else {
                Err(GrepError::SearchTimeout {
                    partial: Some(Box::new(output)),
                })
            }
        }
        None => Ok(state.finish()),
    }
}

struct GrepMatcher {
    inner: grep_regex::RegexMatcher,
}

impl GrepMatcher {
    fn compile(request: &GrepRequest) -> Result<Self, GrepError> {
        let mut builder = RegexMatcherBuilder::new();
        builder
            .fixed_strings(request.mode == GrepMode::Literal)
            .multi_line(true)
            .crlf(true)
            .line_terminator(Some(b'\n'));
        builder
            .build(&request.query)
            .map(|inner| Self { inner })
            .map_err(|error| GrepError::InvalidRegex {
                message: error.to_string(),
            })
    }

    fn find(&self, line: &[u8]) -> Option<std::ops::Range<usize>> {
        self.inner
            .find(line)
            .ok()
            .flatten()
            .map(|found| found.start()..found.end())
    }
}

fn grep_diagnostic_code(code: WalkDiagnosticCode) -> GrepDiagnosticCode {
    match code {
        WalkDiagnosticCode::WorkspaceRejected => GrepDiagnosticCode::Unreadable,
        WalkDiagnosticCode::BrokenLink => GrepDiagnosticCode::TargetChanged,
        WalkDiagnosticCode::MetadataFailed => GrepDiagnosticCode::Unreadable,
        WalkDiagnosticCode::ReadDirectoryFailed => GrepDiagnosticCode::Unreadable,
        WalkDiagnosticCode::ReparseSkipped => GrepDiagnosticCode::ReparseSkipped,
    }
}

fn scan_file(
    guard: &WorkspaceGuard,
    entry: &WalkEntry,
    request: &GrepRequest,
    matcher: &GrepMatcher,
    state: &mut SearchState,
    control: Arc<Mutex<SearchControl>>,
) -> FileOutcome {
    if let Err(error) = control_lock(&control).check() {
        return FileOutcome::Control(error);
    }
    run_scan_hook(&entry.path);
    if let Err(error) = control_lock(&control).check() {
        return FileOutcome::Control(error);
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
            return FileOutcome::Continue;
        }
    };
    if is_reparse_or_symlink(&metadata) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::ReparseSkipped,
            None,
        );
        return FileOutcome::Continue;
    }
    if metadata.is_dir() {
        return FileOutcome::Continue;
    }
    if !metadata.is_file() {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::UnsupportedFile,
            None,
        );
        return FileOutcome::Continue;
    }
    if metadata.len() > MAX_SINGLE_FILE_BYTES {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::FileTooLarge,
            None,
        );
        return FileOutcome::Continue;
    }
    if state.scanned_files >= MAX_SCANNED_FILES as u64 {
        return FileOutcome::Stop(GrepTruncationReason::MaxScannedFiles);
    }
    if state.scanned_bytes.saturating_add(metadata.len()) > MAX_SCANNED_BYTES {
        return FileOutcome::Stop(GrepTruncationReason::MaxScannedBytes);
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
            return FileOutcome::Continue;
        }
    };

    let final_path = match opened_final_path(&file) {
        Ok(path) => path,
        Err(error) => {
            state.skipped_files += 1;
            state.diagnostics.push(
                entry.logical.clone(),
                GrepDiagnosticCode::Unreadable,
                Some(error.kind()),
            );
            return FileOutcome::Continue;
        }
    };
    if !guard.contains_canonical_path(&final_path) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::TargetChanged,
            None,
        );
        return FileOutcome::Continue;
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
            return FileOutcome::Continue;
        }
    };
    if !opened_metadata.is_file() || is_reparse_or_symlink(&opened_metadata) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::TargetChanged,
            None,
        );
        return FileOutcome::Continue;
    }
    if opened_metadata.len() > MAX_SINGLE_FILE_BYTES {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::FileTooLarge,
            None,
        );
        return FileOutcome::Continue;
    }
    if state.scanned_bytes.saturating_add(opened_metadata.len()) > MAX_SCANNED_BYTES {
        return FileOutcome::Stop(GrepTruncationReason::MaxScannedBytes);
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
        return FileOutcome::Continue;
    }
    state.scanned_bytes = state.scanned_bytes.saturating_add(bytes.len() as u64);
    if bytes.len() as u64 > MAX_SINGLE_FILE_BYTES {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::FileTooLarge,
            None,
        );
        return FileOutcome::Continue;
    }
    if bytes.contains(&0) {
        state.skipped_files += 1;
        state.diagnostics.push(
            entry.logical.clone(),
            GrepDiagnosticCode::BinarySkipped,
            None,
        );
        return FileOutcome::Continue;
    }
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(_) => {
            state.skipped_files += 1;
            state.diagnostics.push(
                entry.logical.clone(),
                GrepDiagnosticCode::InvalidUtf8Skipped,
                None,
            );
            return FileOutcome::Continue;
        }
    };
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    scan_text(
        &entry.logical,
        text.as_bytes(),
        request,
        matcher,
        state,
        control,
    )
}

fn scan_text(
    logical_path: &str,
    text: &[u8],
    request: &GrepRequest,
    matcher: &GrepMatcher,
    state: &mut SearchState,
    control: Arc<Mutex<SearchControl>>,
) -> FileOutcome {
    let mut sink = MatchSink {
        lines: Vec::new(),
        limit: request.max_results as usize + 1,
    };
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();
    let reader = ControlReader {
        inner: io::Cursor::new(text),
        control: Arc::clone(&control),
    };
    let search_result = searcher.search_reader(&matcher.inner, reader, &mut sink);
    if let Some(error) = control_lock(&control).last_error() {
        return FileOutcome::Control(error);
    }
    if let Err(error) = search_result {
        state.skipped_files += 1;
        state.diagnostics.push(
            logical_path.to_owned(),
            GrepDiagnosticCode::Unreadable,
            Some(error.kind()),
        );
        return FileOutcome::Continue;
    }

    let over_limit = sink.lines.len() > request.max_results as usize;
    for raw_line in sink.lines {
        if let Err(error) = control_lock(&control).check() {
            return FileOutcome::Control(error);
        }
        let line = match std::str::from_utf8(&raw_line.bytes) {
            Ok(line) => line,
            Err(_) => continue,
        };
        let Some(range) = matcher.find(line.as_bytes()) else {
            continue;
        };
        if state.matches.len() >= request.max_results as usize {
            return FileOutcome::Stop(GrepTruncationReason::MaxResults);
        }
        let (preview, preview_start) = bounded_preview(line, range.start, range.end);
        let cost = logical_path.len().saturating_add(preview.len()) as u64;
        if state.output_bytes.saturating_add(cost) > request.max_output_bytes as u64 {
            return FileOutcome::Stop(GrepTruncationReason::MaxOutputBytes);
        }
        let preview_truncated =
            preview_start != 0 || preview_start.saturating_add(preview.len()) < line.len();
        state.output_bytes = state.output_bytes.saturating_add(cost);
        state.matches.push(GrepMatch {
            path: logical_path.to_owned(),
            line_number: raw_line.number,
            preview,
            preview_start_byte: preview_start as u64,
            match_start_byte: range.start as u64,
            match_end_byte: range.end as u64,
            preview_truncated,
        });
    }
    if over_limit {
        return FileOutcome::Stop(GrepTruncationReason::MaxResults);
    }
    FileOutcome::Continue
}

struct RawLine {
    number: u64,
    bytes: Vec<u8>,
}

struct MatchSink {
    lines: Vec<RawLine>,
    limit: usize,
}

impl Sink for MatchSink {
    type Error = io::Error;

    fn matched(
        &mut self,
        _searcher: &grep_searcher::Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, io::Error> {
        let mut bytes = mat.bytes().to_vec();
        while bytes
            .last()
            .is_some_and(|byte| matches!(byte, b'\n' | b'\r'))
        {
            bytes.pop();
        }
        self.lines.push(RawLine {
            number: mat.line_number().unwrap_or(0),
            bytes,
        });
        Ok(self.lines.len() < self.limit)
    }
}

struct ControlReader<R> {
    inner: R,
    control: Arc<Mutex<SearchControl>>,
}

impl<R: Read> Read for ControlReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        {
            let mut control = control_lock(&self.control);
            if let Err(error) = control.check() {
                control.record_error(error);
                drop(control);
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "search control stopped scanning",
                ));
            }
        }
        self.inner.read(buffer)
    }
}

fn control_lock(control: &Mutex<SearchControl>) -> MutexGuard<'_, SearchControl> {
    control.lock().unwrap_or_else(|error| error.into_inner())
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
    Control(ControlError),
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

#[cfg(not(test))]
fn active_search_budget() -> SearchBudget {
    SearchBudget::default()
}

#[cfg(test)]
thread_local! {
    static SEARCH_TIMEOUT_OVERRIDE: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn active_search_budget() -> SearchBudget {
    SearchBudget {
        timeout: SEARCH_TIMEOUT_OVERRIDE
            .with(std::cell::Cell::get)
            .unwrap_or(SEARCH_WALL_TIMEOUT),
    }
}

#[cfg(test)]
fn effective_search_budget(budget: SearchBudget) -> SearchBudget {
    SEARCH_TIMEOUT_OVERRIDE
        .with(std::cell::Cell::get)
        .map(|timeout| SearchBudget { timeout })
        .unwrap_or(budget)
}

#[cfg(not(test))]
fn effective_search_budget(budget: SearchBudget) -> SearchBudget {
    budget
}

#[cfg(test)]
pub(crate) struct TimeoutOverride;

#[cfg(test)]
impl Drop for TimeoutOverride {
    fn drop(&mut self) {
        SEARCH_TIMEOUT_OVERRIDE.with(|value| value.set(None));
    }
}

#[cfg(test)]
pub(crate) fn override_search_timeout(timeout: Duration) -> TimeoutOverride {
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
static SCAN_HOOK_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
static SCAN_HOOK: std::sync::Mutex<Option<Box<dyn Fn(&Path) + Send + Sync>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn install_toctou_hook(hook: impl FnOnce(&Path) + 'static) {
    TOCTOU_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

#[cfg(test)]
pub(crate) struct ScanHookGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for ScanHookGuard {
    fn drop(&mut self) {
        let mut hook = SCAN_HOOK.lock().unwrap_or_else(|error| error.into_inner());
        *hook = None;
    }
}

#[cfg(test)]
pub(crate) fn install_scan_hook(hook: impl Fn(&Path) + Send + Sync + 'static) -> ScanHookGuard {
    let lock = SCAN_HOOK_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut slot = SCAN_HOOK.lock().unwrap_or_else(|error| error.into_inner());
    *slot = Some(Box::new(hook));
    ScanHookGuard { _lock: lock }
}

#[cfg(test)]
fn run_toctou_hook(path: &Path) {
    TOCTOU_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook(path);
        }
    });
}

#[cfg(test)]
fn run_scan_hook(path: &Path) {
    let hook = SCAN_HOOK.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(hook) = hook.as_ref() {
        hook(path);
    }
}

#[cfg(not(test))]
fn run_toctou_hook(_path: &Path) {}

#[cfg(not(test))]
fn run_scan_hook(_path: &Path) {}

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
    fn gitignore_prunes_build_outputs_and_explicit_include_overrides_it() {
        let workspace = TestWorkspace::new();
        workspace.write(".gitignore", "target/\n");
        workspace.write("target/debug/app.rs", "needle-in-build\n");
        workspace.write("src/main.rs", "needle-in-source\n");
        let default = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(
            default
                .matches
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            ["src/main.rs"]
        );
        assert_eq!(default.scanned_files, 2);

        let explicit = grep(
            &workspace,
            &GrepRequest::new("needle").with_include(vec!["target/**/*.rs".into()]),
        )
        .unwrap();
        assert_eq!(
            explicit
                .matches
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            ["target/debug/app.rs"]
        );

        let excluded = grep(
            &workspace,
            &GrepRequest::new("needle").with_exclude(vec!["target/**".into()]),
        )
        .unwrap();
        assert_eq!(excluded.scanned_files, 2);
        assert!(excluded
            .matches
            .iter()
            .all(|item| !item.path.starts_with("target/")));
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
        assert!(matches!(
            grep(&workspace, &GrepRequest::new("needle")),
            Err(GrepError::SearchTimeout { .. })
        ));
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
    fn discovered_and_scanned_limits_have_exact_and_one_over_controls() {
        let _extended_timeout = override_search_timeout(Duration::from_secs(60));
        let workspace = TestWorkspace::new();
        for index in 0..=MAX_DISCOVERED_ENTRIES {
            File::create(workspace.root.join(format!("{index:05}.txt"))).unwrap();
        }
        let one_over = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert!(one_over.truncated);
        assert!(one_over
            .truncation_reasons
            .contains(&GrepTruncationReason::MaxDiscoveredEntries));
        // Valid bounded candidates are still processed before the truncation
        // marker is reported.
        assert_eq!(one_over.scanned_files, MAX_SCANNED_FILES as u64);

        fs::remove_file(
            workspace
                .root
                .join(format!("{MAX_DISCOVERED_ENTRIES:05}.txt")),
        )
        .unwrap();
        let exact = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert!(!exact
            .truncation_reasons
            .contains(&GrepTruncationReason::MaxDiscoveredEntries));
        assert_eq!(exact.scanned_files, MAX_SCANNED_FILES as u64);
        assert_eq!(
            exact.truncation_reasons,
            [GrepTruncationReason::MaxScannedFiles]
        );

        let exact_files = TestWorkspace::new();
        let files = exact_files.root.join("files");
        fs::create_dir_all(&files).unwrap();
        for index in 0..MAX_SCANNED_FILES {
            fs::write(files.join(format!("{index:05}.txt")), b"needle\n").unwrap();
        }
        let exact_file_result = grep(&exact_files, &GrepRequest::new("absent")).unwrap();
        assert_eq!(exact_file_result.scanned_files, MAX_SCANNED_FILES as u64);
        assert!(!exact_file_result
            .truncation_reasons
            .contains(&GrepTruncationReason::MaxScannedFiles));

        fs::write(files.join("one-more.txt"), b"needle\n").unwrap();
        let file_over = grep(&exact_files, &GrepRequest::new("absent")).unwrap();
        assert_eq!(file_over.scanned_files, MAX_SCANNED_FILES as u64);
        assert!(file_over
            .truncation_reasons
            .contains(&GrepTruncationReason::MaxScannedFiles));

        let bytes_workspace = TestWorkspace::new();
        let large = bytes_workspace.root.join("large");
        fs::create_dir_all(&large).unwrap();
        for index in 0..8 {
            let file = File::create(large.join(format!("{index}.bin"))).unwrap();
            file.set_len(MAX_SINGLE_FILE_BYTES).unwrap();
        }
        let exact_bytes = grep(&bytes_workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(exact_bytes.scanned_bytes, MAX_SCANNED_BYTES);
        assert!(!exact_bytes
            .truncation_reasons
            .contains(&GrepTruncationReason::MaxScannedBytes));

        let extra = File::create(large.join("extra.bin")).unwrap();
        extra.set_len(MAX_SINGLE_FILE_BYTES).unwrap();
        let byte_over = grep(&bytes_workspace, &GrepRequest::new("needle")).unwrap();
        assert!(byte_over.scanned_bytes <= MAX_SCANNED_BYTES);
        assert!(byte_over
            .truncation_reasons
            .contains(&GrepTruncationReason::MaxScannedBytes));
    }

    #[test]
    fn exact_preview_and_output_budgets_do_not_truncate_before_the_boundary() {
        let workspace = TestWorkspace::new();
        let line = format!("needle{}", "x".repeat(MAX_PREVIEW_BYTES - "needle".len()));
        workspace.write("exact.txt", format!("{line}\n"));
        let preview = grep(&workspace, &GrepRequest::new("needle").with_limits(1, 8192)).unwrap();
        assert_eq!(preview.matches.len(), 1);
        assert_eq!(preview.matches[0].preview.len(), MAX_PREVIEW_BYTES);
        assert!(!preview.matches[0].preview_truncated);
        assert!(!preview.truncated);

        let output_workspace = TestWorkspace::new();
        let path = "a.txt";
        let preview_len = 1024 - path.len();
        let line = format!("needle{}", "x".repeat(preview_len - "needle".len()));
        output_workspace.write(path, format!("{line}\n"));
        let exact_output = grep(
            &output_workspace,
            &GrepRequest::new("needle").with_limits(100, 1024),
        )
        .unwrap();
        assert_eq!(exact_output.matches.len(), 1);
        assert!(!exact_output.truncated);
        output_workspace.write(path, format!("{line}\n{line}\n"));
        let over_output = grep(
            &output_workspace,
            &GrepRequest::new("needle").with_limits(100, 1025),
        )
        .unwrap();
        assert_eq!(over_output.matches.len(), 1);
        assert!(over_output.truncated);
        assert_eq!(
            over_output.truncation_reasons,
            [GrepTruncationReason::MaxOutputBytes]
        );
    }

    #[test]
    fn exact_single_file_and_diagnostic_caps_are_not_treated_as_overflow() {
        let workspace = TestWorkspace::new();
        let exact = File::create(workspace.root.join("exact.bin")).unwrap();
        exact.set_len(MAX_SINGLE_FILE_BYTES).unwrap();
        let output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(output.skipped_files, 1);
        assert_eq!(
            output.diagnostics[0].code,
            GrepDiagnosticCode::BinarySkipped
        );
        assert!(!output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == GrepDiagnosticCode::FileTooLarge));

        let diagnostics = TestWorkspace::new();
        for index in 0..MAX_DIAGNOSTICS {
            fs::write(
                diagnostics.root.join(format!("{index:03}.bin")),
                [0_u8, 0xff],
            )
            .unwrap();
        }
        let exact_cap = grep(&diagnostics, &GrepRequest::new("needle")).unwrap();
        assert_eq!(exact_cap.diagnostics.len(), MAX_DIAGNOSTICS);
        assert!(!exact_cap.diagnostics_truncated);
    }

    #[test]
    fn active_cancellation_and_timeout_stop_without_partial_success() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "needle\nneedle\n");
        let cancel = CancellationToken::new();
        let cancel_from_hook = cancel.clone();
        let _hook = install_scan_hook(move |_| cancel_from_hook.cancel());
        assert_eq!(
            execute_grep_cancellable(&workspace.guard(), &GrepRequest::new("needle"), cancel),
            Err(GrepError::Cancelled)
        );
        drop(_hook);

        let _timeout = override_search_timeout(Duration::from_millis(1));
        let _hook = install_scan_hook(|_| std::thread::sleep(Duration::from_millis(10)));
        assert!(matches!(
            execute_grep_cancellable(
                &workspace.guard(),
                &GrepRequest::new("needle"),
                CancellationToken::new(),
            ),
            Err(GrepError::SearchTimeout { .. })
        ));
    }

    #[test]
    fn timeout_after_a_valid_match_returns_marked_partial_results() {
        let workspace = TestWorkspace::new();
        workspace.write("a.txt", "needle\n");
        workspace.write("b.txt", "needle\n");
        let _timeout = override_search_timeout(Duration::from_millis(100));
        let _hook = install_scan_hook(|path| {
            if path.file_name().is_some_and(|name| name == "b.txt") {
                std::thread::sleep(Duration::from_millis(300));
            }
        });
        match execute_grep_cancellable(
            &workspace.guard(),
            &GrepRequest::new("needle"),
            CancellationToken::new(),
        ) {
            Err(GrepError::SearchTimeout {
                partial: Some(output),
            }) => {
                assert_eq!(output.matches.len(), 1);
                assert_eq!(output.matches[0].path, "a.txt");
                assert!(output.truncated);
                assert!(output
                    .truncation_reasons
                    .contains(&GrepTruncationReason::Timeout));
            }
            other => panic!("expected partial timeout, got {other:?}"),
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

    #[cfg(windows)]
    #[test]
    fn reparse_candidates_and_scope_are_skipped_or_rejected_without_outside_bytes() {
        let workspace = TestWorkspace::new();
        workspace.write("inside.txt", "needle-inside\n");
        fs::write(workspace.outside.join("secret.txt"), "needle-outside\n").unwrap();
        let nested = workspace.root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let junction = workspace.root.join("outside-junction");
        let candidate_junction = workspace.root.join("candidate.txt");
        if !create_junction(&junction, &workspace.outside) {
            eprintln!("SKIP junction grep test: mklink /J is unavailable");
            return;
        }
        assert!(create_junction(
            &nested.join("outside-junction"),
            &workspace.outside
        ));
        assert!(create_junction(&candidate_junction, &workspace.outside));
        let output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert_eq!(output.matches.len(), 1);
        assert!(
            output
                .diagnostics
                .iter()
                .filter(|item| item.code == GrepDiagnosticCode::ReparseSkipped)
                .count()
                >= 3
        );
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
        let absolute_scope = workspace.outside.to_string_lossy().into_owned();
        assert!(matches!(
            grep(
                &workspace,
                &GrepRequest::new("needle").with_scope(absolute_scope)
            ),
            Err(GrepError::InvalidScope { .. })
        ));
        let sibling_scope = workspace.root.parent().unwrap().join("workspace-sibling");
        assert!(matches!(
            grep(
                &workspace,
                &GrepRequest::new("needle").with_scope(sibling_scope)
            ),
            Err(GrepError::InvalidScope { .. })
        ));
        let file_link = workspace.root.join("outside-file.txt");
        if create_file_symlink(&workspace.outside.join("secret.txt"), &file_link) {
            let file_link_output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
            assert!(!serde_json::to_string(&file_link_output)
                .unwrap()
                .contains("needle-outside"));
            assert!(file_link_output
                .diagnostics
                .iter()
                .any(|item| item.code == GrepDiagnosticCode::ReparseSkipped));
        } else {
            eprintln!("SKIP file-symlink fixture: symlink creation is unavailable");
        }
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

    #[test]
    fn opened_handle_safe_in_workspace_replacement_is_never_an_outside_read() {
        let workspace = TestWorkspace::new();
        workspace.write("swap/candidate.txt", "needle-inside\n");
        let candidate = workspace.root.join("swap/candidate.txt");
        install_toctou_hook(move |_| {
            fs::remove_file(&candidate).unwrap();
            fs::write(&candidate, "needle-replacement\n").unwrap();
        });
        let output = grep(&workspace, &GrepRequest::new("needle")).unwrap();
        assert!(output
            .matches
            .iter()
            .all(|item| item.preview.contains("needle-replacement")));
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

    #[cfg(windows)]
    fn create_file_symlink(link: &Path, target: &Path) -> bool {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            CreateSymbolicLinkW, SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE,
        };

        let link = link
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let target = target
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        unsafe {
            CreateSymbolicLinkW(
                link.as_ptr(),
                target.as_ptr(),
                SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE,
            )
        }
    }

    /// Manual evidence run:
    /// `CHATGPT_HARNESS_PERF_ROOT=D:\ChatGPT-Harness cargo test -p chatgpt-harness-runtime perf_search_benchmark -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn perf_search_benchmark() {
        use crate::tools::{execute_grep_with_control, GlobRequest};

        let synthetic = std::env::var("CHATGPT_HARNESS_PERF_SYNTHETIC").is_ok();
        let (root, cleanup) = if synthetic {
            let base =
                std::env::temp_dir().join(format!("chatgpt-harness-perf-{}", uuid::Uuid::new_v4()));
            let root = base.join("workspace");
            fs::create_dir_all(root.join("src")).unwrap();
            fs::create_dir_all(root.join("build")).unwrap();
            fs::write(root.join(".gitignore"), "build/\n").unwrap();
            for index in 0..25_000 {
                let directory = root.join("src").join(format!("d{:02}", index % 100));
                fs::create_dir_all(&directory).unwrap();
                fs::write(
                    directory.join(format!("file-{index:05}.rs")),
                    "pub struct Synthetic;\n",
                )
                .unwrap();
            }
            for index in 0..25_000 {
                fs::write(
                    root.join("build").join(format!("artifact-{index:05}.bin")),
                    "ignored\n",
                )
                .unwrap();
            }
            (root, Some(base))
        } else {
            let root = std::env::var("CHATGPT_HARNESS_PERF_ROOT")
                .map(PathBuf::from)
                .or_else(|_| std::env::current_dir())
                .unwrap();
            (root, None)
        };

        let workspace = Workspace::new(&root).unwrap();
        let guard = WorkspaceGuard::new(&workspace);
        let budget = SearchBudget::default();
        let run_glob = |pattern: &str| {
            let mut samples = Vec::new();
            let mut last = None;
            for _ in 0..3 {
                let started = std::time::Instant::now();
                let output = crate::tools::execute_glob_with_control(
                    &guard,
                    &GlobRequest::new(pattern),
                    budget,
                    CancellationToken::new(),
                )
                .unwrap();
                samples.push(started.elapsed().as_millis());
                last = Some(output);
            }
            samples.sort_unstable();
            let output = last.unwrap();
            println!(
                "PERF glob pattern={pattern:?} samples_ms={samples:?} median_ms={} matches={} truncated={} diagnostics={}",
                samples[1],
                output.matches.len(),
                output.truncated,
                output.diagnostics.len()
            );
        };
        let run_grep = |label: &str, request: GrepRequest| {
            let mut samples = Vec::new();
            let mut last = None;
            for _ in 0..3 {
                let started = std::time::Instant::now();
                let output =
                    execute_grep_with_control(&guard, &request, budget, CancellationToken::new())
                        .unwrap();
                samples.push(started.elapsed().as_millis());
                last = Some(output);
            }
            samples.sort_unstable();
            let output = last.unwrap();
            println!(
                "PERF grep case={label} samples_ms={samples:?} median_ms={} scanned_files={} scanned_bytes={} matches={} truncated={:?}",
                samples[1],
                output.scanned_files,
                output.scanned_bytes,
                output.matches.len(),
                output.truncation_reasons
            );
        };

        run_glob("*");
        run_glob("**/*.rs");
        run_glob("**/Cargo.toml");
        run_grep("literal", GrepRequest::new("WorkspaceGuard"));
        run_grep(
            "regex",
            GrepRequest::new(r"fn\s+execute_\w+").with_mode(GrepMode::Regex),
        );
        let (include, exclude, scope) = if synthetic {
            ("src/**/*.rs".into(), "build/**".into(), "src")
        } else {
            (
                "runtime/**/*.rs".into(),
                "runtime/target/**".into(),
                "ui/src",
            )
        };
        run_grep(
            "include_exclude",
            GrepRequest::new("WorkspaceGuard")
                .with_include(vec![include])
                .with_exclude(vec![exclude]),
        );
        run_grep(
            "scope",
            GrepRequest::new("WorkspaceGuard").with_scope(scope),
        );

        if let Some(base) = cleanup {
            let _ = fs::remove_dir_all(base);
        }
    }
}
