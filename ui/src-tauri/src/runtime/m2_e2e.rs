use super::runtime_config_snapshot;
use crate::config::ConfigStore;
use harness_control_protocol::AppConfig;
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
    ($condition:expr $(,)?) => {
        if !$condition {
            return Err(E2eError::new("E2E assertion failed"));
        }
    };
    ($condition:expr, $($argument:tt)*) => {
        if !$condition {
            return Err(E2eError::new(format!($($argument)*)));
        }
    };
}

#[test]
#[ignore = "M2 black-box E2E requires an explicitly built release Runtime"]
fn m2_local_black_box_e2e() {
    if let Err(error) = run_suite() {
        panic!("M2 local black-box E2E failed: {error}");
    }
}

fn run_suite() -> E2eResult<()> {
    let fixture = Fixture::new()?;
    let store = ConfigStore::new(fixture.config_dir.clone());
    let mut config = ConfigStore::safe_defaults();
    config.workspace.root_path = fixture.workspace.to_string_lossy().into_owned();
    config = store
        .save(config)
        .map_err(|_| E2eError::new("save initial M2 E2E config failed"))?;

    let binary = runtime_binary()?;
    println!(
        "E2E_RUNTIME_BINARY={}",
        binary.file_name().unwrap_or_default().to_string_lossy()
    );

    let mut failures = Vec::new();
    let (mut generation_a, mut mcp_a) = RuntimeProcess::start(&binary, &config, "A")?;
    report_case(
        "E2E-01",
        run_e2e_01(&mut generation_a, &mut mcp_a, &config),
        &mut failures,
    );
    report_case(
        "E2E-02",
        run_e2e_02(&mut generation_a, &mut mcp_a, &fixture),
        &mut failures,
    );

    let mut write_off = config.clone();
    write_off.tools.write.enabled = false;
    write_off = store
        .save(write_off)
        .map_err(|_| E2eError::new("save write-off config failed"))?;
    report_case(
        "E2E-03",
        run_e2e_03(&mut generation_a, &mut mcp_a, &store, &write_off, &fixture),
        &mut failures,
    );

    generation_a.stop()?;
    let (mut generation_b, mut mcp_b) = RuntimeProcess::start(&binary, &write_off, "B")?;
    report_case(
        "E2E-04",
        run_e2e_04(&mut generation_b, &mut mcp_b, &write_off),
        &mut failures,
    );
    report_case(
        "E2E-05",
        run_disabled_mutation_case(&mut generation_b, &mut mcp_b, &fixture, "write"),
        &mut failures,
    );

    let mut write_on = write_off.clone();
    write_on.tools.write.enabled = true;
    write_on = store
        .save(write_on)
        .map_err(|_| E2eError::new("save write-on config failed"))?;
    report_case(
        "E2E-06",
        run_e2e_06(&mut generation_b, &mut mcp_b, &binary, &write_on, &fixture),
        &mut failures,
    );

    let (mut generation_c, mut mcp_c) = RuntimeProcess::start(&binary, &write_on, "C")?;
    let mut read_off = write_on.clone();
    read_off.tools.read_file.enabled = false;
    read_off = store
        .save(read_off)
        .map_err(|_| E2eError::new("save read-off config failed"))?;
    report_case(
        "E2E-07",
        run_e2e_07(&mut generation_c, &mut mcp_c, &binary, &read_off, &fixture),
        &mut failures,
    );
    generation_c.stop()?;

    let (mut generation_d, _mcp_d) = RuntimeProcess::start(&binary, &read_off, "D")?;
    let mut multiple_off = read_off.clone();
    multiple_off.tools.write.enabled = false;
    multiple_off = store
        .save(multiple_off)
        .map_err(|_| E2eError::new("save multiple-off config failed"))?;
    generation_d.stop()?;
    let (mut generation_e, mut mcp_e) = RuntimeProcess::start(&binary, &multiple_off, "E")?;
    report_case(
        "E2E-08",
        run_e2e_08(&mut generation_e, &mut mcp_e, &multiple_off, &fixture),
        &mut failures,
    );
    report_case(
        "E2E-09",
        run_e2e_09(&mut generation_e, &mut mcp_e, &fixture),
        &mut failures,
    );
    report_case(
        "E2E-10",
        run_e2e_10(&mut generation_e, &mut mcp_e, &fixture),
        &mut failures,
    );
    generation_e.stop()?;

    let legacy_store = ConfigStore::new(fixture.legacy_config_dir.clone());
    let mut legacy_value = ConfigStore::safe_defaults();
    legacy_value.workspace.root_path = fixture.workspace.to_string_lossy().into_owned();
    legacy_value.config_revision = harness_control_protocol::DecimalU64::new(41);
    let mut legacy_json = serde_json::to_value(&legacy_value)
        .map_err(|_| E2eError::new("serialize legacy config failed"))?;
    legacy_json
        .as_object_mut()
        .ok_or_else(|| E2eError::new("legacy config is not an object"))?
        .remove("tools");
    fs::create_dir_all(&fixture.legacy_config_dir)
        .map_err(|_| E2eError::new("create legacy config directory failed"))?;
    fs::write(
        fixture.legacy_config_dir.join("config.json"),
        serde_json::to_vec_pretty(&legacy_json)
            .map_err(|_| E2eError::new("encode legacy config failed"))?,
    )
    .map_err(|_| E2eError::new("write legacy config failed"))?;
    let loaded_legacy = legacy_store
        .load()
        .map_err(|_| E2eError::new("load legacy config failed"))?
        .ok_or_else(|| E2eError::new("legacy config was not loaded"))?;
    report_case("E2E-11", run_e2e_11(&binary, &loaded_legacy), &mut failures);
    let (mut generation_f, _mcp_f) = RuntimeProcess::start(&binary, &loaded_legacy, "F")?;
    let endpoint = generation_f.endpoint;
    generation_f.stop()?;
    report_case(
        "E2E-12",
        run_e2e_12(&mut generation_f, endpoint, &fixture),
        &mut failures,
    );
    let base = fixture.base.clone();
    fixture.cleanup()?;
    check!(!base.exists(), "M2 E2E fixture cleanup left temporary data");
    check!(failures.is_empty(), "failed E2E cases: {:?}", failures);
    println!("LOCAL_M2_BLACK_BOX_E2E=PASS");
    Ok(())
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

fn runtime_binary() -> E2eResult<PathBuf> {
    if let Some(path) = std::env::var_os("HARNESS_E2E_RUNTIME_BIN") {
        let path = PathBuf::from(path);
        check!(
            path.is_file(),
            "configured Runtime executable is unavailable"
        );
        return Ok(path);
    }
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..\\..\\target\\release\\chatgpt-harness-runtime.exe");
    check!(path.is_file(), "release Runtime executable is unavailable");
    Ok(path)
}

struct Fixture {
    base: PathBuf,
    config_dir: PathBuf,
    legacy_config_dir: PathBuf,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> E2eResult<Self> {
        let base =
            std::env::temp_dir().join(format!("chatgpt-harness-m2-e2e-{}", uuid::Uuid::new_v4()));
        let config_dir = base.join("config");
        let legacy_config_dir = base.join("legacy-config");
        let workspace = base.join("workspace");
        let outside = base.join("outside");
        fs::create_dir_all(&config_dir)
            .map_err(|_| E2eError::new("create config fixture failed"))?;
        fs::create_dir_all(&workspace)
            .map_err(|_| E2eError::new("create workspace fixture failed"))?;
        fs::create_dir_all(&outside).map_err(|_| E2eError::new("create outside fixture failed"))?;
        fs::write(workspace.join("baseline.txt"), b"baseline")
            .map_err(|_| E2eError::new("create baseline fixture failed"))?;
        fs::write(workspace.join("mutable.txt"), b"old")
            .map_err(|_| E2eError::new("create mutable fixture failed"))?;
        fs::write(outside.join("outside.txt"), b"outside")
            .map_err(|_| E2eError::new("create outside file fixture failed"))?;
        Ok(Self {
            base,
            config_dir,
            legacy_config_dir,
            workspace,
            outside,
        })
    }

    fn cleanup(&self) -> E2eResult<()> {
        if self.base.exists() {
            fs::remove_dir_all(&self.base)
                .map_err(|_| E2eError::new("remove M2 E2E fixture failed"))?;
        }
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
    runtime_id: String,
    endpoint: Endpoint,
}

impl RuntimeProcess {
    fn start(binary: &Path, config: &AppConfig, generation: &str) -> E2eResult<(Self, McpClient)> {
        let mut child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| E2eError::new("spawn compiled Runtime failed"))?;
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
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let stderr_thread = thread::spawn(move || {
            let mut output = String::new();
            let mut reader = BufReader::new(stderr);
            let _ = reader.read_to_string(&mut output);
            output
        });
        let pid = child.id();
        println!("E2E_RUNTIME_GENERATION={generation} PID={pid}");
        let mut process = Self {
            child,
            stdin,
            messages,
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            next_request_id: 0,
            runtime_id: String::new(),
            endpoint: Endpoint {
                address: SocketAddr::from(([127, 0, 0, 1], 0)),
            },
        };
        let initialize = process.control_request(
            "initialize",
            json!({
                "clientInfo": { "name": "m2-e2e-runner", "title": "M2 E2E", "version": "1" },
                "protocolVersion": 1,
                "capabilities": { "activityReplay": true },
                "runtimeConfig": serde_json::to_value(runtime_config_snapshot(config))
                    .map_err(|_| E2eError::new("serialize Runtime snapshot failed"))?
            }),
        )?;
        process.runtime_id = initialize
            .get("runtimeId")
            .and_then(Value::as_str)
            .ok_or_else(|| E2eError::new("initialize did not return Runtime identity"))?
            .to_owned();
        process.control_notification("initialized", json!({}))?;
        let endpoint_value = process.control_request("mcp.endpoint.get", json!({}))?;
        let endpoint_url = endpoint_value
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| E2eError::new("mcp.endpoint.get did not return an endpoint"))?;
        process.endpoint = parse_endpoint(endpoint_url)?;
        let mcp = McpClient::connect(process.endpoint)?;
        let status = process.control_request("runtime.status", json!({}))?;
        check!(
            status.get("configRevision")
                == Some(&Value::String(config.config_revision.get().to_string())),
            "Runtime active revision did not match the startup snapshot"
        );
        Ok((process, mcp))
    }

    fn control_request(&mut self, method: &str, params: Value) -> E2eResult<Value> {
        self.next_request_id += 1;
        let id = format!("m2-e2e-{}", self.next_request_id);
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
                .map_err(|_| E2eError::new("Runtime emitted invalid Control JSON"))?;
            if message.get("id") != Some(&Value::String(id.clone())) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(E2eError::new(format!(
                    "Control RPC {method} returned an error: {error}"
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

    fn activity(&mut self, after_sequence: Option<&str>) -> E2eResult<Value> {
        let mut params = json!({ "limit": 256 });
        if let Some(after_sequence) = after_sequence {
            params["afterSequence"] = Value::String(after_sequence.to_owned());
        }
        self.control_request("activity.list", params)
    }

    fn write_json_line(&mut self, value: &Value) -> E2eResult<()> {
        serde_json::to_writer(&mut self.stdin, value)
            .map_err(|_| E2eError::new("serialize Control RPC failed"))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|_| E2eError::new("write Control RPC failed"))?;
        self.stdin
            .flush()
            .map_err(|_| E2eError::new("flush Control RPC failed"))
    }

    fn stop(&mut self) -> E2eResult<()> {
        let result = self.control_request("runtime.stop", json!({}))?;
        check!(
            result.get("accepted") == Some(&Value::Bool(true)),
            "Runtime stop was not accepted"
        );
        self.wait_for_exit()
    }

    fn wait_for_exit(&mut self) -> E2eResult<()> {
        let deadline = Instant::now() + CONTROL_TIMEOUT;
        let status = loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|_| E2eError::new("poll Runtime process failed"))?
            {
                break status;
            }
            if Instant::now() >= deadline {
                return Err(E2eError::new("Runtime did not exit after stop"));
            }
            thread::sleep(Duration::from_millis(10));
        };
        self.join_output_threads();
        check!(status.success(), "Runtime exited unsuccessfully");
        println!("E2E_RUNTIME_EXIT=success runtime_id={}", self.runtime_id);
        Ok(())
    }

    fn force_cleanup(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.join_output_threads();
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
        self.force_cleanup();
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
                    "clientInfo": { "name": "m2-e2e-runner", "version": "1" }
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
            .ok_or_else(|| E2eError::new("MCP initialize did not return a session"))?
            .to_owned();
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
            json!({ "name": name, "arguments": arguments }),
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

    fn json_rpc(&self) -> E2eResult<Value> {
        if let Ok(value) = serde_json::from_slice(&self.body) {
            return Ok(value);
        }
        let body = std::str::from_utf8(&self.body)
            .map_err(|_| E2eError::new("MCP response was not UTF-8"))?;
        body.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .find_map(|line| serde_json::from_str(line.trim()).ok())
            .ok_or_else(|| E2eError::new("MCP response had no JSON-RPC payload"))
    }
}

fn http_json_request(
    address: SocketAddr,
    message: Value,
    session_id: Option<&str>,
) -> E2eResult<HttpResponse> {
    let body =
        serde_json::to_vec(&message).map_err(|_| E2eError::new("encode MCP request failed"))?;
    let mut stream = TcpStream::connect_timeout(&address, HTTP_TIMEOUT)
        .map_err(|_| E2eError::new("connect loopback MCP endpoint failed"))?;
    stream
        .set_read_timeout(Some(HTTP_TIMEOUT))
        .map_err(|_| E2eError::new("set MCP read timeout failed"))?;
    stream
        .set_write_timeout(Some(HTTP_TIMEOUT))
        .map_err(|_| E2eError::new("set MCP write timeout failed"))?;
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
        .map_err(|_| E2eError::new("write MCP request failed"))?;
    let mut raw_response = Vec::new();
    stream
        .read_to_end(&mut raw_response)
        .map_err(|_| E2eError::new("read MCP response failed"))?;
    parse_http_response(&raw_response)
}

fn parse_http_response(raw: &[u8]) -> E2eResult<HttpResponse> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| E2eError::new("MCP response had no HTTP header"))?;
    let header_text = std::str::from_utf8(&raw[..header_end])
        .map_err(|_| E2eError::new("MCP response headers were not UTF-8"))?;
    let mut lines = header_text.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| E2eError::new("MCP response had no status"))?
        .parse::<u16>()
        .map_err(|_| E2eError::new("MCP response status was invalid"))?;
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
            .map_err(|_| E2eError::new("invalid chunk length"))?
            .split(';')
            .next()
            .unwrap_or_default();
        let length = usize::from_str_radix(length_text.trim(), 16)
            .map_err(|_| E2eError::new("invalid chunk size"))?;
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
        .ok_or_else(|| E2eError::new("Runtime endpoint was not loopback HTTP"))?;
    let (port, suffix) = rest
        .split_once("/mcp")
        .ok_or_else(|| E2eError::new("Runtime endpoint path was not /mcp"))?;
    check!(
        suffix.is_empty(),
        "Runtime endpoint had an unexpected suffix"
    );
    let port = port
        .parse::<u16>()
        .map_err(|_| E2eError::new("Runtime endpoint port was invalid"))?;
    Ok(Endpoint {
        address: SocketAddr::from(([127, 0, 0, 1], port)),
    })
}

fn run_e2e_01(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    config: &AppConfig,
) -> E2eResult<()> {
    let names = list_names(mcp.list_tools()?)?;
    check!(
        names == ["read_file", "glob", "write", "edit"],
        "default tools/list did not expose the stable four-tool catalog"
    );
    let status = runtime.control_request("runtime.status", json!({}))?;
    check!(
        status.get("configRevision")
            == Some(&Value::String(config.config_revision.get().to_string())),
        "default generation active revision mismatch"
    );
    let effective = runtime.control_request("control.effective_tools.get", json!({}))?;
    let effective_items = effective
        .as_array()
        .ok_or_else(|| E2eError::new("effective tool observation was not an array"))?;
    check!(
        !effective_items
            .iter()
            .any(|item| item.get("name") == Some(&json!("bash"))),
        "bash was registered by the default generation"
    );
    Ok(())
}

fn run_e2e_02(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    fixture: &Fixture,
) -> E2eResult<()> {
    let before = latest_activity(runtime)?;
    assert_mcp_success(&mcp.call_tool(
        "write",
        json!({ "path": "m2-e2e.txt", "content": "hello from M2" }),
    )?)?;
    assert_bytes(&fixture.workspace.join("m2-e2e.txt"), b"hello from M2")?;
    assert_mcp_success(&mcp.call_tool(
        "edit",
        json!({
            "path": "m2-e2e.txt",
            "oldText": "hello from M2",
            "newText": "hello from M2 enabled"
        }),
    )?)?;
    assert_bytes(
        &fixture.workspace.join("m2-e2e.txt"),
        b"hello from M2 enabled",
    )?;
    let read = mcp.call_tool("read_file", json!({ "path": "m2-e2e.txt" }))?;
    assert_mcp_success(&read)?;
    check!(
        read["result"]["structuredContent"]["content"] == json!("hello from M2 enabled"),
        "read_file did not return the final M2 content"
    );
    let activity = activity_after(runtime, &before)?;
    for tool in ["write", "edit", "read_file"] {
        assert_activity_completed(&activity, tool)?;
    }
    Ok(())
}

fn run_e2e_03(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    store: &ConfigStore,
    write_off: &AppConfig,
    fixture: &Fixture,
) -> E2eResult<()> {
    let active = runtime.control_request("runtime.status", json!({}))?;
    check!(
        active["configRevision"] == json!("1"),
        "active generation changed revision"
    );
    check!(
        list_names(mcp.list_tools()?)?.contains(&"write".to_owned()),
        "active generation lost write after desired save"
    );
    assert_mcp_success(&mcp.call_tool(
        "write",
        json!({ "path": "active-generation.txt", "content": "still enabled" }),
    )?)?;
    assert_bytes(
        &fixture.workspace.join("active-generation.txt"),
        b"still enabled",
    )?;
    let latest = store
        .load()
        .map_err(|_| E2eError::new("reload desired config failed"))?
        .ok_or_else(|| E2eError::new("desired config disappeared"))?;
    check!(
        latest.config_revision == write_off.config_revision
            && latest.tools.write.enabled == write_off.tools.write.enabled
            && latest.config_revision.get() == 2,
        "persisted write-off revision did not become N+1"
    );
    check!(
        active["configRevision"] != json!(latest.config_revision.get().to_string()),
        "restart-required revision mismatch was not observable"
    );
    Ok(())
}

fn run_e2e_04(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    config: &AppConfig,
) -> E2eResult<()> {
    let names = list_names(mcp.list_tools()?)?;
    check!(
        !names.contains(&"write".to_owned()),
        "disabled write remained in tools/list"
    );
    check!(
        names == ["read_file", "glob", "edit"],
        "new disabled generation changed sibling tool exposure"
    );
    let status = runtime.control_request("runtime.status", json!({}))?;
    check!(
        status["configRevision"] == json!(config.config_revision.get().to_string()),
        "new generation did not use persisted write-off revision"
    );
    let effective = runtime.control_request("control.effective_tools.get", json!({}))?;
    let write = effective
        .as_array()
        .and_then(|items| items.iter().find(|item| item["name"] == json!("write")))
        .ok_or_else(|| E2eError::new("effective write observation was missing"))?;
    check!(
        write["availability"] == json!("disabled"),
        "write was not disabled effectively"
    );
    Ok(())
}

fn run_disabled_mutation_case(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    fixture: &Fixture,
    tool: &str,
) -> E2eResult<()> {
    let target = fixture.workspace.join("stale-write.txt");
    let before = latest_activity(runtime)?;
    let result = mcp.call_tool(
        tool,
        json!({ "path": "stale-write.txt", "content": "must not write" }),
    )?;
    assert_mcp_error_code(&result, "TOOL_DISABLED")?;
    check!(!target.exists(), "disabled write created a target file");
    let activity = activity_after(runtime, &before)?;
    assert_disabled_activity(&activity, tool)?;
    Ok(())
}

fn run_e2e_06(
    runtime_b: &mut RuntimeProcess,
    mcp_b: &mut McpClient,
    binary: &Path,
    write_on: &AppConfig,
    fixture: &Fixture,
) -> E2eResult<()> {
    check!(
        !list_names(mcp_b.list_tools()?)?.contains(&"write".to_owned()),
        "Generation B live-mutated after write was re-enabled"
    );
    runtime_b.stop()?;
    let mut generation_c = RuntimeProcess::start(binary, write_on, "C-reenable")?;
    let (ref mut runtime_c, ref mut mcp_c) = generation_c;
    check!(
        list_names(mcp_c.list_tools()?)?.contains(&"write".to_owned()),
        "Generation C did not re-expose write"
    );
    assert_mcp_success(&mcp_c.call_tool(
        "write",
        json!({ "path": "re-enabled.txt", "content": "write is back" }),
    )?)?;
    assert_bytes(&fixture.workspace.join("re-enabled.txt"), b"write is back")?;
    runtime_c.stop()?;
    Ok(())
}

fn run_e2e_07(
    _runtime_c: &mut RuntimeProcess,
    mcp_c: &mut McpClient,
    binary: &Path,
    read_off: &AppConfig,
    fixture: &Fixture,
) -> E2eResult<()> {
    check!(
        list_names(mcp_c.list_tools()?)?.contains(&"read_file".to_owned()),
        "Generation C changed read_file before restart"
    );
    let mut generation_d = RuntimeProcess::start(binary, read_off, "D-read-off")?;
    let (ref mut runtime_d, ref mut mcp_d) = generation_d;
    let names = list_names(mcp_d.list_tools()?)?;
    check!(
        !names.contains(&"read_file".to_owned()),
        "read_file remained exposed after disable"
    );
    for sibling in ["glob", "write", "edit"] {
        check!(
            names.contains(&sibling.to_owned()),
            "sibling {sibling} disappeared"
        );
    }
    let before = latest_activity(runtime_d)?;
    assert_mcp_error_code(
        &mcp_d.call_tool("read_file", json!({ "path": "baseline.txt" }))?,
        "TOOL_DISABLED",
    )?;
    let activity = activity_after(runtime_d, &before)?;
    assert_disabled_activity(&activity, "read_file")?;
    assert_mcp_success(&mcp_d.call_tool("glob", json!({ "pattern": "**/*.txt" }))?)?;
    assert_mcp_success(
        &mcp_d.call_tool("write", json!({ "path": "sibling.txt", "content": "old" }))?,
    )?;
    assert_mcp_success(&mcp_d.call_tool(
        "edit",
        json!({ "path": "sibling.txt", "oldText": "old", "newText": "new" }),
    )?)?;
    assert_bytes(&fixture.workspace.join("sibling.txt"), b"new")?;
    runtime_d.stop()?;
    Ok(())
}

fn run_e2e_08(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    config: &AppConfig,
    fixture: &Fixture,
) -> E2eResult<()> {
    let names = list_names(mcp.list_tools()?)?;
    check!(
        names == ["glob", "edit"],
        "multiple disabled generation exposed the wrong tools"
    );
    let before_read = latest_activity(runtime)?;
    assert_mcp_error_code(
        &mcp.call_tool("read_file", json!({ "path": "baseline.txt" }))?,
        "TOOL_DISABLED",
    )?;
    let after_read = activity_after(runtime, &before_read)?;
    assert_disabled_activity(&after_read, "read_file")?;
    let before_write = latest_activity(runtime)?;
    assert_mcp_error_code(
        &mcp.call_tool(
            "write",
            json!({ "path": "multiple-disabled.txt", "content": "must not write" }),
        )?,
        "TOOL_DISABLED",
    )?;
    check!(!fixture.workspace.join("multiple-disabled.txt").exists());
    let after_write = activity_after(runtime, &before_write)?;
    assert_disabled_activity(&after_write, "write")?;
    assert_mcp_success(&mcp.call_tool("glob", json!({ "pattern": "**/*.txt" }))?)?;
    assert_mcp_success(&mcp.call_tool(
        "edit",
        json!({ "path": "mutable.txt", "oldText": "old", "newText": "edited" }),
    )?)?;
    assert_bytes(&fixture.workspace.join("mutable.txt"), b"edited")?;
    let effective = runtime.control_request("control.effective_tools.get", json!({}))?;
    check!(
        effective.as_array().is_some_and(|items| items
            .iter()
            .any(|item| item["name"] == json!("read_file")
                && item["availability"] == json!("disabled"))
            && items
                .iter()
                .any(|item| item["name"] == json!("write")
                    && item["availability"] == json!("disabled"))
            && items
                .iter()
                .any(|item| item["name"] == json!("edit")
                    && item["availability"] == json!("available"))),
        "multiple disabled effective observation was inaccurate"
    );
    check!(config.tools.read_file.enabled == false && config.tools.write.enabled == false);
    Ok(())
}

fn run_e2e_09(
    _runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    _fixture: &Fixture,
) -> E2eResult<()> {
    let disabled = mcp.call_tool(
        "write",
        json!({ "path": "distinction.txt", "content": "blocked" }),
    )?;
    assert_mcp_error_code(&disabled, "TOOL_DISABLED")?;
    let unknown = mcp.call_tool("does_not_exist", json!({}))?;
    check!(
        unknown.get("error").is_some(),
        "unknown tool was not a protocol error"
    );
    check!(
        unknown.get("result").is_none(),
        "unknown tool looked like a disabled result"
    );
    Ok(())
}

fn run_e2e_10(
    _runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    fixture: &Fixture,
) -> E2eResult<()> {
    let before = fs::read(fixture.workspace.join("mutable.txt"))
        .map_err(|_| E2eError::new("read mutation guard fixture failed"))?;
    assert_mcp_error_code(
        &mcp.call_tool(
            "write",
            json!({ "path": "new-disabled.txt", "content": "blocked" }),
        )?,
        "TOOL_DISABLED",
    )?;
    assert_mcp_error_code(
        &mcp.call_tool(
            "write",
            json!({ "path": "mutable.txt", "content": "blocked overwrite" }),
        )?,
        "TOOL_DISABLED",
    )?;
    check!(!fixture.workspace.join("new-disabled.txt").exists());
    check!(
        fs::read(fixture.workspace.join("mutable.txt"))
            .map_err(|_| E2eError::new("re-read mutation guard fixture failed"))?
            == before,
        "disabled write mutated an existing target"
    );
    Ok(())
}

fn run_e2e_11(binary: &Path, config: &AppConfig) -> E2eResult<()> {
    check!(
        config.schema_version == 1,
        "legacy config schemaVersion changed"
    );
    check!(config.tools.read_file.enabled);
    check!(config.tools.glob.enabled);
    check!(config.tools.write.enabled);
    check!(config.tools.edit.enabled);
    check!(!config.tools.bash.enabled);
    let (mut runtime, mut mcp) = RuntimeProcess::start(binary, config, "F-legacy")?;
    let names = list_names(mcp.list_tools()?)?;
    check!(names == ["read_file", "glob", "write", "edit"]);
    runtime.stop()?;
    Ok(())
}

fn run_e2e_12(
    runtime: &mut RuntimeProcess,
    endpoint: Endpoint,
    fixture: &Fixture,
) -> E2eResult<()> {
    check!(
        TcpStream::connect_timeout(&endpoint.address, Duration::from_millis(500)).is_err(),
        "MCP endpoint remained connectable after formal shutdown"
    );
    check!(
        runtime
            .child
            .try_wait()
            .map_err(|_| E2eError::new("poll final Runtime process failed"))?
            .is_some(),
        "final Runtime process remained alive"
    );
    check!(fixture.outside.join("outside.txt").exists());
    Ok(())
}

fn list_names(value: Value) -> E2eResult<Vec<String>> {
    value["result"]["tools"]
        .as_array()
        .ok_or_else(|| E2eError::new("tools/list did not return a tool array"))
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
}

fn assert_mcp_success(result: &Value) -> E2eResult<()> {
    check!(
        result.get("error").is_none(),
        "MCP returned a JSON-RPC error"
    );
    check!(
        result["result"]["isError"].as_bool() != Some(true),
        "MCP returned a tool error"
    );
    Ok(())
}

fn assert_mcp_error_code(result: &Value, expected: &str) -> E2eResult<()> {
    check!(
        result.get("error").is_none(),
        "MCP returned a transport error"
    );
    check!(
        result["result"]["isError"].as_bool() == Some(true),
        "MCP call unexpectedly succeeded"
    );
    check!(
        result["result"]["structuredContent"]["code"] == json!(expected),
        "MCP returned a different error code"
    );
    Ok(())
}

fn latest_activity(runtime: &mut RuntimeProcess) -> E2eResult<String> {
    runtime
        .activity(None)?
        .get("latestSequence")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| E2eError::new("activity.list did not return latestSequence"))
}

fn activity_after(runtime: &mut RuntimeProcess, after: &str) -> E2eResult<Vec<Value>> {
    runtime
        .activity(Some(after))?
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| E2eError::new("activity.list did not return items"))
}

fn assert_activity_completed(items: &[Value], tool: &str) -> E2eResult<()> {
    check!(
        items
            .iter()
            .any(|item| item["eventType"] == json!("tool_requested")
                && item["metadata"]["tool"] == json!(tool)),
        "Activity had no request evidence for {tool}"
    );
    check!(
        items
            .iter()
            .any(|item| item["eventType"] == json!("execution_completed")
                && item["metadata"]["tool"] == json!(tool)),
        "Activity had no completion evidence for {tool}"
    );
    Ok(())
}

fn assert_disabled_activity(items: &[Value], tool: &str) -> E2eResult<()> {
    check!(
        items
            .iter()
            .any(|item| item["eventType"] == json!("tool_requested")
                && item["metadata"]["tool"] == json!(tool)),
        "Activity had no disabled request evidence for {tool}"
    );
    check!(
        items
            .iter()
            .any(|item| item["eventType"] == json!("execution_failed")
                && item["metadata"]["tool"] == json!(tool)
                && item["metadata"]["failureCategory"] == json!("tool_disabled")),
        "Activity had no tool_disabled rejection evidence for {tool}"
    );
    check!(
        !items
            .iter()
            .any(|item| item["eventType"] == json!("execution_completed")
                && item["metadata"]["tool"] == json!(tool)),
        "Disabled {tool} request produced execution completion evidence"
    );
    Ok(())
}

fn assert_bytes(path: &Path, expected: &[u8]) -> E2eResult<()> {
    let actual = fs::read(path).map_err(|_| E2eError::new("read filesystem assertion failed"))?;
    check!(
        actual == expected,
        "filesystem bytes did not match the expected result"
    );
    Ok(())
}
