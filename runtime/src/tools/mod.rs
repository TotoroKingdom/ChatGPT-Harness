#![allow(dead_code)]

mod effective;
mod glob;
mod read_file;
mod registry;

#[allow(unused_imports)]
pub use effective::{EffectiveTool, EffectiveToolCatalog, ToolAvailability};
pub use glob::{
    GlobDiagnostic, GlobDiagnosticCode, GlobError, GlobMatch, GlobMatchKind, GlobOutput,
    GlobRequest, MAX_GLOB_DIAGNOSTICS, MAX_GLOB_RESULTS,
};
pub use read_file::{ReadFileError, ReadFileOutput, ReadFileRequest};
pub use registry::{ToolCapability, ToolRegistry};

pub(crate) use glob::{execute_glob, validate_glob_pattern};
pub(crate) use read_file::execute_read_file;
