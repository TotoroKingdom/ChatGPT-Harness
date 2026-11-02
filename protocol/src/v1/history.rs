use super::{
    ApprovalDecision, BashClassification, BashRiskFlag, BashWorkingDirectory, HistoryConfig,
    PolicyDecision, PolicyReason,
};
use crate::control_read::{ActivityFailureCategory, ToolCapability};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const EVIDENCE_SCHEMA_VERSION: u32 = 1;
pub const HISTORY_QUERY_DEFAULT_LIMIT: u32 = 50;
pub const HISTORY_QUERY_MAX_LIMIT: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceCoverage {
    Complete,
    Incomplete,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum HistoryTerminalState {
    Open,
    Succeeded,
    Failed,
    Denied,
    Cancelled,
    TimedOut,
    ApprovalExpired,
    ApprovalCancelled,
    Incomplete,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum HistoryApprovalState {
    None,
    RequiredObserved,
    AllowOnce,
    AllowForSession,
    Denied,
    Expired,
    Cancelled,
    Incomplete,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum HistoryOutcome {
    InProgress,
    Success,
    Failed,
    Denied,
    ApprovalRequired,
    Cancelled,
    TimedOut,
    ApprovalExpired,
    ApprovalCancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStage {
    RequestReceived,
    WorkspaceValidated,
    PolicyEvaluated,
    ApprovalRequired,
    ApprovalResolved,
    ApprovalExpired,
    ApprovalCancelled,
    SessionGrantMatched,
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    RequestDenied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLifecycleState {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceWorkspaceValidation {
    NotAttempted,
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "tool",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DurableSanitizedArgumentsEvidence {
    ReadFile {
        #[serde(rename = "logicalPath")]
        #[ts(rename = "logicalPath")]
        logical_path: String,
    },
    Glob {
        pattern: String,
    },
    Grep {
        mode: crate::GrepMode,
        #[serde(rename = "logicalScope")]
        #[ts(rename = "logicalScope")]
        logical_scope: String,
        #[serde(rename = "queryBytes")]
        #[ts(rename = "queryBytes")]
        #[ts(type = "number")]
        query_bytes: u64,
        #[serde(rename = "includeCount")]
        #[ts(rename = "includeCount")]
        include_count: u32,
        #[serde(rename = "excludeCount")]
        #[ts(rename = "excludeCount")]
        exclude_count: u32,
        #[serde(rename = "maxResults")]
        #[ts(rename = "maxResults")]
        max_results: u32,
        #[serde(rename = "maxOutputBytes")]
        #[ts(rename = "maxOutputBytes")]
        max_output_bytes: u32,
    },
    Write {
        #[serde(rename = "logicalPath")]
        #[ts(rename = "logicalPath")]
        logical_path: String,
    },
    Edit {
        #[serde(rename = "logicalPath")]
        #[ts(rename = "logicalPath")]
        logical_path: String,
    },
    Bash {
        mode: crate::SecurityMode,
        mode_eligible: bool,
        capability: ToolCapability,
        cwd: BashWorkingDirectory,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        classification: Option<BashClassification>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        risk_flags: Vec<BashRiskFlag>,
        session_grant_hit: bool,
    },
}

/// Host-only evidence persistence settings. The database path never appears
/// in History DTOs, Activity, Tool arguments, or durable event rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceStoreLaunchConfig {
    pub enabled: bool,
    pub database_path: String,
    pub retention_days: u32,
    #[serde(rename = "maxStorageMiB")]
    #[ts(rename = "maxStorageMiB")]
    pub max_storage_mib: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeHostContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub host_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub supervisor_generation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub evidence_store: Option<EvidenceStoreLaunchConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceEventV1 {
    pub schema_version: u32,
    pub event_id: String,
    pub runtime_id: String,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub host_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub supervisor_generation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approval_id: Option<String>,
    pub tool_name: String,
    pub occurred_at_utc: String,
    pub recorded_at_utc: String,
    pub stage: EvidenceStage,
    pub lifecycle_state: EvidenceLifecycleState,
    pub workspace_validation: EvidenceWorkspaceValidation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub policy_decision: Option<PolicyDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub policy_reason: Option<PolicyReason>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_rule_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approval_decision: Option<ApprovalDecision>,
    pub outcome: HistoryOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failure_category: Option<ActivityFailureCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sanitized_arguments: Option<DurableSanitizedArgumentsEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub result_evidence: Option<super::ExecutionResultEvidence>,
    pub coverage: EvidenceCoverage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryQueryParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub from_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub to_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub outcome: Option<HistoryOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub policy_decision: Option<PolicyDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approval_state: Option<HistoryApprovalState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryRequestSummary {
    pub runtime_id: String,
    pub request_id: String,
    pub tool_name: String,
    pub first_seen_at_utc: String,
    pub last_seen_at_utc: String,
    pub terminal_state: HistoryTerminalState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub outcome: Option<HistoryOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub policy_decision: Option<PolicyDecision>,
    pub approval_state: HistoryApprovalState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failure_category: Option<ActivityFailureCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub duration_ms: Option<u64>,
    pub evidence_coverage: EvidenceCoverage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryGetParams {
    pub runtime_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryApprovalEvidence {
    pub runtime_id: String,
    pub approval_id: String,
    pub request_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub requested_at_utc: Option<String>,
    pub terminal_at_utc: String,
    pub terminal_state: HistoryApprovalState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub decision: Option<ApprovalDecision>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryStoreNotice {
    pub code: EvidenceErrorCode,
    pub coverage: EvidenceCoverage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryQueryResult {
    pub items: Vec<HistoryRequestSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
    pub store_status: EvidenceStoreStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryGetResult {
    pub summary: HistoryRequestSummary,
    pub events: Vec<EvidenceEventV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub terminal_approval: Option<HistoryApprovalEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notice: Option<HistoryStoreNotice>,
    pub store_status: EvidenceStoreStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryClearResult {
    #[ts(type = "number")]
    pub deleted_request_count: u64,
    #[ts(type = "number")]
    pub deleted_event_count: u64,
    #[ts(type = "number")]
    pub deleted_approval_count: u64,
    #[ts(type = "number")]
    pub footprint_bytes: u64,
    pub completed_at_utc: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStoreState {
    Disabled,
    Healthy,
    Degraded,
    Busy,
    Incompatible,
    Corrupt,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceErrorCode {
    EvidenceBusy,
    EvidenceWriterOwnership,
    EvidenceIo,
    EvidenceFull,
    EvidenceCorrupt,
    EvidenceIncompatibleSchema,
    EvidenceMigrationFailed,
    EvidenceQueueOverflow,
    EvidenceCapacityDegraded,
    EvidenceInvalidCursor,
    EvidenceNotFound,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceStoreStatus {
    pub state: EvidenceStoreState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub schema_version: Option<u32>,
    pub queue_depth: u32,
    pub queue_capacity: u32,
    #[ts(type = "number")]
    pub drop_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_drop_at_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_commit_at_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_error_code: Option<EvidenceErrorCode>,
    pub effective_history_config: HistoryConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub footprint_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub coverage_unknown_since_utc: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_bash_evidence_has_no_command_preview_or_arbitrary_fields() {
        let value = serde_json::json!({
            "tool": "bash",
            "mode": "developer_test",
            "modeEligible": true,
            "capability": "developer_shell",
            "cwd": "active_workspace",
            "classification": "classified",
            "riskFlags": ["redirection"],
            "sessionGrantHit": false
        });
        let evidence: DurableSanitizedArgumentsEvidence =
            serde_json::from_value(value).expect("closed Bash evidence");
        assert!(
            !serde_json::to_string(&evidence)
                .unwrap()
                .contains("commandPreview")
        );
        assert!(
            serde_json::from_value::<DurableSanitizedArgumentsEvidence>(serde_json::json!({
                "tool": "bash",
                "mode": "developer_test",
                "modeEligible": true,
                "capability": "developer_shell",
                "cwd": "active_workspace",
                "sessionGrantHit": false,
                "commandPreview": "SECRET"
            }))
            .is_err()
        );
    }

    #[test]
    fn query_defaults_and_limits_are_frozen() {
        assert_eq!(HistoryQueryParams::default().limit, None);
        assert_eq!(HISTORY_QUERY_DEFAULT_LIMIT, 50);
        assert_eq!(HISTORY_QUERY_MAX_LIMIT, 200);
        assert_eq!(EVIDENCE_SCHEMA_VERSION, 1);
    }
}
