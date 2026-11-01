use crate::audit::{
    AuditEvent, AuditEventType, AuditFailureCategory, AuditLifecycleState, WorkspaceValidation,
};
use harness_control_protocol::control_read::{ActivityFailureCategory, ToolCapability};
use harness_control_protocol::{
    ApprovalDecision, DurableSanitizedArgumentsEvidence, EvidenceCoverage, EvidenceEventV1,
    EvidenceLifecycleState, EvidenceStage, EvidenceWorkspaceValidation, HistoryOutcome,
    SanitizedArgumentsEvidence, EVIDENCE_SCHEMA_VERSION,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeEvidenceContext {
    pub(crate) runtime_id: String,
    pub(crate) host_session_id: Option<String>,
    pub(crate) supervisor_generation_id: Option<String>,
    pub(crate) runtime_version: String,
    pub(crate) config_revision: String,
}

#[derive(Clone, Debug)]
pub(crate) struct SafeEvidenceProjector {
    context: RuntimeEvidenceContext,
}

impl SafeEvidenceProjector {
    pub(crate) fn new(context: RuntimeEvidenceContext) -> Self {
        Self { context }
    }

    pub(crate) fn project(&self, event: &AuditEvent) -> EvidenceEventV1 {
        let (stage, outcome) = stage_and_outcome(event);
        let coverage = if is_terminal(event) {
            EvidenceCoverage::Complete
        } else {
            EvidenceCoverage::Incomplete
        };
        EvidenceEventV1 {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            event_id: event.event_id.to_string(),
            runtime_id: self.context.runtime_id.clone(),
            request_id: event.request_id.to_string(),
            host_session_id: self.context.host_session_id.clone(),
            supervisor_generation_id: self.context.supervisor_generation_id.clone(),
            approval_id: event.approval_id.clone(),
            tool_name: event.tool_name.clone(),
            occurred_at_utc: event.timestamp.clone(),
            recorded_at_utc: chrono::Utc::now()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            stage,
            lifecycle_state: lifecycle(event.lifecycle_state),
            workspace_validation: workspace_validation(event.workspace_validation),
            policy_decision: event.policy_decision.clone(),
            policy_reason: event.policy_reason,
            matched_rule_ids: event
                .matched_rule_ids
                .iter()
                .take(64)
                .map(|id| crate::audit::event::sanitize_rule_id(id))
                .collect(),
            approval_decision: event.approval_decision,
            outcome,
            failure_category: event.failure_category.map(failure_category),
            duration_ms: event.duration_ms,
            sanitized_arguments: durable_arguments(event),
            result_evidence: (event.event_type == AuditEventType::ExecutionCompleted)
                .then(|| event.result_evidence.clone())
                .flatten(),
            coverage,
        }
    }
}

fn stage_and_outcome(event: &AuditEvent) -> (EvidenceStage, HistoryOutcome) {
    match event.event_type {
        AuditEventType::RequestReceived => {
            (EvidenceStage::RequestReceived, HistoryOutcome::InProgress)
        }
        AuditEventType::WorkspaceValidated => (
            EvidenceStage::WorkspaceValidated,
            if event.workspace_validation == WorkspaceValidation::Failed {
                HistoryOutcome::Failed
            } else {
                HistoryOutcome::Success
            },
        ),
        AuditEventType::PolicyEvaluated => {
            (EvidenceStage::PolicyEvaluated, HistoryOutcome::InProgress)
        }
        AuditEventType::ExecutionStarted => {
            (EvidenceStage::ExecutionStarted, HistoryOutcome::InProgress)
        }
        AuditEventType::ExecutionCompleted => {
            (EvidenceStage::ExecutionCompleted, HistoryOutcome::Success)
        }
        AuditEventType::ExecutionFailed => {
            let outcome = match event.failure_category {
                Some(AuditFailureCategory::BashTimeout) => HistoryOutcome::TimedOut,
                Some(AuditFailureCategory::BashCancelled) => HistoryOutcome::Cancelled,
                Some(AuditFailureCategory::ApprovalExpired) => HistoryOutcome::ApprovalExpired,
                Some(AuditFailureCategory::ApprovalCancelled) => HistoryOutcome::ApprovalCancelled,
                _ => HistoryOutcome::Failed,
            };
            (EvidenceStage::ExecutionFailed, outcome)
        }
        AuditEventType::RequestDenied => (EvidenceStage::RequestDenied, HistoryOutcome::Denied),
        AuditEventType::ApprovalRequired => (
            EvidenceStage::ApprovalRequired,
            HistoryOutcome::ApprovalRequired,
        ),
        AuditEventType::ApprovalResolved => (
            EvidenceStage::ApprovalResolved,
            if event.approval_decision == Some(ApprovalDecision::Deny) {
                HistoryOutcome::Denied
            } else {
                HistoryOutcome::InProgress
            },
        ),
        AuditEventType::ApprovalExpired => (
            EvidenceStage::ApprovalExpired,
            HistoryOutcome::ApprovalExpired,
        ),
        AuditEventType::ApprovalCancelled => (
            EvidenceStage::ApprovalCancelled,
            HistoryOutcome::ApprovalCancelled,
        ),
        AuditEventType::SessionGrantMatched => (
            EvidenceStage::SessionGrantMatched,
            HistoryOutcome::InProgress,
        ),
    }
}

fn is_terminal(event: &AuditEvent) -> bool {
    matches!(
        event.event_type,
        AuditEventType::ExecutionCompleted
            | AuditEventType::ExecutionFailed
            | AuditEventType::RequestDenied
            | AuditEventType::ApprovalExpired
            | AuditEventType::ApprovalCancelled
    ) || (event.event_type == AuditEventType::WorkspaceValidated
        && event.workspace_validation == WorkspaceValidation::Failed)
        || (event.event_type == AuditEventType::ApprovalResolved
            && event.approval_decision == Some(ApprovalDecision::Deny))
}

fn lifecycle(state: AuditLifecycleState) -> EvidenceLifecycleState {
    match state {
        AuditLifecycleState::Received => EvidenceLifecycleState::Received,
        AuditLifecycleState::Validating => EvidenceLifecycleState::Validating,
        AuditLifecycleState::WorkspaceValidating => EvidenceLifecycleState::WorkspaceValidating,
        AuditLifecycleState::PolicyEvaluating => EvidenceLifecycleState::PolicyEvaluating,
        AuditLifecycleState::Denied => EvidenceLifecycleState::Denied,
        AuditLifecycleState::AwaitingApproval => EvidenceLifecycleState::AwaitingApproval,
        AuditLifecycleState::Executing => EvidenceLifecycleState::Executing,
        AuditLifecycleState::Completed => EvidenceLifecycleState::Completed,
        AuditLifecycleState::Failed => EvidenceLifecycleState::Failed,
    }
}

fn workspace_validation(value: WorkspaceValidation) -> EvidenceWorkspaceValidation {
    match value {
        WorkspaceValidation::NotAttempted => EvidenceWorkspaceValidation::NotAttempted,
        WorkspaceValidation::Passed => EvidenceWorkspaceValidation::Passed,
        WorkspaceValidation::Failed => EvidenceWorkspaceValidation::Failed,
    }
}

fn failure_category(value: AuditFailureCategory) -> ActivityFailureCategory {
    match value {
        AuditFailureCategory::ApprovalDenied => ActivityFailureCategory::ApprovalDenied,
        AuditFailureCategory::ApprovalExpired => ActivityFailureCategory::ApprovalExpired,
        AuditFailureCategory::ApprovalCancelled => ActivityFailureCategory::ApprovalCancelled,
        AuditFailureCategory::ToolDisabled => ActivityFailureCategory::ToolDisabled,
        AuditFailureCategory::TargetChanged => ActivityFailureCategory::TargetChanged,
        AuditFailureCategory::AtomicFailure => ActivityFailureCategory::AtomicFailure,
        AuditFailureCategory::IoFailure => ActivityFailureCategory::IoFailure,
        AuditFailureCategory::WorkspaceViolation => ActivityFailureCategory::WorkspaceViolation,
        AuditFailureCategory::InvalidArguments => ActivityFailureCategory::InvalidArguments,
        AuditFailureCategory::ExecutionFailed => ActivityFailureCategory::ExecutionFailed,
        AuditFailureCategory::BashUnavailable => ActivityFailureCategory::BashUnavailable,
        AuditFailureCategory::BashInvalidCommand => ActivityFailureCategory::BashInvalidCommand,
        AuditFailureCategory::BashCommandTooLong => ActivityFailureCategory::BashCommandTooLong,
        AuditFailureCategory::BashTimeout => ActivityFailureCategory::BashTimeout,
        AuditFailureCategory::BashOutputLimit => ActivityFailureCategory::BashOutputLimit,
        AuditFailureCategory::BashConcurrencyLimit => ActivityFailureCategory::BashConcurrencyLimit,
        AuditFailureCategory::BashSpawnFailed => ActivityFailureCategory::BashSpawnFailed,
        AuditFailureCategory::BashCancelled => ActivityFailureCategory::BashCancelled,
        AuditFailureCategory::BashProcessFailed => ActivityFailureCategory::BashProcessFailed,
    }
}

fn durable_arguments(event: &AuditEvent) -> Option<DurableSanitizedArgumentsEvidence> {
    if event.tool_name == "bash" {
        return event
            .bash
            .as_ref()
            .map(|bash| DurableSanitizedArgumentsEvidence::Bash {
                mode: bash.mode.clone(),
                mode_eligible: bash.mode_eligible,
                capability: ToolCapability::DeveloperShell,
                cwd: bash.cwd.clone(),
                classification: bash.classification.clone(),
                risk_flags: bash.risk_flags.iter().cloned().take(16).collect(),
                session_grant_hit: bash.session_grant_hit,
            });
    }
    match event.sanitized_arguments.clone()?.sanitized()? {
        SanitizedArgumentsEvidence::ReadFile { logical_path } => {
            Some(DurableSanitizedArgumentsEvidence::ReadFile { logical_path })
        }
        SanitizedArgumentsEvidence::Glob { pattern } => {
            Some(DurableSanitizedArgumentsEvidence::Glob { pattern })
        }
        SanitizedArgumentsEvidence::Write { logical_path } => {
            Some(DurableSanitizedArgumentsEvidence::Write { logical_path })
        }
        SanitizedArgumentsEvidence::Edit { logical_path } => {
            Some(DurableSanitizedArgumentsEvidence::Edit { logical_path })
        }
        // Bash command preview text is live-only and is never durable.
        SanitizedArgumentsEvidence::Bash { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditOutcome;
    use harness_control_protocol::{
        control_read::BashActivityEvidence, BashClassification, BashRiskFlag, BashWorkingDirectory,
        ExecutionResultEvidence, PolicyDecision, SanitizedArgumentsEvidence, SecurityMode,
    };
    use uuid::Uuid;

    fn context() -> RuntimeEvidenceContext {
        RuntimeEvidenceContext {
            runtime_id: "runtime-a".into(),
            host_session_id: Some("host-session-a".into()),
            supervisor_generation_id: Some("generation-a".into()),
            runtime_version: "0.1.3".into(),
            config_revision: "7".into(),
        }
    }

    fn event(event_type: AuditEventType) -> AuditEvent {
        AuditEvent {
            tool_enabled: Some(true),
            capability: Some(ToolCapability::WorkspaceRead),
            sanitized_arguments: None,
            approval_id: None,
            duration_ms: None,
            result_evidence: None,
            event_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            timestamp: "2026-11-01T00:00:00.000Z".into(),
            event_type,
            tool_name: "read_file".into(),
            lifecycle_state: AuditLifecycleState::Received,
            workspace_id: "active".into(),
            workspace_validation: WorkspaceValidation::NotAttempted,
            policy_decision: None,
            matched_rule_ids: Vec::new(),
            policy_reason: None,
            approval_decision: None,
            outcome: AuditOutcome::InProgress,
            failure_category: None,
            bash: None,
        }
    }

    #[test]
    fn canonical_event_and_runtime_request_identity_are_preserved() {
        let source = event(AuditEventType::RequestReceived);
        let evidence = SafeEvidenceProjector::new(context()).project(&source);
        assert_eq!(evidence.event_id, source.event_id.to_string());
        assert_eq!(evidence.runtime_id, "runtime-a");
        assert_eq!(evidence.request_id, source.request_id.to_string());
        assert_eq!(evidence.host_session_id.as_deref(), Some("host-session-a"));
        assert_eq!(
            evidence.supervisor_generation_id.as_deref(),
            Some("generation-a")
        );
        assert_eq!(evidence.coverage, EvidenceCoverage::Incomplete);
    }

    #[test]
    fn request_key_distinguishes_equal_request_ids_in_two_runtimes() {
        let source = event(AuditEventType::RequestReceived);
        let mut second = context();
        second.runtime_id = "runtime-b".into();
        let first = SafeEvidenceProjector::new(context()).project(&source);
        let second = SafeEvidenceProjector::new(second).project(&source);
        assert_eq!(first.request_id, second.request_id);
        assert_ne!(
            (first.runtime_id, first.request_id),
            (second.runtime_id, second.request_id)
        );
    }

    #[test]
    fn every_audit_stage_projects_to_a_closed_stage_without_inference() {
        let stages = [
            (
                AuditEventType::RequestReceived,
                EvidenceStage::RequestReceived,
            ),
            (
                AuditEventType::WorkspaceValidated,
                EvidenceStage::WorkspaceValidated,
            ),
            (
                AuditEventType::PolicyEvaluated,
                EvidenceStage::PolicyEvaluated,
            ),
            (
                AuditEventType::ExecutionStarted,
                EvidenceStage::ExecutionStarted,
            ),
            (
                AuditEventType::ExecutionCompleted,
                EvidenceStage::ExecutionCompleted,
            ),
            (
                AuditEventType::ExecutionFailed,
                EvidenceStage::ExecutionFailed,
            ),
            (AuditEventType::RequestDenied, EvidenceStage::RequestDenied),
            (
                AuditEventType::ApprovalRequired,
                EvidenceStage::ApprovalRequired,
            ),
            (
                AuditEventType::ApprovalResolved,
                EvidenceStage::ApprovalResolved,
            ),
            (
                AuditEventType::ApprovalExpired,
                EvidenceStage::ApprovalExpired,
            ),
            (
                AuditEventType::ApprovalCancelled,
                EvidenceStage::ApprovalCancelled,
            ),
            (
                AuditEventType::SessionGrantMatched,
                EvidenceStage::SessionGrantMatched,
            ),
        ];
        let projector = SafeEvidenceProjector::new(context());
        for (source, expected) in stages {
            assert_eq!(projector.project(&event(source)).stage, expected);
        }
    }

    #[test]
    fn durable_bash_projection_omits_command_preview_and_private_sentinels() {
        let mut source = event(AuditEventType::ExecutionCompleted);
        source.tool_name = "bash".into();
        source.capability = Some(ToolCapability::DeveloperShell);
        source.sanitized_arguments = Some(SanitizedArgumentsEvidence::Bash {
            command_preview: "API_KEY_SENTINEL --path C:\\PRIVATE\\workspace".into(),
        });
        source.bash = Some(BashActivityEvidence {
            mode: SecurityMode::DeveloperTest,
            mode_eligible: true,
            capability: ToolCapability::DeveloperShell,
            cwd: BashWorkingDirectory::ActiveWorkspace,
            command_preview: Some("API_KEY_SENTINEL".into()),
            classification: Some(BashClassification::Classified),
            risk_flags: vec![BashRiskFlag::Redirection],
            approval_request_id: None,
            session_grant_hit: false,
            exit_code: Some(0),
            duration_ms: Some(12),
        });
        let evidence = SafeEvidenceProjector::new(context()).project(&source);
        let json = serde_json::to_string(&evidence).unwrap();
        for sentinel in [
            "API_KEY_SENTINEL",
            "commandPreview",
            "PRIVATE",
            "stdout",
            "stderr",
        ] {
            assert!(
                !json.contains(sentinel),
                "durable sentinel leaked: {sentinel}"
            );
        }
        assert!(matches!(
            evidence.sanitized_arguments,
            Some(DurableSanitizedArgumentsEvidence::Bash { .. })
        ));
    }

    #[test]
    fn unsafe_paths_and_nonterminal_events_do_not_become_complete() {
        let mut source = event(AuditEventType::PolicyEvaluated);
        source.sanitized_arguments = Some(SanitizedArgumentsEvidence::Write {
            logical_path: "C:\\PRIVATE\\file.txt".into(),
        });
        source.result_evidence = Some(ExecutionResultEvidence::ReadFile { bytes_read: 4 });
        source.failure_category = Some(AuditFailureCategory::IoFailure);
        let evidence = SafeEvidenceProjector::new(context()).project(&source);
        assert!(evidence.sanitized_arguments.is_none());
        assert!(evidence.result_evidence.is_none());
        assert_eq!(evidence.coverage, EvidenceCoverage::Incomplete);
        assert_eq!(evidence.policy_decision, Option::<PolicyDecision>::None);
    }
}
