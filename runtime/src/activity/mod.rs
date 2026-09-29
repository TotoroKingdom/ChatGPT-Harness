//! Runtime-owned bounded observations of existing audit emission.
//! No execution, policy, persistence, transport, or UI authority lives here.

use crate::audit::{
    AuditEvent, AuditEventType, AuditFailureCategory, AuditLifecycleState, AuditOutcome, AuditSink,
};
use crate::tools::ToolRegistry;
use harness_control_protocol::PolicyDecision;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Fixed per RuntimeCore lifetime; independent of the existing IPC event buffer.
pub(crate) const ACTIVITY_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActivityCategory {
    Request,
    Workspace,
    Policy,
    Execution,
    Approval,
}

/// A safe display projection, not an audit record or a security decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivityEntry {
    pub(crate) event_id: Uuid,
    pub(crate) request_id: Uuid,
    /// Normalized UTC; None when the source timestamp is invalid.
    pub(crate) timestamp: Option<String>,
    pub(crate) category: ActivityCategory,
    pub(crate) event_type: AuditEventType,
    pub(crate) lifecycle_state: AuditLifecycleState,
    /// Only a source-controlled registry identifier, otherwise None.
    pub(crate) tool_name: Option<String>,
    /// None means policy was not evaluated, never an implicit ALLOW.
    pub(crate) decision_summary: Option<PolicyDecision>,
    pub(crate) outcome_summary: AuditOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) failure_category: Option<AuditFailureCategory>,
}

impl ActivityEntry {
    fn from_audit(event: &AuditEvent, registry: &ToolRegistry) -> Self {
        let category = match event.event_type {
            AuditEventType::RequestReceived => ActivityCategory::Request,
            AuditEventType::WorkspaceValidated => ActivityCategory::Workspace,
            AuditEventType::PolicyEvaluated | AuditEventType::RequestDenied => {
                ActivityCategory::Policy
            }
            AuditEventType::ExecutionStarted
            | AuditEventType::ExecutionCompleted
            | AuditEventType::ExecutionFailed => ActivityCategory::Execution,
            AuditEventType::ApprovalRequired => ActivityCategory::Approval,
        };
        // Do not copy arbitrary strings, rule IDs, paths, or future audit fields.
        // Registry lookup is metadata validation, not policy/exposure evaluation.
        let tool_name = registry
            .declared_capability(&event.tool_name)
            .map(|_| event.tool_name.clone());
        let timestamp = (event.timestamp.len() <= 64)
            .then(|| chrono::DateTime::parse_from_rfc3339(&event.timestamp).ok())
            .flatten()
            .map(|time| {
                time.with_timezone(&chrono::Utc)
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            });
        Self {
            event_id: event.event_id,
            request_id: event.request_id,
            timestamp,
            category,
            event_type: event.event_type,
            lifecycle_state: event.lifecycle_state,
            tool_name,
            decision_summary: event.policy_decision.clone(),
            outcome_summary: event.outcome,
            failure_category: event.failure_category,
        }
    }
}

pub(crate) struct ActivityStore {
    entries: Mutex<VecDeque<ActivityEntry>>,
    registry: Arc<ToolRegistry>,
}

impl ActivityStore {
    pub(crate) fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(ACTIVITY_CAPACITY)),
            registry,
        }
    }

    pub(crate) fn snapshot(&self, limit: usize) -> Vec<ActivityEntry> {
        // Poison recovery is only for observational data, never security state.
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        entries
            .iter()
            .skip(entries.len().saturating_sub(limit))
            .cloned()
            .collect()
    }
}

impl AuditSink for ActivityStore {
    fn on_audit_event(&self, event: &AuditEvent) {
        let entry = ActivityEntry::from_audit(event, &self.registry);
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if entries.len() == ACTIVITY_CAPACITY {
            entries.pop_front();
        }
        entries.push_back(entry);
    }
}

#[cfg(test)]
mod tests;
