//! Runtime-owned enforcement. Transport adapters only parse and format data.
mod counters;
pub(crate) use counters::RequestCounters;
mod context;
mod executor;
mod pipeline;
mod state;

pub(crate) use context::{BashEvidenceSnapshot, RequestContext, SecurityContext};
pub(crate) use executor::{BashToolRequest, ToolExecutor, WorkspaceTarget};
#[cfg(test)]
pub(crate) use pipeline::ExecutionHook;
pub(crate) use pipeline::{BashCancellationKind, BashCancellationObserver, PipelineEvent};
pub(crate) use pipeline::{ExecutionError, ExecutionPermit, ExecutionPipeline, ExecutionReport};
pub(crate) use state::{RequestState, TransitionError};

#[cfg(test)]
mod tests;
