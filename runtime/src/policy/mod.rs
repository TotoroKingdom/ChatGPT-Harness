//! Deterministic Runtime authorization, enforced by the M6-B execution pipeline.
//!
//! The unified request pipeline calls this after WorkspaceGuard validation.
//! Nothing here resolves paths, executes tools, talks to MCP, or creates approvals.

mod engine;

#[cfg(test)]
pub(crate) use engine::PolicyEvaluationReason;
pub(crate) use engine::{PolicyContext, PolicyEngine, PolicyEvaluation};

#[cfg(test)]
mod tests;
