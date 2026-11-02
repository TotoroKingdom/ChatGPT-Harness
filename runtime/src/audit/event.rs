use crate::execution::{PipelineEvent, RequestContext, RequestState};
use crate::policy::PolicyEvaluationReason;
use harness_control_protocol::{ApprovalDecision, PolicyDecision, PolicyReason};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) use harness_control_protocol::control_read::AuditEventType;

/// A safe projection of the internal request lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuditLifecycleState {
    Received,
    Validating,
    WorkspaceValidating,
    PolicyEvaluating,
    Denied,
    AwaitingApproval,
    Executing,
    Completed,
    Failed,
}

impl From<RequestState> for AuditLifecycleState {
    fn from(state: RequestState) -> Self {
        match state {
            RequestState::Received => Self::Received,
            RequestState::Validating => Self::Validating,
            RequestState::WorkspaceValidating => Self::WorkspaceValidating,
            RequestState::PolicyEvaluating => Self::PolicyEvaluating,
            RequestState::Denied => Self::Denied,
            RequestState::AwaitingApproval => Self::AwaitingApproval,
            RequestState::Executing => Self::Executing,
            RequestState::Completed => Self::Completed,
            RequestState::Failed => Self::Failed,
        }
    }
}

/// Result of the single WorkspaceGuard check. No path is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkspaceValidation {
    NotAttempted,
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuditOutcome {
    InProgress,
    Success,
    Failed,
    Denied,
    ApprovalRequired,
}

/// Sanitized failure categories for operational evidence. These categories
/// intentionally contain no paths, OS messages, payloads, or temp names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuditFailureCategory {
    ApprovalDenied,
    ApprovalExpired,
    ApprovalCancelled,
    ToolDisabled,
    TargetChanged,
    AtomicFailure,
    IoFailure,
    WorkspaceViolation,
    InvalidArguments,
    ExecutionFailed,
    BashUnavailable,
    BashInvalidCommand,
    BashCommandTooLong,
    BashTimeout,
    BashOutputLimit,
    BashConcurrencyLimit,
    BashSpawnFailed,
    BashCancelled,
    BashProcessFailed,
    GrepTimeout,
    GrepCancelled,
}

/// Structured, serde-serializable audit evidence for one pipeline observation.
///
/// `workspace_id` is a logical identifier only; it is never a filesystem path.
/// Only typed, bounded argument facts and scalar results cross this boundary;
/// raw payloads, physical paths, error messages and result bodies are excluded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuditEvent {
    pub(crate) tool_enabled: Option<bool>,
    pub(crate) capability: Option<harness_control_protocol::control_read::ToolCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sanitized_arguments: Option<harness_control_protocol::SanitizedArgumentsEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result_evidence: Option<harness_control_protocol::ExecutionResultEvidence>,
    pub(crate) event_id: Uuid,
    pub(crate) request_id: Uuid,
    pub(crate) timestamp: String,
    pub(crate) event_type: AuditEventType,
    pub(crate) tool_name: String,
    pub(crate) lifecycle_state: AuditLifecycleState,
    pub(crate) workspace_id: String,
    pub(crate) workspace_validation: WorkspaceValidation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) policy_decision: Option<PolicyDecision>,
    pub(crate) matched_rule_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) policy_reason: Option<PolicyReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) approval_decision: Option<ApprovalDecision>,
    pub(crate) outcome: AuditOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) failure_category: Option<AuditFailureCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bash: Option<harness_control_protocol::control_read::BashActivityEvidence>,
}

impl AuditEvent {
    pub(crate) fn from_pipeline_event(
        context: &RequestContext,
        event: PipelineEvent,
    ) -> Option<Self> {
        let (event_type, outcome, workspace_validation, approval_decision) = match event {
            PipelineEvent::RequestReceived => (
                AuditEventType::RequestReceived,
                AuditOutcome::InProgress,
                WorkspaceValidation::NotAttempted,
                None,
            ),
            PipelineEvent::WorkspaceValidated { passed } => (
                AuditEventType::WorkspaceValidated,
                if passed {
                    AuditOutcome::Success
                } else {
                    AuditOutcome::Failed
                },
                if passed {
                    WorkspaceValidation::Passed
                } else {
                    WorkspaceValidation::Failed
                },
                None,
            ),
            PipelineEvent::PolicyDecision => (
                AuditEventType::PolicyEvaluated,
                AuditOutcome::InProgress,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::ExecutionStarted => (
                AuditEventType::ExecutionStarted,
                AuditOutcome::InProgress,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::ExecutionCompleted => (
                AuditEventType::ExecutionCompleted,
                AuditOutcome::Success,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::ExecutionFailed | PipelineEvent::ExecutionCancelled => (
                AuditEventType::ExecutionFailed,
                AuditOutcome::Failed,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::RequestDenied => (
                AuditEventType::RequestDenied,
                AuditOutcome::Denied,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::ApprovalRequired => (
                AuditEventType::ApprovalRequired,
                AuditOutcome::ApprovalRequired,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::ApprovalResolved { decision } => (
                AuditEventType::ApprovalResolved,
                if decision == ApprovalDecision::Deny {
                    AuditOutcome::Denied
                } else {
                    AuditOutcome::InProgress
                },
                WorkspaceValidation::Passed,
                Some(decision),
            ),
            PipelineEvent::ApprovalExpired => (
                AuditEventType::ApprovalExpired,
                AuditOutcome::Failed,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::ApprovalCancelled => (
                AuditEventType::ApprovalCancelled,
                AuditOutcome::Failed,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::SessionGrantMatched => (
                AuditEventType::SessionGrantMatched,
                AuditOutcome::InProgress,
                WorkspaceValidation::Passed,
                None,
            ),
            PipelineEvent::StateChanged { .. } | PipelineEvent::RequestFinished => return None,
        };

        let workspace_validation = if context.tool_name == "bash"
            && workspace_validation == WorkspaceValidation::Passed
            && !context.policy.has_workspace_scope()
        {
            if context.failure_category == Some(AuditFailureCategory::WorkspaceViolation) {
                WorkspaceValidation::Failed
            } else {
                WorkspaceValidation::NotAttempted
            }
        } else {
            workspace_validation
        };
        let evaluation = context.policy_evaluation.as_ref();
        let terminal = matches!(
            event,
            PipelineEvent::WorkspaceValidated { passed: false }
                | PipelineEvent::ExecutionCompleted
                | PipelineEvent::ExecutionFailed
                | PipelineEvent::ExecutionCancelled
                | PipelineEvent::RequestDenied
                | PipelineEvent::ApprovalExpired
                | PipelineEvent::ApprovalCancelled
        ) || (matches!(
            event,
            PipelineEvent::ApprovalResolved {
                decision: ApprovalDecision::Deny
            }
        ));
        Some(Self {
            tool_enabled: Some(context.tool_enabled),
            capability: context.capability,
            sanitized_arguments: context
                .sanitized_arguments
                .clone()
                .and_then(|v| v.sanitized()),
            approval_id: context
                .approval_request_id
                .as_ref()
                .and_then(|id| Uuid::parse_str(id).ok())
                .map(|id| id.to_string()),
            duration_ms: terminal.then(|| {
                context
                    .started_at
                    .elapsed()
                    .as_millis()
                    .min(u64::MAX as u128) as u64
            }),
            result_evidence: matches!(event, PipelineEvent::ExecutionCompleted)
                .then(|| context.result_evidence.clone())
                .flatten(),
            event_id: Uuid::new_v4(),
            request_id: context.request_id,
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            event_type,
            tool_name: context.tool_name.to_owned(),
            lifecycle_state: context.state().into(),
            // This is deliberately a stable logical label, not the configured
            // workspace root or any canonicalized path.
            workspace_id: "active".into(),
            workspace_validation,
            policy_decision: evaluation.map(|item| item.decision.clone()),
            matched_rule_ids: evaluation
                .map(|item| {
                    item.matched_rule_ids
                        .iter()
                        .take(64)
                        .map(|id| sanitize_rule_id(id))
                        .collect()
                })
                .unwrap_or_default(),
            policy_reason: evaluation.map(|item| policy_reason(item.reason)),
            approval_decision: approval_decision.or(context.approval_decision),
            outcome,
            failure_category: match event {
                PipelineEvent::ApprovalExpired => Some(AuditFailureCategory::ApprovalExpired),
                PipelineEvent::ApprovalCancelled => Some(AuditFailureCategory::ApprovalCancelled),
                PipelineEvent::RequestDenied
                    if context.approval_decision == Some(ApprovalDecision::Deny) =>
                {
                    Some(AuditFailureCategory::ApprovalDenied)
                }
                PipelineEvent::WorkspaceValidated { passed: false }
                | PipelineEvent::ExecutionFailed => context.failure_category,
                PipelineEvent::ExecutionCancelled => Some(if context.tool_name == "bash" {
                    AuditFailureCategory::BashCancelled
                } else if context.tool_name == "grep" {
                    AuditFailureCategory::GrepCancelled
                } else {
                    AuditFailureCategory::ExecutionFailed
                }),
                _ => None,
            },
            bash: bash_evidence(context),
        })
    }
}

fn bash_evidence(
    context: &RequestContext,
) -> Option<harness_control_protocol::control_read::BashActivityEvidence> {
    if context.tool_name != "bash" {
        return None;
    }
    use harness_control_protocol::{BashRiskFlag as Flag, BashWorkingDirectory, SecurityMode};
    let details = context.bash_details.as_ref();
    let flags = [
        Flag::CommandSubstitution,
        Flag::Backticks,
        Flag::Eval,
        Flag::NestedShell,
        Flag::InterpreterInlineCode,
        Flag::Redirection,
        Flag::DynamicExpansion,
        Flag::MalformedQuoting,
        Flag::UnsupportedGrammar,
    ];
    let risk_flags = flags
        .into_iter()
        .filter(|flag| details.is_some_and(|facts| facts.risk_flags.contains(flag)))
        .collect();
    let command_preview = details.map(|facts| {
        // The Pipeline supplies an analyzer-sanitized display projection. Never
        // copy logical argv or the private normalized grant scope into audit.
        let text = &facts.command_preview;
        let mut end = text.len().min(1027);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text[..end].to_owned()
    });
    Some(
        harness_control_protocol::control_read::BashActivityEvidence {
            mode: context.security.mode.clone(),
            mode_eligible: context.security.mode == SecurityMode::DeveloperTest,
            capability: harness_control_protocol::control_read::ToolCapability::DeveloperShell,
            cwd: BashWorkingDirectory::ActiveWorkspace,
            command_preview,
            classification: details.map(|facts| facts.classification.clone()),
            risk_flags,
            approval_request_id: context
                .approval_request_id
                .as_ref()
                .and_then(|id| Uuid::parse_str(id).ok())
                .map(|id| id.to_string()),
            session_grant_hit: context.session_grant_hit,
            exit_code: context.bash_exit_code,
            duration_ms: context.bash_duration_ms,
        },
    )
}

fn policy_reason(reason: PolicyEvaluationReason) -> PolicyReason {
    match reason {
        PolicyEvaluationReason::MatchedRules | PolicyEvaluationReason::MatchedUserRules => {
            PolicyReason::MatchedUserRules
        }
        PolicyEvaluationReason::NoApplicableRules | PolicyEvaluationReason::ToolDefault => {
            PolicyReason::ToolDefault
        }
        PolicyEvaluationReason::SystemGuardrail => PolicyReason::SystemGuardrail,
        PolicyEvaluationReason::InvalidRuleSet => PolicyReason::InvalidPolicySnapshot,
        PolicyEvaluationReason::InvalidContext => PolicyReason::InvalidContext,
        PolicyEvaluationReason::UnsupportedCapability => PolicyReason::UnsupportedCapability,
        PolicyEvaluationReason::ToolDisabled => PolicyReason::ToolDisabled,
    }
}

#[cfg(test)]
mod bash_evidence_tests {
    use super::*;
    use harness_control_protocol::{
        BashApprovalDetails, BashApprovalSegment, BashClassification, BashRiskFlag,
        BashWorkingDirectory, SecurityMode,
    };

    fn facts() -> BashApprovalDetails {
        BashApprovalDetails {
            command_preview: "git '<redacted>'".into(),
            classification: BashClassification::UnclassifiedHighRisk,
            normalized_segments: vec![BashApprovalSegment {
                executable: "PRIVATE_EXECUTABLE_PATH".into(),
                argv: vec!["PRIVATE_ARGV_SECRET".into()],
                argv_redacted: true,
            }],
            risk_flags: vec![
                BashRiskFlag::DynamicExpansion,
                BashRiskFlag::DynamicExpansion,
            ],
            cwd: BashWorkingDirectory::ActiveWorkspace,
            preview_redacted: true,
            preview_truncated: false,
        }
    }

    #[test]
    fn bash_audit_retains_correlation_and_scalars_without_copying_argv_or_output() {
        let mut context = RequestContext::for_evidence_test("bash", SecurityMode::DeveloperTest);
        context.bash_details = Some(facts());
        context.bash_exit_code = Some(7);
        context.bash_duration_ms = Some(123);
        context.approval_request_id = Some(context.request_id.to_string());
        context.session_grant_hit = true;
        let event =
            AuditEvent::from_pipeline_event(&context, PipelineEvent::ExecutionCompleted).unwrap();
        let serialized = serde_json::to_value(&event).unwrap();
        let bash = &serialized["bash"];
        assert_eq!(event.request_id, context.request_id);
        assert_eq!(bash["approvalRequestId"], context.request_id.to_string());
        assert_eq!(bash["exitCode"], 7);
        assert_eq!(bash["durationMs"], 123);
        assert_eq!(bash["sessionGrantHit"], true);
        assert_eq!(bash["riskFlags"], serde_json::json!(["dynamic_expansion"]));
        assert_eq!(bash["cwd"], "active_workspace");
        let text = serialized.to_string();
        for forbidden in [
            "PRIVATE_",
            "normalizedSegments",
            "argv",
            "stdout",
            "stderr",
            "fingerprint",
            "grantScope",
        ] {
            assert!(
                !text.contains(forbidden),
                "unexpected private evidence field: {forbidden}"
            );
        }
    }

    #[test]
    fn mode_rejection_does_not_invent_workspace_or_analyzer_success() {
        let mut context = RequestContext::for_evidence_test("bash", SecurityMode::Normal);
        context.failure_category = Some(AuditFailureCategory::BashUnavailable);
        let event =
            AuditEvent::from_pipeline_event(&context, PipelineEvent::ExecutionFailed).unwrap();
        assert_eq!(
            event.workspace_validation,
            WorkspaceValidation::NotAttempted
        );
        let bash = event.bash.unwrap();
        assert!(!bash.mode_eligible);
        assert_eq!(bash.command_preview, None);
        assert_eq!(bash.classification, None);
        assert!(bash.risk_flags.is_empty());
    }

    #[test]
    fn file_tool_wire_shape_does_not_gain_bash_evidence() {
        let mut context = RequestContext::for_evidence_test("write", SecurityMode::Normal);
        context.bash_details = Some(facts());
        let event =
            AuditEvent::from_pipeline_event(&context, PipelineEvent::RequestReceived).unwrap();
        assert!(event.bash.is_none());
        assert!(serde_json::to_value(event).unwrap().get("bash").is_none());
    }

    #[test]
    fn invalid_approval_identity_is_dropped_and_preview_is_utf8_bounded() {
        let mut context = RequestContext::for_evidence_test("bash", SecurityMode::DeveloperTest);
        let mut details = facts();
        details.command_preview = "界".repeat(2000);
        context.bash_details = Some(details);
        context.approval_request_id = Some("PRIVATE_APPROVAL_SECRET".into());
        let event =
            AuditEvent::from_pipeline_event(&context, PipelineEvent::PolicyDecision).unwrap();
        let bash = event.bash.unwrap();
        assert!(bash.command_preview.unwrap().len() <= 1027);
        assert!(bash.approval_request_id.is_none());
    }
}

pub(crate) fn sanitize_rule_id(id: &str) -> String {
    if !id.is_empty()
        && id.len() <= 64
        && id.bytes().any(|b| b.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        id.to_owned()
    } else {
        "<redacted>".into()
    }
}
