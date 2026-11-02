#![allow(dead_code)]

mod atomic_file;
mod edit;
mod effective;
mod glob;
mod grep;
mod read_file;
mod registry;
mod write;

#[cfg(test)]
pub(crate) use atomic_file::AtomicPhase;

pub use edit::{EditError, EditOutput, EditRequest};
#[allow(unused_imports)]
pub use effective::{EffectiveTool, EffectiveToolCatalog, ToolAvailability};
pub use glob::{
    GlobDiagnostic, GlobDiagnosticCode, GlobError, GlobMatch, GlobMatchKind, GlobOutput,
    GlobRequest, MAX_GLOB_DIAGNOSTICS, MAX_GLOB_RESULTS,
};
pub use grep::{
    GrepDiagnostic, GrepDiagnosticCode, GrepError, GrepMatch, GrepOutput, GrepRequest,
    GrepTruncationReason, DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_RESULTS, MAX_DIAGNOSTICS,
    MAX_DISCOVERED_ENTRIES, MAX_FILTER_PATTERNS_PER_KIND, MAX_FILTER_PATTERN_BYTES,
    MAX_MAX_OUTPUT_BYTES, MAX_MAX_RESULTS, MAX_PREVIEW_BYTES, MAX_QUERY_BYTES, MAX_SCANNED_BYTES,
    MAX_SCANNED_FILES, MAX_SCOPE_BYTES, MAX_SINGLE_FILE_BYTES, REGEX_COMPILED_PROGRAM_LIMIT,
    SEARCH_WALL_TIMEOUT,
};
pub use read_file::{ReadFileError, ReadFileOutput, ReadFileRequest};
pub use registry::{ToolCapability, ToolRegistry};
pub use write::{WriteError, WriteOutput, WriteRequest};

pub(crate) use edit::execute_edit;
pub(crate) use glob::{execute_glob, validate_glob_pattern};
pub(crate) use grep::{
    execute_grep, execute_grep_cancellable, validate_grep_request, validate_grep_scope,
};
pub(crate) use read_file::execute_read_file;
pub(crate) use write::execute_write;
