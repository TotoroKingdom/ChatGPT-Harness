use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// JSON-RPC request IDs may be either an integer or a string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RequestId {
    Integer(i64),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcRequest {
    #[ts(type = "number | string")]
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "unknown", optional)]
    pub params: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcResponse {
    #[ts(type = "number | string")]
    pub id: RequestId,
    #[ts(type = "unknown")]
    pub result: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcNotification {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "unknown", optional)]
    pub params: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorData {
    /// Stable machine-readable error code. Do not parse `message`.
    pub code: MachineErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub details: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: RpcErrorData,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorResponse {
    #[ts(type = "number | string")]
    pub id: RequestId,
    pub error: RpcError,
}

/// A single UTF-8 JSONL protocol message. `jsonrpc: "2.0"` is intentionally
/// omitted on the wire while retaining JSON-RPC 2.0 semantics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum RpcMessage {
    Error(RpcErrorResponse),
    Response(RpcResponse),
    Request(RpcRequest),
    Notification(RpcNotification),
}

/// Standard JSON-RPC errors and the frozen Harness-specific error range.
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const HARNESS_MIN: i32 = -32099;
    pub const HARNESS_MAX: i32 = -32000;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MachineErrorCode {
    NotInitialized,
    AlreadyInitialized,
    ProtocolVersionMismatch,
    RuntimeNotReady,
    InvalidArgument,
    ApprovalNotFound,
    ApprovalAlreadyResolved,
    ApprovalSessionGrantUnavailable,
    BashUnavailable,
    BashInvalidCommand,
    BashCommandTooLong,
    BashTimeout,
    BashOutputLimit,
    BashConcurrencyLimit,
    BashSpawnFailed,
    BashCancelled,
    BashProcessFailed,
    InternalError,
}
