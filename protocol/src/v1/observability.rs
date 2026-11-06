//! Additive M08 observability contracts.
//!
//! These DTOs are intentionally projections of the canonical evidence store.
//! They contain bounded, sanitized metadata only; raw Tool payloads and
//! authorization state are never represented here.

use super::{
    ApprovalDecision, EvidenceCoverage, EvidenceStoreStatus, HistoryApprovalState, HistoryOutcome,
    PolicyDecision, PolicyReason,
};
use crate::control_read::ActivityFailureCategory;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The statistical status of a metric sample.  A low sample count may still
/// expose an average, but never a stable P95 conclusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MetricsSampleStatus {
    NoData,
    Insufficient,
    Sufficient,
}

impl Default for MetricsSampleStatus {
    fn default() -> Self {
        Self::NoData
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetricsQueryParams {
    pub from_utc: String,
    pub to_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetricsCounts {
    #[ts(type = "number")]
    pub received_calls: u64,
    #[ts(type = "number")]
    pub execution_attempts: u64,
    #[ts(type = "number")]
    pub successful_calls: u64,
    #[ts(type = "number")]
    pub failed_calls: u64,
    #[ts(type = "number")]
    pub denied_calls: u64,
    #[ts(type = "number")]
    pub cancelled_calls: u64,
    #[ts(type = "number")]
    pub timed_out_calls: u64,
    #[ts(type = "number")]
    pub pre_execution_failed_calls: u64,
    #[ts(type = "number")]
    pub approval_expired_calls: u64,
    #[ts(type = "number")]
    pub approval_cancelled_calls: u64,
    #[ts(type = "number")]
    pub incomplete_calls: u64,
    #[ts(type = "number")]
    pub unknown_calls: u64,
    #[ts(type = "number")]
    pub legacy_untraced_calls: u64,
    #[ts(type = "number")]
    pub completed_execution_denominator: u64,
    #[ts(type = "number")]
    pub execution_success_numerator: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub execution_success_rate: Option<f64>,
    pub execution_success_sample_status: MetricsSampleStatus,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DurationMetric {
    #[ts(type = "number")]
    pub sample_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub average_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub p95_ms: Option<u64>,
    pub sample_status: MetricsSampleStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetricsCoverage {
    #[ts(type = "number")]
    pub complete_request_count: u64,
    #[ts(type = "number")]
    pub incomplete_request_count: u64,
    #[ts(type = "number")]
    pub unknown_request_count: u64,
    #[ts(type = "number")]
    pub legacy_untraced_request_count: u64,
    #[ts(type = "number")]
    pub execution_duration_eligible_count: u64,
    #[ts(type = "number")]
    pub approval_wait_duration_eligible_count: u64,
    #[ts(type = "number")]
    pub request_wall_duration_eligible_count: u64,
    pub store_status: EvidenceStoreStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_commit_at_utc: Option<String>,
    #[ts(type = "number")]
    pub drop_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub coverage_unknown_since_utc: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetricsQueryResult {
    pub counts: MetricsCounts,
    pub execution: DurationMetric,
    pub approval_wait: DurationMetric,
    pub request_wall: DurationMetric,
    pub coverage: MetricsCoverage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceGetParams {
    pub runtime_id: String,
    pub request_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TraceAvailability {
    Available,
    LegacyUntraced,
    MigrationRequired,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TraceCoverage {
    Complete,
    Incomplete,
    Unknown,
    LegacyUntraced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TraceSpanName {
    RequestWall,
    RequestReceived,
    ToolResolution,
    WorkspaceValidation,
    PolicyEvaluation,
    ApprovalWait,
    Execution,
    Terminal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TraceSpanStatus {
    Open,
    Complete,
    Success,
    Failed,
    Denied,
    Cancelled,
    TimedOut,
    Incomplete,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestTraceSpan {
    pub span_id: String,
    pub trace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parent_span_id: Option<String>,
    pub runtime_id: String,
    pub request_id: String,
    #[ts(type = "number")]
    pub span_sequence: u32,
    pub span_name: TraceSpanName,
    pub started_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ended_at_utc: Option<String>,
    #[ts(type = "number")]
    pub started_offset_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub duration_ms: Option<u64>,
    pub status: TraceSpanStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failure_category: Option<ActivityFailureCategory>,
    pub coverage: EvidenceCoverage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceGetResult {
    pub runtime_id: String,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub trace_id: Option<String>,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub outcome: Option<HistoryOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub policy_decision: Option<PolicyDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub policy_reason: Option<PolicyReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approval_state: Option<HistoryApprovalState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approval_decision: Option<ApprovalDecision>,
    pub executor_started: bool,
    pub coverage: TraceCoverage,
    pub availability: TraceAvailability,
    pub spans: Vec<RequestTraceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failure_stage: Option<TraceSpanName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slowest_measured_span: Option<TraceSpanName>,
    pub store_status: EvidenceStoreStatus,
}
