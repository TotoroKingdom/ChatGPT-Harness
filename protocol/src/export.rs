//! Explicit, deterministic protocol artifact generation.
//!
//! This module is intentionally invoked only by `protocol-export`; builds and
//! tests never mutate the working tree.

use crate::{rpc::*, v1::*};
use schemars::{JsonSchema, schema_for};
use std::{fs, io, path::Path};
use ts_rs::TS;

const HEADER: &str =
    "// GENERATED FILE — DO NOT EDIT.\n// Source: harness-control-protocol Rust models.\n\n";

pub fn generate_all(root: &Path) -> io::Result<()> {
    generate_typescript(root)?;
    generate_json_schema(root)
}

pub fn generate_typescript(root: &Path) -> io::Result<()> {
    let mut content = String::from(HEADER);
    macro_rules! decls { ($($type:ty),+ $(,)?) => { $(append_decl::<$type>(&mut content);)+ }; }
    decls!(
        RpcRequest,
        RpcResponse,
        RpcNotification,
        RpcErrorData,
        RpcError,
        RpcErrorResponse,
        RpcMessage,
        MachineErrorCode,
        EmptyParams,
        PolicyDecision,
        AppConfig,
        WorkspaceConfig,
        TunnelConfig,
        SecretReference,
        SecurityMode,
        SecurityConfig,
        UiConfig,
        StartEligibilityStatus,
        StartEligibility,
        RuntimeConfigSnapshot,
        RuntimeLimits,
        ClientInfo,
        ServerInfo,
        InitializeParams,
        InitializeResult,
        InitializedParams,
        RuntimeLifecycle,
        RuntimeReadiness,
        RuntimeErrorSummary,
        RuntimeState,
        RuntimeStopResult,
        McpEndpointResult,
        ToolReadiness,
        RiskLevel,
        ToolDescriptor,
        ToolsListResult,
        PolicySource,
        PolicyRule,
        PoliciesListResult,
        ActivityEventType,
        ActivityEvent,
        ActivityListParams,
        ActivityListResult,
        ActivitySubscribeParams,
        ActivitySubscribeResult,
        ApprovalStatus,
        ApprovalDecision,
        ApprovalRequest,
        ApprovalsListParams,
        ApprovalsListResult,
        ApprovalsResolveParams,
        ApprovalsResolveResult,
        DiagnosticStatus,
        DiagnosticCheck,
        DiagnosticRun,
    );
    write(root.join("generated/typescript/v1.ts"), content)
}

pub fn generate_json_schema(root: &Path) -> io::Result<()> {
    macro_rules! schemas { ($($file:literal => $type:ty),+ $(,)?) => { $(write_schema::<$type>(root, $file)?;)+ }; }
    schemas!(
        "rpc-message.schema.json" => RpcMessage,
        "rpc-request.schema.json" => RpcRequest,
        "rpc-response.schema.json" => RpcResponse,
        "rpc-notification.schema.json" => RpcNotification,
        "rpc-error-response.schema.json" => RpcErrorResponse,
        "initialize-params.schema.json" => InitializeParams,
        "initialize-result.schema.json" => InitializeResult,
        "initialized-params.schema.json" => InitializedParams,
        "app-config.schema.json" => AppConfig,
        "runtime-config-snapshot.schema.json" => RuntimeConfigSnapshot,
        "runtime-state.schema.json" => RuntimeState,
        "mcp-endpoint-result.schema.json" => McpEndpointResult,
        "tools-list-result.schema.json" => ToolsListResult,
        "policies-list-result.schema.json" => PoliciesListResult,
        "activity-list-result.schema.json" => ActivityListResult,
        "activity-event.schema.json" => ActivityEvent,
        "approvals-list-result.schema.json" => ApprovalsListResult,
        "approvals-resolve-params.schema.json" => ApprovalsResolveParams,
        "diagnostic-run.schema.json" => DiagnosticRun,
    );
    Ok(())
}

fn append_decl<T: TS>(content: &mut String) {
    let declaration = T::decl();
    content.push_str(&declaration.replacen("type ", "export type ", 1));
    content.push('\n');
}

fn write_schema<T: JsonSchema>(root: &Path, file: &str) -> io::Result<()> {
    let mut schema = serde_json::to_value(schema_for!(T))
        .expect("schemas derived from protocol models must serialize");
    schema
        .as_object_mut()
        .expect("root schemas are JSON objects")
        .insert(
            "$comment".to_owned(),
            serde_json::Value::String("GENERATED FILE — DO NOT EDIT".to_owned()),
        );
    let json = serde_json::to_string_pretty(&schema)
        .expect("schemas derived from protocol models must serialize");
    write(
        root.join("generated/json-schema").join(file),
        format!("{json}\n"),
    )
}

fn write(path: impl AsRef<Path>, content: String) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}
