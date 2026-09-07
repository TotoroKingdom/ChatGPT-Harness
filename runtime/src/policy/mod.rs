//! Deterministic Runtime authorization, not yet execution enforcement (M6-A).
//!
//! M6-B must call this from the unified request pipeline after WorkspaceGuard.
//! Nothing here resolves paths, executes tools, talks to MCP, or creates approvals.
#![allow(dead_code, unused_imports)] // The production caller arrives in M6-B.

mod engine;

pub(crate) use engine::{PolicyContext, PolicyEngine, PolicyEvaluation, PolicyEvaluationReason};

#[cfg(test)]
mod tests;
