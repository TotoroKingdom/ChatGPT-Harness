#![allow(unused_imports)]
//! Runtime-owned, non-secret audit event primitives.
//!
//! Audit events intentionally contain only bounded request metadata. Raw tool
//! arguments, physical paths, credentials, environment, raw errors and process
//! output are excluded. M5 adds a bounded sanitized command preview and scalar
//! execution evidence; private grant scopes and logical argv remain internal.

mod event;
mod redaction;

pub(crate) use event::{
    AuditEvent, AuditEventType, AuditFailureCategory, AuditLifecycleState, AuditOutcome,
    WorkspaceValidation,
};
pub(crate) use redaction::redact_value;

/// Internal observation-only receiver of an already-created audit event.
/// It cannot return an authorization or execution result.
pub(crate) trait AuditSink: Send + Sync {
    fn on_audit_event(&self, event: &AuditEvent);
}
