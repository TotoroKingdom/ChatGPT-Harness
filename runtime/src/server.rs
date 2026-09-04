use crate::state::{Phase, RuntimeCore};
use harness_control_protocol::{
    error_codes, ActivityEvent, ActivityListParams, ActivityListResult, ActivitySubscribeParams,
    ActivitySubscribeResult, ApprovalsListResult, ApprovalsResolveParams, DiagnosticCheck,
    DiagnosticRun, DiagnosticStatus, EmptyParams, InitializeParams, InitializeResult,
    MachineErrorCode, PoliciesListResult, RpcError, RpcErrorData, RpcErrorResponse, RpcMessage,
    RpcNotification, RpcRequest, RpcResponse, RuntimeStopResult, ToolsListResult, PROTOCOL_VERSION,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const NOT_INITIALIZED_ERROR: i32 = error_codes::HARNESS_MIN;
const ALREADY_INITIALIZED_ERROR: i32 = -32001;
const PROTOCOL_VERSION_MISMATCH_ERROR: i32 = -32002;
const APPROVAL_NOT_FOUND_ERROR: i32 = -32003;
const RUNTIME_NOT_READY_ERROR: i32 = error_codes::HARNESS_MAX;

pub fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_io(BufReader::new(stdin.lock()), BufWriter::new(stdout.lock()))
}

fn run_io<R: BufRead, W: Write>(mut input: R, mut output: W) -> io::Result<()> {
    let mut core = RuntimeCore::new();
    let mut subscribed = false;
    let mut line = String::new();

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
                Ok(message) => dispatch(message, &mut core, &mut subscribed, &mut output)?,
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
}

fn empty_request_id() -> harness_control_protocol::RequestId {
    harness_control_protocol::RequestId::String(String::new())
}

fn dispatch<W: Write>(
    message: RpcMessage,
    core: &mut RuntimeCore,
    subscribed: &mut bool,
    output: &mut W,
) -> io::Result<bool> {
    match message {
        RpcMessage::Request(request) => handle_request(request, core, subscribed, output),
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
        "runtime.stop" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            if let Some(event) = core.stop() {
                if *subscribed {
                    write_event(output, &event)?;
                }
            }
            write_result(output, id, RuntimeStopResult { accepted: true })?;
            Ok(true)
        }
        "tools.list" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            write_result(output, id, ToolsListResult { items: Vec::new() })?;
            Ok(false)
        }
        "policies.list" => {
            let _: EmptyParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            write_result(output, id, PoliciesListResult { items: Vec::new() })?;
            Ok(false)
        }
        "activity.list" => {
            let params: ActivityListParams = match params_or_default(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
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
            let _: harness_control_protocol::ApprovalsListParams =
                match params_or_default(&request.params) {
                    Ok(value) => value,
                    Err(error) => return write_invalid_params(output, id, error),
                };
            write_result(output, id, ApprovalsListResult { items: Vec::new() })?;
            Ok(false)
        }
        "approvals.resolve" => {
            let _: ApprovalsResolveParams = match params(&request.params) {
                Ok(value) => value,
                Err(error) => return write_invalid_params(output, id, error),
            };
            write_error(
                output,
                id,
                APPROVAL_NOT_FOUND_ERROR,
                MachineErrorCode::ApprovalNotFound,
                "Approval actions are not implemented in the Runtime skeleton.",
            )?;
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
    use super::*;
    use harness_control_protocol::{
        DecimalU64, RuntimeConfigSnapshot, RuntimeLifecycle, RuntimeLimits, RuntimeReadiness,
        SecurityConfig, SecurityMode, WorkspaceConfig,
    };
    use serde_json::json;
    use std::fs;
    use std::io::Cursor;
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
                "runtimeLimits": { "maxConcurrentRequests": 4, "maxActivityEvents": 2 }
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
            },
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

        assert!(!handle_request(request, &mut core, &mut subscribed, &mut output).unwrap());

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
}
