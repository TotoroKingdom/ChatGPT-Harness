#![allow(unused_imports)]
//! Runtime-owned, non-secret audit event primitives.
//!
//! Audit events intentionally contain only bounded request metadata. Raw tool
//! arguments, physical paths, credentials, environment, raw errors and process
//! output are excluded. M5 adds a bounded sanitized command preview and scalar
//! execution evidence; private grant scopes and logical argv remain internal.

pub(crate) mod event;
mod fanout;
mod redaction;
pub(crate) mod trace;

pub(crate) use event::{
    AuditEvent, AuditEventType, AuditFailureCategory, AuditLifecycleState, AuditOutcome,
    WorkspaceValidation,
};
pub(crate) use fanout::AuditFanout;
pub(crate) use redaction::redact_value;
pub(crate) use trace::{
    TraceCoverage, TraceMutation, TraceObserver, TraceProjector, TraceSpan, TraceSpanName,
    TraceSpanStatus, MAX_ACTIVE_TRACE_REQUESTS, MAX_SPANS_PER_REQUEST,
};

/// Internal observation-only receiver of an already-created audit event.
/// It cannot return an authorization or execution result.
pub(crate) trait AuditSink: Send + Sync {
    fn on_audit_event(&self, event: &AuditEvent);
}

// M00 is a contract/feasibility gate. Keep its projection proof in the test
// build so it cannot become a production evidence store or authorization
// surface by accident.
#[cfg(test)]
mod m00_contract;
