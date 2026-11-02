use super::common::DecimalU64;
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
    ApprovalDenied,
    ApprovalExpired,
    ApprovalCancelled,
    SessionGrantMatched,
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
    pub runtime_id: String,
    /// Runtime-local monotonically increasing sequence number.
    #[ts(type = "string")]
    pub sequence: DecimalU64,
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
    #[ts(type = "string")]
    pub after_sequence: Option<DecimalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListResult {
    pub items: Vec<ActivityEvent>,
    #[ts(type = "string")]
    pub latest_sequence: DecimalU64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySubscribeParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "string")]
    pub after_sequence: Option<DecimalU64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySubscribeResult {
    #[ts(type = "string")]
    pub latest_sequence: DecimalU64,
}
pub type ActivityEventNotification = ActivityEvent;

/// Closed, payload-free projections produced by typed Runtime adapters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub enum SanitizedArgumentsEvidence {
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
        #[serde(rename = "commandPreview")]
        #[ts(rename = "commandPreview")]
        command_preview: String,
    },
}
impl SanitizedArgumentsEvidence {
    pub fn sanitized(self) -> Option<Self> {
        let (text, path) = match &self {
            Self::ReadFile { logical_path }
            | Self::Write { logical_path }
            | Self::Edit { logical_path } => (logical_path, true),
            Self::Glob { pattern } => (pattern, false),
            Self::Grep { logical_scope, .. } => (logical_scope, true),
            Self::Bash { command_preview } => (command_preview, false),
        };
        if text.is_empty() || text.chars().any(char::is_control) {
            return None;
        }
        if path
            && (text.starts_with('/')
                || text.contains('\\')
                || text.contains(':')
                || text.split('/').any(|p| p == ".."))
        {
            return None;
        }
        let mut value = self;
        let text = match &mut value {
            Self::ReadFile { logical_path }
            | Self::Write { logical_path }
            | Self::Edit { logical_path } => logical_path,
            Self::Glob { pattern } => pattern,
            Self::Grep { logical_scope, .. } => logical_scope,
            Self::Bash { command_preview } => command_preview,
        };
        let limit = if path { 1024 } else { 1027 };
        let mut end = text.len().min(limit);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        Some(value)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum WriteDisposition {
    Created,
    Overwritten,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionResultEvidence {
    ReadFile {
        #[serde(rename = "bytesRead")]
        #[ts(rename = "bytesRead")]
        #[ts(type = "number")]
        bytes_read: u64,
    },
    Glob {
        #[serde(rename = "matchCount")]
        #[ts(rename = "matchCount")]
        #[ts(type = "number")]
        match_count: u64,
    },
    Grep {
        #[serde(rename = "matchCount")]
        #[ts(rename = "matchCount")]
        #[ts(type = "number")]
        match_count: u64,
        #[serde(rename = "scannedFiles")]
        #[ts(rename = "scannedFiles")]
        #[ts(type = "number")]
        scanned_files: u64,
        #[serde(rename = "scannedBytes")]
        #[ts(rename = "scannedBytes")]
        #[ts(type = "number")]
        scanned_bytes: u64,
        #[serde(rename = "skippedFiles")]
        #[ts(rename = "skippedFiles")]
        #[ts(type = "number")]
        skipped_files: u64,
        truncated: bool,
    },
    Write {
        #[serde(rename = "bytesWritten")]
        #[ts(rename = "bytesWritten")]
        #[ts(type = "number")]
        bytes_written: u64,
        disposition: WriteDisposition,
    },
    Edit {
        #[ts(type = "number")]
        replacements: u64,
        #[serde(rename = "bytesWritten")]
        #[ts(rename = "bytesWritten")]
        #[ts(type = "number")]
        bytes_written: u64,
    },
    Bash {
        #[serde(rename = "exitCode")]
        #[ts(rename = "exitCode")]
        exit_code: i32,
    },
}

#[cfg(test)]
mod evidence_tests {
    use super::*;
    #[test]
    fn unsafe_evidence_is_redacted_and_utf8_bounds_are_enforced() {
        for path in ["../secret", "/private/file", "C:/private/file", "a\nSECRET"] {
            assert!(
                SanitizedArgumentsEvidence::Write {
                    logical_path: path.into()
                }
                .sanitized()
                .is_none()
            );
        }
        let value = SanitizedArgumentsEvidence::Glob {
            pattern: "界".repeat(2000),
        }
        .sanitized()
        .unwrap();
        assert!(serde_json::to_string(&value).unwrap().len() < 1100);
        assert!(
            serde_json::from_str::<SanitizedArgumentsEvidence>(
                r#"{"tool":"write","logicalPath":"ok","content":"SECRET"}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<ExecutionResultEvidence>(
                r#"{"tool":"read_file","bytesRead":2,"content":"SECRET"}"#
            )
            .is_err()
        );
    }
}
