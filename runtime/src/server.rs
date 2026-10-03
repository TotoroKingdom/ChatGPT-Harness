use crate::{
    mcp::{McpExecutionContext, McpRuntimeService, McpRuntimeServiceError},
    state::{Phase, RuntimeCore},
};
use harness_control_protocol::{
    error_codes, ActivityEvent, ActivityListParams, ActivityListResult, ActivitySubscribeParams,
    ActivitySubscribeResult, ApprovalsListResult, ApprovalsResolveParams, DiagnosticCheck,
    DiagnosticRun, DiagnosticStatus, EmptyParams, InitializeParams, InitializeResult,
    MachineErrorCode, McpEndpointResult, PoliciesListResult, RpcError, RpcErrorData,
    RpcErrorResponse, RpcMessage, RpcNotification, RpcRequest, RpcResponse, RuntimeStopResult,
    ToolsListResult, PROTOCOL_VERSION,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const NOT_INITIALIZED_ERROR: i32 = error_codes::HARNESS_MIN;
const ALREADY_INITIALIZED_ERROR: i32 = -32001;
const PROTOCOL_VERSION_MISMATCH_ERROR: i32 = -32002;
const APPROVAL_NOT_FOUND_ERROR: i32 = -32003;
const APPROVAL_ALREADY_RESOLVED_ERROR: i32 = -32004;
const RUNTIME_NOT_READY_ERROR: i32 = error_codes::HARNESS_MAX;

pub fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_io(BufReader::new(stdin.lock()), BufWriter::new(stdout.lock()))
}

fn run_io<R: BufRead, W: Write>(mut input: R, mut output: W) -> io::Result<()> {
    let mut core = RuntimeCore::new();
    let mut mcp_service = McpRuntimeService::new();
    let mut subscribed = false;
    let mut line = String::new();

    let run_result = (|| -> io::Result<()> {
        loop {
            line.clear();
            if input.read_line(&mut line)? == 0 {
                return Ok(());
            }
            if line.trim().is_empty() {
                continue;
            }

            let exit = match serde_json::from_str::<Value>(line.trim_end()) {
                Ok(value) => match serde_json::from_value::<RpcMessage>(value) {
                    Ok(message) => dispatch(
                        message,
                        &mut core,
                        &mut mcp_service,
                        &mut subscribed,
                        &mut output,
                    )?,
                    Err(error) => {
                        write_error(
                            &mut output,
                            empty_request_id(),
                            error_codes::INVALID_REQUEST,
                            MachineErrorCode::InvalidArgument,
                            format!("Invalid request: {error}"),
                        )?;
                        false
                    }
                },
                Err(error) => {
                    write_error(
                        &mut output,
                        empty_request_id(),
                        error_codes::PARSE_ERROR,
                        MachineErrorCode::InternalError,
                        format!("Parse error: {error}"),
                    )?;
                    false
                }
            };
            output.flush()?;
            if exit {
                return Ok(());
            }
        }
    })();
    let shutdown_result = shutdown_runtime_services(&mut core, &mut mcp_service);
    run_result.and(shutdown_result)
}

/// Common control-loop exit cleanup for `runtime.stop`, stdin EOF, and I/O
/// errors. `runtime.stop` performs its own joined shutdown before replying;
/// this call is then an idempotent final guard.
fn shutdown_runtime_services(
    core: &mut RuntimeCore,
    mcp_service: &mut McpRuntimeService,
) -> io::Result<()> {
    if core.config.is_some() {
        let _ = core.stop();
    }
    mcp_service.shutdown().map_err(io::Error::other)
}

fn empty_request_id() -> harness_control_protocol::RequestId {
    harness_control_protocol::RequestId::String(String::new())
}

fn dispatch<W: Write>(
    message: RpcMessage,
    core: &mut RuntimeCore,
    mcp_service: &mut McpRuntimeService,
    subscribed: &mut bool,
    output: &mut W,
) -> io::Result<bool> {
    match message {
        RpcMessage::Request(request) => {
            handle_request(request, core, mcp_service, subscribed, output)
        }
        RpcMessage::Notification(notification) => {
            handle_notification(notification, core, subscribed, output)?;
            Ok(false)
        }
        RpcMessage::Response(_) | RpcMessage::Error(_) => Ok(false),
    }
}

fn handle_notification<W: Write>(
    notification: RpcNotification,
    core: &mut RuntimeCore,
    subscribed: &mut bool,
    output: &mut W,
) -> io::Result<()> {
    if notification.method != "initialized" || core.phase != Phase::InitializedPendingAck {
        return Ok(());
    }
    if let Some(event) = core.acknowledge() {
        if *subscribed {
            write_event(output, &event)?;
        }
    }
    Ok(())
}

fn handle_request<W: Write>(
    request: RpcRequest,
    core: &mut RuntimeCore,
    mcp_service: &mut McpRuntimeService,
    subscribed: &mut bool,
    output: &mut W,
) -> io::Result<bool> {
    let id = request.id.clone();
    let method = request.method.as_str();
    if method != "initialize" && core.config.is_none() {
        write_error(
            output,
            id,
            NOT_INITIALIZED_ERROR,
            MachineErrorCode::NotInitialized,
            "Runtime has not been initialized.",
        )?;
        return Ok(false);
    }
    if method != "initialize" && core.phase != Phase::Ready {
        write_error(
            output,
            id,
            RUNTIME_NOT_READY_ERROR,
            MachineErrorCode::RuntimeNotReady,
            "Runtime is waiting for the initialized acknowledgement.",
        )?;
        return Ok(false);
    }

    if crate::control_read::handles(method) {
        match crate::control_read::read(core, method, request.params.clone()) {
            Ok(result) => write_result(output, id, result)?,
            Err(error) => {
                use harness_control_protocol::control_read::ReadErrorCode;
                let (code, machine) = match error {
                    ReadErrorCode::RuntimeNotReady => {
                        (RUNTIME_NOT_READY_ERROR, MachineErrorCode::RuntimeNotReady)
                    }
                    ReadErrorCode::InvalidRequest => (
                        error_codes::INVALID_PARAMS,
                        MachineErrorCode::InvalidArgument,
                    ),
                    _ => (error_codes::INTERNAL_ERROR, MachineErrorCode::InternalError),
                };
                write_error(output, id, code, machine, error.message())?;
            }
        }
        return Ok(false);
    }

    match method {
        "initialize" => {
            if core.config.is_some() {
                write_error(
                    output,
                    id,
                    ALREADY_INITIALIZED_ERROR,
                    MachineErrorCode::AlreadyInitialized,
                    "Runtime is already initialized.",
                )?;
                return Ok(false);
            }
            let params: InitializeParams = match params(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            if params.protocol_version != PROTOCOL_VERSION {
                write_error(
                    output,
                    id,
                    PROTOCOL_VERSION_MISMATCH_ERROR,
                    MachineErrorCode::ProtocolVersionMismatch,
                    "Unsupported protocol version.",
                )?;
                return Ok(false);
            }
            if let Err(error) = core.initialize(params.runtime_config) {
                write_error(
                    output,
                    id,
                    error_codes::INVALID_PARAMS,
                    MachineErrorCode::InvalidArgument,
                    format!("Invalid workspace: {error}"),
                )?;
                return Ok(false);
            }
            if start_mcp_after_initialize(core, mcp_service, |service, context| {
                service.start(context)
            })
            .is_err()
            {
                write_error(
                    output,
                    id,
                    error_codes::INTERNAL_ERROR,
                    MachineErrorCode::InternalError,
                    "Failed to start Runtime MCP endpoint.",
                )?;
                return Ok(false);
            }
            let result = InitializeResult {
                server_info: harness_control_protocol::ServerInfo {
                    name: "chatgpt-harness-runtime".into(),
                    title: "ChatGPT-Harness Runtime".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                },
                protocol_version: PROTOCOL_VERSION,
                runtime_id: core.runtime_id.clone(),
                capabilities: Default::default(),
            };
            write_result(output, id, result)?;
            Ok(false)
        }
        "runtime.status" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            write_result(output, id, core.status().expect("initialized state exists"))?;
            Ok(false)
        }
        "mcp.endpoint.get" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            let Some(url) = mcp_service.endpoint_url() else {
                write_error(
                    output,
                    id,
                    RUNTIME_NOT_READY_ERROR,
                    MachineErrorCode::RuntimeNotReady,
                    "Runtime MCP endpoint is not active.",
                )?;
                return Ok(false);
            };
            write_result(output, id, McpEndpointResult { url: url.into() })?;
            Ok(false)
        }
        "runtime.stop" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            core.drain_activity_events();
            if let Some(event) = core.stop() {
                if *subscribed {
                    write_event(output, &event)?;
                }
            }
            if mcp_service.shutdown().is_err() {
                write_error(
                    output,
                    id,
                    error_codes::INTERNAL_ERROR,
                    MachineErrorCode::InternalError,
                    "Failed to stop Runtime MCP endpoint.",
                )?;
                return Ok(true);
            }
            write_result(output, id, RuntimeStopResult { accepted: true })?;
            Ok(true)
        }
        "tools.list" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            write_result(
                output,
                id,
                ToolsListResult {
                    items: core.tool_registry().list(),
                },
            )?;
            Ok(false)
        }
        "policies.list" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            write_result(
                output,
                id,
                PoliciesListResult {
                    items: core.policy_rules(),
                    effective: Some(core.effective_policy()),
                },
            )?;
            Ok(false)
        }
        "activity.list" => {
            let params: ActivityListParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            core.drain_activity_events();
            let bus = core.events.as_ref().expect("event bus exists");
            write_result(
                output,
                id,
                ActivityListResult {
                    items: bus.after(params.after_sequence, params.limit),
                    latest_sequence: bus.latest_sequence(),
                },
            )?;
            Ok(false)
        }
        "activity.subscribe" => {
            let params: ActivitySubscribeParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            core.drain_activity_events();
            *subscribed = true;
            let bus = core.events.as_ref().expect("event bus exists");
            let replay = bus.after(params.after_sequence, None);
            let latest_sequence = bus.latest_sequence();
            write_result(output, id, ActivitySubscribeResult { latest_sequence })?;
            for event in replay {
                write_event(output, &event)?;
            }
            Ok(false)
        }
        "approvals.list" => {
            let params: harness_control_protocol::ApprovalsListParams =
                match params_or_default(&request.params) {
                    Ok(value) => value,
                    Err(error) => return write_invalid_params(output, id, error),
                };
            let manager = core.approval_manager();
            write_result(
                output,
                id,
                ApprovalsListResult {
                    items: manager.list(params.status),
                },
            )?;
            Ok(false)
        }
        "approvals.resolve" => {
            let params: ApprovalsResolveParams = match params(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            let manager = core.approval_manager();
            let grants = core.session_grants();
            let decision = params.decision;
            let result = manager.resolve_with(&params.request_id, decision, |approval, scope| {
                if decision == harness_control_protocol::ApprovalDecision::AllowForSession {
                    let Some(scope) = scope else {
                        return;
                    };
                    grants.insert_from_approved_scope(
                        &approval.generation_id,
                        &approval.request_id,
                        &approval.tool,
                        scope,
                        chrono::Utc::now(),
                    );
                }
            });
            match result {
                Ok(approval) => write_result(
                    output,
                    id,
                    harness_control_protocol::ApprovalsResolveResult { approval },
                )?,
                Err(crate::approval::ApprovalManagerError::ApprovalNotFound) => write_error(
                    output,
                    id,
                    APPROVAL_NOT_FOUND_ERROR,
                    MachineErrorCode::ApprovalNotFound,
                    "Approval request was not found.",
                )?,
                Err(crate::approval::ApprovalManagerError::ApprovalAlreadyResolved) => write_error(
                    output,
                    id,
                    APPROVAL_ALREADY_RESOLVED_ERROR,
                    MachineErrorCode::ApprovalAlreadyResolved,
                    "Approval request has already reached a terminal state.",
                )?,
                Err(crate::approval::ApprovalManagerError::DuplicateRequest) => write_error(
                    output,
                    id,
                    error_codes::INTERNAL_ERROR,
                    MachineErrorCode::InternalError,
                    "Approval request could not be resolved.",
                )?,
                Err(crate::approval::ApprovalManagerError::SessionGrantNotEligible) => write_error(
                    output,
                    id,
                    error_codes::INVALID_PARAMS,
                    MachineErrorCode::ApprovalSessionGrantUnavailable,
                    "AllowForSession is unavailable for this Bash request.",
                )?,
            }
            Ok(false)
        }
        "diagnostics.run" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            write_result(
                output,
                id,
                DiagnosticRun {
                    run_id: format!("diagnostic_{}", unix_millis()),
                    started_at: now.clone(),
                    finished_at: now,
                    overall_status: DiagnosticStatus::Pass,
                    checks: vec![DiagnosticCheck {
                        id: "runtime_skeleton".into(),
                        status: DiagnosticStatus::Pass,
                        summary: "Runtime control protocol is available.".into(),
                        remediation: None,
                    }],
                },
            )?;
            Ok(false)
        }
        _ => {
            write_error(
                output,
                id,
                error_codes::METHOD_NOT_FOUND,
                MachineErrorCode::InvalidArgument,
                "Method not found.",
            )?;
            Ok(false)
        }
    }
}

/// Starts the process-owned MCP service only after RuntimeCore initialization.
/// A failed startup restores the core to its pre-initialize business state.
fn start_mcp_after_initialize<F>(
    core: &mut RuntimeCore,
    mcp_service: &mut McpRuntimeService,
    start: F,
) -> Result<(), McpRuntimeServiceError>
where
    F: FnOnce(&mut McpRuntimeService, McpExecutionContext) -> Result<(), McpRuntimeServiceError>,
{
    let context = core
        .mcp_context()
        .expect("initialized RuntimeCore always supplies an MCP context");
    if let Err(error) = start(mcp_service, context) {
        core.rollback_initialize();
        return Err(error);
    }
    Ok(())
}

fn params<T: DeserializeOwned>(value: &Option<Value>) -> Result<T, String> {
    value
        .clone()
        .ok_or_else(|| "Request params are required.".into())
        .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
}

fn params_or_default<T: DeserializeOwned + Default>(value: &Option<Value>) -> Result<T, String> {
    value.clone().map_or_else(
        || Ok(T::default()),
        |value| serde_json::from_value(value).map_err(|error| error.to_string()),
    )
}

fn write_invalid_params<W: Write>(
    output: &mut W,
    id: harness_control_protocol::RequestId,
    error: String,
) -> io::Result<bool> {
    write_error(
        output,
        id,
        error_codes::INVALID_PARAMS,
        MachineErrorCode::InvalidArgument,
        error,
    )?;
    Ok(false)
}

fn write_result<W: Write, T: serde::Serialize>(
    output: &mut W,
    id: harness_control_protocol::RequestId,
    value: T,
) -> io::Result<()> {
    write_message(
        output,
        &RpcMessage::Response(RpcResponse {
            id,
            result: serde_json::to_value(value).map_err(io::Error::other)?,
        }),
    )
}

fn write_error<W: Write>(
    output: &mut W,
    id: harness_control_protocol::RequestId,
    code: i32,
    machine: MachineErrorCode,
    message: impl Into<String>,
) -> io::Result<()> {
    write_message(
        output,
        &RpcMessage::Error(error_response(id, code, machine, message.into())),
    )
}

fn error_response(
    id: harness_control_protocol::RequestId,
    code: i32,
    machine: MachineErrorCode,
    message: String,
) -> RpcErrorResponse {
    RpcErrorResponse {
        id,
        error: RpcError {
            code,
            message,
            data: RpcErrorData {
                code: machine,
                details: None,
            },
        },
    }
}

fn write_event<W: Write>(output: &mut W, event: &ActivityEvent) -> io::Result<()> {
    write_message(
        output,
        &RpcMessage::Notification(RpcNotification {
            method: "activity.event".into(),
            params: Some(serde_json::to_value(event).map_err(io::Error::other)?),
        }),
    )
}

fn write_message<W: Write>(output: &mut W, message: &RpcMessage) -> io::Result<()> {
    serde_json::to_writer(&mut *output, message).map_err(io::Error::other)?;
    output.write_all(b"\n")
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    mod control_read;
    use super::*;
    use harness_control_protocol::{
        DecimalU64, RuntimeConfigSnapshot, RuntimeLifecycle, RuntimeLimits, RuntimeReadiness,
        SecurityConfig, SecurityMode, ToolConfig, WorkspaceConfig,
    };
    use serde_json::json;
    use std::fs;
    use std::io::Cursor;
    use std::net::{SocketAddr, TcpListener};
    use std::path::{Path, PathBuf};

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "chatgpt-harness-runtime-server-{}",
                uuid::Uuid::new_v4()
            ));
            let root = base.join("workspace");
            fs::create_dir_all(&root).expect("create workspace");
            Self { base, root }
        }

        fn missing_path(&self) -> PathBuf {
            self.base.join("missing")
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn initialize_params(protocol_version: u32, root: &Path) -> Value {
        json!({
            "clientInfo": { "name": "test", "title": "Test", "version": "1" },
            "protocolVersion": protocol_version,
            "capabilities": {},
            "runtimeConfig": {
                "configRevision": "7",
                "workspace": { "rootPath": root.to_string_lossy() },
                "security": { "mode": "normal", "developerTestMode": false },
                "runtimeLimits": { "maxConcurrentRequests": 4, "maxActivityEvents": 2, "maxReadBytes": 1048576 },
                "tools": {
                    "readFile": { "enabled": true },
                    "glob": { "enabled": true },
                    "write": { "enabled": true },
                    "edit": { "enabled": true },
                    "bash": { "enabled": false }
                }
            }
        })
    }

    fn runtime_config(root: &Path) -> RuntimeConfigSnapshot {
        RuntimeConfigSnapshot {
            config_revision: DecimalU64::new(7),
            workspace: WorkspaceConfig {
                root_path: root.to_string_lossy().into_owned(),
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: 4,
                max_activity_events: 2,
                max_read_bytes: 1_048_576,
            },
            tools: ToolConfig::default(),
            bash: harness_control_protocol::BashConfig::default(),
            policies: harness_control_protocol::PolicyConfig::default(),
        }
    }

    fn request(id: &str, method: &str, params: Value) -> String {
        serde_json::to_string(&RpcMessage::Request(RpcRequest {
            id: harness_control_protocol::RequestId::String(id.into()),
            method: method.into(),
            params: Some(params),
        }))
        .unwrap()
    }

    fn notification(method: &str) -> String {
        serde_json::to_string(&RpcMessage::Notification(RpcNotification {
            method: method.into(),
            params: Some(json!({})),
        }))
        .unwrap()
    }

    fn run_lines(lines: &[String]) -> Vec<RpcMessage> {
        let input = Cursor::new(format!("{}\n", lines.join("\n")).into_bytes());
        let mut output = Vec::new();
        run_io(input, &mut output).unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn response_error(message: &RpcMessage) -> &RpcErrorResponse {
        match message {
            RpcMessage::Error(error) => error,
            _ => panic!("expected error response"),
        }
    }

    #[test]
    fn rejects_requests_before_initialize() {
        let output = run_lines(&[
            request("status", "runtime.status", json!({})),
            request("stop", "runtime.stop", json!({})),
        ]);
        assert_eq!(
            response_error(&output[0]).error.data.code,
            MachineErrorCode::NotInitialized
        );
        assert_eq!(
            response_error(&output[1]).error.data.code,
            MachineErrorCode::NotInitialized
        );
    }

    #[test]
    fn rejects_protocol_mismatch_without_initializing() {
        let workspace = TestWorkspace::new();
        let output = run_lines(&[
            request(
                "wrong",
                "initialize",
                initialize_params(PROTOCOL_VERSION + 1, &workspace.root),
            ),
            request("status", "runtime.status", json!({})),
        ]);
        assert_eq!(
            response_error(&output[0]).error.data.code,
            MachineErrorCode::ProtocolVersionMismatch
        );
        assert_eq!(
            response_error(&output[1]).error.data.code,
            MachineErrorCode::NotInitialized
        );
    }

    #[test]
    fn requires_initialized_ack_and_reports_ready_afterwards() {
        let workspace = TestWorkspace::new();
        let output = run_lines(&[
            request(
                "initialize",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.root),
            ),
            request("pending", "runtime.status", json!({})),
            notification("initialized"),
            request("status", "runtime.status", json!({})),
            request("stop", "runtime.stop", json!({})),
        ]);
        assert!(matches!(output[0], RpcMessage::Response(_)));
        assert_eq!(
            response_error(&output[1]).error.data.code,
            MachineErrorCode::RuntimeNotReady
        );
        let RpcMessage::Response(status) = &output[2] else {
            panic!("expected ready status response");
        };
        let state: harness_control_protocol::RuntimeState =
            serde_json::from_value(status.result.clone()).unwrap();
        assert_eq!(state.lifecycle, RuntimeLifecycle::Ready);
        assert_eq!(state.readiness, RuntimeReadiness::Ready);
        let RpcMessage::Response(stop) = &output[3] else {
            panic!("expected stop response");
        };
        assert_eq!(stop.result, json!({ "accepted": true }));
    }

    #[test]
    fn endpoint_rpc_returns_the_active_loopback_mcp_url() {
        let workspace = TestWorkspace::new();
        let output = run_lines(&[
            request(
                "initialize",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.root),
            ),
            notification("initialized"),
            request("endpoint", "mcp.endpoint.get", json!({})),
            request("stop", "runtime.stop", json!({})),
        ]);
        let RpcMessage::Response(endpoint) = &output[1] else {
            panic!("expected endpoint response");
        };
        let endpoint: harness_control_protocol::McpEndpointResult =
            serde_json::from_value(endpoint.result.clone()).expect("endpoint result");
        assert!(endpoint.url.starts_with("http://127.0.0.1:"));
        assert!(endpoint.url.ends_with("/mcp"));
    }

    #[test]
    fn tools_list_returns_the_runtime_registry_catalog() {
        let workspace = TestWorkspace::new();
        let output = run_lines(&[
            request(
                "initialize",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.root),
            ),
            notification("initialized"),
            request("tools", "tools.list", json!({})),
        ]);

        let RpcMessage::Response(response) = &output[1] else {
            panic!("expected tools.list response");
        };
        let result: ToolsListResult = serde_json::from_value(response.result.clone()).unwrap();
        assert_eq!(
            result
                .items
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            ["read_file", "glob", "write", "edit", "bash"]
        );
        let bash = result
            .items
            .iter()
            .find(|tool| tool.name == "bash")
            .unwrap();
        assert!(!bash.enabled);
        assert_eq!(bash.risk_level, harness_control_protocol::RiskLevel::High);
    }

    #[test]
    fn policies_list_returns_runtime_effective_snapshot_and_typed_evidence() {
        let workspace = TestWorkspace::new();
        let output = run_lines(&[
            request(
                "initialize",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.root),
            ),
            notification("initialized"),
            request("policies", "policies.list", json!({})),
        ]);
        let RpcMessage::Response(response) = &output[1] else {
            panic!("expected policies.list response");
        };
        let result: PoliciesListResult = serde_json::from_value(response.result.clone()).unwrap();
        let effective = result.effective.expect("effective policy projection");
        assert!(effective.valid);
        assert!(effective.user_rules.is_empty());
        let read = effective
            .per_tool
            .iter()
            .find(|tool| tool.tool == "read_file")
            .expect("read_file effective policy");
        assert_eq!(
            read.decision,
            harness_control_protocol::PolicyDecision::Allow
        );
        assert!(read.matched_rule_ids.is_empty());
        assert_eq!(
            read.reason,
            harness_control_protocol::PolicyReason::ToolDefault
        );
    }

    #[test]
    fn rejects_duplicate_initialize() {
        let workspace = TestWorkspace::new();
        let output = run_lines(&[
            request(
                "first",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.root),
            ),
            request(
                "duplicate",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.root),
            ),
        ]);
        assert!(matches!(output[0], RpcMessage::Response(_)));
        assert_eq!(
            response_error(&output[1]).error.data.code,
            MachineErrorCode::AlreadyInitialized
        );
    }

    #[test]
    fn malformed_and_invalid_params_do_not_break_following_requests() {
        let workspace = TestWorkspace::new();
        let output = run_lines(&[
            "not json".into(),
            "{}".into(),
            request("initialize", "initialize", json!({})),
            request(
                "valid",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.root),
            ),
        ]);
        assert_eq!(
            response_error(&output[0]).error.code,
            error_codes::PARSE_ERROR
        );
        assert_eq!(
            response_error(&output[1]).error.code,
            error_codes::INVALID_REQUEST
        );
        assert_eq!(
            response_error(&output[2]).error.code,
            error_codes::INVALID_PARAMS
        );
        assert!(matches!(output[3], RpcMessage::Response(_)));
    }

    #[test]
    fn missing_workspace_returns_invalid_argument_without_initializing_core() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        let mut mcp_service = McpRuntimeService::new();
        let mut subscribed = false;
        let mut output = Vec::new();
        let request = RpcRequest {
            id: harness_control_protocol::RequestId::String("initialize".into()),
            method: "initialize".into(),
            params: Some(initialize_params(
                PROTOCOL_VERSION,
                &workspace.missing_path(),
            )),
        };

        assert!(!handle_request(
            request,
            &mut core,
            &mut mcp_service,
            &mut subscribed,
            &mut output,
        )
        .unwrap());

        let message: RpcMessage = serde_json::from_slice(&output).unwrap();
        let error = response_error(&message);
        assert_eq!(error.error.code, error_codes::INVALID_PARAMS);
        assert_eq!(error.error.data.code, MachineErrorCode::InvalidArgument);
        assert!(error.error.message.starts_with("Invalid workspace:"));
        assert!(core.config.is_none());
        assert!(core.events.is_none());
        assert!(core.workspace_guard().is_none());
        assert_eq!(core.phase, Phase::WaitingInitialize);
    }

    #[test]
    fn invalid_initialize_can_retry_with_valid_workspace() {
        let workspace = TestWorkspace::new();
        let output = run_lines(&[
            request(
                "invalid",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.missing_path()),
            ),
            request(
                "valid",
                "initialize",
                initialize_params(PROTOCOL_VERSION, &workspace.root),
            ),
        ]);

        assert_eq!(
            response_error(&output[0]).error.data.code,
            MachineErrorCode::InvalidArgument
        );
        assert!(matches!(output[1], RpcMessage::Response(_)));
    }

    #[test]
    fn two_runtime_cores_have_distinct_runtime_ids() {
        assert_ne!(RuntimeCore::new().runtime_id, RuntimeCore::new().runtime_id);
    }

    #[test]
    fn initialization_snapshot_is_reflected_in_status() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(runtime_config(&workspace.root))
            .expect("valid workspace initializes runtime");

        let state = core.status().unwrap();
        assert_eq!(state.config_revision, DecimalU64::new(7));
        assert_eq!(state.active_workspace, workspace.root.to_string_lossy());
        assert_eq!(state.security_mode, SecurityMode::Normal);
    }

    #[test]
    fn failed_mcp_startup_rolls_back_and_allows_retry() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        let mut mcp_service = McpRuntimeService::new();
        core.initialize(runtime_config(&workspace.root))
            .expect("initialize core");

        let result = start_mcp_after_initialize(&mut core, &mut mcp_service, |_, _| {
            Err(McpRuntimeServiceError::StartupChannelClosed)
        });
        assert!(matches!(
            result,
            Err(McpRuntimeServiceError::StartupChannelClosed)
        ));
        assert_eq!(core.phase, Phase::WaitingInitialize);
        assert!(core.config.is_none());
        assert!(core.events.is_none());
        assert!(core.workspace_guard().is_none());
        assert!(core.status().is_none());
        core.initialize(runtime_config(&workspace.root))
            .expect("retry initialize");
    }

    #[test]
    fn control_lifecycle_starts_one_endpoint_and_stops_it_before_replying() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        let mut mcp_service = McpRuntimeService::new();
        let mut subscribed = false;
        let mut output = Vec::new();

        let initialize = RpcRequest {
            id: harness_control_protocol::RequestId::String("initialize".into()),
            method: "initialize".into(),
            params: Some(initialize_params(PROTOCOL_VERSION, &workspace.root)),
        };
        assert!(!handle_request(
            initialize,
            &mut core,
            &mut mcp_service,
            &mut subscribed,
            &mut output,
        )
        .expect("initialize request"));
        let endpoint_url = mcp_service.endpoint_url().expect("endpoint URL").to_owned();

        handle_notification(
            RpcNotification {
                method: "initialized".into(),
                params: Some(json!({})),
            },
            &mut core,
            &mut subscribed,
            &mut output,
        )
        .expect("first acknowledgement");
        handle_notification(
            RpcNotification {
                method: "initialized".into(),
                params: Some(json!({})),
            },
            &mut core,
            &mut subscribed,
            &mut output,
        )
        .expect("duplicate acknowledgement");
        assert_eq!(mcp_service.endpoint_url(), Some(endpoint_url.as_str()));

        let stop = RpcRequest {
            id: harness_control_protocol::RequestId::String("stop".into()),
            method: "runtime.stop".into(),
            params: Some(json!({})),
        };
        assert!(handle_request(
            stop,
            &mut core,
            &mut mcp_service,
            &mut subscribed,
            &mut output,
        )
        .expect("stop request"));
        assert!(mcp_service.endpoint_url().is_none());
        let address: SocketAddr = endpoint_url
            .strip_prefix("http://")
            .and_then(|value| value.strip_suffix("/mcp"))
            .expect("endpoint authority")
            .parse()
            .expect("socket address");
        TcpListener::bind(address).expect("endpoint port released before stop response");

        let messages = String::from_utf8(output)
            .expect("UTF-8 output")
            .lines()
            .map(|line| serde_json::from_str::<RpcMessage>(line).expect("control response"))
            .collect::<Vec<_>>();
        assert!(matches!(messages[0], RpcMessage::Response(_)));
        let RpcMessage::Response(stop_response) = &messages[1] else {
            panic!("expected stop response");
        };
        assert_eq!(stop_response.result, json!({ "accepted": true }));
    }

    #[test]
    fn eof_cleanup_stops_and_joins_the_mcp_endpoint() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        let mut mcp_service = McpRuntimeService::new();
        core.initialize(runtime_config(&workspace.root))
            .expect("initialize core");
        start_mcp_after_initialize(&mut core, &mut mcp_service, |service, context| {
            service.start(context)
        })
        .expect("start MCP service");
        let endpoint_url = mcp_service.endpoint_url().expect("endpoint URL").to_owned();

        shutdown_runtime_services(&mut core, &mut mcp_service).expect("EOF cleanup");
        assert_eq!(core.phase, Phase::Stopping);
        assert!(mcp_service.endpoint_url().is_none());
        let address: SocketAddr = endpoint_url
            .strip_prefix("http://")
            .and_then(|value| value.strip_suffix("/mcp"))
            .expect("endpoint authority")
            .parse()
            .expect("socket address");
        TcpListener::bind(address).expect("endpoint port released during EOF cleanup");
    }

    #[test]
    fn approvals_control_methods_list_filter_resolve_and_reject_duplicate_actions() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(runtime_config(&workspace.root))
            .expect("initialize core");
        core.acknowledge().expect("acknowledge core");
        let request_id = uuid::Uuid::new_v4();
        core.approval_manager()
            .register(crate::approval::ApprovalFacts {
                request_id,
                tool: "write".into(),
                sanitized_arguments: [("path".into(), "notes.txt".into())].into_iter().collect(),
                scope_summary: "notes.txt".into(),
                fingerprint: "write|notes.txt|secret-is-internal-only".into(),
                grant_scope: Some("notes.txt".into()),
                bash_details: None,
                allow_for_session_eligible: None,
                matched_rule_ids: vec!["review-writes".into()],
                policy_reason: harness_control_protocol::PolicyReason::MatchedUserRules,
                risk_level: harness_control_protocol::RiskLevel::Medium,
            })
            .expect("register pending approval");

        let mut mcp_service = McpRuntimeService::new();
        let mut subscribed = false;
        let mut output = Vec::new();
        let list_request = RpcRequest {
            id: harness_control_protocol::RequestId::String("list".into()),
            method: "approvals.list".into(),
            params: Some(json!({ "status": "pending" })),
        };
        handle_request(
            list_request,
            &mut core,
            &mut mcp_service,
            &mut subscribed,
            &mut output,
        )
        .expect("list approvals");
        let list_message: RpcMessage = serde_json::from_slice(
            output
                .split(|byte| *byte == b'\n')
                .find(|line| !line.is_empty())
                .expect("list response"),
        )
        .expect("decode list response");
        let RpcMessage::Response(list_response) = list_message else {
            panic!("expected list response");
        };
        let list: ApprovalsListResult = serde_json::from_value(list_response.result).unwrap();
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].scope_summary, "notes.txt");
        assert!(!serde_json::to_string(&list)
            .unwrap()
            .contains("secret-is-internal-only"));

        let resolve = RpcRequest {
            id: harness_control_protocol::RequestId::String("resolve".into()),
            method: "approvals.resolve".into(),
            params: Some(json!({
                "requestId": list.items[0].request_id,
                "decision": "allow_for_session"
            })),
        };
        handle_request(
            resolve,
            &mut core,
            &mut mcp_service,
            &mut subscribed,
            &mut output,
        )
        .expect("resolve approval");
        let resolved_message = String::from_utf8(output.clone())
            .unwrap()
            .lines()
            .last()
            .map(|line| serde_json::from_str::<RpcMessage>(line).unwrap())
            .expect("resolved response");
        let RpcMessage::Response(resolved_response) = resolved_message else {
            panic!("expected resolved response");
        };
        let resolved: harness_control_protocol::ApprovalsResolveResult =
            serde_json::from_value(resolved_response.result).unwrap();
        assert_eq!(
            resolved.approval.status,
            harness_control_protocol::ApprovalStatus::Approved
        );
        assert_eq!(core.session_grants().len(), 1);

        let duplicate = RpcRequest {
            id: harness_control_protocol::RequestId::String("duplicate".into()),
            method: "approvals.resolve".into(),
            params: Some(json!({
                "requestId": resolved.approval.request_id,
                "decision": "deny"
            })),
        };
        handle_request(
            duplicate,
            &mut core,
            &mut mcp_service,
            &mut subscribed,
            &mut output,
        )
        .expect("duplicate response");
        let duplicate_message: RpcMessage = String::from_utf8(output)
            .unwrap()
            .lines()
            .last()
            .map(|line| serde_json::from_str(line).unwrap())
            .expect("duplicate error response");
        assert_eq!(
            response_error(&duplicate_message).error.data.code,
            MachineErrorCode::ApprovalAlreadyResolved
        );
    }

    #[test]
    fn approval_control_unknown_and_malformed_params_are_stable() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(runtime_config(&workspace.root))
            .expect("initialize core");
        core.acknowledge().expect("acknowledge core");
        let mut mcp_service = McpRuntimeService::new();
        let mut subscribed = false;
        let mut output = Vec::new();

        handle_request(
            RpcRequest {
                id: harness_control_protocol::RequestId::String("unknown".into()),
                method: "approvals.resolve".into(),
                params: Some(json!({
                    "requestId": uuid::Uuid::new_v4().to_string(),
                    "decision": "deny"
                })),
            },
            &mut core,
            &mut mcp_service,
            &mut subscribed,
            &mut output,
        )
        .expect("unknown approval response");
        let unknown = String::from_utf8(output.clone())
            .unwrap()
            .lines()
            .last()
            .map(|line| serde_json::from_str::<RpcMessage>(line).unwrap())
            .expect("unknown approval error response");
        assert_eq!(
            response_error(&unknown).error.data.code,
            MachineErrorCode::ApprovalNotFound
        );

        handle_request(
            RpcRequest {
                id: harness_control_protocol::RequestId::String("malformed".into()),
                method: "approvals.resolve".into(),
                params: Some(json!({
                    "requestId": "not-a-valid-live-request",
                    "decision": "not-a-decision"
                })),
            },
            &mut core,
            &mut mcp_service,
            &mut subscribed,
            &mut output,
        )
        .expect("malformed approval response");
        let malformed = String::from_utf8(output)
            .unwrap()
            .lines()
            .last()
            .map(|line| serde_json::from_str::<RpcMessage>(line).unwrap())
            .expect("malformed approval error response");
        assert_eq!(
            response_error(&malformed).error.code,
            error_codes::INVALID_PARAMS
        );
        assert_eq!(
            response_error(&malformed).error.data.code,
            MachineErrorCode::InvalidArgument
        );
    }
}
