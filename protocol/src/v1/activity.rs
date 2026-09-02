use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ActivityEventType {
    RuntimeStarted,
    RuntimeStopped,
    ToolRequested,
    ValidationFailed,
    WorkspaceViolation,
    PolicyEvaluated,
    ApprovalRequested,
    ApprovalResolved,
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    ExecutionTimedOut,
    ExecutionCancelled,
}

/// Metadata is pre-sanitized human-safe text. Secret values are forbidden.
pub type SanitizedMetadata = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub event_id: String,
    /// Runtime-local monotonically increasing sequence number.
    pub sequence: u64,
    pub event_type: ActivityEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub request_id: Option<String>,
    /// RFC3339 UTC timestamp.
    pub timestamp: String,
    pub summary: String,
    pub metadata: SanitizedMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListResult {
    pub items: Vec<ActivityEvent>,
    pub latest_sequence: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySubscribeParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after_sequence: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySubscribeResult {
    pub latest_sequence: u64,
}
pub type ActivityEventNotification = ActivityEvent;
