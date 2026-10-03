use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const PENDING_DETECTION_TIMEOUT: Duration = Duration::from_secs(3);

type E2eResult<T> = Result<T, E2eError>;

#[derive(Debug)]
struct E2eError(String);

impl E2eError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for E2eError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
#[ignore = "M4 black-box E2E requires an explicitly built release Runtime"]
fn m4_local_black_box_e2e() {
    if let Err(error) = run_suite() {
        panic!("M4 local black-box E2E failed: {error}");
    }
}

fn run_suite() -> E2eResult<()> {
    let binary = runtime_binary()?;
    let fixture = Fixture::new()?;
    let mut failures = Vec::new();
    println!("E2E_RUNTIME_BINARY={}", binary.display());
    println!("E2E_RUNTIME_BINARY_KIND=release");

    let mut generation_a = match Generation::start(
        &binary,
        &fixture,
        "A",
        runtime_config(&fixture, "1", "ask", "ask", true),
    ) {
        Ok(generation) => generation,
        Err(error) => {
            for case in 1..=18 {
                println!("E2E-{case:02} BLOCKED: Generation A could not start");
            }
            return Err(error);
        }
    };

    record_case("E2E-01", run_e2e_01(&mut generation_a), &mut failures);
    record_case("E2E-02", run_e2e_02(&mut generation_a), &mut failures);

    let first_pending = record_case(
        "E2E-03",
        start_pending_write(&mut generation_a, "approval-once.txt", "approved once"),
        &mut failures,
    );
    let first_request_id = first_pending
        .as_ref()
        .map(|(_, request)| request_id(request).to_owned());
    if let Some((call, request)) = first_pending {
        record_case(
            "E2E-04",
            resolve_same_call(
                &mut generation_a,
                call,
                &request,
                "allow_once",
                None,
                "approval-once.txt",
                b"approved once",
            ),
            &mut failures,
        );
    } else {
        println!("E2E-04 BLOCKED: E2E-03 did not produce a pending request");
        failures.push("E2E-04".into());
    }

    record_case(
        "E2E-05",
        run_e2e_05(&mut generation_a, first_request_id.as_deref()),
        &mut failures,
    );
    record_case("E2E-06", run_e2e_06(&mut generation_a), &mut failures);
    record_case("E2E-07", run_e2e_07(&mut generation_a), &mut failures);
    record_case("E2E-08", run_e2e_08(&mut generation_a), &mut failures);
    record_case("E2E-09", run_e2e_09(&mut generation_a), &mut failures);
    record_case("E2E-10", run_e2e_10(&mut generation_a), &mut failures);
    record_case(
        "E2E-17",
        run_e2e_17(&mut generation_a, first_request_id.as_deref()),
        &mut failures,
    );

    let cancelled_request_id = record_case("E2E-11", run_e2e_11(&mut generation_a), &mut failures);

    let cancelled_request_id = cancelled_request_id
        .or_else(|| first_request_id.clone())
        .unwrap_or_else(|| "missing-generation-a-request".into());

    let mut generation_b = match Generation::start(
        &binary,
        &fixture,
        "B",
        runtime_config(&fixture, "2", "ask", "ask", true),
    ) {
        Ok(generation) => generation,
        Err(error) => {
            println!("E2E-12 BLOCKED: Generation B could not start");
            println!("E2E-13 BLOCKED: Generation B could not start");
            failures.extend(["E2E-12".into(), "E2E-13".into()]);
            generation_a.runtime.force_cleanup()?;
            fixture.cleanup()?;
            return Err(error);
        }
    };
    record_case("E2E-12", run_e2e_12(&mut generation_b), &mut failures);
    record_case(
        "E2E-13",
        run_e2e_13(&mut generation_b, &cancelled_request_id),
        &mut failures,
    );
    generation_b.graceful_stop()?;

    let mut generation_c = Generation::start(
        &binary,
        &fixture,
        "C-disabled",
        runtime_config(&fixture, "3", "ask", "ask", false),
    )?;
    record_case("E2E-14", run_e2e_14(&mut generation_c), &mut failures);
    generation_c.graceful_stop()?;

    let mut generation_d = Generation::start(
        &binary,
        &fixture,
        "D-deny",
        runtime_config(&fixture, "4", "deny", "allow", true),
    )?;
    record_case("E2E-15", run_e2e_15(&mut generation_d), &mut failures);
    generation_d.graceful_stop()?;

    let mut generation_e = Generation::start(
        &binary,
        &fixture,
        "E-boundary",
        runtime_config(&fixture, "5", "ask", "allow", true),
    )?;
    record_case("E2E-16", run_e2e_16(&mut generation_e), &mut failures);
    record_case(
        "E2E-18",
        run_e2e_18(&mut generation_e, &fixture),
        &mut failures,
    );

    if !failures.is_empty() {
        return Err(E2eError::new(format!(
            "failed cases: {}",
            failures.join(", ")
        )));
    }
    println!("LOCAL_M4_BLACK_BOX_E2E=PASS");
    Ok(())
}

fn record_case<T>(case_id: &str, result: E2eResult<T>, failures: &mut Vec<String>) -> Option<T> {
    match result {
        Ok(value) => {
            println!("{case_id} PASS");
            Some(value)
        }
        Err(error) => {
            println!("{case_id} FAIL: {error}");
            failures.push(case_id.into());
            None
        }
    }
}

fn runtime_binary() -> E2eResult<PathBuf> {
    let binary = std::env::var_os("HARNESS_M4_E2E_RUNTIME_BIN")
        .map(PathBuf::from)
        .ok_or_else(|| {
            E2eError::new(
                "HARNESS_M4_E2E_RUNTIME_BIN is required; pass the freshly built release Runtime",
            )
        })?;
    let binary = binary
        .canonicalize()
        .map_err(|error| E2eError::new(format!("resolve release Runtime path failed: {error}")))?;
    check!(
        binary.is_file(),
        "release Runtime binary does not exist: {}",
        binary.display()
    );
    let normalized = binary
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    check!(
        normalized.contains("\\target\\release\\"),
        "E2E requires target\\release Runtime, got {}",
        binary.display()
    );
    check!(
        binary
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("chatgpt-harness-runtime.exe")),
        "unexpected Runtime executable name: {}",
        binary.display()
    );
    Ok(binary)
}

struct Fixture {
    base: PathBuf,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> E2eResult<Self> {
        let base =
            std::env::temp_dir().join(format!("chatgpt-harness-m4-e2e-{}", uuid::Uuid::new_v4()));
        let workspace = base.join("workspace");
        let outside = base.join("outside");
        fs::create_dir_all(&workspace)
            .map_err(|error| E2eError::new(format!("create E2E workspace failed: {error}")))?;
        fs::create_dir_all(&outside).map_err(|error| {
            E2eError::new(format!("create E2E outside directory failed: {error}"))
        })?;
        fs::write(outside.join("outside.txt"), b"must not change")
            .map_err(|error| E2eError::new(format!("create outside sentinel failed: {error}")))?;
        Ok(Self {
            base,
            workspace,
            outside,
        })
    }

    fn cleanup(&self) -> E2eResult<()> {
        if self.base.exists() {
            fs::remove_dir_all(&self.base)
                .map_err(|error| E2eError::new(format!("cleanup E2E fixture failed: {error}")))?;
        }
        check!(!self.base.exists(), "E2E fixture remains after cleanup");
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn runtime_config(
    fixture: &Fixture,
    revision: &str,
    write_policy: &str,
    edit_policy: &str,
    write_enabled: bool,
) -> Value {
    json!({
        "clientInfo": { "name": "m4-e2e-runner", "title": "M4 Local Black-box E2E", "version": "1" },
        "protocolVersion": 1,
        "capabilities": {},
        "runtimeConfig": {
            "configRevision": revision,
            "workspace": { "rootPath": fixture.workspace.to_string_lossy() },
            "security": { "mode": "normal", "developerTestMode": false },
            "runtimeLimits": {
                "maxConcurrentRequests": 8,
                "maxActivityEvents": 256,
                "maxReadBytes": 1048576
            },
            "tools": {
                "readFile": { "enabled": true },
                "glob": { "enabled": true },
                "write": { "enabled": write_enabled },
                "edit": { "enabled": true },
                "bash": { "enabled": false }
            },
            "policies": {
                "toolDefaults": {
                    "readFile": "allow",
                    "glob": "allow",
                    "write": write_policy,
                    "edit": edit_policy,
                    "bash": "ask"
                },
                "userRules": []
            }
        }
    })
}

struct Generation {
    runtime: RuntimeProcess,
    endpoint: Endpoint,
    mcp: McpClient,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Generation {
    fn start(binary: &Path, fixture: &Fixture, label: &str, config: Value) -> E2eResult<Self> {
        let mut runtime = RuntimeProcess::start(binary, label)?;
        let initialize = runtime.control_request("initialize", config)?;
        let runtime_id = initialize
            .get("runtimeId")
            .and_then(Value::as_str)
            .ok_or_else(|| E2eError::new("initialize did not return runtimeId"))?
            .to_owned();
        runtime.control_notification("initialized", json!({}))?;
        let endpoint_result = runtime.control_request("mcp.endpoint.get", json!({}))?;
        let endpoint_url = endpoint_result
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| E2eError::new("mcp.endpoint.get did not return url"))?;
        let endpoint = parse_endpoint(endpoint_url)?;
        let mcp = McpClient::connect(endpoint)?;
        println!(
            "E2E_GENERATION label={label} runtimeId={runtime_id} pid={} mcpEndpoint=loopback:{}",
            runtime.pid(),
            endpoint.address.port()
        );
        check!(
            fixture.workspace.is_dir(),
            "E2E generation started without disposable workspace"
        );
        Ok(Self {
            runtime,
            endpoint,
            mcp,
            workspace: fixture.workspace.clone(),
            outside: fixture.outside.clone(),
        })
    }

    fn status(&mut self) -> E2eResult<Value> {
        self.runtime.control_request("runtime.status", json!({}))
    }

    fn approvals(&mut self, status: Option<&str>) -> E2eResult<Vec<Value>> {
        let params = status
            .map(|status| json!({ "status": status }))
            .unwrap_or_else(|| json!({}));
        let result = self.runtime.control_request("approvals.list", params)?;
        result
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| E2eError::new("approvals.list did not return items"))
    }

    fn activity(&mut self) -> E2eResult<Vec<Value>> {
        let result = self
            .runtime
            .control_request("activity.list", json!({ "limit": 256 }))?;
        result
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| E2eError::new("activity.list did not return items"))
    }

    fn graceful_stop(&mut self) -> E2eResult<()> {
        self.runtime.graceful_stop()
    }
}

struct RuntimeProcess {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<String>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<String>>,
    next_request_id: u64,
    label: String,
}

impl RuntimeProcess {
    fn start(binary: &Path, label: &str) -> E2eResult<Self> {
        let mut child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| E2eError::new(format!("spawn release Runtime failed: {error}")))?;
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
        println!("E2E_RUNTIME_PID label={label} pid={}", child.id());
        Ok(Self {
            child,
            stdin,
            messages,
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            next_request_id: 0,
            label: label.into(),
        })
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn control_request(&mut self, method: &str, params: Value) -> E2eResult<Value> {
        let message = self.control_message(method, params)?;
        if let Some(error) = message.get("error") {
            return Err(E2eError::new(format!(
                "Control RPC {method} returned an unexpected error code {}",
                control_error_code(error).unwrap_or("unknown")
            )));
        }
        message
            .get("result")
            .cloned()
            .ok_or_else(|| E2eError::new(format!("Control RPC {method} has no result")))
    }

    fn control_expect_error(
        &mut self,
        method: &str,
        params: Value,
        expected_code: &str,
    ) -> E2eResult<()> {
        let message = self.control_message(method, params)?;
        let error = message
            .get("error")
            .ok_or_else(|| E2eError::new(format!("Control RPC {method} unexpectedly succeeded")))?;
        let actual = control_error_code(error).unwrap_or("unknown");
        check!(
            actual == expected_code,
            "Control RPC {method} error code mismatch: expected {expected_code}, got {actual}"
        );
        Ok(())
    }

    fn control_message(&mut self, method: &str, params: Value) -> E2eResult<Value> {
        self.next_request_id += 1;
        let id = format!("m4-{}-{}", self.label, self.next_request_id);
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
                        RecvTimeoutError::Disconnected => {
                            E2eError::new(format!("Runtime stdout closed during {method}"))
                        }
                    })?;
            let message: Value = serde_json::from_str(&line)
                .map_err(|error| E2eError::new(format!("invalid Control JSON: {error}")))?;
            if message.get("id") != Some(&Value::String(id.clone())) {
                continue;
            }
            return Ok(message);
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
            .map_err(|error| E2eError::new(format!("serialize Control request failed: {error}")))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|error| E2eError::new(format!("write Control request failed: {error}")))?;
        self.stdin
            .flush()
            .map_err(|error| E2eError::new(format!("flush Control request failed: {error}")))
    }

    fn graceful_stop(&mut self) -> E2eResult<()> {
        let result = self.control_request("runtime.stop", json!({}))?;
        check!(
            result.get("accepted") == Some(&Value::Bool(true)),
            "runtime.stop did not return accepted=true"
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
                        "Runtime process did not exit after runtime.stop",
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            };
        self.join_output_threads();
        check!(
            status.success(),
            "Runtime process exited unsuccessfully: {status}"
        );
        println!("E2E_RUNTIME_EXIT label={} status=success", self.label);
        Ok(())
    }

    fn force_cleanup(&mut self) -> E2eResult<()> {
        if self
            .child
            .try_wait()
            .map_err(|error| E2eError::new(format!("poll leaked Runtime failed: {error}")))?
            .is_none()
        {
            self.child
                .kill()
                .map_err(|error| E2eError::new(format!("kill leaked Runtime failed: {error}")))?;
            self.child
                .wait()
                .map_err(|error| E2eError::new(format!("wait leaked Runtime failed: {error}")))?;
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
                println!("E2E_RUNTIME_STDERR label={} {}", self.label, stderr.trim());
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

#[derive(Clone)]
struct McpClient {
    endpoint: Endpoint,
    session_id: String,
    next_request_id: Arc<AtomicU64>,
}

struct PendingMcpCall {
    receiver: Receiver<E2eResult<Value>>,
    thread: JoinHandle<()>,
}

impl PendingMcpCall {
    fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }

    fn receive(self) -> E2eResult<Value> {
        let result = self
            .receiver
            .recv_timeout(HTTP_TIMEOUT)
            .map_err(|error| E2eError::new(format!("original MCP call did not finish: {error}")))?;
        let _ = self.thread.join();
        result
    }
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
                    "clientInfo": { "name": "m4-e2e-runner", "version": "1" }
                }
            }),
            None,
        )?;
        check!(
            response.status == 200,
            "MCP initialize returned HTTP {}",
            response.status
        );
        let _ = response.json_rpc()?;
        let session_id = response
            .header("mcp-session-id")
            .ok_or_else(|| E2eError::new("MCP initialize did not return Mcp-Session-Id"))?
            .to_owned();
        check!(!session_id.is_empty(), "MCP session id is empty");
        Ok(Self {
            endpoint,
            session_id,
            next_request_id: Arc::new(AtomicU64::new(1)),
        })
    }

    fn list_tools(&self) -> E2eResult<Value> {
        self.request("tools/list", json!({}))
    }

    fn call_tool(&self, name: &str, arguments: Value) -> E2eResult<Value> {
        self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        )
    }

    fn pending_call(&self, name: &str, arguments: Value) -> PendingMcpCall {
        let client = self.clone();
        let name = name.to_owned();
        let (sender, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let _ = sender.send(client.call_tool(&name, arguments));
        });
        PendingMcpCall { receiver, thread }
    }

    fn request(&self, method: &str, params: Value) -> E2eResult<Value> {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed) + 1;
        let response = http_json_request(
            self.endpoint.address,
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }),
            Some(&self.session_id),
        )?;
        check!(
            response.status == 200,
            "MCP {method} returned HTTP {}",
            response.status
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
        Err(E2eError::new("MCP response did not contain JSON-RPC data"))
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
        .map_err(|error| E2eError::new(format!("write MCP request failed: {error}")))?;
    read_mcp_response(&mut stream)
}

/// rmcp keeps Streamable HTTP request-wise SSE connections open after the
/// priming event. Read until the first JSON-RPC response instead of waiting
/// for EOF; this also keeps approval-wait and Runtime-stop regression cases
/// observable with the current M5 transport.
fn read_mcp_response(stream: &mut TcpStream) -> E2eResult<HttpResponse> {
    let deadline = Instant::now() + HTTP_TIMEOUT;
    let mut raw = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                return Err(E2eError::new(
                    "MCP response closed before JSON-RPC data arrived",
                ));
            }
            Ok(read) => raw.extend_from_slice(&buffer[..read]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) && Instant::now() < deadline =>
            {
                continue;
            }
            Err(error) => {
                return Err(E2eError::new(format!("read MCP response failed: {error}")));
            }
        }

        let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
            if Instant::now() >= deadline {
                return Err(E2eError::new("MCP response had no complete HTTP header"));
            }
            continue;
        };
        let header_text = std::str::from_utf8(&raw[..header_end]).map_err(|error| {
            E2eError::new(format!("MCP response headers were not UTF-8: {error}"))
        })?;
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
        let (body, chunked_complete) = if headers
            .get("transfer-encoding")
            .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
        {
            decode_chunked_available(wire_body)?
        } else {
            (wire_body.to_vec(), true)
        };
        if body_contains_json_rpc(&body)
            || (chunked_complete && headers.contains_key("content-length") && !body.is_empty())
        {
            return Ok(HttpResponse {
                status,
                headers,
                body,
            });
        }
        if Instant::now() >= deadline {
            return Err(E2eError::new("MCP response did not contain JSON-RPC data"));
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

fn decode_chunked_available(wire: &[u8]) -> E2eResult<(Vec<u8>, bool)> {
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
            .map_err(|error| E2eError::new(format!("invalid chunk length: {error}")))?;
        let length =
            usize::from_str_radix(length_text.split(';').next().unwrap_or_default().trim(), 16)
                .map_err(|error| E2eError::new(format!("invalid chunk size: {error}")))?;
        cursor = line_end + 2;
        if length == 0 {
            return Ok((body, true));
        }
        if wire.len() < cursor + length + 2 {
            return Ok((body, false));
        }
        body.extend_from_slice(&wire[cursor..cursor + length]);
        if &wire[cursor + length..cursor + length + 2] != b"\r\n" {
            return Err(E2eError::new("invalid chunk terminator"));
        }
        cursor += length + 2;
    }
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
            .map_err(|error| E2eError::new(format!("invalid chunk length: {error}")))?;
        let length =
            usize::from_str_radix(length_text.split(';').next().unwrap_or_default().trim(), 16)
                .map_err(|error| E2eError::new(format!("invalid chunk size: {error}")))?;
        wire = &wire[line_end + 2..];
        if length == 0 {
            return Ok(body);
        }
        check!(wire.len() >= length + 2, "truncated chunked MCP response");
        body.extend_from_slice(&wire[..length]);
        check!(
            &wire[length..length + 2] == b"\r\n",
            "invalid chunk terminator"
        );
        wire = &wire[length + 2..];
    }
}

fn parse_endpoint(url: &str) -> E2eResult<Endpoint> {
    let rest = url
        .strip_prefix("http://127.0.0.1:")
        .ok_or_else(|| E2eError::new("Runtime did not return a loopback MCP endpoint"))?;
    let (port, suffix) = rest
        .split_once("/mcp")
        .ok_or_else(|| E2eError::new("Runtime MCP endpoint path is not /mcp"))?;
    check!(
        suffix.is_empty(),
        "Runtime MCP endpoint has an unexpected suffix"
    );
    let port = port
        .parse::<u16>()
        .map_err(|error| E2eError::new(format!("invalid MCP endpoint port: {error}")))?;
    Ok(Endpoint {
        address: SocketAddr::from(([127, 0, 0, 1], port)),
    })
}

fn control_error_code(error: &Value) -> Option<&str> {
    error
        .get("data")
        .and_then(|data| data.get("code"))
        .and_then(Value::as_str)
}

fn request_id(request: &Value) -> &str {
    request
        .get("requestId")
        .and_then(Value::as_str)
        .unwrap_or("<missing-request-id>")
}

fn request_scope(request: &Value) -> Option<&str> {
    request.get("scopeSummary").and_then(Value::as_str)
}

fn pending_request(generation: &mut Generation, scope: &str) -> E2eResult<Value> {
    let deadline = Instant::now() + PENDING_DETECTION_TIMEOUT;
    loop {
        let status = generation.status()?;
        if status
            .get("pendingApprovals")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 1)
        {
            let pending = generation.approvals(Some("pending"))?;
            if let Some(request) = pending
                .into_iter()
                .find(|request| request_scope(request) == Some(scope))
            {
                return Ok(request);
            }
        }
        if Instant::now() >= deadline {
            return Err(E2eError::new(format!(
                "pending approval for scope {scope} was not observable"
            )));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn start_pending_write(
    generation: &mut Generation,
    path: &str,
    content: &str,
) -> E2eResult<(PendingMcpCall, Value)> {
    let call = generation
        .mcp
        .pending_call("write", json!({ "path": path, "content": content }));
    check!(
        !call.is_finished(),
        "original MCP write call returned before approval was resolved"
    );
    let request = pending_request(generation, path)?;
    Ok((call, request))
}

fn resolve_same_call(
    generation: &mut Generation,
    call: PendingMcpCall,
    request: &Value,
    decision: &str,
    expected_error: Option<&str>,
    path: &str,
    expected_bytes: &[u8],
) -> E2eResult<()> {
    let id = request_id(request).to_owned();
    let resolved = generation.runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": id, "decision": decision }),
    )?;
    check!(
        resolved
            .get("approval")
            .and_then(|approval| approval.get("status"))
            .and_then(Value::as_str)
            .is_some_and(|status| status == "approved" || status == "denied"),
        "approvals.resolve did not return a terminal approval"
    );
    let result = call.receive()?;
    if let Some(expected_error) = expected_error {
        assert_mcp_error_code(&result, expected_error)?;
        check!(
            !generation_path(generation, path).exists(),
            "denied/cancelled request created an unexpected relative target"
        );
    } else {
        assert_mcp_success(&result)?;
        assert_bytes(&generation_path(generation, path), expected_bytes)?;
    }
    println!("E2E_ORIGINAL_MCP_CALL_COMPLETED requestId={id} decision={decision}");
    Ok(())
}

fn run_e2e_01(generation: &mut Generation) -> E2eResult<()> {
    let status = generation.status()?;
    check!(
        status.get("readiness").and_then(Value::as_str) == Some("ready"),
        "Runtime did not reach ready readiness"
    );
    check!(
        status.get("lifecycle").and_then(Value::as_str) == Some("ready"),
        "Runtime did not reach ready lifecycle"
    );
    check!(
        generation.endpoint.address.ip().is_loopback(),
        "MCP endpoint is not loopback"
    );
    let activity = generation.activity()?;
    check!(
        activity
            .iter()
            .any(|event| event.get("eventType").and_then(Value::as_str) == Some("runtime_started")),
        "activity.list did not expose runtime_started"
    );
    Ok(())
}

fn run_e2e_02(generation: &mut Generation) -> E2eResult<()> {
    let list = generation.mcp.list_tools()?;
    let tools = list
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(Value::as_array)
        .ok_or_else(|| E2eError::new("tools/list did not return tools"))?;
    check!(
        tools
            .iter()
            .any(|tool| tool.get("name").and_then(Value::as_str) == Some("write")),
        "ASK write was not exposed by MCP tools/list"
    );
    let policies = generation
        .runtime
        .control_request("policies.list", json!({}))?;
    let write = policies
        .get("effective")
        .and_then(|effective| effective.get("perTool"))
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("tool").and_then(Value::as_str) == Some("write"))
        })
        .ok_or_else(|| E2eError::new("policies.list omitted effective write"))?;
    check!(
        write.get("decision").and_then(Value::as_str) == Some("ask"),
        "effective write decision was not ASK"
    );
    let descriptors = generation
        .runtime
        .control_request("tools.list", json!({}))?;
    let write_descriptor = descriptors
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("name").and_then(Value::as_str) == Some("write"))
        })
        .ok_or_else(|| E2eError::new("tools.list omitted write"))?;
    check!(
        write_descriptor.get("exposed") == Some(&Value::Bool(true)),
        "ASK write was not exposed by Runtime control projection"
    );
    Ok(())
}

fn run_e2e_05(generation: &mut Generation, allow_once_request_id: Option<&str>) -> E2eResult<()> {
    let allow_once_request_id = allow_once_request_id
        .ok_or_else(|| E2eError::new("E2E-04 did not provide the AllowOnce request ID"))?;
    generation.runtime.control_expect_error(
        "approvals.resolve",
        json!({ "requestId": allow_once_request_id, "decision": "deny" }),
        "APPROVAL_ALREADY_RESOLVED",
    )?;
    let (call, request) = start_pending_write(generation, "approval-once.txt", "second attempt")?;
    let id = request_id(&request).to_owned();
    check!(
        generation
            .approvals(Some("approved"))?
            .iter()
            .any(|item| item.get("decision").and_then(Value::as_str) == Some("allow_once")),
        "AllowOnce terminal history was not observable"
    );
    let resolved = generation.runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": id, "decision": "deny" }),
    )?;
    check!(
        resolved
            .get("approval")
            .and_then(|approval| approval.get("decision"))
            .and_then(Value::as_str)
            == Some("deny"),
        "cleanup Deny did not return deny decision"
    );
    let result = call.receive()?;
    assert_mcp_error_code(&result, "APPROVAL_DENIED")?;
    let status = generation.status()?;
    check!(
        status.get("pendingApprovals").and_then(Value::as_u64) == Some(0),
        "AllowOnce cleanup left a pending approval"
    );
    Ok(())
}

fn run_e2e_06(generation: &mut Generation) -> E2eResult<()> {
    let (call, request) = start_pending_write(generation, "denied.txt", "denied content")?;
    let id = request_id(&request).to_owned();
    let resolved = generation.runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": id, "decision": "deny" }),
    )?;
    check!(
        resolved
            .get("approval")
            .and_then(|approval| approval.get("status"))
            .and_then(Value::as_str)
            == Some("denied"),
        "Deny did not produce denied terminal approval"
    );
    let result = call.receive()?;
    assert_mcp_error_code(&result, "APPROVAL_DENIED")?;
    check!(
        !generation_path(generation, "denied.txt").exists(),
        "Deny mutated the workspace"
    );
    let activity = generation.activity()?;
    assert_activity_lifecycle(&activity, &id, &["approval_requested", "approval_denied"])?;
    check!(
        !activity.iter().any(|event| {
            event.get("requestId").and_then(Value::as_str) == Some(&id)
                && event.get("eventType").and_then(Value::as_str) == Some("execution_completed")
        }),
        "Deny emitted execution_completed"
    );
    Ok(())
}

fn run_e2e_07(generation: &mut Generation) -> E2eResult<()> {
    let (call, request) = start_pending_write(generation, "session.txt", "first")?;
    let id = request_id(&request).to_owned();
    let resolved = generation.runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": id, "decision": "allow_for_session" }),
    )?;
    check!(
        resolved
            .get("approval")
            .and_then(|approval| approval.get("decision"))
            .and_then(Value::as_str)
            == Some("allow_for_session"),
        "AllowForSession decision was not returned"
    );
    let result = call.receive()?;
    assert_mcp_success(&result)?;
    assert_bytes(&generation_path(generation, "session.txt"), b"first")?;
    check!(
        generation
            .status()?
            .get("pendingApprovals")
            .and_then(Value::as_u64)
            == Some(0),
        "AllowForSession left a pending approval"
    );
    Ok(())
}

fn run_e2e_08(generation: &mut Generation) -> E2eResult<()> {
    let before = generation.activity()?.len();
    let result = generation.mcp.call_tool(
        "write",
        json!({ "path": "session.txt", "content": "second" }),
    )?;
    assert_mcp_success(&result)?;
    assert_bytes(&generation_path(generation, "session.txt"), b"second")?;
    check!(
        generation
            .status()?
            .get("pendingApprovals")
            .and_then(Value::as_u64)
            == Some(0),
        "matching session grant generated a new pending approval"
    );
    let activity = generation.activity()?;
    let recent = activity.iter().skip(before);
    check!(
        recent.clone().any(|event| {
            event.get("eventType").and_then(Value::as_str) == Some("approval_resolved")
                && event
                    .get("metadata")
                    .and_then(|metadata| metadata.get("tool"))
                    .and_then(Value::as_str)
                    == Some("write")
        }),
        "activity did not expose session grant match evidence"
    );
    check!(
        !recent.clone().any(|event| {
            event.get("eventType").and_then(Value::as_str) == Some("approval_requested")
        }),
        "matching session grant unexpectedly emitted approval_requested"
    );
    Ok(())
}

fn run_e2e_09(generation: &mut Generation) -> E2eResult<()> {
    let (call, request) = start_pending_write(generation, "other.txt", "other")?;
    let id = request_id(&request).to_owned();
    generation.runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": id, "decision": "deny" }),
    )?;
    let result = call.receive()?;
    assert_mcp_error_code(&result, "APPROVAL_DENIED")?;
    check!(
        !generation_path(generation, "other.txt").exists(),
        "different path grant miss was not denied without mutation"
    );
    Ok(())
}

fn run_e2e_10(generation: &mut Generation) -> E2eResult<()> {
    let before = fs::read(generation_path(generation, "session.txt"))
        .map_err(|error| E2eError::new(format!("read session fixture failed: {error}")))?;
    let call = generation.mcp.pending_call(
        "edit",
        json!({
            "path": "session.txt",
            "oldText": "second",
            "newText": "edit must be denied"
        }),
    );
    let request = pending_request(generation, "session.txt")?;
    let id = request_id(&request).to_owned();
    generation.runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": id, "decision": "deny" }),
    )?;
    let result = call.receive()?;
    assert_mcp_error_code(&result, "APPROVAL_DENIED")?;
    assert_bytes(&generation_path(generation, "session.txt"), &before)?;
    Ok(())
}

fn run_e2e_11(generation: &mut Generation) -> E2eResult<String> {
    let (call, request) = start_pending_write(generation, "stopped.txt", "must not execute")?;
    let id = request_id(&request).to_owned();
    generation.graceful_stop()?;
    match call.receive() {
        Ok(result) => assert_mcp_error_code(&result, "APPROVAL_CANCELLED")?,
        Err(error) => check!(
            error.to_string().contains("closed before JSON-RPC data"),
            "Runtime stop returned an unexpected pending MCP error: {error}"
        ),
    }
    check!(
        !generation_path(generation, "stopped.txt").exists(),
        "Runtime stop allowed a pending write to execute"
    );
    check!(
        TcpStream::connect_timeout(&generation.endpoint.address, Duration::from_millis(500))
            .is_err(),
        "MCP endpoint remained connectable after Runtime stop"
    );
    Ok(id)
}

fn run_e2e_12(generation: &mut Generation) -> E2eResult<()> {
    let (call, request) = start_pending_write(generation, "session.txt", "generation b")?;
    let id = request_id(&request).to_owned();
    generation.runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": id, "decision": "deny" }),
    )?;
    let result = call.receive()?;
    assert_mcp_error_code(&result, "APPROVAL_DENIED")?;
    check!(
        fs::read(generation_path(generation, "session.txt")).map_err(|error| E2eError::new(
            format!("read generation reset fixture failed: {error}")
        ))? == b"second",
        "Generation B inherited Generation A session grant or mutated the target"
    );
    Ok(())
}

fn run_e2e_13(generation: &mut Generation, old_request_id: &str) -> E2eResult<()> {
    generation.runtime.control_expect_error(
        "approvals.resolve",
        json!({ "requestId": old_request_id, "decision": "allow_once" }),
        "APPROVAL_NOT_FOUND",
    )?;
    check!(
        generation
            .status()?
            .get("pendingApprovals")
            .and_then(Value::as_u64)
            == Some(0),
        "stale Generation A resolve affected Generation B"
    );
    Ok(())
}

fn run_e2e_14(generation: &mut Generation) -> E2eResult<()> {
    let tools = generation.mcp.list_tools()?;
    let names = tools
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(Value::as_array)
        .ok_or_else(|| E2eError::new("disabled tools/list did not return tools"))?;
    check!(
        !names
            .iter()
            .any(|tool| tool.get("name").and_then(Value::as_str) == Some("write")),
        "disabled write remained in tools/list"
    );
    let result = generation.mcp.call_tool(
        "write",
        json!({ "path": "disabled.txt", "content": "disabled" }),
    )?;
    assert_mcp_error_code(&result, "TOOL_DISABLED")?;
    check!(
        generation
            .status()?
            .get("pendingApprovals")
            .and_then(Value::as_u64)
            == Some(0),
        "disabled write entered approval"
    );
    check!(
        !generation_path(generation, "disabled.txt").exists(),
        "disabled write mutated the workspace"
    );
    Ok(())
}

fn run_e2e_15(generation: &mut Generation) -> E2eResult<()> {
    let result = generation.mcp.call_tool(
        "write",
        json!({ "path": "policy-denied.txt", "content": "denied" }),
    )?;
    assert_mcp_error_code(&result, "POLICY_DENIED")?;
    check!(
        generation
            .status()?
            .get("pendingApprovals")
            .and_then(Value::as_u64)
            == Some(0),
        "explicit DENY created an approval"
    );
    check!(
        !generation_path(generation, "policy-denied.txt").exists(),
        "explicit DENY mutated the workspace"
    );
    Ok(())
}

fn run_e2e_16(generation: &mut Generation) -> E2eResult<()> {
    let result = generation.mcp.call_tool(
        "write",
        json!({
            "path": generation_outside_path(generation),
            "content": "must not change"
        }),
    )?;
    assert_mcp_error_code(&result, "WORKSPACE_VIOLATION")?;
    check!(
        generation
            .status()?
            .get("pendingApprovals")
            .and_then(Value::as_u64)
            == Some(0),
        "outside workspace request created an approval"
    );
    Ok(())
}

fn run_e2e_17(generation: &mut Generation, allow_once_request_id: Option<&str>) -> E2eResult<()> {
    let activity = generation.activity()?;
    let request_id = allow_once_request_id
        .ok_or_else(|| E2eError::new("E2E-04 did not provide an approval request ID"))?;
    assert_activity_lifecycle(
        &activity,
        request_id,
        &[
            "tool_requested",
            "policy_evaluated",
            "approval_requested",
            "approval_resolved",
            "execution_started",
            "execution_completed",
        ],
    )?;
    let serialized = serde_json::to_string(&activity)
        .map_err(|error| E2eError::new(format!("serialize activity evidence failed: {error}")))?;
    let workspace_root = generation.workspace.to_string_lossy().into_owned();
    check!(
        !serialized.contains("approved once")
            && !serialized.contains("M4_SENTINEL")
            && !serialized.contains(&workspace_root),
        "Activity evidence leaked raw content or the absolute workspace root"
    );
    Ok(())
}

fn run_e2e_18(generation: &mut Generation, fixture: &Fixture) -> E2eResult<()> {
    check!(
        generation
            .status()?
            .get("pendingApprovals")
            .and_then(Value::as_u64)
            == Some(0),
        "final generation has an unresolved pending approval"
    );
    generation.graceful_stop()?;
    check!(
        TcpStream::connect_timeout(&generation.endpoint.address, Duration::from_millis(500))
            .is_err(),
        "final MCP endpoint remained connectable"
    );
    check!(
        !generation.runtime.is_running()?,
        "final Runtime process is still running"
    );
    check!(
        fs::read(fixture.outside.join("outside.txt"))
            .map_err(|error| E2eError::new(format!("read outside sentinel failed: {error}")))?
            == b"must not change",
        "outside fixture changed"
    );
    assert_no_temp_residue(fixture)?;
    fixture.cleanup()?;
    println!("E2E_CLEAN_SHUTDOWN=PASS");
    Ok(())
}

fn assert_mcp_success(result: &Value) -> E2eResult<()> {
    check!(
        result.get("error").is_none(),
        "MCP returned transport error"
    );
    check!(
        result
            .get("result")
            .and_then(|result| result.get("isError"))
            != Some(&Value::Bool(true)),
        "MCP returned a tool error"
    );
    Ok(())
}

fn assert_mcp_error_code(result: &Value, expected: &str) -> E2eResult<()> {
    check!(
        result.get("error").is_none(),
        "MCP returned transport error"
    );
    check!(
        result
            .get("result")
            .and_then(|result| result.get("isError"))
            == Some(&Value::Bool(true)),
        "MCP unexpectedly succeeded"
    );
    let actual = result
        .get("result")
        .and_then(|result| result.get("structuredContent"))
        .and_then(|content| content.get("code"))
        .and_then(Value::as_str)
        .ok_or_else(|| E2eError::new("MCP tool error had no stable code"))?;
    check!(
        actual == expected,
        "MCP error code mismatch: expected {expected}, got {actual}"
    );
    Ok(())
}

fn assert_activity_lifecycle(
    activity: &[Value],
    request_id: &str,
    required_types: &[&str],
) -> E2eResult<()> {
    for required in required_types {
        check!(
            activity.iter().any(|event| {
                event.get("requestId").and_then(Value::as_str) == Some(request_id)
                    && event.get("eventType").and_then(Value::as_str) == Some(*required)
            }),
            "activity lifecycle for request {request_id} omitted {required}"
        );
    }
    Ok(())
}

fn assert_bytes(path: &Path, expected: &[u8]) -> E2eResult<()> {
    let actual = fs::read(path)
        .map_err(|error| E2eError::new(format!("read filesystem assertion failed: {error}")))?;
    check!(actual == expected, "filesystem assertion failed for target");
    Ok(())
}

fn generation_path(generation: &Generation, relative: &str) -> PathBuf {
    generation.workspace.join(relative)
}

fn generation_outside_path(generation: &Generation) -> String {
    generation
        .outside
        .join("outside.txt")
        .to_string_lossy()
        .into_owned()
}

fn assert_no_temp_residue(fixture: &Fixture) -> E2eResult<()> {
    let mut residue = Vec::new();
    collect_temp_residue(&fixture.base, &mut residue)
        .map_err(|error| E2eError::new(format!("scan E2E temp residue failed: {error}")))?;
    check!(residue.is_empty(), "temporary atomic residue remains");
    Ok(())
}

fn collect_temp_residue(root: &Path, residue: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.contains(".chatgpt-harness-") {
            residue.push(name);
        }
        if entry.file_type()?.is_dir() {
            collect_temp_residue(&entry.path(), residue)?;
        }
    }
    Ok(())
}

impl RuntimeProcess {
    fn is_running(&mut self) -> E2eResult<bool> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|error| E2eError::new(format!("poll Runtime process failed: {error}")))
    }
}
