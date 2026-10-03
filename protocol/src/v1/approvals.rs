use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// Runtime-owned, display-safe Bash approval evidence.  This projection is
/// intentionally separate from the internal normalized command scope used by
/// SessionGrant matching: the latter may contain sensitive literal argv and
/// never crosses the protocol boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BashClassification {
    Classified,
    UnclassifiedHighRisk,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BashRiskFlag {
    CommandSubstitution,
    Backticks,
    Eval,
    NestedShell,
    InterpreterInlineCode,
    Redirection,
    DynamicExpansion,
    MalformedQuoting,
    UnsupportedGrammar,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BashWorkingDirectory {
    ActiveWorkspace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BashApprovalSegment {
    /// Sanitized normalized executable identity.  This is never a raw shell
    /// source fragment or a canonical filesystem path.
    pub executable: String,
    /// Logical argv suitable for display only.  A value of `true` in
    /// `argv_redacted` means this vector cannot be used to create an exact
    /// BashCommand matcher draft.
    pub argv: Vec<String>,
    pub argv_redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BashApprovalDetails {
    /// Bounded, shell-display-safe preview; it is never the raw command.
    pub command_preview: String,
    pub classification: BashClassification,
    pub normalized_segments: Vec<BashApprovalSegment>,
    pub risk_flags: Vec<BashRiskFlag>,
    pub cwd: BashWorkingDirectory,
    pub preview_redacted: bool,
    pub preview_truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BashSessionGrantUnavailableReason {
    UnclassifiedHighRisk,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    AllowOnce,
    AllowForSession,
    Deny,
}
pub type SanitizedArguments = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub request_id: String,
    pub runtime_id: String,
    pub generation_id: String,
    pub tool: String,
    pub sanitized_arguments: SanitizedArguments,
    pub scope_summary: String,
    pub matched_rule_ids: Vec<String>,
    pub policy_reason: super::policies::PolicyReason,
    pub risk_level: super::tools::RiskLevel,
    /// Optional M5 Bash projection.  Omitted for existing file-tool approval
    /// records so older consumers retain their current wire shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bash_details: Option<BashApprovalDetails>,
    /// `None` is the legacy/non-Bash value.  Bash always supplies `Some` so
    /// unclassified commands can explicitly disable session grants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub allow_for_session_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub allow_for_session_unavailable_reason: Option<BashSessionGrantUnavailableReason>,
    pub created_at: String,
    pub expires_at: String,
    pub status: ApprovalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub resolved_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub decision: Option<ApprovalDecision>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalsListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status: Option<ApprovalStatus>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalsListResult {
    pub items: Vec<ApprovalRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalsResolveParams {
    pub request_id: String,
    pub decision: ApprovalDecision,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalsResolveResult {
    pub approval: ApprovalRequest,
}
