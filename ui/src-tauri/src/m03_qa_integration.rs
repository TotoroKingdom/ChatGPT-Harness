//! Formal M03 Integration QA.
//!
//! These cases are intentionally black-box at the product boundaries.  The
//! Runtime subprocess, its stdio control protocol, loopback MCP endpoint and
//! the public evidence-store API are real.  Every fixture is disposable and
//! every case reports its own PASS/FAIL line so it can be run with a test
//! filter without depending on a suite-level fixture.

use crate::config::{ConfigStore, DesktopPreferencesPatch};
use crate::history;
use chatgpt_harness_evidence_store::{
    EvidenceCoverage, EvidenceEventV1, EvidenceReader, EvidenceStage, EvidenceWorkspaceValidation,
    EvidenceWriter, OfflineMaintenance, RuntimeSession, StoreError, StorePaths, WriterLock,
    WriterOptions,
};
use harness_control_protocol::{
    AppConfig, ApprovalDecision, DecimalU64, DurableSanitizedArgumentsEvidence,
    EvidenceLifecycleState, HistoryApprovalState, HistoryConfig, HistoryGetParams, HistoryOutcome,
    HistoryQueryParams, PolicyDecision, PolicyReason, RuntimeConfigSnapshot, RuntimeHostContext,
    RuntimeLimits, UiLanguage,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

type QaResult<T> = Result<T, String>;

const CONTROL_TIMEOUT: Duration = Duration::from_secs(15);
const MCP_TIMEOUT: Duration = Duration::from_secs(45);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

// The existing M01/M02 runner is the source of truth for the real subprocess
// harness.  Its M02 module contains the reusable stdio/control/MCP process
// implementation used by M01.  It is private to the runtime module in the
// production test tree, so this test-only module includes the existing M02
// source instead of creating a mock or changing production visibility.  The
// local hostContext-capable runner below uses the same protocol for the M03
// storage cases.
fn runtime_config_snapshot(config: &AppConfig) -> RuntimeConfigSnapshot {
    RuntimeConfigSnapshot {
        config_revision: config.config_revision,
        workspace: config.workspace.clone(),
        security: config.security.clone(),
        runtime_limits: RuntimeLimits {
            max_concurrent_requests: 16,
            max_activity_events: 1024,
            max_read_bytes: 1_048_576,
        },
        tools: config.tools.clone(),
        bash: config.bash.clone(),
        policies: config.policies.clone(),
    }
}

#[allow(dead_code)]
mod m2_e2e {
    include!("runtime/m2_e2e.rs");
}

macro_rules! check {
    ($condition:expr $(,)?) => {
        if !$condition {
            return Err(format!(
                "M03 integration assertion failed at {}:{}",
                file!(),
                line!()
            ));
        }
    };
    ($condition:expr, $($argument:tt)*) => {
        if !$condition {
            return Err(format!($($argument)*));
        }
    };
}

fn report_case(case_id: &str, result: QaResult<()>) {
    match result {
        Ok(()) => println!("{case_id} PASS"),
        Err(error) => {
            println!("{case_id} FAIL: {error}");
            panic!("{case_id} failed: {error}");
        }
    }
}

fn debug_runtime_binary() -> QaResult<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join("chatgpt-harness-runtime.exe");
    if !path.is_file() {
        return Err(format!(
            "required debug Runtime executable is absent: {} (build target/debug/chatgpt-harness-runtime.exe first)",
            path.display()
        ));
    }
    Ok(path)
}

struct QaFixture {
    base: PathBuf,
    config_dir: PathBuf,
    workspace: PathBuf,
    outside: PathBuf,
    database: PathBuf,
}

impl QaFixture {
    fn new(case_id: &str) -> QaResult<Self> {
        let base = std::env::temp_dir().join(format!(
            "chatgpt-harness-m03-{case_id}-{}",
            uuid::Uuid::new_v4()
        ));
        let config_dir = base.join("appdata").join("config");
        let workspace = base.join("appdata").join("workspace");
        let outside = base.join("outside");
        let database = base.join("appdata").join("history").join("history.sqlite3");
        fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
        fs::create_dir_all(&workspace).map_err(|error| error.to_string())?;
        fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
        fs::write(workspace.join("sample.txt"), b"M03 sample content\n")
            .map_err(|error| error.to_string())?;
        fs::write(outside.join("outside.txt"), b"outside fixture")
            .map_err(|error| error.to_string())?;
        Ok(Self {
            base,
            config_dir,
            workspace,
            outside,
            database,
        })
    }

    fn config(&self) -> AppConfig {
        let mut config = ConfigStore::safe_defaults();
        config.workspace.root_path = self.workspace.display().to_string();
        config
    }
}

impl Drop for QaFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

struct QaRuntime {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<String>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<String>>,
    next_control_id: u64,
    runtime_id: String,
    endpoint: SocketAddr,
}

impl QaRuntime {
    fn start(
        binary: &Path,
        config: &AppConfig,
        database: Option<&Path>,
        host_session_id: Option<&str>,
        supervisor_generation_id: Option<&str>,
    ) -> QaResult<(Self, QaMcpClient)> {
        let mut command = Command::new(binary);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("spawn debug Runtime: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "debug Runtime stdin was not piped".to_owned())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "debug Runtime stdout was not piped".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "debug Runtime stderr was not piped".to_owned())?;
        let (sender, messages) = mpsc::channel();
        let stdout_thread = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let stderr_thread = thread::spawn(move || {
            let mut output = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut output);
            output
        });
        let mut runtime = Self {
            child,
            stdin,
            messages,
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            next_control_id: 0,
            runtime_id: String::new(),
            endpoint: SocketAddr::from(([127, 0, 0, 1], 0)),
        };

        let mut initialize = json!({
            "clientInfo": { "name": "m03-qa", "title": "M03 Integration QA", "version": "1" },
            "protocolVersion": 1,
            "capabilities": { "activityReplay": true },
            "runtimeConfig": serde_json::to_value(runtime_config_snapshot(config))
                .map_err(|error| format!("serialize Runtime snapshot: {error}"))?
        });
        if let Some(database) = database {
            initialize["hostContext"] = serde_json::to_value(RuntimeHostContext {
                host_session_id: host_session_id.map(ToOwned::to_owned),
                supervisor_generation_id: supervisor_generation_id.map(ToOwned::to_owned),
                evidence_store: Some(harness_control_protocol::EvidenceStoreLaunchConfig {
                    enabled: config.history.enabled,
                    database_path: database.display().to_string(),
                    retention_days: config.history.retention_days,
                    max_storage_mib: config.history.max_storage_mib,
                }),
            })
            .map_err(|error| format!("serialize Runtime host context: {error}"))?;
        }
        let initialized = runtime.control_request("initialize", initialize)?;
        runtime.runtime_id = initialized
            .get("runtimeId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Runtime initialize omitted runtimeId".to_owned())?
            .to_owned();
        runtime.control_notification("initialized", json!({}))?;
        let endpoint = runtime.control_request("mcp.endpoint.get", json!({}))?;
        let endpoint_url = endpoint
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| "mcp.endpoint.get omitted URL".to_owned())?;
        runtime.endpoint = parse_endpoint(endpoint_url)?;
        let status = runtime.control_request("runtime.status", json!({}))?;
        check!(
            status.get("configRevision")
                == Some(&Value::String(config.config_revision.get().to_string())),
            "Runtime generation revision did not match startup snapshot"
        );
        let mcp = QaMcpClient::connect(runtime.endpoint)?;
        Ok((runtime, mcp))
    }

    fn control_request_raw(&mut self, method: &str, params: Value) -> QaResult<Value> {
        self.next_control_id += 1;
        let id = format!("m03-control-{}", self.next_control_id);
        self.write_json_line(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;
        loop {
            let line =
                self.messages
                    .recv_timeout(CONTROL_TIMEOUT)
                    .map_err(|error| match error {
                        RecvTimeoutError::Timeout => {
                            format!("Runtime control request {method} timed out")
                        }
                        RecvTimeoutError::Disconnected => {
                            format!("Runtime stdout closed during {method}")
                        }
                    })?;
            let message: Value = serde_json::from_str(&line)
                .map_err(|error| format!("Runtime emitted invalid control JSON: {error}"))?;
            if message.get("id") != Some(&Value::String(id.clone())) {
                continue;
            }
            return Ok(message);
        }
    }

    fn control_request(&mut self, method: &str, params: Value) -> QaResult<Value> {
        let message = self.control_request_raw(method, params)?;
        if let Some(error) = message.get("error") {
            return Err(format!("Runtime control {method} returned error: {error}"));
        }
        message
            .get("result")
            .cloned()
            .ok_or_else(|| format!("Runtime control {method} omitted result"))
    }

    fn control_notification(&mut self, method: &str, params: Value) -> QaResult<()> {
        self.write_json_line(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    fn write_json_line(&mut self, value: &Value) -> QaResult<()> {
        serde_json::to_writer(&mut self.stdin, value)
            .map_err(|error| format!("serialize control JSON: {error}"))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|error| format!("write control JSON: {error}"))?;
        self.stdin
            .flush()
            .map_err(|error| format!("flush control JSON: {error}"))
    }

    fn activity_items(&mut self) -> QaResult<Vec<Value>> {
        self.control_request("activity.list", json!({ "limit": 256 }))?
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "activity.list omitted items".to_owned())
    }

    fn runtime_id(&self) -> &str {
        &self.runtime_id
    }

    fn stop(&mut self) -> QaResult<()> {
        let result = self.control_request("runtime.stop", json!({}))?;
        check!(
            result.get("accepted") == Some(&Value::Bool(true)),
            "Runtime stop was not accepted"
        );
        self.wait_for_exit()
    }

    fn crash(&mut self) -> QaResult<()> {
        if self
            .child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            self.child.kill().map_err(|error| error.to_string())?;
            self.child.wait().map_err(|error| error.to_string())?;
        }
        self.join_output_threads();
        Ok(())
    }

    fn wait_for_exit(&mut self) -> QaResult<()> {
        let deadline = Instant::now() + CONTROL_TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().map_err(|error| error.to_string())? {
                self.join_output_threads();
                check!(status.success(), "Runtime exited unsuccessfully: {status}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("Runtime did not exit after runtime.stop".to_owned());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn join_output_threads(&mut self) {
        if let Some(thread) = self.stdout_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.stderr_thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for QaRuntime {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.join_output_threads();
    }
}

struct QaMcpClient {
    endpoint: SocketAddr,
    session_id: String,
    next_request_id: u64,
}

struct PendingQaCall {
    receiver: Receiver<QaResult<Value>>,
    thread: JoinHandle<()>,
}

impl PendingQaCall {
    fn receive(self) -> QaResult<Value> {
        let result = self
            .receiver
            .recv_timeout(MCP_TIMEOUT)
            .map_err(|error| format!("pending MCP call did not finish: {error}"))?;
        let _ = self.thread.join();
        result
    }
}

impl QaMcpClient {
    fn connect(endpoint: SocketAddr) -> QaResult<Self> {
        let response = http_json_request(
            endpoint,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "m03-qa", "version": "1" }
                }
            }),
            None,
        )?;
        check!(
            response.status == 200,
            "MCP initialize did not return HTTP 200"
        );
        let session_id = response
            .header("mcp-session-id")
            .ok_or_else(|| "MCP initialize omitted session id".to_owned())?
            .to_owned();
        Ok(Self {
            endpoint,
            session_id,
            next_request_id: 1,
        })
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> QaResult<Value> {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }

    fn pending_tool(&mut self, name: &str, arguments: Value) -> PendingQaCall {
        self.next_request_id += 1;
        let endpoint = self.endpoint;
        let session_id = self.session_id.clone();
        let id = self.next_request_id;
        let name = name.to_owned();
        let (sender, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let result = http_json_request(
                endpoint,
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "tools/call",
                    "params": { "name": name, "arguments": arguments },
                }),
                Some(&session_id),
            )
            .and_then(|response| response.json_rpc());
            let _ = sender.send(result);
        });
        PendingQaCall { receiver, thread }
    }

    fn request(&mut self, method: &str, params: Value) -> QaResult<Value> {
        self.next_request_id += 1;
        let response = http_json_request(
            self.endpoint,
            json!({
                "jsonrpc": "2.0",
                "id": self.next_request_id,
                "method": method,
                "params": params,
            }),
            Some(&self.session_id),
        )?;
        check!(
            response.status == 200,
            "MCP {method} did not return HTTP 200"
        );
        response.json_rpc()
    }
}

struct HttpResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl HttpResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    fn json_rpc(&self) -> QaResult<Value> {
        if let Ok(value) = serde_json::from_slice(&self.body) {
            return Ok(value);
        }
        let body = std::str::from_utf8(&self.body)
            .map_err(|error| format!("MCP response was not UTF-8: {error}"))?;
        body.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .find_map(|line| serde_json::from_str(line.trim()).ok())
            .ok_or_else(|| "MCP response had no JSON-RPC payload".to_owned())
    }
}

fn http_json_request(
    address: SocketAddr,
    message: Value,
    session_id: Option<&str>,
) -> QaResult<HttpResponse> {
    let body = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
    let mut stream = TcpStream::connect_timeout(&address, MCP_TIMEOUT)
        .map_err(|error| format!("connect loopback MCP endpoint: {error}"))?;
    stream
        .set_read_timeout(Some(MCP_TIMEOUT))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(MCP_TIMEOUT))
        .map_err(|error| error.to_string())?;
    let mut request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: keep-alive\r\nAccept: application/json, text/event-stream\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        address.port(),
        body.len()
    );
    if let Some(session_id) = session_id {
        request.push_str(&format!(
            "Mcp-Session-Id: {session_id}\r\nMCP-Protocol-Version: 2025-03-26\r\n"
        ));
    }
    request.push_str("\r\n");
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(&body))
        .and_then(|_| stream.flush())
        .map_err(|error| format!("write MCP request: {error}"))?;
    read_mcp_response(&mut stream)
}

fn read_mcp_response(stream: &mut TcpStream) -> QaResult<HttpResponse> {
    let deadline = Instant::now() + MCP_TIMEOUT;
    let mut raw = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => return Err("MCP response closed before JSON-RPC data".to_owned()),
            Ok(read) => raw.extend_from_slice(&buffer[..read]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) && Instant::now() < deadline =>
            {
                continue;
            }
            Err(error) => return Err(format!("read MCP response: {error}")),
        }
        let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
            if Instant::now() >= deadline {
                return Err("MCP response had no complete HTTP header".to_owned());
            }
            continue;
        };
        let header_text = std::str::from_utf8(&raw[..header_end])
            .map_err(|error| format!("MCP response headers were not UTF-8: {error}"))?;
        let mut lines = header_text.lines();
        let status = lines
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .ok_or_else(|| "MCP response omitted status".to_owned())?
            .parse::<u16>()
            .map_err(|error| format!("MCP response status invalid: {error}"))?;
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
            .collect::<BTreeMap<_, _>>();
        let wire_body = &raw[header_end + 4..];
        let (body, complete) = if headers
            .get("transfer-encoding")
            .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
        {
            decode_chunked_available(wire_body)?
        } else {
            (wire_body.to_vec(), true)
        };
        if body_contains_json_rpc(&body)
            || (complete && headers.contains_key("content-length") && !body.is_empty())
        {
            return Ok(HttpResponse {
                status,
                headers,
                body,
            });
        }
        if Instant::now() >= deadline {
            return Err("MCP response did not contain JSON-RPC data".to_owned());
        }
    }
}

fn body_contains_json_rpc(body: &[u8]) -> bool {
    if serde_json::from_slice::<Value>(body).is_ok() {
        return true;
    }
    std::str::from_utf8(body).ok().is_some_and(|text| {
        text.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .any(|data| {
                !data.trim().is_empty() && serde_json::from_str::<Value>(data.trim()).is_ok()
            })
    })
}

fn decode_chunked_available(wire: &[u8]) -> QaResult<(Vec<u8>, bool)> {
    let mut cursor = 0usize;
    let mut body = Vec::new();
    loop {
        let Some(relative_end) = wire[cursor..]
            .windows(2)
            .position(|window| window == b"\r\n")
        else {
            return Ok((body, false));
        };
        let line_end = cursor + relative_end;
        let length_text = std::str::from_utf8(&wire[cursor..line_end])
            .map_err(|error| format!("invalid chunk length: {error}"))?;
        let length =
            usize::from_str_radix(length_text.split(';').next().unwrap_or_default().trim(), 16)
                .map_err(|error| format!("invalid chunk size: {error}"))?;
        cursor = line_end + 2;
        if length == 0 {
            return Ok((body, true));
        }
        if wire.len() < cursor + length + 2 {
            return Ok((body, false));
        }
        body.extend_from_slice(&wire[cursor..cursor + length]);
        check!(
            &wire[cursor + length..cursor + length + 2] == b"\r\n",
            "invalid chunk terminator"
        );
        cursor += length + 2;
    }
}

fn parse_endpoint(url: &str) -> QaResult<SocketAddr> {
    let rest = url
        .strip_prefix("http://127.0.0.1:")
        .ok_or_else(|| format!("Runtime endpoint is not loopback HTTP: {url}"))?;
    let (port, suffix) = rest
        .split_once("/mcp")
        .ok_or_else(|| format!("Runtime endpoint is not /mcp: {url}"))?;
    check!(suffix.is_empty(), "Runtime endpoint has unexpected suffix");
    let port = port
        .parse::<u16>()
        .map_err(|error| format!("Runtime endpoint port invalid: {error}"))?;
    Ok(SocketAddr::from(([127, 0, 0, 1], port)))
}

fn assert_mcp_success(value: &Value) -> QaResult<()> {
    check!(
        value.get("error").is_none(),
        "MCP returned a JSON-RPC error"
    );
    check!(
        value["result"]["isError"].as_bool() != Some(true),
        "MCP returned a tool error: {value}"
    );
    Ok(())
}

fn assert_mcp_error(value: &Value, code: &str) -> QaResult<()> {
    check!(value.get("error").is_none(), "MCP transport error: {value}");
    check!(
        value["result"]["isError"].as_bool() == Some(true),
        "MCP unexpectedly succeeded: {value}"
    );
    check!(
        value["result"]["structuredContent"]["code"].as_str() == Some(code),
        "MCP error code was not {code}: {value}"
    );
    Ok(())
}

fn wait_for_approval(runtime: &mut QaRuntime) -> QaResult<Value> {
    let deadline = Instant::now() + CONTROL_TIMEOUT;
    loop {
        let result = runtime.control_request("approvals.list", json!({ "status": "pending" }))?;
        if let Some(item) = result
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
        {
            return Ok(item.clone());
        }
        if Instant::now() >= deadline {
            return Err("real Runtime did not expose Pending approval".to_owned());
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn approval_request_id(approval: &Value) -> QaResult<String> {
    approval
        .get("requestId")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("approval omitted requestId: {approval}"))
}

fn activity_identity(
    items: &[Value],
    request_id: Option<&str>,
) -> QaResult<(String, String, String)> {
    let item = items.iter().rev().find(|item| {
        item.get("eventType").and_then(Value::as_str) == Some("tool_requested")
            && request_id.is_none_or(|request| {
                item.get("requestId").and_then(Value::as_str) == Some(request)
            })
    });
    let item = item.ok_or_else(|| "activity.list has no tool_requested identity".to_owned())?;
    let event_id = item
        .get("eventId")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("activity event omitted eventId: {item}"))?;
    let runtime_id = item
        .get("runtimeId")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("activity event omitted runtimeId: {item}"))?;
    let request_id = item
        .get("requestId")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("activity event omitted requestId: {item}"))?;
    let sequence = item
        .get("sequence")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("activity event omitted sequence: {item}"))?;
    check!(
        event_id != sequence,
        "activity sequence was incorrectly used as canonical event identity"
    );
    check!(
        runtime_id
            == item
                .get("runtimeId")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        "activity runtime identity was unstable"
    );
    Ok((
        event_id.to_owned(),
        runtime_id.to_owned(),
        request_id.to_owned(),
    ))
}

fn request_id_from_activity(items: &[Value], excluded: &[&str]) -> QaResult<String> {
    items
        .iter()
        .rev()
        .find_map(|item| {
            if item.get("eventType").and_then(Value::as_str) != Some("tool_requested") {
                return None;
            }
            let request = item.get("requestId").and_then(Value::as_str)?;
            (!excluded.contains(&request)).then(|| request.to_owned())
        })
        .ok_or_else(|| "activity.list has no new tool request identity".to_owned())
}

fn wait_for_history(
    database: &Path,
    runtime_id: &str,
    request_id: &str,
) -> QaResult<harness_control_protocol::HistoryGetResult> {
    let deadline = Instant::now() + CONTROL_TIMEOUT;
    loop {
        let reader = EvidenceReader::open(database).map_err(|error| error.to_string())?;
        if let Some(result) = reader
            .get(runtime_id, request_id)
            .map_err(|error| error.to_string())?
        {
            return Ok(result);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "durable History did not expose ({runtime_id}, {request_id})"
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn event_fixture(
    runtime_id: &str,
    request_id: &str,
    event_id: &str,
    tool_name: &str,
    occurred_at_utc: &str,
    stage: EvidenceStage,
    lifecycle_state: EvidenceLifecycleState,
    outcome: HistoryOutcome,
    policy_decision: Option<PolicyDecision>,
    approval_id: Option<&str>,
    approval_decision: Option<ApprovalDecision>,
    coverage: EvidenceCoverage,
    host_session_id: Option<&str>,
    supervisor_generation_id: Option<&str>,
) -> EvidenceEventV1 {
    EvidenceEventV1 {
        schema_version: 1,
        event_id: event_id.to_owned(),
        runtime_id: runtime_id.to_owned(),
        request_id: request_id.to_owned(),
        host_session_id: host_session_id.map(ToOwned::to_owned),
        supervisor_generation_id: supervisor_generation_id.map(ToOwned::to_owned),
        approval_id: approval_id.map(ToOwned::to_owned),
        tool_name: tool_name.to_owned(),
        occurred_at_utc: occurred_at_utc.to_owned(),
        recorded_at_utc: occurred_at_utc.to_owned(),
        stage,
        lifecycle_state,
        workspace_validation: EvidenceWorkspaceValidation::Passed,
        policy_decision: policy_decision.clone(),
        policy_reason: policy_decision.map(|_| PolicyReason::ToolDefault),
        matched_rule_ids: Vec::new(),
        approval_decision,
        outcome,
        failure_category: None,
        duration_ms: Some(1),
        sanitized_arguments: Some(DurableSanitizedArgumentsEvidence::ReadFile {
            logical_path: "sample.txt".into(),
        }),
        result_evidence: None,
        coverage,
    }
}

fn persist_events(database: &Path, events: &[EvidenceEventV1]) -> QaResult<()> {
    let mut writer = EvidenceWriter::open_with_options(
        database,
        WriterOptions {
            history_config: HistoryConfig::default(),
            runtime_version: "m03-qa".into(),
            config_revision: "1".into(),
        },
    )
    .map_err(|error| error.to_string())?;
    let mut sessions = BTreeSet::new();
    for event in events {
        if sessions.insert(event.runtime_id.clone()) {
            writer
                .open_session(&RuntimeSession {
                    runtime_id: event.runtime_id.clone(),
                    host_session_id: event.host_session_id.clone(),
                    supervisor_generation_id: event.supervisor_generation_id.clone(),
                    started_at_utc: event.occurred_at_utc.clone(),
                    runtime_version: "m03-qa".into(),
                    config_revision: "1".into(),
                })
                .map_err(|error| error.to_string())?;
        }
    }
    writer
        .record_events(events)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn history_query(
    database: &Path,
    params: HistoryQueryParams,
) -> QaResult<harness_control_protocol::HistoryQueryResult> {
    EvidenceReader::open(database)
        .map_err(|error| error.to_string())?
        .query(&params)
        .map_err(|error| error.to_string())
}

fn history_get(
    database: &Path,
    runtime_id: &str,
    request_id: &str,
) -> QaResult<Option<harness_control_protocol::HistoryGetResult>> {
    EvidenceReader::open(database)
        .map_err(|error| error.to_string())?
        .get(runtime_id, request_id)
        .map_err(|error| error.to_string())
}

fn store_file_snapshot(database: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    [
        database.to_path_buf(),
        PathBuf::from(format!("{}-wal", database.display())),
        PathBuf::from(format!("{}-shm", database.display())),
    ]
    .into_iter()
    .filter_map(|path| fs::read(&path).ok().map(|bytes| (path, bytes)))
    .collect()
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_01_canonical_identity_reaches_activity_and_history() {
    report_case("INT-01", run_int_01());
}

fn run_int_01() -> QaResult<()> {
    let binary = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-01")?;
    let mut observations = Vec::new();

    for (label, decision) in [
        ("success", PolicyDecision::Allow),
        ("policy-deny", PolicyDecision::Deny),
        ("ask", PolicyDecision::Ask),
    ] {
        let mut config = fixture.config();
        config.policies.tool_defaults.write = decision;
        let (mut runtime, mut mcp) = QaRuntime::start(
            &binary,
            &config,
            Some(&fixture.database),
            Some("m03-int-01-host"),
            Some(&format!("m03-int-01-{label}")),
        )?;

        match label {
            "success" => {
                let result = mcp.call_tool("read_file", json!({ "path": "sample.txt" }))?;
                assert_mcp_success(&result)?;
            }
            "policy-deny" => {
                let result = mcp.call_tool(
                    "write",
                    json!({ "path": "policy-deny.txt", "content": "must-not-write" }),
                )?;
                assert_mcp_error(&result, "POLICY_DENIED")?;
                check!(!fixture.workspace.join("policy-deny.txt").exists());
            }
            "ask" => {
                let pending = mcp.pending_tool(
                    "write",
                    json!({ "path": "ask-deny.txt", "content": "must-not-write" }),
                );
                let approval = wait_for_approval(&mut runtime)?;
                let request_id = approval_request_id(&approval)?;
                runtime.control_request(
                    "approvals.resolve",
                    json!({ "requestId": request_id, "decision": "deny" }),
                )?;
                let result = pending.receive()?;
                assert_mcp_error(&result, "APPROVAL_DENIED")?;
                check!(!fixture.workspace.join("ask-deny.txt").exists());
            }
            _ => return Err(format!("unknown INT-01 flow {label}")),
        }

        let activity = runtime.activity_items()?;
        let (event_id, activity_runtime_id, request_id) = activity_identity(&activity, None)?;
        check!(
            activity_runtime_id == runtime.runtime_id(),
            "{label}: Activity runtimeId did not equal the live Runtime identity"
        );
        observations.push((label, event_id, activity_runtime_id, request_id));
        runtime.stop()?;
    }

    for (label, event_id, runtime_id, request_id) in observations {
        let detail = wait_for_history(&fixture.database, &runtime_id, &request_id)?;
        check!(
            detail.summary.runtime_id == runtime_id && detail.summary.request_id == request_id,
            "{label}: durable request key was not runtime_id + request_id"
        );
        check!(
            detail.events.iter().any(|event| event.event_id == event_id),
            "{label}: live Activity eventId {event_id} was not durable"
        );
        check!(
            detail
                .events
                .iter()
                .all(|event| event.runtime_id == runtime_id && event.request_id == request_id),
            "{label}: durable event identity was not stable"
        );
        check!(
            history_get(&fixture.database, "wrong-runtime-id", &request_id)?.is_none(),
            "{label}: timestamp proximity or request-only lookup replaced the composite key"
        );
    }
    check!(
        fixture.database.is_file(),
        "INT-01 did not create the disposable History database"
    );
    Ok(())
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_02_runtime_identity_host_mapping_and_retry_boundaries() {
    report_case("INT-02", run_int_02());
}

fn run_int_02() -> QaResult<()> {
    let binary = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-02")?;
    let mut config = fixture.config();
    config.policies.tool_defaults.write = PolicyDecision::Ask;

    let (mut runtime_a, mut mcp_a) = QaRuntime::start(
        &binary,
        &config,
        Some(&fixture.database),
        Some("explicit-host-session"),
        Some("explicit-supervisor-generation"),
    )?;
    let runtime_a_id = runtime_a.runtime_id().to_owned();
    let pending_a = mcp_a.pending_tool(
        "write",
        json!({ "path": "binding-a.txt", "content": "deny" }),
    );
    let approval_a = wait_for_approval(&mut runtime_a)?;
    let approval_request = approval_request_id(&approval_a)?;

    // A second real process is intentionally started without a Host mapping
    // and without a shared writer.  The control-plane request proves that a
    // request from Runtime A cannot be resolved by Runtime B.
    let (mut runtime_b, mut mcp_b) = QaRuntime::start(&binary, &config, None, None, None)?;
    let runtime_b_id = runtime_b.runtime_id().to_owned();
    check!(
        runtime_a_id != runtime_b_id,
        "two Runtime processes reused runtime_id"
    );
    let wrong_resolution = runtime_b.control_request_raw(
        "approvals.resolve",
        json!({ "requestId": approval_request, "decision": "deny" }),
    )?;
    check!(
        wrong_resolution.get("error").is_some(),
        "Runtime B resolved Runtime A's approval identity"
    );
    let b_result = mcp_b.call_tool("read_file", json!({ "path": "sample.txt" }))?;
    assert_mcp_success(&b_result)?;
    runtime_b.stop()?;

    runtime_a.control_request(
        "approvals.resolve",
        json!({ "requestId": approval_request, "decision": "deny" }),
    )?;
    assert_mcp_error(&pending_a.receive()?, "APPROVAL_DENIED")?;
    let a_activity = runtime_a.activity_items()?;
    let (_, _, a_request_id) = activity_identity(&a_activity, Some(&approval_request))?;
    check!(a_request_id == approval_request);
    runtime_a.stop()?;

    // A restart on the same disposable database gets no Host mapping and a
    // new Runtime identity.  Two real MCP calls also demonstrate retry/new
    // request boundaries rather than timestamp correlation.
    let (mut runtime_b2, mut mcp_b2) =
        QaRuntime::start(&binary, &config, Some(&fixture.database), None, None)?;
    let runtime_b2_id = runtime_b2.runtime_id().to_owned();
    check!(runtime_b2_id != runtime_a_id && runtime_b2_id != runtime_b_id);
    let first = mcp_b2.call_tool("read_file", json!({ "path": "sample.txt" }))?;
    assert_mcp_success(&first)?;
    let first_activity = runtime_b2.activity_items()?;
    let first_request = request_id_from_activity(&first_activity, &[])?;
    let second = mcp_b2.call_tool("read_file", json!({ "path": "sample.txt" }))?;
    assert_mcp_success(&second)?;
    let second_activity = runtime_b2.activity_items()?;
    let second_request = request_id_from_activity(&second_activity, &[first_request.as_str()])?;
    check!(
        first_request != second_request,
        "retry reused the previous request_id"
    );
    runtime_b2.stop()?;

    let (mut runtime_c, _mcp_c) =
        QaRuntime::start(&binary, &config, Some(&fixture.database), None, None)?;
    let runtime_c_id = runtime_c.runtime_id().to_owned();
    check!(
        runtime_c_id != runtime_a_id && runtime_c_id != runtime_b2_id,
        "Runtime restart reused the previous runtime_id"
    );
    let old_after_restart = runtime_c.control_request_raw(
        "approvals.resolve",
        json!({ "requestId": approval_request, "decision": "deny" }),
    )?;
    check!(
        old_after_restart.get("error").is_some(),
        "historical approval ID became actionable after restart"
    );
    runtime_c.stop()?;

    let detail_a = wait_for_history(&fixture.database, &runtime_a_id, &a_request_id)?;
    let detail_b2 = wait_for_history(&fixture.database, &runtime_b2_id, &first_request)?;
    check!(
        detail_a
            .events
            .iter()
            .all(
                |event| event.host_session_id.as_deref() == Some("explicit-host-session")
                    && event.supervisor_generation_id.as_deref()
                        == Some("explicit-supervisor-generation")
            ),
        "explicit Host mapping was not preserved in durable evidence"
    );
    check!(
        detail_b2.events.iter().all(
            |event| event.host_session_id.is_none() && event.supervisor_generation_id.is_none()
        ),
        "missing Host mapping was inferred instead of remaining null"
    );

    // MCP request IDs are Runtime-generated and deliberately not caller
    // controllable.  The controlled store boundary uses the two real Runtime
    // identities with the same textual request_id to prove that the durable
    // key remains composite and never collapses by timestamp.
    let controlled_request = "m03-same-textual-request";
    persist_events(
        &fixture.database,
        &[
            event_fixture(
                &runtime_a_id,
                controlled_request,
                "m03-int-02-event-a",
                "read_file",
                "2026-11-01T00:00:00.000Z",
                EvidenceStage::ExecutionCompleted,
                EvidenceLifecycleState::Completed,
                HistoryOutcome::Success,
                Some(PolicyDecision::Allow),
                None,
                None,
                EvidenceCoverage::Complete,
                Some("explicit-host-session"),
                Some("explicit-supervisor-generation"),
            ),
            event_fixture(
                &runtime_b2_id,
                controlled_request,
                "m03-int-02-event-b",
                "read_file",
                "2026-11-01T00:00:01.000Z",
                EvidenceStage::ExecutionCompleted,
                EvidenceLifecycleState::Completed,
                HistoryOutcome::Success,
                Some(PolicyDecision::Allow),
                None,
                None,
                EvidenceCoverage::Complete,
                None,
                None,
            ),
        ],
    )?;
    check!(
        history_get(&fixture.database, &runtime_a_id, controlled_request)?.is_some()
            && history_get(&fixture.database, &runtime_b2_id, controlled_request)?.is_some(),
        "same textual request identity was not separated by runtime_id"
    );
    Ok(())
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_03_history_config_generation_semantics() {
    report_case("INT-03", run_int_03());
}

fn run_int_03() -> QaResult<()> {
    let binary = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-03")?;
    let store = ConfigStore::new(fixture.config_dir.clone());
    let initial = store
        .save(fixture.config())
        .map_err(|error| error.message())?;
    check!(initial.history == HistoryConfig::default());
    check!(initial.history.enabled);
    check!(initial.history.retention_days == 30 && initial.history.max_storage_mib == 256);
    for value in [1, 3650] {
        check!(HistoryConfig {
            enabled: true,
            retention_days: value,
            max_storage_mib: 32
        }
        .validate()
        .is_ok());
    }
    for value in [0, 3651] {
        check!(HistoryConfig {
            enabled: true,
            retention_days: value,
            max_storage_mib: 32
        }
        .validate()
        .is_err());
    }
    for value in [32, 4096] {
        check!(HistoryConfig {
            enabled: true,
            retention_days: 1,
            max_storage_mib: value
        }
        .validate()
        .is_ok());
    }
    for value in [31, 4097] {
        check!(HistoryConfig {
            enabled: true,
            retention_days: 1,
            max_storage_mib: value
        }
        .validate()
        .is_err());
    }

    let (mut runtime, _mcp) = QaRuntime::start(
        &binary,
        &initial,
        Some(&fixture.database),
        Some("int-03-host"),
        Some("int-03-generation-a"),
    )?;
    let old_status = runtime.control_request("evidence.status", json!({}))?;
    check!(old_status["effectiveHistoryConfig"]["retentionDays"] == json!(30));
    check!(old_status["effectiveHistoryConfig"]["maxStorageMiB"] == json!(256));

    let mut desired = initial.clone();
    desired.history.retention_days = 3650;
    desired.history.max_storage_mib = 4096;
    let desired = store.save(desired).map_err(|error| error.message())?;
    check!(desired.config_revision == DecimalU64::new(2));
    let while_running = runtime.control_request("runtime.status", json!({}))?;
    check!(while_running["configRevision"] == json!("1"));
    let old_effective = runtime.control_request("evidence.status", json!({}))?;
    check!(old_effective["effectiveHistoryConfig"]["retentionDays"] == json!(30));
    check!(old_effective["effectiveHistoryConfig"]["maxStorageMiB"] == json!(256));

    let ui_only = store
        .set_ui_language(Some(UiLanguage::En))
        .map_err(|error| error.message())?;
    check!(ui_only.config_revision == DecimalU64::new(2));
    let desktop_only = store
        .set_desktop_preferences(DesktopPreferencesPatch {
            approval_notifications: Some(false),
            connection_notifications: None,
            background_notice_acknowledged: None,
            auto_start_harness_on_app_launch: None,
        })
        .map_err(|error| error.message())?;
    check!(desktop_only.config_revision == DecimalU64::new(2));
    runtime.stop()?;

    let loaded = store
        .load()
        .map_err(|error| error.message())?
        .ok_or_else(|| "saved M03 config disappeared".to_owned())?;
    let (mut restarted, _mcp) = QaRuntime::start(
        &binary,
        &loaded,
        Some(&fixture.database),
        Some("int-03-host"),
        Some("int-03-generation-b"),
    )?;
    let new_status = restarted.control_request("evidence.status", json!({}))?;
    check!(new_status["effectiveHistoryConfig"]["retentionDays"] == json!(3650));
    check!(new_status["effectiveHistoryConfig"]["maxStorageMiB"] == json!(4096));
    let app_wire = serde_json::to_string(&loaded).map_err(|error| error.to_string())?;
    let snapshot_wire = serde_json::to_string(&runtime_config_snapshot(&loaded))
        .map_err(|error| error.to_string())?;
    check!(!app_wire.contains(&fixture.database.display().to_string()));
    check!(!snapshot_wire.contains(&fixture.database.display().to_string()));
    check!(!snapshot_wire.contains("databasePath"));
    restarted.stop()?;
    Ok(())
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_05_evidence_store_public_boundary() {
    report_case("INT-05", run_int_05());
}

fn run_int_05() -> QaResult<()> {
    let _ = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-05")?;
    let source_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
        .join("evidence-store")
        .join("src");
    let forbidden = [
        "PolicyEngine",
        "ApprovalManager",
        "SessionGrantStore",
        "ExecutionPermit",
        "ToolExecutor",
        "tauri::",
        "AppHandle",
    ];
    for entry in fs::read_dir(&source_dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        for marker in forbidden {
            check!(
                !text.contains(marker),
                "evidence-store source boundary imports or constructs {marker}: {}",
                path.display()
            );
        }
    }
    let ui_manifest =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .map_err(|error| error.to_string())?;
    check!(!ui_manifest
        .lines()
        .any(|line| line.trim_start().starts_with("rusqlite")));

    let mut writer = EvidenceWriter::open(&fixture.database).map_err(|error| error.to_string())?;
    writer
        .open_session(&RuntimeSession {
            runtime_id: "int-05-runtime".into(),
            host_session_id: None,
            supervisor_generation_id: None,
            started_at_utc: "2026-11-01T00:00:00.000Z".into(),
            runtime_version: "m03-qa".into(),
            config_revision: "1".into(),
        })
        .map_err(|error| error.to_string())?;
    let event = event_fixture(
        "int-05-runtime",
        "int-05-request",
        "int-05-event",
        "read_file",
        "2026-11-01T00:00:01.000Z",
        EvidenceStage::ExecutionCompleted,
        EvidenceLifecycleState::Completed,
        HistoryOutcome::Success,
        Some(PolicyDecision::Allow),
        None,
        None,
        EvidenceCoverage::Complete,
        None,
        None,
    );
    check!(
        writer
            .record_event(&event)
            .map_err(|error| error.to_string())?
            .inserted
    );
    check!(
        !writer
            .record_event(&event)
            .map_err(|error| error.to_string())?
            .inserted
    );
    check!(!writer
        .connection_is_query_only()
        .map_err(|error| error.to_string())?);
    check!(writer.database_path() == fixture.database.as_path());
    check!(writer.lock_path().is_file());
    let paths = StorePaths::from_database(&fixture.database);
    check!(matches!(
        WriterLock::try_acquire(&paths.lock),
        Err(StoreError::WriterBusy { .. })
    ));
    let reader = EvidenceReader::open(&fixture.database).map_err(|error| error.to_string())?;
    check!(reader.is_initialized());
    check!(reader
        .connection_is_query_only()
        .map_err(|error| error.to_string())?);
    check!(
        reader
            .query(&HistoryQueryParams::default())
            .map_err(|error| error.to_string())?
            .items
            .len()
            == 1
    );
    check!(reader
        .get_required("int-05-runtime", "int-05-request")
        .is_ok());
    check!(
        reader
            .status()
            .map_err(|error| error.to_string())?
            .schema_version
            == Some(1)
    );
    drop(reader);
    drop(writer);
    let maintenance =
        OfflineMaintenance::open(&fixture.database).map_err(|error| error.to_string())?;
    check!(maintenance.database_path() == fixture.database.as_path());
    drop(maintenance);
    Ok(())
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_11_history_filter_matrix_and_keyset_pagination() {
    report_case("INT-11", run_int_11());
}

fn run_int_11() -> QaResult<()> {
    let _ = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-11")?;
    let mut events = Vec::new();
    for index in 0..205usize {
        let tool = if index % 2 == 0 { "read_file" } else { "write" };
        let (policy, outcome, stage, lifecycle) = match index % 5 {
            0 => (
                Some(PolicyDecision::Allow),
                HistoryOutcome::Success,
                EvidenceStage::ExecutionCompleted,
                EvidenceLifecycleState::Completed,
            ),
            1 => (
                Some(PolicyDecision::Deny),
                HistoryOutcome::Denied,
                EvidenceStage::RequestDenied,
                EvidenceLifecycleState::Denied,
            ),
            2 => (
                Some(PolicyDecision::Ask),
                HistoryOutcome::Success,
                EvidenceStage::ApprovalResolved,
                EvidenceLifecycleState::Completed,
            ),
            3 => (
                Some(PolicyDecision::Ask),
                HistoryOutcome::ApprovalExpired,
                EvidenceStage::ApprovalExpired,
                EvidenceLifecycleState::Failed,
            ),
            _ => (
                Some(PolicyDecision::Ask),
                HistoryOutcome::ApprovalCancelled,
                EvidenceStage::ApprovalCancelled,
                EvidenceLifecycleState::Failed,
            ),
        };
        let approval_decision = if index % 5 == 2 {
            Some(ApprovalDecision::AllowOnce)
        } else {
            None
        };
        let approval_id = matches!(
            stage,
            EvidenceStage::ApprovalResolved
                | EvidenceStage::ApprovalExpired
                | EvidenceStage::ApprovalCancelled
        )
        .then_some(format!("int-11-approval-{index}"));
        events.push(event_fixture(
            &format!("int-11-runtime-{index:03}"),
            &format!("int-11-request-{index:03}"),
            &format!("int-11-event-{index:03}"),
            tool,
            &format!("2026-11-01T00:{:02}:{:02}.000Z", index / 60, index % 60),
            stage,
            lifecycle,
            outcome,
            policy,
            approval_id.as_deref(),
            approval_decision,
            EvidenceCoverage::Complete,
            None,
            None,
        ));
    }
    persist_events(&fixture.database, &events)?;

    let default_page = history_query(&fixture.database, HistoryQueryParams::default())?;
    check!(
        default_page.items.len() == 50,
        "default History page was not 50"
    );
    let max_page = history_query(
        &fixture.database,
        HistoryQueryParams {
            limit: Some(200),
            ..HistoryQueryParams::default()
        },
    )?;
    check!(max_page.items.len() == 200, "max History page was not 200");
    let clamped_page = EvidenceReader::open(&fixture.database)
        .map_err(|error| error.to_string())?
        .query(&HistoryQueryParams {
            limit: Some(201),
            ..HistoryQueryParams::default()
        });
    check!(
        clamped_page.is_ok_and(|page| page.items.len() == 200),
        "limit > 200 was not clamped to the maximum page size"
    );

    let mut page_params = HistoryQueryParams {
        limit: Some(50),
        ..HistoryQueryParams::default()
    };
    let mut keys = Vec::new();
    loop {
        let page = history_query(&fixture.database, page_params.clone())?;
        keys.extend(
            page.items
                .iter()
                .map(|item| (item.runtime_id.clone(), item.request_id.clone())),
        );
        let Some(cursor) = page.next_cursor else {
            break;
        };
        page_params.cursor = Some(cursor);
    }
    check!(
        keys.len() == 205,
        "keyset pagination lost rows: {}",
        keys.len()
    );
    check!(
        keys.iter().collect::<BTreeSet<_>>().len() == 205,
        "keyset pagination duplicated a row"
    );
    check!(
        keys.windows(2).all(|window| window[0] != window[1]),
        "adjacent keyset page boundary duplicated a row"
    );

    let matrix_database = fixture.base.join("matrix-history.sqlite3");
    let matrix = vec![
        event_fixture(
            "matrix-runtime",
            "matrix-read-success",
            "matrix-event-1",
            "read_file",
            "2026-11-01T01:00:00.000Z",
            EvidenceStage::ExecutionCompleted,
            EvidenceLifecycleState::Completed,
            HistoryOutcome::Success,
            Some(PolicyDecision::Allow),
            None,
            None,
            EvidenceCoverage::Complete,
            None,
            None,
        ),
        event_fixture(
            "matrix-runtime",
            "matrix-write-denied",
            "matrix-event-2",
            "write",
            "2026-11-01T01:01:00.000Z",
            EvidenceStage::RequestDenied,
            EvidenceLifecycleState::Denied,
            HistoryOutcome::Denied,
            Some(PolicyDecision::Deny),
            None,
            None,
            EvidenceCoverage::Complete,
            None,
            None,
        ),
        event_fixture(
            "matrix-runtime",
            "matrix-approval-required",
            "matrix-event-3",
            "write",
            "2026-11-01T01:02:00.000Z",
            EvidenceStage::ApprovalRequired,
            EvidenceLifecycleState::AwaitingApproval,
            HistoryOutcome::ApprovalRequired,
            Some(PolicyDecision::Ask),
            Some("matrix-approval-required"),
            None,
            EvidenceCoverage::Complete,
            None,
            None,
        ),
        event_fixture(
            "matrix-runtime",
            "matrix-allow-once",
            "matrix-event-4",
            "write",
            "2026-11-01T01:03:00.000Z",
            EvidenceStage::ApprovalResolved,
            EvidenceLifecycleState::Completed,
            HistoryOutcome::Success,
            Some(PolicyDecision::Ask),
            Some("matrix-allow-once"),
            Some(ApprovalDecision::AllowOnce),
            EvidenceCoverage::Complete,
            None,
            None,
        ),
        event_fixture(
            "matrix-runtime",
            "matrix-allow-session",
            "matrix-event-5",
            "write",
            "2026-11-01T01:04:00.000Z",
            EvidenceStage::ApprovalResolved,
            EvidenceLifecycleState::Completed,
            HistoryOutcome::Success,
            Some(PolicyDecision::Ask),
            Some("matrix-allow-session"),
            Some(ApprovalDecision::AllowForSession),
            EvidenceCoverage::Complete,
            None,
            None,
        ),
        event_fixture(
            "matrix-runtime",
            "matrix-denied-approval",
            "matrix-event-6",
            "write",
            "2026-11-01T01:05:00.000Z",
            EvidenceStage::ApprovalResolved,
            EvidenceLifecycleState::Denied,
            HistoryOutcome::Denied,
            Some(PolicyDecision::Ask),
            Some("matrix-denied-approval"),
            Some(ApprovalDecision::Deny),
            EvidenceCoverage::Complete,
            None,
            None,
        ),
        event_fixture(
            "matrix-runtime",
            "matrix-expired",
            "matrix-event-7",
            "write",
            "2026-11-01T01:06:00.000Z",
            EvidenceStage::ApprovalExpired,
            EvidenceLifecycleState::Failed,
            HistoryOutcome::ApprovalExpired,
            Some(PolicyDecision::Ask),
            Some("matrix-expired"),
            None,
            EvidenceCoverage::Complete,
            None,
            None,
        ),
        event_fixture(
            "matrix-runtime",
            "matrix-cancelled",
            "matrix-event-8",
            "write",
            "2026-11-01T01:07:00.000Z",
            EvidenceStage::ApprovalCancelled,
            EvidenceLifecycleState::Failed,
            HistoryOutcome::ApprovalCancelled,
            Some(PolicyDecision::Ask),
            Some("matrix-cancelled"),
            None,
            EvidenceCoverage::Complete,
            None,
            None,
        ),
        event_fixture(
            "matrix-runtime",
            "matrix-glob-failed",
            "matrix-event-9",
            "glob",
            "2026-11-01T01:08:00.000Z",
            EvidenceStage::ExecutionFailed,
            EvidenceLifecycleState::Failed,
            HistoryOutcome::Failed,
            Some(PolicyDecision::Allow),
            None,
            None,
            EvidenceCoverage::Complete,
            None,
            None,
        ),
    ];
    persist_events(&matrix_database, &matrix)?;
    for (params, expected) in [
        (
            HistoryQueryParams {
                from_utc: Some("2026-11-01T01:03:00.000Z".into()),
                to_utc: Some("2026-11-01T01:06:00.000Z".into()),
                ..HistoryQueryParams::default()
            },
            3,
        ),
        (
            HistoryQueryParams {
                tool: Some("write".into()),
                ..HistoryQueryParams::default()
            },
            7,
        ),
        (
            HistoryQueryParams {
                outcome: Some(HistoryOutcome::Success),
                ..HistoryQueryParams::default()
            },
            3,
        ),
        (
            HistoryQueryParams {
                policy_decision: Some(PolicyDecision::Deny),
                ..HistoryQueryParams::default()
            },
            1,
        ),
        (
            HistoryQueryParams {
                approval_state: Some(HistoryApprovalState::AllowOnce),
                ..HistoryQueryParams::default()
            },
            1,
        ),
        (
            HistoryQueryParams {
                tool: Some("write".into()),
                policy_decision: Some(PolicyDecision::Ask),
                approval_state: Some(HistoryApprovalState::Denied),
                ..HistoryQueryParams::default()
            },
            1,
        ),
    ] {
        let result = history_query(&matrix_database, params)?;
        check!(
            result.items.len() == expected,
            "filter matrix returned {} not {expected}",
            result.items.len()
        );
    }
    let mut cursor_params = HistoryQueryParams {
        tool: Some("read_file".into()),
        limit: Some(1),
        ..HistoryQueryParams::default()
    };
    let first_filtered = history_query(&fixture.database, cursor_params.clone())?;
    let cursor = first_filtered
        .next_cursor
        .ok_or_else(|| "filter cursor was not emitted".to_owned())?;
    cursor_params.tool = Some("write".into());
    cursor_params.cursor = Some(cursor);
    let invalid = EvidenceReader::open(&fixture.database)
        .map_err(|error| error.to_string())?
        .query(&cursor_params);
    check!(matches!(invalid, Err(StoreError::InvalidCursor)));
    let query_source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("runtime")
            .join("evidence-store")
            .join("src")
            .join("query.rs"),
    )
    .map_err(|error| error.to_string())?;
    check!(!query_source.contains("OFFSET"));
    check!(query_source.contains("LIMIT ?"));
    Ok(())
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_13_history_facade_orders_detail_and_preserves_read_only_semantics() {
    report_case("INT-13", run_int_13());
}

fn run_int_13() -> QaResult<()> {
    let binary = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-13")?;
    let mut config = fixture.config();
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    let (mut runtime, mut mcp) = QaRuntime::start(
        &binary,
        &config,
        Some(&fixture.database),
        Some("int-13-host"),
        Some("int-13-generation"),
    )?;
    let complete = mcp.call_tool("read_file", json!({ "path": "sample.txt" }))?;
    assert_mcp_success(&complete)?;
    let complete_activity = runtime.activity_items()?;
    let (_, complete_runtime_id, complete_request_id) =
        activity_identity(&complete_activity, None)?;
    let _complete_detail = wait_for_history(
        &fixture.database,
        &complete_runtime_id,
        &complete_request_id,
    )?;

    let denied_call = mcp.pending_tool(
        "write",
        json!({ "path": "int-13-denied.txt", "content": "denied" }),
    );
    let denied_approval = wait_for_approval(&mut runtime)?;
    let denied_request_id = approval_request_id(&denied_approval)?;
    runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": denied_request_id, "decision": "deny" }),
    )?;
    assert_mcp_error(&denied_call.receive()?, "APPROVAL_DENIED")?;
    check!(!fixture.workspace.join("int-13-denied.txt").exists());
    let denied_activity = runtime.activity_items()?;
    let (_, denied_runtime_id, denied_activity_request_id) =
        activity_identity(&denied_activity, Some(&denied_request_id))?;
    check!(denied_activity_request_id == denied_request_id);
    let _denied_detail =
        wait_for_history(&fixture.database, &denied_runtime_id, &denied_request_id)?;

    let crash_call = mcp.pending_tool(
        "write",
        json!({ "path": "int-13-crash.txt", "content": "crash-only" }),
    );
    let crash_approval = wait_for_approval(&mut runtime)?;
    let crash_request_id = approval_request_id(&crash_approval)?;
    let crash_activity = runtime.activity_items()?;
    let (_, crash_runtime_id, crash_activity_request_id) =
        activity_identity(&crash_activity, Some(&crash_request_id))?;
    check!(crash_activity_request_id == crash_request_id);
    // Ensure the real writer has committed the open request before the
    // process is terminated, then exercise recovery in a fresh Runtime.
    let open_detail = wait_for_history(&fixture.database, &crash_runtime_id, &crash_request_id)?;
    check!(open_detail.summary.approval_state == HistoryApprovalState::RequiredObserved);
    check!(open_detail.summary.evidence_coverage == EvidenceCoverage::Incomplete);
    runtime.crash()?;
    drop(crash_call);

    let (mut recovery, _recovery_mcp) = QaRuntime::start(
        &binary,
        &config,
        Some(&fixture.database),
        Some("int-13-recovery-host"),
        Some("int-13-recovery-generation"),
    )?;
    recovery.stop()?;

    let query = history::query_at_for_test(
        fixture.database.clone(),
        &config,
        HistoryQueryParams::default(),
    )
    .map_err(|error| {
        format!(
            "Tauri History query facade failed: {} ({})",
            error.code, error.message
        )
    })?;
    check!(
        query.items.iter().any(|item| {
            item.runtime_id == complete_runtime_id && item.request_id == complete_request_id
        }),
        "complete request absent from facade query: {:?}",
        query.items
    );
    check!(
        query.items.iter().any(|item| {
            item.runtime_id == denied_runtime_id && item.request_id == denied_request_id
        }),
        "denied request absent from facade query: {:?}",
        query.items
    );
    check!(
        query.items.iter().any(|item| {
            item.runtime_id == crash_runtime_id
                && item.request_id == crash_request_id
                && item.terminal_state == harness_control_protocol::HistoryTerminalState::Incomplete
                && item.evidence_coverage == EvidenceCoverage::Incomplete
        }),
        "crash request not recovered as incomplete: {:?}",
        query.items
    );

    let complete_detail = history::get_at_for_test(
        fixture.database.clone(),
        &config,
        HistoryGetParams {
            runtime_id: complete_runtime_id.clone(),
            request_id: complete_request_id.clone(),
        },
    )
    .map_err(|error| {
        format!(
            "Tauri complete History get facade failed: {} ({})",
            error.code, error.message
        )
    })?;
    check!(!complete_detail.events.is_empty());
    check!(complete_detail.notice.is_none());
    check!(complete_detail
        .events
        .windows(2)
        .all(|events| { events[0].occurred_at_utc <= events[1].occurred_at_utc }));

    let denied_detail = history::get_at_for_test(
        fixture.database.clone(),
        &config,
        HistoryGetParams {
            runtime_id: denied_runtime_id.clone(),
            request_id: denied_request_id.clone(),
        },
    )
    .map_err(|error| {
        format!(
            "Tauri denied History get facade failed: {} ({})",
            error.code, error.message
        )
    })?;
    check!(denied_detail
        .terminal_approval
        .as_ref()
        .is_some_and(|approval| {
            approval.request_id == denied_request_id
                && approval.runtime_id == denied_runtime_id
                && approval.decision == Some(ApprovalDecision::Deny)
                && approval.terminal_state == HistoryApprovalState::Denied
        }));

    let incomplete_detail = history::get_at_for_test(
        fixture.database.clone(),
        &config,
        HistoryGetParams {
            runtime_id: crash_runtime_id.clone(),
            request_id: crash_request_id.clone(),
        },
    )
    .map_err(|error| {
        format!(
            "Tauri incomplete History get facade failed: {} ({})",
            error.code, error.message
        )
    })?;
    check!(
        incomplete_detail.summary.terminal_state
            == harness_control_protocol::HistoryTerminalState::Incomplete
    );
    check!(incomplete_detail.summary.evidence_coverage == EvidenceCoverage::Incomplete);
    check!(incomplete_detail
        .notice
        .as_ref()
        .is_some_and(|notice| { notice.coverage == EvidenceCoverage::Incomplete }));

    let missing_database = fixture.base.join("missing").join("history.sqlite3");
    let missing_result = history::query_at_for_test(
        missing_database.clone(),
        &config,
        HistoryQueryParams::default(),
    )
    .map_err(|error| {
        format!(
            "missing History query facade failed: {} ({})",
            error.code, error.message
        )
    })?;
    check!(missing_result.items.is_empty());
    check!(
        !missing_database.exists(),
        "missing History query created a database"
    );
    let missing_get = history::get_at_for_test(
        missing_database.clone(),
        &config,
        HistoryGetParams {
            runtime_id: "missing-runtime".into(),
            request_id: "missing-request".into(),
        },
    )
    .expect_err("missing History get unexpectedly returned a detail");
    check!(missing_get.code == "EVIDENCE_NOT_FOUND");
    check!(
        !missing_database.exists(),
        "missing History get created a database"
    );

    let before_read_only = store_file_snapshot(&fixture.database);
    let _ = history::query_at_for_test(
        fixture.database.clone(),
        &config,
        HistoryQueryParams::default(),
    )
    .map_err(|error| {
        format!(
            "second read-only query failed: {} ({})",
            error.code, error.message
        )
    })?;
    let _ = history::get_at_for_test(
        fixture.database.clone(),
        &config,
        HistoryGetParams {
            runtime_id: complete_runtime_id,
            request_id: complete_request_id,
        },
    )
    .map_err(|error| {
        format!(
            "second read-only get failed: {} ({})",
            error.code, error.message
        )
    })?;
    check!(before_read_only == store_file_snapshot(&fixture.database));
    check!(
        fs::read(fixture.workspace.join("sample.txt"))
            .map_err(|error| format!("read INT-15 workspace sentinel: {error}"))?
            == b"M03 sample content\n"
    );
    Ok(())
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_14_approval_terminal_matrix_is_observational_after_restart() {
    report_case("INT-14", run_int_14());
}

fn run_int_14() -> QaResult<()> {
    let binary = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-14")?;
    let mut config = fixture.config();
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    let (mut runtime, mut mcp) = QaRuntime::start(
        &binary,
        &config,
        Some(&fixture.database),
        Some("int-14-host"),
        Some("int-14-generation-a"),
    )?;
    let runtime_id = runtime.runtime_id().to_owned();

    let allow_once_call = mcp.pending_tool(
        "write",
        json!({ "path": "int-14-once.txt", "content": "allow once" }),
    );
    let allow_once_approval = wait_for_approval(&mut runtime)?;
    let allow_once_id = approval_request_id(&allow_once_approval)?;
    runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": allow_once_id, "decision": "allow_once" }),
    )?;
    assert_mcp_success(&allow_once_call.receive()?)?;
    check!(fixture.workspace.join("int-14-once.txt").is_file());
    let once_activity = runtime.activity_items()?;
    let (_, once_runtime_id, once_request_id) =
        activity_identity(&once_activity, Some(&allow_once_id))?;
    check!(once_runtime_id == runtime_id && once_request_id == allow_once_id);
    wait_for_history(&fixture.database, &runtime_id, &allow_once_id)?;

    let allow_session_call = mcp.pending_tool(
        "write",
        json!({ "path": "int-14-session.txt", "content": "allow session" }),
    );
    let allow_session_approval = wait_for_approval(&mut runtime)?;
    let allow_session_id = approval_request_id(&allow_session_approval)?;
    runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": allow_session_id, "decision": "allow_for_session" }),
    )?;
    assert_mcp_success(&allow_session_call.receive()?)?;
    check!(fixture.workspace.join("int-14-session.txt").is_file());
    // The exact same tool/scope is allowed by the memory-only session grant;
    // no second live ApprovalRequest may be created in this generation.
    let second_session = mcp.call_tool(
        "write",
        json!({ "path": "int-14-session.txt", "content": "session reuse" }),
    )?;
    assert_mcp_success(&second_session)?;
    check!(
        runtime.control_request("approvals.list", json!({ "status": "pending" }))?["items"]
            .as_array()
            .is_some_and(|items| items.is_empty())
    );
    let session_activity = runtime.activity_items()?;
    let (_, session_runtime_id, session_request_id) =
        activity_identity(&session_activity, Some(&allow_session_id))?;
    check!(session_runtime_id == runtime_id && session_request_id == allow_session_id);
    let session_detail = wait_for_history(&fixture.database, &runtime_id, &allow_session_id)?;
    check!(session_detail
        .terminal_approval
        .as_ref()
        .is_some_and(|approval| {
            approval.decision == Some(ApprovalDecision::AllowForSession)
                && approval.terminal_state == HistoryApprovalState::AllowForSession
        }));

    let denied_call = mcp.pending_tool(
        "write",
        json!({ "path": "int-14-denied.txt", "content": "must deny" }),
    );
    let denied_approval = wait_for_approval(&mut runtime)?;
    let denied_id = approval_request_id(&denied_approval)?;
    runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": denied_id, "decision": "deny" }),
    )?;
    assert_mcp_error(&denied_call.receive()?, "APPROVAL_DENIED")?;
    check!(!fixture.workspace.join("int-14-denied.txt").exists());
    wait_for_history(&fixture.database, &runtime_id, &denied_id)?;

    let crash_call = mcp.pending_tool(
        "write",
        json!({ "path": "int-14-crash.txt", "content": "crash only" }),
    );
    let crash_approval = wait_for_approval(&mut runtime)?;
    let crash_id = approval_request_id(&crash_approval)?;
    let _ = wait_for_history(&fixture.database, &runtime_id, &crash_id)?;
    runtime.crash()?;
    drop(crash_call);

    // A real restart recovers the unclean request and starts a new approval
    // authority.  Historical IDs remain evidence only.
    let (mut restarted, mut restarted_mcp) = QaRuntime::start(
        &binary,
        &config,
        Some(&fixture.database),
        Some("int-14-host-restarted"),
        Some("int-14-generation-b"),
    )?;
    check!(
        restarted.control_request("approvals.list", json!({ "status": "pending" }))?["items"]
            .as_array()
            .is_some_and(|items| items.is_empty())
    );
    for historical_id in [&allow_once_id, &allow_session_id, &denied_id, &crash_id] {
        let response = restarted.control_request_raw(
            "approvals.resolve",
            json!({ "requestId": historical_id, "decision": "allow_once" }),
        )?;
        check!(response.get("error").is_some());
    }

    let post_restart_call = restarted_mcp.pending_tool(
        "write",
        json!({ "path": "int-14-post-restart.txt", "content": "post restart" }),
    );
    let post_restart_approval = wait_for_approval(&mut restarted)?;
    let post_restart_id = approval_request_id(&post_restart_approval)?;
    check!(![
        allow_once_id.as_str(),
        allow_session_id.as_str(),
        denied_id.as_str(),
        crash_id.as_str(),
    ]
    .contains(&post_restart_id.as_str()));
    let historical_again = restarted.control_request_raw(
        "approvals.resolve",
        json!({ "requestId": crash_id, "decision": "deny" }),
    )?;
    check!(historical_again.get("error").is_some());
    check!(
        restarted.control_request("approvals.list", json!({ "status": "pending" }))?["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| {
                item.get("requestId").and_then(Value::as_str) == Some(post_restart_id.as_str())
            }))
    );
    restarted.control_request(
        "approvals.resolve",
        json!({ "requestId": post_restart_id, "decision": "deny" }),
    )?;
    assert_mcp_error(&post_restart_call.receive()?, "APPROVAL_DENIED")?;
    restarted.stop()?;

    // Expired and cancelled are persisted through the real public evidence
    // writer.  The live Runtime clock is intentionally not slept for five
    // minutes; only the transition clock is synthetic, while persistence and
    // restart above remain real.
    let synthetic_runtime = "int-14-terminal-matrix";
    let mut expired_required = event_fixture(
        synthetic_runtime,
        "expired-request",
        "expired-required",
        "write",
        "2026-11-01T00:10:00.000Z",
        EvidenceStage::ApprovalRequired,
        EvidenceLifecycleState::AwaitingApproval,
        HistoryOutcome::ApprovalRequired,
        Some(PolicyDecision::Ask),
        Some("expired-approval"),
        None,
        EvidenceCoverage::Incomplete,
        None,
        None,
    );
    expired_required.sanitized_arguments = Some(DurableSanitizedArgumentsEvidence::Write {
        logical_path: "expired.txt".into(),
    });
    let mut expired_terminal = event_fixture(
        synthetic_runtime,
        "expired-request",
        "expired-terminal",
        "write",
        "2026-11-01T00:10:01.000Z",
        EvidenceStage::ApprovalExpired,
        EvidenceLifecycleState::Failed,
        HistoryOutcome::ApprovalExpired,
        Some(PolicyDecision::Ask),
        Some("expired-approval"),
        None,
        EvidenceCoverage::Complete,
        None,
        None,
    );
    expired_terminal.sanitized_arguments = Some(DurableSanitizedArgumentsEvidence::Write {
        logical_path: "expired.txt".into(),
    });
    let mut cancelled_required = expired_required.clone();
    cancelled_required.request_id = "cancelled-request".into();
    cancelled_required.event_id = "cancelled-required".into();
    cancelled_required.approval_id = Some("cancelled-approval".into());
    cancelled_required.occurred_at_utc = "2026-11-01T00:11:00.000Z".into();
    cancelled_required.recorded_at_utc = cancelled_required.occurred_at_utc.clone();
    let mut cancelled_terminal = expired_terminal.clone();
    cancelled_terminal.request_id = "cancelled-request".into();
    cancelled_terminal.event_id = "cancelled-terminal".into();
    cancelled_terminal.approval_id = Some("cancelled-approval".into());
    cancelled_terminal.stage = EvidenceStage::ApprovalCancelled;
    cancelled_terminal.outcome = HistoryOutcome::ApprovalCancelled;
    cancelled_terminal.occurred_at_utc = "2026-11-01T00:11:01.000Z".into();
    cancelled_terminal.recorded_at_utc = cancelled_terminal.occurred_at_utc.clone();
    persist_events(
        &fixture.database,
        &[
            expired_required,
            expired_terminal,
            cancelled_required,
            cancelled_terminal,
        ],
    )?;

    for (request_id, expected_state, expected_outcome) in [
        (
            allow_once_id.as_str(),
            HistoryApprovalState::AllowOnce,
            HistoryOutcome::Success,
        ),
        (
            allow_session_id.as_str(),
            HistoryApprovalState::AllowForSession,
            HistoryOutcome::Success,
        ),
        (
            denied_id.as_str(),
            HistoryApprovalState::Denied,
            HistoryOutcome::Denied,
        ),
        (
            "expired-request",
            HistoryApprovalState::Expired,
            HistoryOutcome::ApprovalExpired,
        ),
        (
            "cancelled-request",
            HistoryApprovalState::Cancelled,
            HistoryOutcome::ApprovalCancelled,
        ),
    ] {
        let detail = history_get(&fixture.database, &runtime_id, request_id)?
            .or_else(|| {
                history_get(&fixture.database, synthetic_runtime, request_id)
                    .ok()
                    .flatten()
            })
            .ok_or_else(|| format!("approval matrix detail missing for {request_id}"))?;
        check!(detail.summary.approval_state == expected_state);
        check!(detail.summary.outcome == Some(expected_outcome));
        check!(detail.terminal_approval.as_ref().is_some_and(|approval| {
            approval.terminal_state == expected_state && approval.request_id == request_id
        }));
    }
    let crash_detail = history_get(&fixture.database, &runtime_id, &crash_id)?
        .ok_or_else(|| "crash-only approval history disappeared".to_owned())?;
    check!(
        crash_detail.summary.terminal_state
            == harness_control_protocol::HistoryTerminalState::Incomplete
    );
    check!(crash_detail.terminal_approval.is_none());
    let serialized_history =
        serde_json::to_string(&crash_detail).map_err(|error| error.to_string())?;
    for forbidden in ["ApprovalRequest", "SessionGrant", "ExecutionPermit"] {
        check!(!serialized_history.contains(forbidden));
    }
    Ok(())
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_15_history_clear_barrier_retains_open_requests_and_releases_offline() {
    report_case("INT-15", run_int_15());
}

fn run_int_15() -> QaResult<()> {
    let binary = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-15")?;
    let mut config = fixture.config();
    config.policies.tool_defaults.write = PolicyDecision::Ask;
    let (mut runtime, mut mcp) = QaRuntime::start(
        &binary,
        &config,
        Some(&fixture.database),
        Some("int-15-host"),
        Some("int-15-generation"),
    )?;
    let terminal_result = mcp.call_tool("read_file", json!({ "path": "sample.txt" }))?;
    assert_mcp_success(&terminal_result)?;
    let terminal_activity = runtime.activity_items()?;
    let (_, runtime_id, terminal_request_id) = activity_identity(&terminal_activity, None)?;
    wait_for_history(&fixture.database, &runtime_id, &terminal_request_id)?;

    let open_call = mcp.pending_tool(
        "write",
        json!({ "path": "int-15-open.txt", "content": "must remain open" }),
    );
    let open_approval = wait_for_approval(&mut runtime)?;
    let open_request_id = approval_request_id(&open_approval)?;
    let open_before = wait_for_history(&fixture.database, &runtime_id, &open_request_id)?;
    check!(
        open_before.summary.terminal_state == harness_control_protocol::HistoryTerminalState::Open
    );
    let pending_before =
        runtime.control_request("approvals.list", json!({ "status": "pending" }))?;
    check!(pending_before["items"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.get("requestId").and_then(Value::as_str) == Some(open_request_id.as_str())
        })
    }));

    let status_value = runtime.control_request("evidence.status", json!({}))?;
    let status: harness_control_protocol::EvidenceStoreStatus =
        serde_json::from_value(status_value).map_err(|error| error.to_string())?;
    let online_clear =
        history::clear_online_at_for_test(fixture.database.clone(), &config, status, || {
            let value = runtime
                .control_request("history.clear", json!({}))
                .map_err(|error| crate::commands::ControlError {
                    code: "EVIDENCE_IO".into(),
                    message: error,
                })?;
            serde_json::from_value(value).map_err(|error| crate::commands::ControlError {
                code: "EVIDENCE_IO".into(),
                message: format!("decode Runtime history.clear result: {error}"),
            })
        })
        .map_err(|error| {
            format!(
                "Tauri online History clear failed: {} ({})",
                error.code, error.message
            )
        })?;
    check!(online_clear.deleted_request_count >= 1);
    check!(online_clear.deleted_event_count >= 1);
    check!(history_get(&fixture.database, &runtime_id, &terminal_request_id)?.is_none());
    let open_after = history_get(&fixture.database, &runtime_id, &open_request_id)?
        .ok_or_else(|| "online clear deleted the open request".to_owned())?;
    check!(
        open_after.summary.terminal_state == harness_control_protocol::HistoryTerminalState::Open
    );
    let pending_after =
        runtime.control_request("approvals.list", json!({ "status": "pending" }))?;
    check!(pending_after == pending_before);

    // New evidence after the barrier is allowed to reappear in the same live
    // writer, while the open approval remains untouched.
    let after_clear = mcp.call_tool("read_file", json!({ "path": "sample.txt" }))?;
    assert_mcp_success(&after_clear)?;
    let after_activity = runtime.activity_items()?;
    let after_request_id = request_id_from_activity(
        &after_activity,
        &[terminal_request_id.as_str(), open_request_id.as_str()],
    )?;
    wait_for_history(&fixture.database, &runtime_id, &after_request_id)?;
    check!(history_get(&fixture.database, &runtime_id, &after_request_id)?.is_some());
    check!(
        runtime.control_request("approvals.list", json!({ "status": "pending" }))?["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| {
                item.get("requestId").and_then(Value::as_str) == Some(open_request_id.as_str())
            }))
    );

    // Offline clear is rejected while the real Runtime still owns the writer.
    let busy = history::clear_offline_at_for_test(fixture.database.clone(), &config)
        .expect_err("offline clear acquired the live Runtime writer lock");
    check!(
        busy.code == "EVIDENCE_BUSY",
        "unexpected busy error: {}",
        busy.code
    );

    runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": open_request_id, "decision": "deny" }),
    )?;
    assert_mcp_error(&open_call.receive()?, "APPROVAL_DENIED")?;
    runtime.stop()?;

    let offline_clear = history::clear_offline_at_for_test(fixture.database.clone(), &config)
        .map_err(|error| {
            format!(
                "Tauri offline History clear failed: {} ({})",
                error.code, error.message
            )
        })?;
    check!(offline_clear.deleted_request_count >= 2);
    check!(fixture.database.is_file());
    let schema_status = EvidenceReader::open(&fixture.database)
        .map_err(|error| error.to_string())?
        .status()
        .map_err(|error| error.to_string())?;
    check!(schema_status.schema_version == Some(1));
    check!(
        history_query(&fixture.database, HistoryQueryParams::default())?
            .items
            .is_empty()
    );
    check!(
        fs::read(fixture.workspace.join("sample.txt")).map_err(|error| error.to_string())?
            == b"M03 sample content\n"
    );
    check!(!fixture.workspace.join("int-15-open.txt").exists());
    let history_source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("history.rs"),
    )
    .map_err(|error| format!("read INT-15 history facade source: {error}"))?;
    check!(history_source.contains("runtime.history_clear()"));
    Ok(())
}

#[test]
#[ignore = "M03 formal Integration QA; run via ui/scripts/m03-integration.ps1"]
fn m03_int_18_m01_m02_v3_regression_and_repository_boundary() {
    report_case("INT-18", run_int_18());
}

fn run_int_18() -> QaResult<()> {
    let binary = debug_runtime_binary()?;
    let fixture = QaFixture::new("int-18")?;
    let config = fixture.config();
    let (mut runtime, mut mcp) = m2_e2e::RuntimeProcess::start(&binary, &config, "M03-INT-18")
        .map_err(|error| error.to_string())?;
    let tools = mcp.list_tools().map_err(|error| error.to_string())?;
    let tool_names = tools["result"]["tools"]
        .as_array()
        .ok_or_else(|| "M02 real tools/list was not an array".to_owned())?
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    check!(tool_names.contains("read_file"));
    check!(tool_names.contains("glob"));
    check!(tool_names.contains("write"));
    check!(tool_names.contains("edit"));
    let write = mcp
        .call_tool(
            "write",
            json!({ "path": "int-18-regression.txt", "content": "M02 regression" }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_success(&write)?;
    let edit = mcp
        .call_tool(
            "edit",
            json!({
                "path": "int-18-regression.txt",
                "oldText": "M02 regression",
                "newText": "M02 regression edited"
            }),
        )
        .map_err(|error| error.to_string())?;
    assert_mcp_success(&edit)?;
    let read = mcp
        .call_tool("read_file", json!({ "path": "int-18-regression.txt" }))
        .map_err(|error| error.to_string())?;
    assert_mcp_success(&read)?;
    check!(read["result"]["structuredContent"]["content"] == json!("M02 regression edited"));
    check!(
        fs::read(fixture.workspace.join("int-18-regression.txt"))
            .map_err(|error| error.to_string())?
            == b"M02 regression edited"
    );
    runtime.stop().map_err(|error| error.to_string())?;

    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let layout_script = repository_root
        .join("ui")
        .join("scripts")
        .join("check-repository-layout.ps1");
    let layout = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            layout_script.to_string_lossy().as_ref(),
        ])
        .current_dir(&repository_root)
        .output()
        .map_err(|error| format!("run repository layout script: {error}"))?;
    check!(
        layout.status.success(),
        "repository layout script failed: {}",
        String::from_utf8_lossy(&layout.stderr)
    );
    check!(String::from_utf8_lossy(&layout.stdout).contains("PASS"));
    check!(repository_root
        .join("runtime")
        .join("evidence-store")
        .is_dir());

    for filter in [
        "runtime::m01_integration::activity_sink_dispatch_does_not_block_runtime_reader",
        "runtime::m01_integration::activity_sink_preserves_host_refresh_order",
        "config::m01_contract::desktop_mutation_is_focused_revision_neutral_and_idempotent",
        "config::m00_contract::production_config_is_v3_and_wire_protocol_remains_v1",
        "startup::tests::successful_attempt_has_fixed_phases_and_monotonic_offsets",
        "diagnostics::tests::safe_evidence_is_bounded_and_control_free",
        "desktop::notifications::tests::identity_is_runtime_and_request_and_distinct_approvals_notify_twice",
        "harness::tests::stale_runtime_generation_cannot_change_a_new_session",
        "config::tests::secret_values_are_not_serialized",
        "secrets::tests::secret_reference_contains_only_locator_data",
    ] {
        run_cargo_regression_test(&repository_root, filter)?;
    }
    for filter in [
        "workspace::guard::tests::parent_traversal_outside_workspace_is_rejected",
        "execution::tests::disabled_tool_is_rejected_before_workspace_policy_or_executor",
        "policy::tests::typed_user_rules_aggregate_deny_ask_allow_in_stable_order",
        "approval::tests::registers_lists_filters_and_resolves_each_decision",
        "session_grant::tests::matches_only_current_generation_tool_and_exact_scope",
        "bash::runtime::tests::startup_environment_projection_removes_known_injection_entries_only",
        "mcp::service::tests::allow_for_session_creates_a_narrow_grant_for_later_same_scope_calls",
        "evidence::service::tests::bounded_stop_drains_enqueued_evidence_and_closes_open_requests_as_incomplete",
    ] {
        run_runtime_regression_test(&repository_root, filter)?;
    }
    check!(harness_control_protocol::PROTOCOL_VERSION == 1);
    run_protocol_contract_regression(&repository_root)?;
    let m02_check = Command::new("git")
        .args([
            "diff",
            "--quiet",
            "1224fa9",
            "--",
            "docs-codex/v4/evidence/M02-startup-performance-diagnostics",
            "docs-codex/v4/milestones/M02-startup-performance-diagnostics",
        ])
        .current_dir(&repository_root)
        .status()
        .map_err(|error| format!("check frozen M02 evidence diff: {error}"))?;
    check!(
        m02_check.success(),
        "frozen M02 performance evidence changed during M03 QA"
    );
    Ok(())
}

fn run_protocol_contract_regression(repository_root: &Path) -> QaResult<()> {
    let output = Command::new("cargo")
        .args([
            "test",
            "--manifest-path",
            "protocol/Cargo.toml",
            "--test",
            "contracts",
            "history_v1_contract_is_additive_closed_and_has_no_live_authority_shape",
            "--",
            "--exact",
        ])
        .current_dir(repository_root)
        .output()
        .map_err(|error| format!("run protocol v1 additive contract: {error}"))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    check!(
        output.status.success() && combined.contains("1 passed"),
        "protocol v1 additive contract did not pass:\n{}",
        combined.chars().take(4000).collect::<String>()
    );
    Ok(())
}

fn run_cargo_regression_test(repository_root: &Path, filter: &str) -> QaResult<()> {
    let output = Command::new("cargo")
        .args([
            "test",
            "--manifest-path",
            "ui/src-tauri/Cargo.toml",
            "--lib",
            filter,
            "--",
            "--nocapture",
        ])
        .current_dir(repository_root)
        .output()
        .map_err(|error| format!("run regression {filter}: {error}"))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    check!(
        output.status.success(),
        "regression {filter} failed:\n{}",
        combined.chars().take(4000).collect::<String>()
    );
    check!(
        combined.contains("1 passed") || combined.contains("2 passed"),
        "regression {filter} ran no passing test:\n{}",
        combined.chars().take(2000).collect::<String>()
    );
    Ok(())
}

fn run_runtime_regression_test(repository_root: &Path, filter: &str) -> QaResult<()> {
    let output = Command::new("cargo")
        .args([
            "test",
            "--manifest-path",
            "runtime/Cargo.toml",
            filter,
            "--",
            "--exact",
            "--nocapture",
        ])
        .current_dir(repository_root)
        .output()
        .map_err(|error| format!("run runtime regression {filter}: {error}"))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    check!(
        output.status.success() && combined.contains("1 passed"),
        "runtime regression {filter} failed or matched no exact test:\n{}",
        combined.chars().take(4000).collect::<String>()
    );
    Ok(())
}
