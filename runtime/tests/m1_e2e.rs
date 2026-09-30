use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

type E2eResult<T> = Result<T, E2eError>;

#[derive(Debug)]
struct E2eError(String);

impl E2eError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for E2eError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for E2eError {}

macro_rules! check {
    ($condition:expr, $($argument:tt)*) => {
        if !$condition {
            return Err(E2eError::new(format!($($argument)*)));
        }
    };
}

#[test]
fn m1_local_black_box_e2e() {
    if let Err(error) = run_suite() {
        panic!("M1 local black-box E2E failed: {error}");
    }
}

fn run_suite() -> E2eResult<()> {
    let fixture = Fixture::new()?;
    let runtime = RuntimeProcess::start()?;
    let mut context = E2eContext {
        fixture,
        runtime,
        endpoint: None,
        mcp: None,
    };
    let mut failures = Vec::new();

    report_case("E2E-01", run_e2e_01(&mut context), &mut failures);
    report_case("E2E-02", run_e2e_02(&mut context), &mut failures);
    report_case("E2E-03", run_e2e_03(&mut context), &mut failures);
    report_case("E2E-04", run_e2e_04(&mut context), &mut failures);
    report_case("E2E-05", run_e2e_05(&mut context), &mut failures);
    report_case("E2E-06", run_e2e_06(&mut context), &mut failures);
    report_case("E2E-07", run_e2e_07(&mut context), &mut failures);
    report_case("E2E-08", run_e2e_08(&mut context), &mut failures);

    if context.runtime.is_running()? {
        context.runtime.force_cleanup()?;
    }
    if context.fixture.base.exists() {
        context.fixture.cleanup()?;
    }

    if failures.is_empty() {
        println!("LOCAL_BLACK_BOX_E2E=PASS");
        Ok(())
    } else {
        Err(E2eError::new(format!(
            "failed cases: {}",
            failures.join(", ")
        )))
    }
}

fn report_case(case_id: &str, result: E2eResult<()>, failures: &mut Vec<String>) {
    match result {
        Ok(()) => println!("{case_id} PASS"),
        Err(error) => {
            println!("{case_id} FAIL: {error}");
            failures.push(case_id.to_owned());
        }
    }
}

struct E2eContext {
    fixture: Fixture,
    runtime: RuntimeProcess,
    endpoint: Option<Endpoint>,
    mcp: Option<McpClient>,
}

impl E2eContext {
    fn activity(&mut self) -> E2eResult<ActivitySnapshot> {
        let result = self.runtime.control_request(
            "activity.list",
            json!({
                "limit": 256
            }),
        )?;
        let items = result
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| E2eError::new("activity.list result has no items array"))?
            .clone();
        let latest = decimal_value(result.get("latestSequence"), "latestSequence")?;
        Ok(ActivitySnapshot { items, latest })
    }

    fn mcp(&mut self) -> E2eResult<&mut McpClient> {
        self.mcp
            .as_mut()
            .ok_or_else(|| E2eError::new("MCP client is not initialized"))
    }
}

struct ActivitySnapshot {
    items: Vec<Value>,
    latest: u64,
}

struct Fixture {
    base: PathBuf,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> E2eResult<Self> {
        let base =
            std::env::temp_dir().join(format!("chatgpt-harness-m1-e2e-{}", uuid::Uuid::new_v4()));
        let workspace = base.join("workspace");
        let outside = base.join("outside");
        fs::create_dir_all(&workspace).map_err(|error| {
            E2eError::new(format!("create disposable workspace failed: {error}"))
        })?;
        fs::create_dir_all(&outside).map_err(|error| {
            E2eError::new(format!(
                "create disposable outside directory failed: {error}"
            ))
        })?;
        fs::write(workspace.join("duplicate.txt"), b"same same")
            .map_err(|error| E2eError::new(format!("create duplicate fixture failed: {error}")))?;
        fs::write(outside.join("outside.txt"), b"must not change")
            .map_err(|error| E2eError::new(format!("create outside fixture failed: {error}")))?;
        Ok(Self {
            base,
            workspace,
            outside,
        })
    }

    fn cleanup(&self) -> E2eResult<()> {
        if self.base.exists() {
            fs::remove_dir_all(&self.base)
                .map_err(|error| E2eError::new(format!("cleanup fixture failed: {error}")))?;
        }
        check!(
            !self.base.exists(),
            "disposable fixture still exists after cleanup"
        );
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

struct RuntimeProcess {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<String>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<String>>,
    next_request_id: u64,
}

impl RuntimeProcess {
    fn start() -> E2eResult<Self> {
        let binary = std::env::var_os("HARNESS_E2E_RUNTIME_BIN")
            .map(PathBuf::from)
            .or_else(|| option_env!("CARGO_BIN_EXE_chatgpt-harness-runtime").map(PathBuf::from))
            .ok_or_else(|| {
                E2eError::new(
                    "CARGO_BIN_EXE_chatgpt-harness-runtime is unavailable; build the release runtime first",
                )
            })?;
        check!(
            binary.is_file(),
            "Runtime binary does not exist: {}",
            binary.display()
        );

        let mut child = Command::new(&binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| E2eError::new(format!("spawn Runtime binary failed: {error}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| E2eError::new("Runtime stdin was not piped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| E2eError::new("Runtime stdout was not piped"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| E2eError::new("Runtime stderr was not piped"))?;

        let (sender, messages) = mpsc::channel();
        let stdout_thread = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        let stderr_thread = thread::spawn(move || {
            let mut output = String::new();
            let mut reader = BufReader::new(stderr);
            let _ = reader.read_to_string(&mut output);
            output
        });

        let process_id = child.id();
        println!("E2E_RUNTIME_BINARY={}", binary.display());
        println!("E2E_RUNTIME_PID={process_id}");
        println!("E2E_RUNTIME_TRANSPORT=stdin/stdout Control RPC + loopback MCP HTTP");
        Ok(Self {
            child,
            stdin,
            messages,
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            next_request_id: 0,
        })
    }

    fn control_request(&mut self, method: &str, params: Value) -> E2eResult<Value> {
        self.next_request_id += 1;
        let id = format!("m1-e2e-{}", self.next_request_id);
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
                            E2eError::new(format!("Control RPC {method} timed out"))
                        }
                        RecvTimeoutError::Disconnected => E2eError::new(format!(
                            "Runtime stdout closed during Control RPC {method}"
                        )),
                    })?;
            let message: Value = serde_json::from_str(&line)
                .map_err(|error| E2eError::new(format!("invalid Runtime Control JSON: {error}")))?;
            if message.get("id") != Some(&Value::String(id.clone())) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(E2eError::new(format!(
                    "Control RPC {method} returned error: {error}"
                )));
            }
            return message
                .get("result")
                .cloned()
                .ok_or_else(|| E2eError::new(format!("Control RPC {method} has no result")));
        }
    }

    fn control_notification(&mut self, method: &str, params: Value) -> E2eResult<()> {
        self.write_json_line(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    fn write_json_line(&mut self, value: &Value) -> E2eResult<()> {
        serde_json::to_writer(&mut self.stdin, value)
            .map_err(|error| E2eError::new(format!("serialize Control RPC failed: {error}")))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|error| E2eError::new(format!("write Control RPC failed: {error}")))?;
        self.stdin
            .flush()
            .map_err(|error| E2eError::new(format!("flush Control RPC failed: {error}")))
    }

    fn graceful_stop(&mut self) -> E2eResult<()> {
        let result = self.control_request("runtime.stop", json!({}))?;
        check!(
            result.get("accepted") == Some(&Value::Bool(true)),
            "runtime.stop did not return accepted=true: {result}"
        );
        self.wait_for_exit()
    }

    fn wait_for_exit(&mut self) -> E2eResult<()> {
        let deadline = Instant::now() + CONTROL_TIMEOUT;
        let status =
            loop {
                if let Some(status) = self.child.try_wait().map_err(|error| {
                    E2eError::new(format!("poll Runtime process failed: {error}"))
                })? {
                    break status;
                }
                if Instant::now() >= deadline {
                    return Err(E2eError::new(
                        "Runtime process did not exit after graceful stop",
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            };
        self.join_output_threads();
        check!(
            status.success(),
            "Runtime process exited unsuccessfully: {status}"
        );
        println!("E2E_RUNTIME_EXIT=success");
        Ok(())
    }

    fn is_running(&mut self) -> E2eResult<bool> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|error| E2eError::new(format!("poll Runtime process failed: {error}")))
    }

    fn force_cleanup(&mut self) -> E2eResult<()> {
        if self.is_running()? {
            self.child.kill().map_err(|error| {
                E2eError::new(format!("kill leaked Runtime process failed: {error}"))
            })?;
            self.child.wait().map_err(|error| {
                E2eError::new(format!("wait leaked Runtime process failed: {error}"))
            })?;
        }
        self.join_output_threads();
        Ok(())
    }

    fn join_output_threads(&mut self) {
        if let Some(thread) = self.stdout_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.stderr_thread.take() {
            let stderr = thread.join().unwrap_or_default();
            if !stderr.trim().is_empty() {
                println!("E2E_RUNTIME_STDERR={}", stderr.trim());
            }
        }
    }
}

impl Drop for RuntimeProcess {
    fn drop(&mut self) {
        let _ = self.force_cleanup();
    }
}

#[derive(Clone, Copy)]
struct Endpoint {
    address: SocketAddr,
}

struct McpClient {
    endpoint: Endpoint,
    session_id: String,
    next_request_id: u64,
}

impl McpClient {
    fn connect(endpoint: Endpoint) -> E2eResult<Self> {
        let response = http_json_request(
            endpoint.address,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "m1-e2e-runner", "version": "1" }
                }
            }),
            None,
        )?;
        check!(
            response.status == 200,
            "MCP initialize returned HTTP {}: {:?}",
            response.status,
            response.body
        );
        let session_id = response
            .header("mcp-session-id")
            .ok_or_else(|| E2eError::new("MCP initialize did not return Mcp-Session-Id"))?
            .to_owned();
        check!(!session_id.is_empty(), "MCP session id is empty");
        Ok(Self {
            endpoint,
            session_id,
            next_request_id: 1,
        })
    }

    fn list_tools(&mut self) -> E2eResult<Value> {
        self.request("tools/list", json!({}))
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> E2eResult<Value> {
        self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        )
    }

    fn request(&mut self, method: &str, params: Value) -> E2eResult<Value> {
        self.next_request_id += 1;
        let response = http_json_request(
            self.endpoint.address,
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
            "MCP {method} returned HTTP {}: {:?}",
            response.status,
            response.body
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

    fn json_rpc(&self) -> E2eResult<Value> {
        if let Ok(value) = serde_json::from_slice(&self.body) {
            return Ok(value);
        }
        let body = std::str::from_utf8(&self.body)
            .map_err(|error| E2eError::new(format!("MCP response is not UTF-8: {error}")))?;
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data:") {
                if let Ok(value) = serde_json::from_str(data.trim()) {
                    return Ok(value);
                }
            }
        }
        Err(E2eError::new(format!(
            "MCP response did not contain JSON-RPC data: {body}"
        )))
    }
}

fn http_json_request(
    address: SocketAddr,
    message: Value,
    session_id: Option<&str>,
) -> E2eResult<HttpResponse> {
    let body = serde_json::to_vec(&message)
        .map_err(|error| E2eError::new(format!("serialize MCP request failed: {error}")))?;
    let mut stream = TcpStream::connect_timeout(&address, HTTP_TIMEOUT)
        .map_err(|error| E2eError::new(format!("connect loopback MCP endpoint failed: {error}")))?;
    stream
        .set_read_timeout(Some(HTTP_TIMEOUT))
        .map_err(|error| E2eError::new(format!("set MCP read timeout failed: {error}")))?;
    stream
        .set_write_timeout(Some(HTTP_TIMEOUT))
        .map_err(|error| E2eError::new(format!("set MCP write timeout failed: {error}")))?;

    let mut request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\nAccept: application/json, text/event-stream\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
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
        .map_err(|error| E2eError::new(format!("write MCP request failed: {error}")))?;

    let mut raw_response = Vec::new();
    stream
        .read_to_end(&mut raw_response)
        .map_err(|error| E2eError::new(format!("read MCP response failed: {error}")))?;
    parse_http_response(&raw_response)
}

fn parse_http_response(raw: &[u8]) -> E2eResult<HttpResponse> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| E2eError::new("MCP response has no HTTP header terminator"))?;
    let header_text = std::str::from_utf8(&raw[..header_end])
        .map_err(|error| E2eError::new(format!("MCP response headers are not UTF-8: {error}")))?;
    let mut lines = header_text.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| E2eError::new("MCP response has no HTTP status"))?
        .parse::<u16>()
        .map_err(|error| E2eError::new(format!("invalid MCP HTTP status: {error}")))?;
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect::<BTreeMap<_, _>>();
    let wire_body = &raw[header_end + 4..];
    let body = if headers
        .get("transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        decode_chunked(wire_body)?
    } else {
        wire_body.to_vec()
    };
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn decode_chunked(mut wire: &[u8]) -> E2eResult<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line_end = wire
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| E2eError::new("invalid chunked MCP response"))?;
        let length_text = std::str::from_utf8(&wire[..line_end])
            .map_err(|error| E2eError::new(format!("invalid chunk length: {error}")))?
            .split(';')
            .next()
            .unwrap_or_default();
        let length = usize::from_str_radix(length_text.trim(), 16)
            .map_err(|error| E2eError::new(format!("invalid chunk size: {error}")))?;
        wire = &wire[line_end + 2..];
        if length == 0 {
            return Ok(body);
        }
        check!(wire.len() >= length + 2, "truncated chunked MCP response");
        body.extend_from_slice(&wire[..length]);
        check!(
            &wire[length..length + 2] == b"\r\n",
            "invalid chunk terminator in MCP response"
        );
        wire = &wire[length + 2..];
    }
}

fn run_e2e_01(context: &mut E2eContext) -> E2eResult<()> {
    let initialize = context.runtime.control_request(
        "initialize",
        json!({
            "clientInfo": { "name": "m1-e2e-runner", "title": "M1 E2E", "version": "1" },
            "protocolVersion": 1,
            "capabilities": {},
            "runtimeConfig": {
                "configRevision": "1",
                "workspace": { "rootPath": context.fixture.workspace.to_string_lossy() },
                "security": { "mode": "normal", "developerTestMode": false },
                "runtimeLimits": {
                    "maxConcurrentRequests": 1,
                    "maxActivityEvents": 256,
                    "maxReadBytes": 1048576
                }
            }
        }),
    )?;
    check!(
        initialize
            .get("serverInfo")
            .and_then(|value| value.get("name"))
            == Some(&Value::String("chatgpt-harness-runtime".into())),
        "initialize did not identify chatgpt-harness-runtime: {initialize}"
    );
    let runtime_id = initialize
        .get("runtimeId")
        .and_then(Value::as_str)
        .ok_or_else(|| E2eError::new("initialize did not return runtimeId"))?;
    check!(
        !runtime_id.is_empty(),
        "initialize returned an empty runtimeId"
    );

    context
        .runtime
        .control_notification("initialized", json!({}))?;
    let endpoint_result = context
        .runtime
        .control_request("mcp.endpoint.get", json!({}))?;
    let endpoint_url = endpoint_result
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| E2eError::new("mcp.endpoint.get did not return url"))?;
    let endpoint = parse_endpoint(endpoint_url)?;
    println!("E2E_MCP_ENDPOINT=loopback:{}", endpoint.address.port());
    context.endpoint = Some(endpoint);

    let mut mcp = McpClient::connect(endpoint)?;
    let list = mcp.list_tools()?;
    let tools = list
        .get("result")
        .and_then(|value| value.get("tools"))
        .and_then(Value::as_array)
        .ok_or_else(|| E2eError::new("tools/list did not return result.tools"))?;
    for required in ["read_file", "glob", "write", "edit"] {
        check!(
            tools
                .iter()
                .any(|tool| tool.get("name").and_then(Value::as_str) == Some(required)),
            "tools/list is missing required tool {required}: {list}"
        );
    }
    context.mcp = Some(mcp);

    let activity = context.activity()?;
    check!(
        activity
            .items
            .iter()
            .any(|item| item.get("eventType").and_then(Value::as_str) == Some("runtime_started")),
        "activity.list did not expose RuntimeStarted evidence: {:?}",
        activity.items
    );
    Ok(())
}

fn run_e2e_02(context: &mut E2eContext) -> E2eResult<()> {
    let before = context.activity()?.latest;
    let result = context.mcp()?.call_tool(
        "write",
        json!({ "path": "m1-e2e.txt", "content": "hello from M1" }),
    )?;
    assert_mcp_success(&result)?;
    assert_bytes(
        &context.fixture.workspace.join("m1-e2e.txt"),
        b"hello from M1",
    )?;
    assert_no_temp_residue(&context.fixture)?;
    let after = context.activity()?;
    assert_success_activity(&after.items, before, "write")
}

fn run_e2e_03(context: &mut E2eContext) -> E2eResult<()> {
    let before = context.activity()?.latest;
    let result = context.mcp()?.call_tool(
        "write",
        json!({ "path": "m1-e2e.txt", "content": "hello from M1 overwrite" }),
    )?;
    assert_mcp_success(&result)?;
    assert_bytes(
        &context.fixture.workspace.join("m1-e2e.txt"),
        b"hello from M1 overwrite",
    )?;
    assert_no_temp_residue(&context.fixture)?;
    let after = context.activity()?;
    assert_success_activity(&after.items, before, "write")
}

fn run_e2e_04(context: &mut E2eContext) -> E2eResult<()> {
    let before_edit = context.activity()?.latest;
    let edit = context.mcp()?.call_tool(
        "edit",
        json!({
            "path": "m1-e2e.txt",
            "oldText": "hello from M1 overwrite",
            "newText": "hello from M1 hardened"
        }),
    )?;
    assert_mcp_success(&edit)?;
    assert_bytes(
        &context.fixture.workspace.join("m1-e2e.txt"),
        b"hello from M1 hardened",
    )?;
    assert_no_temp_residue(&context.fixture)?;
    let after_edit = context.activity()?;
    assert_success_activity(&after_edit.items, before_edit, "edit")?;

    let before_read = after_edit.latest;
    let read = context
        .mcp()?
        .call_tool("read_file", json!({ "path": "m1-e2e.txt" }))?;
    assert_mcp_success(&read)?;
    check!(
        read.get("result")
            .and_then(|value| value.get("structuredContent"))
            .and_then(|value| value.get("content"))
            .and_then(Value::as_str)
            == Some("hello from M1 hardened"),
        "read_file did not return the final edit content: {read}"
    );
    assert_no_temp_residue(&context.fixture)?;
    let after_read = context.activity()?;
    assert_success_activity(&after_read.items, before_read, "read_file")
}

fn run_e2e_05(context: &mut E2eContext) -> E2eResult<()> {
    let before = context.activity()?.latest;
    let result = context.mcp()?.call_tool(
        "edit",
        json!({ "path": "duplicate.txt", "oldText": "same", "newText": "changed" }),
    )?;
    assert_mcp_error_code(&result, "EDIT_MATCH_NOT_UNIQUE")?;
    assert_bytes(
        &context.fixture.workspace.join("duplicate.txt"),
        b"same same",
    )?;
    assert_no_temp_residue(&context.fixture)?;
    let after = context.activity()?;
    assert_failure_activity(&after.items, before, "edit", "execution_failed")
}

fn run_e2e_06(context: &mut E2eContext) -> E2eResult<()> {
    let before = context.activity()?.latest;
    let outside_target = context.fixture.outside.join("outside.txt");
    let outside_path = outside_target.to_string_lossy().into_owned();
    let result = context.mcp()?.call_tool(
        "write",
        json!({ "path": outside_path, "content": "must not change" }),
    )?;
    assert_mcp_error_code(&result, "WORKSPACE_VIOLATION")?;
    check!(
        !result.to_string().contains(&outside_path),
        "workspace rejection leaked the outside absolute path"
    );
    assert_bytes(&outside_target, b"must not change")?;
    assert_no_temp_residue(&context.fixture)?;
    let after = context.activity()?;
    assert_failure_activity(&after.items, before, "write", "workspace_violation")
}

fn run_e2e_07(context: &mut E2eContext) -> E2eResult<()> {
    let content = "你好，M1 👋";
    let before_write = context.activity()?.latest;
    let write = context.mcp()?.call_tool(
        "write",
        json!({ "path": "m1-utf8.txt", "content": content }),
    )?;
    assert_mcp_success(&write)?;
    assert_bytes(
        &context.fixture.workspace.join("m1-utf8.txt"),
        content.as_bytes(),
    )?;
    assert_no_temp_residue(&context.fixture)?;
    let after_write = context.activity()?;
    assert_success_activity(&after_write.items, before_write, "write")?;

    let before_read = after_write.latest;
    let read = context
        .mcp()?
        .call_tool("read_file", json!({ "path": "m1-utf8.txt" }))?;
    assert_mcp_success(&read)?;
    check!(
        read.get("result")
            .and_then(|value| value.get("structuredContent"))
            .and_then(|value| value.get("content"))
            .and_then(Value::as_str)
            == Some(content),
        "read_file changed UTF-8 content: {read}"
    );
    assert_no_temp_residue(&context.fixture)?;
    let after_read = context.activity()?;
    assert_success_activity(&after_read.items, before_read, "read_file")
}

fn run_e2e_08(context: &mut E2eContext) -> E2eResult<()> {
    let endpoint = context
        .endpoint
        .ok_or_else(|| E2eError::new("E2E-08 has no MCP endpoint to probe"))?;
    assert_bytes(
        &context.fixture.workspace.join("m1-e2e.txt"),
        b"hello from M1 hardened",
    )?;
    assert_bytes(
        &context.fixture.workspace.join("duplicate.txt"),
        b"same same",
    )?;
    assert_bytes(
        &context.fixture.workspace.join("m1-utf8.txt"),
        "你好，M1 👋".as_bytes(),
    )?;
    assert_bytes(
        &context.fixture.outside.join("outside.txt"),
        b"must not change",
    )?;
    assert_no_temp_residue(&context.fixture)?;

    context.runtime.graceful_stop()?;
    check!(
        TcpStream::connect_timeout(&endpoint.address, Duration::from_millis(500)).is_err(),
        "MCP endpoint remained connectable after Runtime shutdown"
    );
    check!(
        !context.runtime.is_running()?,
        "Runtime process is still running"
    );
    assert_no_temp_residue(&context.fixture)?;

    let base = context.fixture.base.clone();
    context.fixture.cleanup()?;
    check!(!base.exists(), "temporary E2E fixture residue remains");
    println!("E2E_SHUTDOWN_ENDPOINT=closed");
    println!("E2E_TEMP_FIXTURE=clean");
    Ok(())
}

fn parse_endpoint(url: &str) -> E2eResult<Endpoint> {
    let rest = url
        .strip_prefix("http://127.0.0.1:")
        .ok_or_else(|| E2eError::new(format!("MCP endpoint is not loopback HTTP: {url}")))?;
    let (port, path) = rest
        .split_once("/mcp")
        .ok_or_else(|| E2eError::new(format!("MCP endpoint path is not /mcp: {url}")))?;
    check!(
        path.is_empty(),
        "MCP endpoint has an unexpected suffix: {url}"
    );
    let port = port
        .parse::<u16>()
        .map_err(|error| E2eError::new(format!("invalid MCP endpoint port: {error}")))?;
    Ok(Endpoint {
        address: SocketAddr::from(([127, 0, 0, 1], port)),
    })
}

fn decimal_value(value: Option<&Value>, name: &str) -> E2eResult<u64> {
    let value = value
        .and_then(Value::as_str)
        .ok_or_else(|| E2eError::new(format!("activity.list has no string {name}")))?;
    value
        .parse::<u64>()
        .map_err(|error| E2eError::new(format!("invalid activity {name}: {error}")))
}

fn assert_mcp_success(result: &Value) -> E2eResult<()> {
    check!(
        result.get("error").is_none(),
        "MCP returned JSON-RPC error: {result}"
    );
    check!(
        result
            .get("result")
            .and_then(|value| value.get("isError"))
            .and_then(Value::as_bool)
            != Some(true),
        "MCP returned tool error: {result}"
    );
    Ok(())
}

fn assert_mcp_error_code(result: &Value, expected: &str) -> E2eResult<()> {
    check!(
        result.get("error").is_none(),
        "MCP returned transport error: {result}"
    );
    check!(
        result
            .get("result")
            .and_then(|value| value.get("isError"))
            .and_then(Value::as_bool)
            == Some(true),
        "MCP tool unexpectedly succeeded: {result}"
    );
    let code = result
        .get("result")
        .and_then(|value| value.get("structuredContent"))
        .and_then(|value| value.get("code"))
        .and_then(Value::as_str);
    check!(
        code == Some(expected),
        "expected MCP error {expected}, got {result}"
    );
    Ok(())
}

fn assert_success_activity(items: &[Value], after_sequence: u64, tool: &str) -> E2eResult<()> {
    let events = events_after(items, after_sequence);
    let requests = events
        .iter()
        .filter(|event| {
            event.get("eventType").and_then(Value::as_str) == Some("tool_requested")
                && event
                    .get("metadata")
                    .and_then(|value| value.get("tool"))
                    .and_then(Value::as_str)
                    == Some(tool)
        })
        .collect::<Vec<_>>();
    check!(
        !requests.is_empty(),
        "activity.list has no tool_requested evidence for {tool}: {events:?}"
    );
    for request in requests {
        let request_id = request
            .get("requestId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                E2eError::new(format!("tool_requested event has no requestId: {request}"))
            })?;
        check!(
            events.iter().any(|event| {
                event.get("requestId").and_then(Value::as_str) == Some(request_id)
                    && event.get("eventType").and_then(Value::as_str) == Some("execution_started")
            }),
            "activity.list has no execution_started evidence for {tool} request {request_id}"
        );
        check!(
            events.iter().any(|event| {
                event.get("requestId").and_then(Value::as_str) == Some(request_id)
                    && event.get("eventType").and_then(Value::as_str) == Some("execution_completed")
            }),
            "activity.list has no execution_completed evidence for {tool} request {request_id}"
        );
    }
    Ok(())
}

fn assert_failure_activity(
    items: &[Value],
    after_sequence: u64,
    tool: &str,
    failure_event: &str,
) -> E2eResult<()> {
    let events = events_after(items, after_sequence);
    let request = events.iter().find(|event| {
        event.get("eventType").and_then(Value::as_str) == Some("tool_requested")
            && event
                .get("metadata")
                .and_then(|value| value.get("tool"))
                .and_then(Value::as_str)
                == Some(tool)
    });
    let request = request.ok_or_else(|| {
        E2eError::new(format!(
            "activity.list has no failed tool_requested evidence for {tool}: {events:?}"
        ))
    })?;
    let request_id = request
        .get("requestId")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            E2eError::new(format!(
                "failed tool_requested event has no requestId: {request}"
            ))
        })?;
    check!(
        events.iter().any(|event| {
            event.get("requestId").and_then(Value::as_str) == Some(request_id)
                && event.get("eventType").and_then(Value::as_str) == Some(failure_event)
        }),
        "activity.list has no {failure_event} evidence for {tool} request {request_id}"
    );
    check!(
        !events.iter().any(|event| {
            event.get("requestId").and_then(Value::as_str) == Some(request_id)
                && event.get("eventType").and_then(Value::as_str) == Some("execution_completed")
        }),
        "failed {tool} request {request_id} has execution_completed evidence"
    );
    Ok(())
}

fn events_after(items: &[Value], after_sequence: u64) -> Vec<Value> {
    items
        .iter()
        .filter(|item| {
            decimal_value(item.get("sequence"), "sequence").unwrap_or(0) > after_sequence
        })
        .cloned()
        .collect()
}

fn assert_bytes(path: &Path, expected: &[u8]) -> E2eResult<()> {
    let actual = fs::read(path).map_err(|error| {
        E2eError::new(format!("read filesystem assertion target failed: {error}"))
    })?;
    check!(
        actual == expected,
        "filesystem bytes mismatch for {}: expected {:?}, got {:?}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("target"),
        expected,
        actual
    );
    Ok(())
}

fn assert_no_temp_residue(fixture: &Fixture) -> E2eResult<()> {
    let residue = atomic_temp_residue(&fixture.base)
        .map_err(|error| E2eError::new(format!("scan temporary residue failed: {error}")))?;
    check!(
        residue.is_empty(),
        "atomic temporary residue remains: {:?}",
        residue
    );
    Ok(())
}

fn atomic_temp_residue(root: &Path) -> std::io::Result<Vec<String>> {
    fn visit(directory: &Path, residue: &mut Vec<String>) -> std::io::Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.contains(".chatgpt-harness-") {
                residue.push(name.clone());
            }
            if entry.file_type()?.is_dir() {
                visit(&entry.path(), residue)?;
            }
        }
        Ok(())
    }

    let mut residue = Vec::new();
    visit(root, &mut residue)?;
    Ok(residue)
}
