//! Runtime-owned bounded observations of existing audit emission.
//! No execution, policy, persistence, transport, or UI authority lives here.

use crate::audit::{
    AuditEvent, AuditEventType, AuditFailureCategory, AuditLifecycleState, AuditOutcome, AuditSink,
};
use crate::tools::ToolRegistry;
use harness_control_protocol::{PolicyDecision, PolicyReason};
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
    pub(crate) tool_enabled: Option<bool>,
    pub(crate) capability: Option<harness_control_protocol::control_read::ToolCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sanitized_arguments: Option<harness_control_protocol::SanitizedArgumentsEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) approval_decision: Option<harness_control_protocol::ApprovalDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result_evidence: Option<harness_control_protocol::ExecutionResultEvidence>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) matched_rule_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) policy_reason: Option<PolicyReason>,
    pub(crate) outcome_summary: AuditOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) failure_category: Option<AuditFailureCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bash: Option<harness_control_protocol::control_read::BashActivityEvidence>,
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
            AuditEventType::ApprovalRequired
            | AuditEventType::ApprovalResolved
            | AuditEventType::ApprovalExpired
            | AuditEventType::ApprovalCancelled
            | AuditEventType::SessionGrantMatched => ActivityCategory::Approval,
        };
        // Do not copy arbitrary strings, paths, or future audit fields.
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
            tool_enabled: event.tool_enabled,
            capability: event.capability,
            sanitized_arguments: event
                .sanitized_arguments
                .clone()
                .and_then(|v| v.sanitized()),
            approval_id: event.approval_id.clone(),
            approval_decision: event.approval_decision,
            duration_ms: event.duration_ms,
            result_evidence: event.result_evidence.clone(),
            event_id: event.event_id,
            request_id: event.request_id,
            timestamp,
            category,
            event_type: event.event_type,
            lifecycle_state: event.lifecycle_state,
            tool_name,
            decision_summary: event.policy_decision.clone(),
            matched_rule_ids: event
                .matched_rule_ids
                .iter()
                .map(|id| redact_rule_id(id))
                .collect(),
            policy_reason: event.policy_reason,
            outcome_summary: event.outcome,
            failure_category: event.failure_category,
            bash: (event.tool_name == "bash" && registry.declared_capability("bash").is_some())
                .then(|| event.bash.clone())
                .flatten(),
        }
    }
}

fn redact_rule_id(id: &str) -> String {
    if id.len() <= 64
        && id.bytes().any(|byte| byte.is_ascii_lowercase())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        id.to_owned()
    } else {
        "<redacted>".into()
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

    /// Rebinds the observation projection to a new generation registry while
    /// preserving the RuntimeCore-lifetime activity history. Tool toggle only
    /// changes enabled state; declared tool identity remains stable, but the
    /// new store must still use the new generation registry for future events.
    pub(crate) fn with_previous_entries(
        previous: &ActivityStore,
        registry: Arc<ToolRegistry>,
    ) -> Self {
        let entries = previous
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Self {
            entries: Mutex::new(entries),
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
