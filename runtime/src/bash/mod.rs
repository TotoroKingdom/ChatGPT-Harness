pub(crate) mod analyzer;
pub(crate) mod discovery;
pub(crate) mod output;
pub(crate) mod process_tree;
pub(crate) mod runtime;

pub(crate) use analyzer::{
    analyze, BashClassification, BashCommandAnalysis, BashCommandSegment, BashOperator,
    BashRiskFlag,
};
pub(crate) use discovery::{
    resolve_bash, BashExecutableSource, BashExecutableState, BashUnavailableReason,
};
pub(crate) use output::MAX_CAPTURED_OUTPUT_BYTES;
pub(crate) use process_tree::{ProcessTreeError, ProcessTreeOwner};
pub(crate) use runtime::{
    validate_command, BashLimits, BashOutput, BashRequest, BashRuntime, BashRuntimeError,
    EXECUTION_TIMEOUT, MAX_COMMAND_BYTES, MAX_CONCURRENT_BASH,
};
