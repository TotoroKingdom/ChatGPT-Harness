//! M01 real Runtime -> Host notification/approval integration coverage.
//!
//! This runner deliberately starts the packaged/release Runtime executable and
//! drives its real MCP and control protocol. The only test double is the
//! notification sink at the OS boundary, which is the sink explicitly allowed
//! by the M01 Integration plan; Runtime approval state and activity events are
//! never synthesized for the primary cases.

use super::m2_e2e::{runtime_binary, McpClient, PendingMcpCall, RuntimeProcess};
use super::runtime_config_snapshot;
use crate::desktop::notifications::{
    format_approval_notification, route_notification_click, ApprovalClickRoute,
    ApprovalNotificationCoordinator,
};
use harness_control_protocol::{
    ActivityEvent, ActivityEventType, ActivityListResult, AppConfig, ApprovalDecision,
    ApprovalRequest, ApprovalStatus, ApprovalsListResult, PolicyDecision,
};
use serde_json::json;
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(25);
const POLL_TIMEOUT: Duration = Duration::from_secs(5);
const ACTIVITY_DRAIN_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn activity_sink_dispatch_does_not_block_runtime_reader() {
    let supervisor = super::RuntimeSupervisor::new();
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let release_receiver = Arc::new(std::sync::Mutex::new(release_receiver));
    let (returned_sender, returned_receiver) = mpsc::channel();

    supervisor.set_activity_sink(Arc::new(move |_event| {
        started_sender.send(()).expect("activity callback started");
        release_receiver
            .lock()
            .expect("activity callback release lock")
            .recv()
            .expect("activity callback release signal");
    }));
    let sink = supervisor
        .inner
        .lock()
        .expect("runtime supervisor lock")
        .activity_sink
        .clone()
        .expect("activity sink installed");
    let event = ActivityEvent {
        event_id: "m01-reader-dispatch".into(),
        sequence: harness_control_protocol::DecimalU64::new(1),
        event_type: harness_control_protocol::ActivityEventType::ApprovalRequested,
        request_id: Some("m01-request".into()),
        timestamp: "2026-10-24T00:00:00Z".into(),
        summary: "safe".into(),
        metadata: Default::default(),
    };

    let invocation = thread::spawn(move || {
        sink(event);
        returned_sender
            .send(())
            .expect("activity dispatch returned");
    });
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("activity callback did not start");
    let returned = returned_receiver
        .recv_timeout(Duration::from_millis(100))
        .is_ok();
    release_sender.send(()).expect("release activity callback");
    invocation.join().expect("activity dispatch thread");

    assert!(
        returned,
        "activity sink callback blocked the Runtime reader dispatch"
    );
}

#[test]
fn activity_sink_preserves_host_refresh_order() {
    let supervisor = super::RuntimeSupervisor::new();
    let (delivered_sender, delivered_receiver) = mpsc::channel();
    supervisor.set_activity_sink(Arc::new(move |event| {
        delivered_sender
            .send(event.sequence)
            .expect("activity sequence receiver");
    }));
    let sink = supervisor
        .inner
        .lock()
        .expect("runtime supervisor lock")
        .activity_sink
        .clone()
        .expect("activity sink installed");
    let event = |sequence| ActivityEvent {
        event_id: format!("m01-ordered-{sequence}"),
        sequence: harness_control_protocol::DecimalU64::new(sequence),
        event_type: ActivityEventType::ApprovalRequested,
        request_id: Some(format!("m01-request-{sequence}")),
        timestamp: "2026-10-24T00:00:00Z".into(),
        summary: "safe".into(),
        metadata: Default::default(),
    };

    sink(event(1));
    sink(event(2));
    assert_eq!(
        delivered_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("first ordered activity"),
        harness_control_protocol::DecimalU64::new(1)
    );
    assert_eq!(
        delivered_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("second ordered activity"),
        harness_control_protocol::DecimalU64::new(2)
    );
}

/// This is a black-box probe for the real Runtime control stream. It starts a
/// production Runtime executable, establishes the real activity subscription,
/// then creates a real MCP Pending approval. `approvals.list` is intentionally
/// the only control request during the observation window because that method
/// does not drain the Runtime activity channel. The assertion records whether
/// an unsolicited `ApprovalRequested` notification reached stdout before an
/// explicit `activity.list` drain.
#[test]
#[ignore = "M01 Runtime activity-drain probe requires an explicitly built candidate Runtime"]
fn m01_real_pending_activity_requires_control_drain() {
    if let Err(error) = run_pending_activity_drain_probe() {
        panic!("M01 Runtime activity-drain probe failed: {error}");
    }
}

fn run_pending_activity_drain_probe() -> IntegrationResult<()> {
    let fixture = Fixture::new()?;
    let binary = runtime_binary().map_err(|error| error.to_string())?;
    let mut config = crate::config::ConfigStore::safe_defaults();
    config.workspace.root_path = fixture.workspace.display().to_string();
    config.policies.tool_defaults.write = PolicyDecision::Ask;

    let (mut runtime, mut mcp) = ActivityProbeRuntime::start(&binary, &config)?;
    // This is the only subscription before Pending is created. It replays the
    // initial RuntimeStarted event and then returns; it is not a live stream.
    runtime.control_request("activity.subscribe", json!({}))?;
    runtime.clear_activity_events();

    let pending = mcp.pending_tool(
        "write",
        json!({
            "path": "m01-activity-drain.txt",
            "content": "M01 activity drain probe"
        }),
    );

    // Polling approvals.list is allowed here: it reads the authoritative
    // Pending projection, but server.rs does not call drain_activity_events()
    // for this method. The probe records any unsolicited activity.event that
    // arrives while these requests are in flight.
    let deadline = Instant::now() + POLL_TIMEOUT;
    let approval = loop {
        let value = runtime.control_request("approvals.list", json!({ "status": "pending" }))?;
        if let Some(approval) = value
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .cloned()
        {
            break approval;
        }
        if Instant::now() >= deadline {
            return Err("real MCP call did not create a Pending approval".into());
        }
        thread::sleep(POLL_INTERVAL);
    };
    let request_id = approval
        .get("requestId")
        .and_then(Value::as_str)
        .ok_or_else(|| "Pending approval omitted requestId".to_owned())?
        .to_owned();

    // Allow a delayed notification to arrive, while still avoiding any
    // activity.list/subscribe drain request.
    runtime.observe_for(Duration::from_millis(500));
    check(
        !runtime.has_approval_event(&request_id),
        "ApprovalRequested was pushed before an activity drain",
    )?;

    // The first explicit drain materializes the channel and returns the real
    // ApprovalRequested evidence for the same request.
    let value = runtime.control_request("activity.list", json!({ "limit": 256 }))?;
    let activity: ActivityListResult = serde_json::from_value(value)
        .map_err(|error| format!("decode drained ActivityEvent list: {error}"))?;
    check(
        activity.items.iter().any(|event| {
            event.request_id.as_deref() == Some(request_id.as_str())
                && event.event_type == ActivityEventType::ApprovalRequested
        }),
        "activity.list did not expose the real ApprovalRequested after drain",
    )?;

    runtime.control_request(
        "approvals.resolve",
        json!({ "requestId": request_id, "decision": "deny" }),
    )?;
    pending
        .receive()
        .map_err(|error| format!("real MCP Pending call did not resolve: {error}"))?;
    runtime.stop()?;
    println!(
        "M01-APPROVAL-ACTIVITY-DRAIN PASS: real Pending emitted no unsolicited ApprovalRequested before activity.list drain"
    );
    fixture.cleanup()
}

/// Minimal real-process control probe. The existing M2 RuntimeProcess hides
/// its stdout receiver behind a request helper that discards unmatched lines;
/// this local probe retains unsolicited activity.event lines so the black-box
/// test can distinguish a live push from a later control-request drain.
struct ActivityProbeRuntime {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<String>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<String>>,
    deferred: VecDeque<Value>,
    observed_events: Vec<ActivityEvent>,
    next_request_id: u64,
    endpoint: SocketAddr,
}

impl ActivityProbeRuntime {
    fn start(binary: &Path, config: &AppConfig) -> IntegrationResult<(Self, ActivityProbeMcp)> {
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
            .map_err(|error| format!("spawn Runtime activity probe: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Runtime activity probe stdin unavailable".to_owned())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Runtime activity probe stdout unavailable".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Runtime activity probe stderr unavailable".to_owned())?;
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
            deferred: VecDeque::new(),
            observed_events: Vec::new(),
            next_request_id: 0,
            endpoint: SocketAddr::from(([127, 0, 0, 1], 0)),
        };

        let initialize = runtime.control_request(
            "initialize",
            json!({
                "clientInfo": { "name": "m01-activity-probe", "title": "M01 activity probe", "version": "1" },
                "protocolVersion": 1,
                "capabilities": { "activityReplay": true },
                "runtimeConfig": serde_json::to_value(runtime_config_snapshot(config))
                    .map_err(|error| format!("serialize Runtime snapshot: {error}"))?
            }),
        )?;
        if initialize
            .get("runtimeId")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err("initialize omitted Runtime identity".into());
        }
        runtime.control_notification("initialized", json!({}))?;
        let endpoint = runtime.control_request("mcp.endpoint.get", json!({}))?;
        let endpoint_url = endpoint
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| "mcp.endpoint.get omitted endpoint URL".to_owned())?;
        runtime.endpoint = parse_activity_probe_endpoint(endpoint_url)?;
        runtime.control_request("runtime.status", json!({}))?;
        let mcp = ActivityProbeMcp::connect(runtime.endpoint)?;
        Ok((runtime, mcp))
    }

    fn control_request(&mut self, method: &str, params: Value) -> IntegrationResult<Value> {
        self.next_request_id += 1;
        let id = format!("m01-activity-probe-{}", self.next_request_id);
        self.write_json_line(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;
        loop {
            let message = self.receive_message()?;
            if message.get("id") != Some(&Value::String(id.clone())) {
                self.observe_message(message);
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(format!("Control RPC {method} returned an error: {error}"));
            }
            return message
                .get("result")
                .cloned()
                .ok_or_else(|| format!("Control RPC {method} omitted result"));
        }
    }

    fn control_notification(&mut self, method: &str, params: Value) -> IntegrationResult<()> {
        self.write_json_line(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    fn receive_message(&mut self) -> IntegrationResult<Value> {
        if let Some(message) = self.deferred.pop_front() {
            return Ok(message);
        }
        let line = self
            .messages
            .recv_timeout(ACTIVITY_DRAIN_PROBE_TIMEOUT)
            .map_err(|error| format!("Runtime control output unavailable: {error}"))?;
        serde_json::from_str(&line)
            .map_err(|error| format!("Runtime emitted invalid JSON: {error}"))
    }

    fn observe_message(&mut self, message: Value) {
        if let Some(params) = message
            .get("params")
            .filter(|_| message.get("method") == Some(&Value::String("activity.event".into())))
        {
            if let Ok(event) = serde_json::from_value::<ActivityEvent>(params.clone()) {
                self.observed_events.push(event);
                return;
            }
        }
        self.deferred.push_back(message);
    }

    fn clear_activity_events(&mut self) {
        self.observed_events.clear();
    }

    fn observe_for(&mut self, duration: Duration) {
        let deadline = Instant::now() + duration;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.messages.recv_timeout(remaining) {
                Ok(line) => match serde_json::from_str::<Value>(&line) {
                    Ok(message) => self.observe_message(message),
                    Err(_) => {}
                },
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn has_approval_event(&self, request_id: &str) -> bool {
        self.observed_events.iter().any(|event| {
            event.request_id.as_deref() == Some(request_id)
                && event.event_type == ActivityEventType::ApprovalRequested
        })
    }

    fn write_json_line(&mut self, value: &Value) -> IntegrationResult<()> {
        serde_json::to_writer(&mut self.stdin, value)
            .map_err(|error| format!("serialize control request: {error}"))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|error| format!("write control request: {error}"))?;
        self.stdin
            .flush()
            .map_err(|error| format!("flush control request: {error}"))
    }

    fn stop(&mut self) -> IntegrationResult<()> {
        let result = self.control_request("runtime.stop", json!({}))?;
        check(
            result.get("accepted") == Some(&Value::Bool(true)),
            "Runtime activity probe stop was not accepted",
        )?;
        let deadline = Instant::now() + ACTIVITY_DRAIN_PROBE_TIMEOUT;
        while Instant::now() < deadline {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| format!("poll Runtime activity probe exit: {error}"))?
            {
                if !status.success() {
                    return Err(format!(
                        "Runtime activity probe exited unsuccessfully: {status}"
                    ));
                }
                self.join_output_threads();
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err("Runtime activity probe did not exit after stop".into())
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

impl Drop for ActivityProbeRuntime {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.join_output_threads();
    }
}

struct ActivityProbeMcp {
    endpoint: SocketAddr,
    session_id: String,
    next_request_id: u64,
}

struct ActivityProbePendingCall {
    receiver: Receiver<IntegrationResult<Value>>,
    thread: JoinHandle<()>,
}

impl ActivityProbePendingCall {
    fn receive(self) -> IntegrationResult<Value> {
        let result = self
            .receiver
            .recv_timeout(ACTIVITY_DRAIN_PROBE_TIMEOUT)
            .map_err(|error| format!("Pending MCP response unavailable: {error}"))?;
        let _ = self.thread.join();
        result
    }
}

impl ActivityProbeMcp {
    fn connect(endpoint: SocketAddr) -> IntegrationResult<Self> {
        let response = activity_probe_http_request(
            endpoint,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "m01-activity-probe", "version": "1" }
                }
            }),
            None,
        )?;
        check(
            response.status == 200,
            "MCP initialize did not return HTTP 200",
        )?;
        let session_id = response
            .headers
            .get("mcp-session-id")
            .cloned()
            .ok_or_else(|| "MCP initialize omitted session id".to_owned())?;
        Ok(Self {
            endpoint,
            session_id,
            next_request_id: 1,
        })
    }

    fn pending_tool(&mut self, name: &str, arguments: Value) -> ActivityProbePendingCall {
        self.next_request_id += 1;
        let id = self.next_request_id;
        let endpoint = self.endpoint;
        let session_id = self.session_id.clone();
        let name = name.to_owned();
        let (sender, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let result = activity_probe_http_request(
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
        ActivityProbePendingCall { receiver, thread }
    }
}

struct ActivityProbeHttpResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl ActivityProbeHttpResponse {
    fn json_rpc(&self) -> IntegrationResult<Value> {
        if let Ok(value) = serde_json::from_slice(&self.body) {
            return Ok(value);
        }
        let body = std::str::from_utf8(&self.body)
            .map_err(|error| format!("MCP response was not UTF-8: {error}"))?;
        body.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .find_map(|line| serde_json::from_str(line.trim()).ok())
            .ok_or_else(|| "MCP response had no JSON-RPC payload".into())
    }
}

fn activity_probe_http_request(
    address: SocketAddr,
    message: Value,
    session_id: Option<&str>,
) -> IntegrationResult<ActivityProbeHttpResponse> {
    let body =
        serde_json::to_vec(&message).map_err(|error| format!("encode MCP request: {error}"))?;
    let mut stream = TcpStream::connect_timeout(&address, ACTIVITY_DRAIN_PROBE_TIMEOUT)
        .map_err(|error| format!("connect MCP endpoint: {error}"))?;
    stream
        .set_read_timeout(Some(ACTIVITY_DRAIN_PROBE_TIMEOUT))
        .map_err(|error| format!("set MCP read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(ACTIVITY_DRAIN_PROBE_TIMEOUT))
        .map_err(|error| format!("set MCP write timeout: {error}"))?;
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
        .map_err(|error| format!("write MCP request: {error}"))?;
    let mut raw = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => raw.extend_from_slice(&buffer[..read]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                break;
            }
            Err(error) => return Err(format!("read MCP response: {error}")),
        }
    }
    parse_activity_probe_http_response(&raw)
}

fn parse_activity_probe_http_response(raw: &[u8]) -> IntegrationResult<ActivityProbeHttpResponse> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "MCP response had no complete HTTP header".to_owned())?;
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
    let body = if headers
        .get("transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        decode_activity_probe_chunked(wire_body)?
    } else if let Some(length) = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
    {
        wire_body
            .get(..length)
            .ok_or_else(|| "MCP response body was truncated".to_owned())?
            .to_vec()
    } else {
        wire_body.to_vec()
    };
    Ok(ActivityProbeHttpResponse {
        status,
        headers,
        body,
    })
}

fn decode_activity_probe_chunked(mut wire: &[u8]) -> IntegrationResult<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line_end = wire
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| "MCP response chunk length missing".to_owned())?;
        let length = usize::from_str_radix(
            wire[..line_end]
                .split(|byte| *byte == b';')
                .next()
                .map(|value| String::from_utf8_lossy(value).trim().to_owned())
                .as_deref()
                .unwrap_or_default(),
            16,
        )
        .map_err(|error| format!("MCP response chunk length invalid: {error}"))?;
        wire = &wire[line_end + 2..];
        if length == 0 {
            return Ok(body);
        }
        if wire.len() < length + 2 || &wire[length..length + 2] != b"\r\n" {
            return Err("MCP response chunk was truncated".into());
        }
        body.extend_from_slice(&wire[..length]);
        wire = &wire[length + 2..];
    }
}

fn parse_activity_probe_endpoint(url: &str) -> IntegrationResult<SocketAddr> {
    let rest = url
        .strip_prefix("http://127.0.0.1:")
        .ok_or_else(|| "Runtime MCP endpoint was not loopback HTTP".to_owned())?;
    let (port, suffix) = rest
        .split_once("/mcp")
        .ok_or_else(|| "Runtime MCP endpoint path was not /mcp".to_owned())?;
    check(
        suffix.is_empty(),
        "Runtime MCP endpoint had an unexpected suffix",
    )?;
    let port = port
        .parse::<u16>()
        .map_err(|error| format!("Runtime MCP endpoint port invalid: {error}"))?;
    Ok(SocketAddr::from(([127, 0, 0, 1], port)))
}

/// Runs the production Host helper against a real Wry tray and a real Runtime
/// process. The test is Windows-only because the tray backend and the hidden
/// WebView event loop are the boundary under investigation. No configured
/// user window is created: the generated Context's window list is cleared and
/// its app-data root is redirected into the disposable M01 fixture.
#[cfg(all(windows, feature = "native-tray-tests"))]
#[test]
#[ignore = "M01 Wry tray integration requires a built release Runtime and Windows Shell"]
fn m01_real_wry_pending_helper_refreshes_tray_projection() {
    if let Err(error) = run_real_wry_pending_helper_probe() {
        panic!("M01 Wry tray helper probe failed: {error}");
    }
}

#[cfg(all(windows, feature = "native-tray-tests"))]
fn run_real_wry_pending_helper_probe() -> IntegrationResult<()> {
    use crate::desktop::PendingApprovalCount;
    use crate::harness::{HarnessComponentStatus, HarnessLifecycle, HarnessStatus};
    use crate::tunnel::{TunnelHealth, TunnelProcessManager, TunnelRecoveryStatus};
    use harness_control_protocol::ApprovalStatus;
    use std::sync::mpsc;

    let fixture = Fixture::new()?;
    let mut config = crate::config::ConfigStore::safe_defaults();
    config.workspace.root_path = fixture.workspace.display().to_string();
    config.config_revision = harness_control_protocol::DecimalU64::new(1);
    config.policies.tool_defaults.write = PolicyDecision::Ask;

    let runtime = super::RuntimeSupervisor::new();
    let tunnel = TunnelProcessManager::new();
    let harness = crate::harness::HarnessController::new(runtime.clone(), tunnel);
    let desktop = crate::desktop::DesktopLifecycleCoordinator::new(harness);
    let setup_desktop = desktop.clone();
    let app_data_root = fixture.base.join("wry-app-data");
    fs::create_dir_all(&app_data_root)
        .map_err(|error| format!("create isolated Wry app-data root: {error}"))?;
    let identifier = format!("com.chatgpt.harness.m01.{}", uuid::Uuid::new_v4());
    let (handle_sender, handle_receiver) = mpsc::channel();
    let app_thread = thread::spawn(move || {
        let mut context = tauri::generate_context!();
        context.config_mut().identifier = identifier;
        context.config_mut().app.windows.clear();
        context.config_mut().app.app_directories_override = Some(
            tauri::utils::config::AppDirectoriesOverride::Root(app_data_root),
        );
        let builder = tauri::Builder::<tauri::Wry>::default()
            .any_thread()
            .setup(move |app| {
                crate::desktop::tray::install(&app.handle(), setup_desktop.clone())?;
                handle_sender
                    .send(app.handle().clone())
                    .map_err(|error| std::io::Error::other(error.to_string()))?;
                Ok(())
            });
        let app = builder.build(context).expect("build isolated Wry M01 app");
        let _ = app.run_return(|_, _| {});
    });
    let handle = handle_receiver
        .recv_timeout(Duration::from_secs(15))
        .map_err(|error| format!("Wry setup did not publish AppHandle: {error}"))?;

    let result = (|| {
        let started = runtime
            .start(&handle, &config)
            .map_err(|error| format!("start production Runtime through Host: {error:?}"))?;
        let session = Some(started.runtime_id.clone());
        desktop.observe_status(HarnessStatus {
            lifecycle: HarnessLifecycle::Ready,
            runtime: HarnessComponentStatus::Ready,
            mcp: HarnessComponentStatus::Ready,
            tunnel: HarnessComponentStatus::Stopped,
            active_session_id: session,
            config_revision: Some(started.config_revision.get().to_string()),
            last_error: None,
            tunnel_health: TunnelHealth::stopped(),
            tunnel_readiness: None,
            tunnel_recovery: TunnelRecoveryStatus::default(),
        });

        let endpoint = parse_activity_probe_endpoint(
            &runtime
                .mcp_endpoint()
                .map_err(|error| format!("read production Runtime MCP endpoint: {error:?}"))?,
        )?;
        let mut mcp = ActivityProbeMcp::connect(endpoint)?;
        let pending = mcp.pending_tool(
            "write",
            json!({
                "path": "m01-wry-pending.txt",
                "content": "M01 Wry pending helper probe"
            }),
        );
        let deadline = Instant::now() + POLL_TIMEOUT;
        let approval = loop {
            let result = runtime
                .approvals_list(harness_control_protocol::ApprovalsListParams::default())
                .map_err(|error| format!("query production Pending approvals: {error:?}"))?;
            if let Some(approval) = result
                .items
                .into_iter()
                .find(|approval| approval.status == ApprovalStatus::Pending)
            {
                break approval;
            }
            if Instant::now() >= deadline {
                return Err("production Runtime did not create Pending approval".into());
            }
            thread::sleep(POLL_INTERVAL);
        };

        crate::desktop::tray::refresh_pending_from_runtime(&handle, &desktop, &runtime);
        check(
            desktop.snapshot().pending_approvals == PendingApprovalCount::Known(1),
            "production helper did not project Pending=1",
        )?;
        check(
            desktop.has_active_tray_flash(),
            "production helper did not start the tray flash worker",
        )?;
        check(
            handle.tray_by_id(crate::desktop::tray::TRAY_ID).is_some(),
            "production Wry app did not create the tray icon",
        )?;
        thread::sleep(Duration::from_millis(650));
        check(
            desktop.has_active_tray_flash(),
            "tray flash stopped while Pending remained authoritative",
        )?;

        runtime
            .approvals_resolve(harness_control_protocol::ApprovalsResolveParams {
                request_id: approval.request_id.clone(),
                decision: ApprovalDecision::Deny,
            })
            .map_err(|error| format!("deny production Pending approval: {error:?}"))?;
        pending
            .receive()
            .map_err(|error| format!("production MCP Pending call did not resolve: {error}"))?;
        crate::desktop::tray::refresh_pending_from_runtime(&handle, &desktop, &runtime);
        check(
            desktop.snapshot().pending_approvals == PendingApprovalCount::Known(0),
            "production helper did not project Pending=0 after deny",
        )?;
        thread::sleep(Duration::from_millis(650));
        check(
            !desktop.has_active_tray_flash(),
            "tray flash remained active after Pending returned to zero",
        )?;
        println!(
            "M01-WRY-TRAY-HELPER PASS: real Wry tray helper converged Pending 1 -> 0 and timer state"
        );
        Ok(())
    })();

    let _ = runtime.stop(&handle);
    handle.exit(0);
    let _ = app_thread.join();
    result
}

type IntegrationResult<T> = Result<T, String>;

#[derive(Default)]
struct RecordingNotificationSink {
    delivered: Vec<String>,
    failed: bool,
}

impl RecordingNotificationSink {
    fn deliver(&mut self, title: &str, body: &str) -> Result<(), ()> {
        if self.failed {
            return Err(());
        }
        self.delivered.push(format!("{title}\n{body}"));
        Ok(())
    }
}

#[test]
#[ignore = "M01 Integration requires the explicitly built candidate Runtime executable"]
fn m01_real_runtime_approval_integration_cases() {
    if let Err(error) = run_suite() {
        panic!("M01 real Runtime integration failed: {error}");
    }
}

fn run_suite() -> IntegrationResult<()> {
    let fixture = Fixture::new()?;
    let binary = runtime_binary().map_err(|error| error.to_string())?;
    let mut config = crate::config::ConfigStore::safe_defaults();
    config.workspace.root_path = fixture.workspace.display().to_string();
    config.policies.tool_defaults.write = PolicyDecision::Ask;

    let mut failures = Vec::new();
    let (mut runtime, mut mcp) =
        RuntimeProcess::start(&binary, &config, "M01-A").map_err(|error| error.to_string())?;

    report_case(
        "M01-INT-04",
        int_04_runtime_pending_projection(&mut runtime, &mut mcp),
        &mut failures,
    );
    report_case(
        "M01-INT-08",
        int_08_bootstrap_and_dedupe(&mut runtime, &mut mcp),
        &mut failures,
    );
    report_case(
        "M01-INT-09",
        int_09_delivery_failure_is_advisory(&mut runtime, &mut mcp),
        &mut failures,
    );
    report_case(
        "M01-INT-10",
        int_10_live_click_routing(&mut runtime, &mut mcp),
        &mut failures,
    );
    report_case(
        "M01-INT-11",
        int_11_bounded_private_content(&mut runtime, &mut mcp),
        &mut failures,
    );

    let old_runtime_id = runtime.runtime_id().to_owned();
    runtime.stop().map_err(|error| error.to_string())?;
    let (mut rollover_runtime, mut rollover_mcp) =
        RuntimeProcess::start(&binary, &config, "M01-B").map_err(|error| error.to_string())?;
    report_case(
        "M01-INT-08-session-rollover",
        int_08_session_rollover(&mut rollover_runtime, &mut rollover_mcp, &old_runtime_id),
        &mut failures,
    );
    rollover_runtime.stop().map_err(|error| error.to_string())?;

    fixture.cleanup()?;
    if !failures.is_empty() {
        return Err(format!("failed cases: {failures:?}"));
    }
    println!("M01_REAL_RUNTIME_APPROVAL_INTEGRATION=PASS");
    Ok(())
}

fn int_04_runtime_pending_projection(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let mut coordinator = ApprovalNotificationCoordinator::default();
    let initial = pending_approvals(runtime)?;
    let initial_refresh = coordinator.bootstrap(&initial);
    check(
        initial_refresh.pending_count.label() == "0",
        "initial Pending count was not Known(0)",
    )?;

    let pending = start_pending_write(mcp)?;
    let first = wait_for_pending(runtime)?;
    let event = wait_for_approval_event(runtime, &first[0].request_id)?;
    let refresh = coordinator.observe_activity(&event, &first);
    check(
        refresh.pending_count.label() == "1",
        "new Pending count was not Known(1)",
    )?;

    let second = pending_approvals(runtime)?;
    check(
        second.len() == 1,
        "Runtime approvals.list did not retain one Pending",
    )?;
    let duplicate = coordinator.observe_activity(&event, &second);
    check(
        duplicate.pending_count.label() == "1" && duplicate.notification.is_none(),
        "duplicate activity changed the authoritative Pending projection",
    )?;

    resolve(runtime, &first[0].request_id, ApprovalDecision::Deny)?;
    let terminal = wait_for_zero_pending(runtime)?;
    let terminal_event = wait_for_terminal_event(runtime, &first[0].request_id)?;
    let terminal_refresh = coordinator.observe_activity(&terminal_event, &terminal);
    check(
        terminal_refresh.pending_count.label() == "0",
        "resolved approval did not refresh Pending count to zero",
    )?;
    pending.receive().map_err(|error| error.to_string())?;

    let unavailable = coordinator.runtime_unavailable();
    check(
        unavailable.pending_count == crate::desktop::PendingApprovalCount::Unavailable,
        "Runtime loss did not project Unavailable",
    )?;
    record(
        "M01-INT-04",
        "real Runtime approvals.list + MCP ASK + authoritative refresh",
    )
}

fn int_08_bootstrap_and_dedupe(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let mut coordinator = ApprovalNotificationCoordinator::default();
    let pending = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let bootstrap = coordinator.bootstrap(&approvals);
    check(
        bootstrap.notification.is_none() && bootstrap.pending_count.label() == "1",
        "pre-existing Pending approval caused a notification burst",
    )?;
    let event = wait_for_approval_event(runtime, &approvals[0].request_id)?;
    let seeded_event = coordinator.observe_activity(&event, &approvals);
    check(
        seeded_event.notification.is_none(),
        "bootstrap-seeded approval notified again",
    )?;
    resolve(runtime, &approvals[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &approvals[0].request_id)?;
    pending.receive().map_err(|error| error.to_string())?;

    let next = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let event = wait_for_approval_event(runtime, &approvals[0].request_id)?;
    let first = coordinator.observe_activity(&event, &approvals);
    check(
        first.notification.is_some(),
        "new approval did not produce one decision",
    )?;
    let duplicate = coordinator.observe_activity(&event, &approvals);
    check(
        duplicate.notification.is_none(),
        "duplicate ApprovalRequested produced a second decision",
    )?;
    resolve(runtime, &approvals[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &approvals[0].request_id)?;
    next.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-08",
        "real Runtime bootstrap, ActivityEvent and dedupe",
    )
}

fn int_09_delivery_failure_is_advisory(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let mut coordinator = ApprovalNotificationCoordinator::default();
    let mut sink = RecordingNotificationSink::default();
    let pending = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let event = wait_for_approval_event(runtime, &approvals[0].request_id)?;
    let refresh = coordinator.observe_activity(&event, &approvals);
    let content = refresh
        .notification
        .ok_or_else(|| "real Pending did not create notification content".to_owned())?;
    sink.failed = true;
    check(
        sink.deliver(&content.title, &content.body).is_err(),
        "failure sink unexpectedly succeeded",
    )?;
    coordinator.record_delivery_failure();
    let current = pending_approvals(runtime)?;
    check(
        current.len() == 1 && current[0].status == ApprovalStatus::Pending,
        "notification delivery failure changed Runtime Approval truth",
    )?;
    let count = coordinator.refresh(&current);
    check(
        count.pending_count.label() == "1",
        "notification failure made Pending count unavailable/stale",
    )?;
    resolve(runtime, &current[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &current[0].request_id)?;
    pending.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-09",
        "recording sink failure leaves Runtime Pending and query path intact",
    )
}

fn int_10_live_click_routing(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let pending = start_pending_write(mcp)?;
    let current = wait_for_pending(runtime)?;
    let pending_route =
        route_notification_click(&current[0].runtime_id, &current[0].request_id, &current);
    check(
        pending_route == ApprovalClickRoute::Pending(current[0].request_id.clone()),
        "live Pending click was not actionable",
    )?;
    let id = current[0].request_id.clone();
    resolve(runtime, &id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    let terminal = all_approvals(runtime)?;
    let terminal_route = route_notification_click(&current[0].runtime_id, &id, &terminal);
    check(
        terminal_route == ApprovalClickRoute::Terminal(id.clone()),
        "click after Denied was not routed to terminal state",
    )?;
    let terminal_history = wait_for_terminal_event(runtime, &id)?;
    check(
        matches!(
            terminal_history.event_type,
            ActivityEventType::ApprovalDenied
        ),
        "terminal click route did not observe the Runtime terminal event",
    )?;
    pending.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-10",
        "click decision re-queried live Runtime Approval state",
    )
}

fn int_11_bounded_private_content(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
) -> IntegrationResult<()> {
    let pending = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let content = format_approval_notification(&approvals[0])
        .ok_or_else(|| "known write approval was not formatable".to_owned())?;
    let payload = format!("{}\n{}", content.title, content.body);
    let sentinels = [
        "M01_RAW_API_KEY_SENTINEL",
        "M01_PHYSICAL_PATH_SENTINEL",
        "M01_RAW_BASH_SENTINEL",
        "M01_FILE_CONTENT_SENTINEL",
        "M01_ERROR_DETAIL_SENTINEL",
        "M01_METADATA_JSON_SENTINEL",
    ];
    for sentinel in sentinels {
        check(
            !payload.contains(sentinel),
            &format!("notification payload leaked {sentinel}"),
        )?;
    }
    check(
        !payload.contains(&approvals[0].scope_summary),
        "notification payload serialized the full approval scope",
    )?;
    resolve(runtime, &approvals[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &approvals[0].request_id)?;
    pending.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-11",
        "real Approval formatted through bounded production formatter",
    )
}

fn int_08_session_rollover(
    runtime: &mut RuntimeProcess,
    mcp: &mut McpClient,
    old_runtime_id: &str,
) -> IntegrationResult<()> {
    let mut coordinator = ApprovalNotificationCoordinator::default();
    let pending = start_pending_write(mcp)?;
    let approvals = wait_for_pending(runtime)?;
    let event = wait_for_approval_event(runtime, &approvals[0].request_id)?;
    let refresh = coordinator.observe_activity(&event, &approvals);
    check(
        refresh.notification.is_some(),
        "new Runtime session did not notify",
    )?;
    check(
        approvals[0].runtime_id == runtime.runtime_id()
            && approvals[0].runtime_id != old_runtime_id,
        "approval identity did not isolate the active Runtime session",
    )?;
    resolve(runtime, &approvals[0].request_id, ApprovalDecision::Deny)?;
    wait_for_zero_pending(runtime)?;
    wait_for_terminal_event(runtime, &approvals[0].request_id)?;
    pending.receive().map_err(|error| error.to_string())?;
    record(
        "M01-INT-08-session-rollover",
        "new Runtime identity has an isolated dedupe key",
    )
}

fn start_pending_write(mcp: &mut McpClient) -> IntegrationResult<PendingMcpCall> {
    Ok(mcp.pending_tool(
        "write",
        json!({
            "path": "m01-pending.txt",
            "content": "M01 approval content"
        }),
    ))
}

fn all_approvals(runtime: &mut RuntimeProcess) -> IntegrationResult<Vec<ApprovalRequest>> {
    let value = runtime
        .control_request("approvals.list", json!({}))
        .map_err(|error| error.to_string())?;
    serde_json::from_value::<ApprovalsListResult>(value)
        .map(|result| result.items)
        .map_err(|error| format!("decode approvals.list: {error}"))
}

fn pending_approvals(runtime: &mut RuntimeProcess) -> IntegrationResult<Vec<ApprovalRequest>> {
    let value = runtime
        .control_request("approvals.list", json!({ "status": "pending" }))
        .map_err(|error| error.to_string())?;
    serde_json::from_value::<ApprovalsListResult>(value)
        .map(|result| result.items)
        .map_err(|error| format!("decode approvals.list pending: {error}"))
}

fn wait_for_pending(runtime: &mut RuntimeProcess) -> IntegrationResult<Vec<ApprovalRequest>> {
    wait_until(runtime, |items| !items.is_empty(), "Pending approval")
}

fn wait_for_zero_pending(runtime: &mut RuntimeProcess) -> IntegrationResult<Vec<ApprovalRequest>> {
    wait_until(runtime, |items| items.is_empty(), "zero Pending approvals")
}

fn wait_until(
    runtime: &mut RuntimeProcess,
    predicate: impl Fn(&[ApprovalRequest]) -> bool,
    label: &str,
) -> IntegrationResult<Vec<ApprovalRequest>> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        let current = pending_approvals(runtime)?;
        if predicate(&current) {
            return Ok(current);
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for {label}"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for_approval_event(
    runtime: &mut RuntimeProcess,
    request_id: &str,
) -> IntegrationResult<ActivityEvent> {
    wait_for_event(runtime, request_id, |event| {
        event.event_type == ActivityEventType::ApprovalRequested
    })
}

fn wait_for_terminal_event(
    runtime: &mut RuntimeProcess,
    request_id: &str,
) -> IntegrationResult<ActivityEvent> {
    wait_for_event(runtime, request_id, |event| {
        matches!(
            event.event_type,
            ActivityEventType::ApprovalDenied
                | ActivityEventType::ApprovalResolved
                | ActivityEventType::ApprovalExpired
                | ActivityEventType::ApprovalCancelled
        )
    })
}

fn wait_for_event(
    runtime: &mut RuntimeProcess,
    request_id: &str,
    predicate: impl Fn(&ActivityEvent) -> bool,
) -> IntegrationResult<ActivityEvent> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        let value = runtime
            .control_request("activity.list", json!({ "limit": 256 }))
            .map_err(|error| error.to_string())?;
        let result: ActivityListResult = serde_json::from_value(value)
            .map_err(|error| format!("decode activity.list: {error}"))?;
        if let Some(event) = result
            .items
            .into_iter()
            .find(|event| event.request_id.as_deref() == Some(request_id) && predicate(event))
        {
            return Ok(event);
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for activity event {request_id}"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn resolve(
    runtime: &mut RuntimeProcess,
    request_id: &str,
    decision: ApprovalDecision,
) -> IntegrationResult<()> {
    runtime
        .control_request(
            "approvals.resolve",
            json!({ "requestId": request_id, "decision": decision }),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn check(condition: bool, message: &str) -> IntegrationResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned())
    }
}

fn report_case(case_id: &str, result: IntegrationResult<()>, failures: &mut Vec<String>) {
    match result {
        Ok(()) => {
            println!("{case_id} PASS");
            record(
                case_id,
                "real Runtime/Host notification integration assertions passed",
            )
            .expect("write M01 integration evidence");
        }
        Err(error) => {
            println!("{case_id} FAIL: {error}");
            record(case_id, &format!("FAIL: {error}")).expect("write M01 integration evidence");
            failures.push(case_id.to_owned());
        }
    }
}

fn record(case_id: &str, evidence: &str) -> IntegrationResult<()> {
    let line = json!({
        "case": case_id,
        "evidence": evidence,
        "candidate": std::env::var("HARNESS_M01_CANDIDATE")
            .unwrap_or_else(|_| "unspecified".into()),
    });
    // Emit evidence to the test runner; disposable test output must not be archived in target/.
    println!("{line}");
    Ok(())
}

struct Fixture {
    base: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new() -> IntegrationResult<Self> {
        let base = std::env::temp_dir().join(format!(
            "chatgpt-harness-m01-real-runtime-{}",
            uuid::Uuid::new_v4()
        ));
        let workspace = base.join("workspace");
        fs::create_dir_all(&workspace).map_err(|error| format!("create fixture: {error}"))?;
        fs::write(workspace.join("m01-existing.txt"), b"M01 existing")
            .map_err(|error| format!("write fixture: {error}"))?;
        Ok(Self { base, workspace })
    }

    fn cleanup(&self) -> IntegrationResult<()> {
        if self.base.exists() {
            fs::remove_dir_all(&self.base).map_err(|error| format!("cleanup fixture: {error}"))?;
        }
        check(!self.base.exists(), "fixture cleanup left temporary data")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
