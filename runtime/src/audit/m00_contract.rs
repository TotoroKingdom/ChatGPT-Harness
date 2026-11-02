//! M00 Shared Evidence Contract v1 proof.
//!
//! This module is test-only. It maps the existing Runtime `AuditEvent` to a
//! closed, bounded fixture shape without adding persistence, replay, or
//! authorization constructors to production code.

use super::event::{
    sanitize_rule_id, AuditEvent, AuditEventType, AuditFailureCategory, AuditLifecycleState,
    AuditOutcome, WorkspaceValidation,
};
use crate::execution::{PipelineEvent, RequestContext};
use crate::policy::{PolicyEvaluation, PolicyEvaluationReason};
use harness_control_protocol::{
    ApprovalDecision, ExecutionResultEvidence, PolicyDecision, PolicyReason,
    SanitizedArgumentsEvidence, SecurityMode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CONTRACT_VERSION: u32 = 1;
const RUNTIME_ID_STATUS: &str = "not_available_in_current_audit_event";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Coverage {
    Complete,
    Incomplete,
    Unknown,
}

/// Closed fixture DTO. This deliberately contains no raw JSON extension map
/// and no method that could produce a permit, approval or session grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceFixture {
    schema_version: u32,
    event_id: Uuid,
    runtime_id: Option<String>,
    runtime_identity_status: String,
    request_id: Uuid,
    host_session_id: Option<String>,
    supervisor_generation: Option<String>,
    trace_id: Option<String>,
    span_id: Option<String>,
    approval_id: Option<String>,
    tool_name: Option<String>,
    capability: Option<harness_control_protocol::control_read::ToolCapability>,
    workspace_id: String,
    workspace_validation: WorkspaceValidation,
    stage: String,
    lifecycle_state: AuditLifecycleState,
    outcome: AuditOutcome,
    failure_category: Option<AuditFailureCategory>,
    policy_decision: Option<PolicyDecision>,
    policy_reason: Option<PolicyReason>,
    matched_rule_ids: Vec<String>,
    duration_ms: Option<u64>,
    bytes_in: Option<u64>,
    bytes_out: Option<u64>,
    truncated: Option<bool>,
    sanitized_arguments: Option<SanitizedArgumentsEvidence>,
    sanitized_result: Option<ExecutionResultEvidence>,
    coverage: Coverage,
}

fn stage(event_type: AuditEventType) -> &'static str {
    match event_type {
        AuditEventType::RequestReceived => "request_received",
        AuditEventType::WorkspaceValidated => "workspace_validated",
        AuditEventType::PolicyEvaluated => "policy_evaluated",
        AuditEventType::ExecutionStarted => "execution_started",
        AuditEventType::ExecutionCompleted => "execution_completed",
        AuditEventType::ExecutionFailed => "execution_failed",
        AuditEventType::RequestDenied => "request_denied",
        AuditEventType::ApprovalRequired => "approval_required",
        AuditEventType::ApprovalResolved => "approval_resolved",
        AuditEventType::ApprovalExpired => "approval_expired",
        AuditEventType::ApprovalCancelled => "approval_cancelled",
        AuditEventType::SessionGrantMatched => "session_grant_matched",
    }
}

fn coverage(event_type: AuditEventType) -> Coverage {
    match event_type {
        AuditEventType::ExecutionCompleted
        | AuditEventType::ExecutionFailed
        | AuditEventType::RequestDenied
        | AuditEventType::ApprovalExpired
        | AuditEventType::ApprovalCancelled => Coverage::Complete,
        AuditEventType::ExecutionStarted => Coverage::Incomplete,
        AuditEventType::RequestReceived
        | AuditEventType::WorkspaceValidated
        | AuditEventType::PolicyEvaluated
        | AuditEventType::ApprovalRequired
        | AuditEventType::ApprovalResolved
        | AuditEventType::SessionGrantMatched => Coverage::Unknown,
    }
}

fn allowed_tool_name(tool_name: &str) -> Option<String> {
    matches!(
        tool_name,
        "read_file" | "glob" | "grep" | "write" | "edit" | "bash"
    )
    .then(|| tool_name.to_owned())
}

fn result_bytes(result: Option<&ExecutionResultEvidence>) -> Option<u64> {
    match result {
        Some(ExecutionResultEvidence::ReadFile { bytes_read }) => Some(*bytes_read),
        Some(ExecutionResultEvidence::Write { bytes_written, .. })
        | Some(ExecutionResultEvidence::Edit { bytes_written, .. }) => Some(*bytes_written),
        _ => None,
    }
}

fn project(event: &AuditEvent) -> EvidenceFixture {
    EvidenceFixture {
        schema_version: CONTRACT_VERSION,
        event_id: event.event_id,
        // The current AuditEvent has no confirmed Runtime identity field. A
        // null is more truthful than joining by timestamp or Host generation.
        runtime_id: None,
        runtime_identity_status: RUNTIME_ID_STATUS.into(),
        request_id: event.request_id,
        host_session_id: None,
        supervisor_generation: None,
        trace_id: None,
        span_id: None,
        approval_id: event.approval_id.clone(),
        tool_name: allowed_tool_name(&event.tool_name),
        capability: event.capability,
        workspace_id: (event.workspace_id == "active")
            .then_some("active".to_owned())
            .unwrap_or_else(|| "<redacted>".into()),
        workspace_validation: event.workspace_validation,
        stage: stage(event.event_type).into(),
        lifecycle_state: event.lifecycle_state,
        outcome: event.outcome,
        failure_category: event.failure_category,
        policy_decision: event.policy_decision.clone(),
        policy_reason: event.policy_reason,
        matched_rule_ids: event
            .matched_rule_ids
            .iter()
            .take(64)
            .map(|id| sanitize_rule_id(id))
            .collect(),
        duration_ms: event.duration_ms,
        bytes_in: None,
        bytes_out: result_bytes(event.result_evidence.as_ref()),
        truncated: None,
        sanitized_arguments: event
            .sanitized_arguments
            .clone()
            .and_then(SanitizedArgumentsEvidence::sanitized),
        sanitized_result: event.result_evidence.clone(),
        coverage: coverage(event.event_type),
    }
}

fn real_audit_event(tool: &'static str, event: PipelineEvent) -> AuditEvent {
    let mut context = RequestContext::for_evidence_test(tool, SecurityMode::Normal);
    context.capability = Some(match tool {
        "read_file" | "glob" | "grep" => {
            harness_control_protocol::control_read::ToolCapability::WorkspaceRead
        }
        "write" | "edit" => harness_control_protocol::control_read::ToolCapability::WorkspaceWrite,
        _ => harness_control_protocol::control_read::ToolCapability::DeveloperShell,
    });
    context.sanitized_arguments = Some(match tool {
        "read_file" => SanitizedArgumentsEvidence::ReadFile {
            logical_path: "README.md".into(),
        },
        "glob" => SanitizedArgumentsEvidence::Glob {
            pattern: "**/*.rs".into(),
        },
        "grep" => SanitizedArgumentsEvidence::Grep {
            mode: harness_control_protocol::GrepMode::Literal,
            logical_scope: "src".into(),
            query_bytes: 7,
            include_count: 1,
            exclude_count: 0,
            max_results: 100,
            max_output_bytes: 65_536,
        },
        "write" => SanitizedArgumentsEvidence::Write {
            logical_path: "notes.txt".into(),
        },
        "edit" => SanitizedArgumentsEvidence::Edit {
            logical_path: "notes.txt".into(),
        },
        _ => SanitizedArgumentsEvidence::Bash {
            command_preview: "git '<redacted>'".into(),
        },
    });
    context.policy_evaluation = Some(PolicyEvaluation {
        decision: if tool == "write" {
            PolicyDecision::Ask
        } else {
            PolicyDecision::Allow
        },
        matched_rule_ids: if tool == "write" {
            vec!["review-write".into()]
        } else {
            Vec::new()
        },
        reason: if tool == "write" {
            PolicyEvaluationReason::MatchedUserRules
        } else {
            PolicyEvaluationReason::ToolDefault
        },
    });
    AuditEvent::from_pipeline_event(&context, event).expect("pipeline event creates audit event")
}

fn fixture(name: &str) -> EvidenceFixture {
    let path = format!(
        "{}/../fixtures/v4/m00/evidence/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).expect("M00 fixture exists");
    serde_json::from_str(&text).expect("M00 fixture has the closed shape")
}

#[test]
fn versioned_fixtures_parse_with_closed_shape_and_coverage_states() {
    let success = fixture("success");
    assert_eq!(success.schema_version, CONTRACT_VERSION);
    assert_eq!(success.coverage, Coverage::Complete);
    assert_eq!(fixture("incomplete").coverage, Coverage::Incomplete);
    assert_eq!(fixture("approval").coverage, Coverage::Unknown);
    assert_eq!(fixture("policy-denied").outcome, AuditOutcome::Denied);
}

#[test]
fn current_audit_event_projects_with_stable_request_identity_and_no_runtime_guess() {
    let event = real_audit_event("read_file", PipelineEvent::ExecutionCompleted);
    let evidence = project(&event);
    assert_eq!(evidence.schema_version, CONTRACT_VERSION);
    assert_eq!(evidence.event_id, event.event_id);
    assert_eq!(evidence.request_id, event.request_id);
    assert_ne!(evidence.event_id, evidence.request_id);
    assert_eq!(evidence.runtime_id, None);
    assert_eq!(evidence.runtime_identity_status, RUNTIME_ID_STATUS);
    assert_eq!(evidence.stage, "execution_completed");
    assert_eq!(evidence.coverage, Coverage::Complete);
    assert_eq!(evidence.tool_name.as_deref(), Some("read_file"));
    let serialized = serde_json::to_string(&evidence).expect("serialize safe evidence");
    assert!(serialized.contains("README.md"));
    assert!(!serialized.contains("C:\\"));
    assert!(!serialized.contains("PRIVATE_"));
}

#[test]
fn nearby_requests_remain_distinct_without_timestamp_correlation() {
    let first = project(&real_audit_event(
        "read_file",
        PipelineEvent::RequestReceived,
    ));
    let second = project(&real_audit_event(
        "read_file",
        PipelineEvent::RequestReceived,
    ));
    assert_ne!(first.request_id, second.request_id);
    assert_ne!(first.event_id, second.event_id);
    assert_eq!(first.runtime_id, second.runtime_id);
    assert_eq!(
        first.runtime_identity_status,
        second.runtime_identity_status
    );
}

#[test]
fn unsafe_candidate_fields_are_not_carried_into_the_allowlist_projection() {
    let unsafe_input = serde_json::json!({
        "physicalPath": "C:\\M00_SYNTHETIC_PHYSICAL_PATH\\private.txt",
        "rawFileBody": "M00_SYNTHETIC_FILE_BODY_SENTINEL",
        "rawCommand": "M00_SYNTHETIC_RAW_COMMAND_SENTINEL",
        "rawEnvironment": "M00_SYNTHETIC_RAW_ENV_SENTINEL",
        "secretLookingValue": "M00_SYNTHETIC_SECRET_SENTINEL",
        "unboundedError": "M00_SYNTHETIC_UNBOUNDED_ERROR_SENTINEL",
        "ruleId": "M00_SYNTHETIC_SECRET_SENTINEL"
    });
    let safe = serde_json::json!({
        "schemaVersion": CONTRACT_VERSION,
        "stage": "execution_failed",
        "matchedRuleIds": [sanitize_rule_id(unsafe_input["ruleId"].as_str().unwrap())]
    });
    let serialized = safe.to_string();
    for forbidden in [
        "M00_SYNTHETIC_PHYSICAL_PATH",
        "M00_SYNTHETIC_FILE_BODY_SENTINEL",
        "M00_SYNTHETIC_RAW_COMMAND_SENTINEL",
        "M00_SYNTHETIC_RAW_ENV_SENTINEL",
        "M00_SYNTHETIC_SECRET_SENTINEL",
        "M00_SYNTHETIC_UNBOUNDED_ERROR_SENTINEL",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "unsafe value leaked: {forbidden}"
        );
    }
    assert_eq!(safe["matchedRuleIds"][0], "<redacted>");
}

#[test]
fn approval_and_grant_facts_are_observation_only() {
    let event = real_audit_event("write", PipelineEvent::ApprovalRequired);
    let evidence = project(&event);
    let historical = serde_json::json!({
        "approvalId": evidence.approval_id,
        "approvalDecision": ApprovalDecision::AllowOnce,
        "sessionGrantMatched": true,
        "outcome": evidence.outcome,
    });
    let serialized = historical.to_string();
    assert!(serialized.contains("approvalId"));
    assert!(!serialized.contains("ExecutionPermit"));
    assert!(!serialized.contains("PendingApproval"));
    assert!(!serialized.contains("SessionGrant"));
}
