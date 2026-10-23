//! M00 controlled Agent/Model evaluation feasibility spike.
//!
//! The runner is provider-neutral and intentionally isolated from the
//! production Runtime. It can use a scripted adapter for local mechanics or a
//! user-supplied JSON-over-HTTP adapter during explicitly authorized E2E. The
//! final-state scorer is independent of MCP transport success.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fmt, fs,
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use url::Url;

pub const RUNNER_VERSION: &str = "m00-evaluation-poc-0.1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    InvalidCase(String),
    Adapter(String),
    Mcp(String),
    Scorer(String),
    Limit(String),
    Io(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCase(message) => write!(formatter, "invalid evaluation case: {message}"),
            Self::Adapter(message) => write!(formatter, "agent adapter error: {message}"),
            Self::Mcp(message) => write!(formatter, "MCP transport error: {message}"),
            Self::Scorer(message) => write!(formatter, "final-state scorer error: {message}"),
            Self::Limit(message) => write!(formatter, "evaluation limit: {message}"),
            Self::Io(message) => write!(formatter, "I/O error: {message}"),
        }
    }
}

impl std::error::Error for EvalError {}

impl From<io::Error> for EvalError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalLimits {
    pub max_steps: u32,
    pub timeout_ms: u64,
    pub max_spend_micros: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalCase {
    pub case_id: String,
    pub version: String,
    pub task: String,
    pub workspace_fixture_hash: String,
    pub allowed_tools: Vec<String>,
    pub expected_tool: Option<String>,
    pub argument_constraints: Value,
    pub expected_final_state: Value,
    pub policy_expectation: String,
    pub limits: EvalLimits,
    pub adapter_id: String,
    pub model_identity: String,
    pub adapter_version: String,
    pub tool_schema_hash: Option<String>,
    pub scorer_version: String,
}

impl EvalCase {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EvalError> {
        let text = fs::read_to_string(path).map_err(|error| EvalError::Io(error.to_string()))?;
        serde_json::from_str(&text).map_err(|error| EvalError::InvalidCase(error.to_string()))
    }

    pub fn hash(&self) -> String {
        sha256_json(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentTurn {
    ToolCall { call: ToolCall },
    Final { answer: String, stop_reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpToolResult {
    pub is_error: bool,
    pub structured_content: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolObservation {
    pub call: ToolCall,
    pub selected_tool_allowed: bool,
    pub arguments_schema_valid: bool,
    pub forwarded_to_runtime: bool,
    pub runtime_call_succeeded: bool,
    pub runtime_result: Option<Value>,
}

pub trait McpTransport {
    fn tools_list(&mut self) -> Result<Vec<ToolDescriptor>, EvalError>;
    fn call_tool(&mut self, call: &ToolCall) -> Result<McpToolResult, EvalError>;
}

pub struct AgentTurnRequest<'a> {
    pub case: &'a EvalCase,
    pub tools: &'a [ToolDescriptor],
    pub observations: &'a [ToolObservation],
}

pub trait AgentAdapter {
    fn identity(&self) -> &str;
    fn next_turn(&mut self, request: AgentTurnRequest<'_>) -> Result<AgentTurn, EvalError>;
}

pub trait FinalStateProbe {
    fn observe(&self, case: &EvalCase) -> Result<Value, EvalError>;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalReport {
    pub runner_version: String,
    pub case_id: String,
    pub case_hash: String,
    pub fixture_hash: String,
    pub adapter_id: String,
    pub model_identity: String,
    pub adapter_version: String,
    pub tool_schema_hash: String,
    pub scorer_version: String,
    pub selected_tool_allowed: Option<bool>,
    pub selected_tool_correct: Option<bool>,
    pub arguments_schema_valid: Option<bool>,
    pub runtime_call_outcome: Option<String>,
    pub task_completed: bool,
    pub expected_final_state: Value,
    pub observed_final_state: Value,
    pub actual_tool_calls: Vec<ToolObservation>,
    pub call_count: u32,
    pub termination_reason: String,
}

pub fn run_case<M, A, P>(
    case: &EvalCase,
    transport: &mut M,
    adapter: &mut A,
    probe: &P,
) -> Result<EvalReport, EvalError>
where
    M: McpTransport,
    A: AgentAdapter,
    P: FinalStateProbe,
{
    if case.limits.max_steps == 0 {
        return Err(EvalError::InvalidCase("max_steps must be positive".into()));
    }
    let started = Instant::now();
    let tools = transport.tools_list()?;
    let tool_schema_hash = sha256_json(&tools);
    let mut observations = Vec::new();
    let mut selected_tool_allowed = None;
    let mut selected_tool_correct = None;
    let mut arguments_schema_valid = None;
    let mut runtime_call_outcome = None;
    let mut termination_reason = "max_steps".to_owned();

    for _step in 0..case.limits.max_steps {
        if started.elapsed() > Duration::from_millis(case.limits.timeout_ms) {
            termination_reason = "timeout".into();
            break;
        }
        let turn = adapter.next_turn(AgentTurnRequest {
            case,
            tools: &tools,
            observations: &observations,
        })?;
        match turn {
            AgentTurn::Final { stop_reason, .. } => {
                termination_reason = format!("agent_final:{stop_reason}");
                break;
            }
            AgentTurn::ToolCall { call } => {
                let allowed = case.allowed_tools.iter().any(|tool| tool == &call.name);
                let correct = case
                    .expected_tool
                    .as_ref()
                    .is_none_or(|expected| expected == &call.name);
                let schema_valid = tools
                    .iter()
                    .find(|tool| tool.name == call.name)
                    .is_some_and(|tool| validate_arguments(&tool.input_schema, &call.arguments));
                selected_tool_allowed.get_or_insert(allowed);
                selected_tool_correct.get_or_insert(correct);
                arguments_schema_valid.get_or_insert(schema_valid);
                if !allowed || !schema_valid {
                    runtime_call_outcome.get_or_insert("not_forwarded_invalid".into());
                    observations.push(ToolObservation {
                        call,
                        selected_tool_allowed: allowed,
                        arguments_schema_valid: schema_valid,
                        forwarded_to_runtime: false,
                        runtime_call_succeeded: false,
                        runtime_result: None,
                    });
                    continue;
                }
                let result = transport.call_tool(&call)?;
                let succeeded = !result.is_error;
                runtime_call_outcome = Some(if succeeded {
                    "success".into()
                } else {
                    "error".into()
                });
                observations.push(ToolObservation {
                    call,
                    selected_tool_allowed: allowed,
                    arguments_schema_valid: schema_valid,
                    forwarded_to_runtime: true,
                    runtime_call_succeeded: succeeded,
                    runtime_result: Some(result.structured_content),
                });
            }
        }
    }

    let observed_final_state = probe.observe(case)?;
    let task_completed = observed_final_state == case.expected_final_state;
    Ok(EvalReport {
        runner_version: RUNNER_VERSION.into(),
        case_id: case.case_id.clone(),
        case_hash: case.hash(),
        fixture_hash: case.workspace_fixture_hash.clone(),
        adapter_id: adapter.identity().into(),
        model_identity: case.model_identity.clone(),
        adapter_version: case.adapter_version.clone(),
        tool_schema_hash,
        scorer_version: case.scorer_version.clone(),
        selected_tool_allowed,
        selected_tool_correct,
        arguments_schema_valid,
        runtime_call_outcome,
        task_completed,
        expected_final_state: case.expected_final_state.clone(),
        observed_final_state,
        actual_tool_calls: observations,
        call_count: selected_tool_allowed.map_or(0, |_| {
            // Each observation is an emitted call, including calls rejected
            // before forwarding to Runtime.
            // The value is replaced below to keep the report explicit.
            0
        }),
        termination_reason,
    })
    .map(|mut report| {
        report.call_count = report.actual_tool_calls.len() as u32;
        report
    })
}

pub fn validate_arguments(schema: &Value, arguments: &Value) -> bool {
    let Some(arguments) = arguments.as_object() else {
        return false;
    };
    let Some(schema) = schema.as_object() else {
        return false;
    };
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return false;
    }
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if required.iter().any(|name| {
        name.as_str()
            .is_none_or(|name| !arguments.contains_key(name))
    }) {
        return false;
    }
    if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|value| value.keys().collect::<Vec<_>>())
            .unwrap_or_default();
        if arguments
            .keys()
            .any(|key| !properties.iter().any(|name| *name == key))
        {
            return false;
        }
    }
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return true;
    };
    arguments.iter().all(|(name, value)| {
        let Some(expected) = properties
            .get(name)
            .and_then(Value::as_object)
            .and_then(|property| property.get("type"))
            .and_then(Value::as_str)
        else {
            return true;
        };
        match expected {
            "string" => value.is_string(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            "object" => value.is_object(),
            "array" => value.is_array(),
            _ => false,
        }
    })
}

pub fn sha256_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("evaluation identity must serialize");
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

pub struct ScriptedAgentAdapter {
    identity: String,
    turns: VecDeque<AgentTurn>,
}

impl ScriptedAgentAdapter {
    pub fn new(identity: impl Into<String>, turns: impl IntoIterator<Item = AgentTurn>) -> Self {
        Self {
            identity: identity.into(),
            turns: turns.into_iter().collect(),
        }
    }
}

impl AgentAdapter for ScriptedAgentAdapter {
    fn identity(&self) -> &str {
        &self.identity
    }

    fn next_turn(&mut self, _request: AgentTurnRequest<'_>) -> Result<AgentTurn, EvalError> {
        self.turns
            .pop_front()
            .ok_or_else(|| EvalError::Adapter("scripted adapter exhausted".into()))
    }
}

#[derive(Clone, Debug)]
pub struct InMemoryMcpTransport {
    tools: Vec<ToolDescriptor>,
    result: McpToolResult,
    calls: Vec<ToolCall>,
}

impl InMemoryMcpTransport {
    pub fn new(tools: Vec<ToolDescriptor>, result: McpToolResult) -> Self {
        Self {
            tools,
            result,
            calls: Vec::new(),
        }
    }

    pub fn calls(&self) -> &[ToolCall] {
        &self.calls
    }
}

impl McpTransport for InMemoryMcpTransport {
    fn tools_list(&mut self) -> Result<Vec<ToolDescriptor>, EvalError> {
        Ok(self.tools.clone())
    }

    fn call_tool(&mut self, call: &ToolCall) -> Result<McpToolResult, EvalError> {
        self.calls.push(call.clone());
        Ok(self.result.clone())
    }
}

pub struct StaticFinalStateProbe {
    state: Value,
}

impl StaticFinalStateProbe {
    pub fn new(state: Value) -> Self {
        Self { state }
    }
}

impl FinalStateProbe for StaticFinalStateProbe {
    fn observe(&self, _case: &EvalCase) -> Result<Value, EvalError> {
        Ok(self.state.clone())
    }
}

pub struct WorkspaceFinalStateProbe {
    root: PathBuf,
}

impl WorkspaceFinalStateProbe {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl FinalStateProbe for WorkspaceFinalStateProbe {
    fn observe(&self, case: &EvalCase) -> Result<Value, EvalError> {
        let path = case
            .expected_final_state
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                EvalError::Scorer("expected final state needs a relative path".into())
            })?;
        let relative = Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(EvalError::Scorer(
                "final-state path must stay relative".into(),
            ));
        }
        let content = fs::read_to_string(self.root.join(relative))?;
        Ok(json!({ "path": path, "content": content }))
    }
}

pub struct HttpMcpTransport {
    endpoint: Url,
    session_id: Option<String>,
    next_id: u64,
    timeout: Duration,
}

impl HttpMcpTransport {
    pub fn new(endpoint: impl AsRef<str>) -> Result<Self, EvalError> {
        let endpoint = Url::parse(endpoint.as_ref())
            .map_err(|error| EvalError::Mcp(format!("invalid endpoint: {error}")))?;
        if endpoint.scheme() != "http" {
            return Err(EvalError::Mcp(
                "M00 loopback client only supports http".into(),
            ));
        }
        Ok(Self {
            endpoint,
            session_id: None,
            next_id: 1,
            timeout: Duration::from_secs(10),
        })
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, EvalError> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let response = post_json(
            &self.endpoint,
            &request,
            self.session_id.as_deref(),
            None,
            self.timeout,
        )?;
        if let Some(session_id) = response.session_id {
            self.session_id = Some(session_id);
        }
        let value = response.json;
        if value.get("error").is_some() {
            return Err(EvalError::Mcp(value.to_string()));
        }
        Ok(value)
    }
}

impl McpTransport for HttpMcpTransport {
    fn tools_list(&mut self) -> Result<Vec<ToolDescriptor>, EvalError> {
        if self.session_id.is_none() {
            let initialized = self.request(
                "initialize",
                json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "m00-evaluation-poc", "version": RUNNER_VERSION }
                }),
            )?;
            if initialized.get("result").is_none() {
                return Err(EvalError::Mcp("initialize did not return result".into()));
            }
        }
        let value = self.request("tools/list", json!({}))?;
        let tools = value["result"]["tools"]
            .as_array()
            .ok_or_else(|| EvalError::Mcp("tools/list result has no tools array".into()))?;
        tools
            .iter()
            .map(|tool| {
                Ok(ToolDescriptor {
                    name: tool["name"]
                        .as_str()
                        .ok_or_else(|| EvalError::Mcp("tool has no name".into()))?
                        .into(),
                    input_schema: tool
                        .get("inputSchema")
                        .cloned()
                        .unwrap_or_else(|| json!({ "type": "object" })),
                })
            })
            .collect()
    }

    fn call_tool(&mut self, call: &ToolCall) -> Result<McpToolResult, EvalError> {
        let value = self.request(
            "tools/call",
            json!({ "name": call.name, "arguments": call.arguments }),
        )?;
        Ok(McpToolResult {
            is_error: value["result"]["isError"].as_bool().unwrap_or(false),
            structured_content: value["result"]
                .get("structuredContent")
                .cloned()
                .unwrap_or_else(|| value["result"].clone()),
        })
    }
}

pub struct HttpJsonAgentAdapter {
    endpoint: Url,
    identity: String,
    auth_token: Option<String>,
    timeout: Duration,
}

impl HttpJsonAgentAdapter {
    pub fn new(
        endpoint: impl AsRef<str>,
        identity: impl Into<String>,
        auth_token: Option<String>,
    ) -> Result<Self, EvalError> {
        let endpoint = Url::parse(endpoint.as_ref())
            .map_err(|error| EvalError::Adapter(format!("invalid adapter endpoint: {error}")))?;
        if endpoint.scheme() != "http" {
            return Err(EvalError::Adapter(
                "M00 JSON adapter only supports an explicitly configured http endpoint".into(),
            ));
        }
        Ok(Self {
            endpoint,
            identity: identity.into(),
            auth_token,
            timeout: Duration::from_secs(30),
        })
    }
}

impl AgentAdapter for HttpJsonAgentAdapter {
    fn identity(&self) -> &str {
        &self.identity
    }

    fn next_turn(&mut self, request: AgentTurnRequest<'_>) -> Result<AgentTurn, EvalError> {
        let body = json!({
            "case": request.case,
            "tools": request.tools,
            "observations": request.observations,
        });
        let response = post_json(
            &self.endpoint,
            &body,
            None,
            self.auth_token.as_deref(),
            self.timeout,
        )?;
        let value = response.json;
        if let Some(call) = value.get("toolCall").or_else(|| value.get("tool_call")) {
            let name = call["name"]
                .as_str()
                .ok_or_else(|| EvalError::Adapter("adapter tool call has no name".into()))?;
            return Ok(AgentTurn::ToolCall {
                call: ToolCall {
                    name: name.into(),
                    arguments: call.get("arguments").cloned().unwrap_or_else(|| json!({})),
                },
            });
        }
        Ok(AgentTurn::Final {
            answer: value["finalAnswer"]
                .as_str()
                .or_else(|| value["final_answer"].as_str())
                .unwrap_or_default()
                .into(),
            stop_reason: value["stopReason"]
                .as_str()
                .or_else(|| value["stop_reason"].as_str())
                .unwrap_or("adapter_final")
                .into(),
        })
    }
}

struct HttpResponse {
    json: Value,
    session_id: Option<String>,
}

fn post_json(
    endpoint: &Url,
    body: &Value,
    session_id: Option<&str>,
    auth_token: Option<&str>,
    timeout: Duration,
) -> Result<HttpResponse, EvalError> {
    let host = endpoint
        .host_str()
        .ok_or_else(|| EvalError::Mcp("endpoint has no host".into()))?;
    let port = endpoint
        .port_or_known_default()
        .ok_or_else(|| EvalError::Mcp("endpoint has no port".into()))?;
    let mut addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| EvalError::Mcp(error.to_string()))?;
    let address = addresses
        .next()
        .ok_or_else(|| EvalError::Mcp("endpoint did not resolve".into()))?;
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| EvalError::Mcp(error.to_string()))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let payload = serde_json::to_vec(body).map_err(|error| EvalError::Mcp(error.to_string()))?;
    let path = if endpoint.path().is_empty() {
        "/"
    } else {
        endpoint.path()
    };
    let host_header = endpoint
        .host_str()
        .map(|host| format!("{host}:{port}"))
        .unwrap_or_else(|| format!("127.0.0.1:{port}"));
    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host_header}\r\nConnection: close\r\nAccept: application/json, text/event-stream\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        payload.len()
    );
    if let Some(session_id) = session_id {
        request.push_str(&format!(
            "Mcp-Session-Id: {session_id}\r\nMCP-Protocol-Version: 2025-03-26\r\n"
        ));
    }
    if let Some(auth_token) = auth_token {
        request.push_str(&format!("Authorization: Bearer {auth_token}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    parse_http_response(&raw)
}

fn parse_http_response(raw: &[u8]) -> Result<HttpResponse, EvalError> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| EvalError::Mcp("HTTP response has no header terminator".into()))?;
    let header_text = std::str::from_utf8(&raw[..header_end])
        .map_err(|error| EvalError::Mcp(error.to_string()))?;
    let mut lines = header_text.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| EvalError::Mcp("HTTP response has no status".into()))?;
    if status >= 400 {
        return Err(EvalError::Mcp(format!("HTTP status {status}")));
    }
    let mut transfer_chunked = false;
    let mut session_id = None;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
            {
                transfer_chunked = true;
            }
            if name.eq_ignore_ascii_case("mcp-session-id") {
                session_id = Some(value.trim().to_owned());
            }
        }
    }
    let body = &raw[header_end + 4..];
    let body = if transfer_chunked {
        decode_chunked(body)?
    } else {
        body.to_vec()
    };
    let text = std::str::from_utf8(&body).map_err(|error| EvalError::Mcp(error.to_string()))?;
    let json = serde_json::from_str(text)
        .or_else(|_| {
            text.lines()
                .filter_map(|line| line.trim().strip_prefix("data:"))
                .find_map(|line| serde_json::from_str(line.trim()).ok())
                .ok_or_else(|| {
                    serde_json::Error::io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "no JSON/SSE data",
                    ))
                })
        })
        .map_err(|error| EvalError::Mcp(error.to_string()))?;
    Ok(HttpResponse { json, session_id })
}

fn decode_chunked(mut bytes: &[u8]) -> Result<Vec<u8>, EvalError> {
    let mut output = Vec::new();
    loop {
        let line_end = bytes
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| EvalError::Mcp("invalid chunked response".into()))?;
        let length = usize::from_str_radix(
            std::str::from_utf8(&bytes[..line_end])
                .map_err(|error| EvalError::Mcp(error.to_string()))?
                .split(';')
                .next()
                .unwrap_or("0")
                .trim(),
            16,
        )
        .map_err(|error| EvalError::Mcp(error.to_string()))?;
        bytes = &bytes[line_end + 2..];
        if length == 0 {
            return Ok(output);
        }
        if bytes.len() < length + 2 {
            return Err(EvalError::Mcp("truncated chunked response".into()));
        }
        output.extend_from_slice(&bytes[..length]);
        bytes = &bytes[length + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn case(expected: Value) -> EvalCase {
        EvalCase {
            case_id: "m00-read-file-smoke".into(),
            version: "1".into(),
            task: "Inspect the known README fixture and report its content.".into(),
            workspace_fixture_hash: "fixture-sha256-m00".into(),
            allowed_tools: vec!["read_file".into()],
            expected_tool: Some("read_file".into()),
            argument_constraints: json!({
                "path": { "type": "relative_workspace_path", "required": true }
            }),
            expected_final_state: expected,
            policy_expectation: "ALLOW".into(),
            limits: EvalLimits {
                max_steps: 3,
                timeout_ms: 2_000,
                max_spend_micros: 0,
            },
            adapter_id: "scripted-test-adapter".into(),
            model_identity: "local-scripted-agent".into(),
            adapter_version: "m00-test-1".into(),
            tool_schema_hash: None,
            scorer_version: "m00-scorer-1".into(),
        }
    }

    fn read_tool() -> ToolDescriptor {
        ToolDescriptor {
            name: "read_file".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
                "additionalProperties": false
            }),
        }
    }

    #[test]
    fn case_hash_and_fixture_identity_are_stable() {
        let test_case = case(json!({ "path": "README.md", "content": "fixture" }));
        assert_eq!(test_case.hash().len(), 64);
        assert_eq!(sha256_json(&test_case), test_case.hash());
        assert_eq!(RUNNER_VERSION, "m00-evaluation-poc-0.1");
    }

    #[test]
    fn versioned_smoke_case_and_workspace_fixture_hash_are_loadable() {
        let case_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cases/read-file-smoke.json");
        let fixture_path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/read-file/README.md");
        let test_case = EvalCase::load(case_path).unwrap();
        assert_eq!(test_case.case_id, "m00-read-file-smoke");
        assert_eq!(test_case.limits.max_steps, 4);
        assert_eq!(test_case.expected_tool.as_deref(), Some("read_file"));
        let digest = Sha256::digest(fs::read(fixture_path).unwrap());
        assert_eq!(hex::encode(digest), test_case.workspace_fixture_hash);
    }

    #[test]
    fn schema_validation_accepts_expected_arguments_and_rejects_unknown_fields() {
        let schema = read_tool().input_schema;
        assert!(validate_arguments(&schema, &json!({ "path": "README.md" })));
        assert!(!validate_arguments(
            &schema,
            &json!({ "path": "README.md", "secret": "x" })
        ));
        assert!(!validate_arguments(&schema, &json!({})));
        assert!(!validate_arguments(&schema, &json!({ "path": 7 })));
    }

    #[test]
    fn runner_separates_tool_selection_arguments_transport_and_final_state() {
        let expected = json!({ "path": "README.md", "content": "fixture" });
        let test_case = case(expected.clone());
        let mut transport = InMemoryMcpTransport::new(
            vec![read_tool()],
            McpToolResult {
                is_error: false,
                structured_content: json!({ "content": "fixture" }),
            },
        );
        let mut adapter = ScriptedAgentAdapter::new(
            "local-scripted-adapter",
            [
                AgentTurn::ToolCall {
                    call: ToolCall {
                        name: "read_file".into(),
                        arguments: json!({ "path": "README.md" }),
                    },
                },
                AgentTurn::Final {
                    answer: "done".into(),
                    stop_reason: "complete".into(),
                },
            ],
        );
        let probe = StaticFinalStateProbe::new(expected);
        let report = run_case(&test_case, &mut transport, &mut adapter, &probe).unwrap();
        assert_eq!(report.selected_tool_allowed, Some(true));
        assert_eq!(report.selected_tool_correct, Some(true));
        assert_eq!(report.arguments_schema_valid, Some(true));
        assert_eq!(report.runtime_call_outcome.as_deref(), Some("success"));
        assert!(report.task_completed);
        assert_eq!(report.call_count, 1);
        assert_eq!(transport.calls().len(), 1);
    }

    #[test]
    fn successful_transport_does_not_make_wrong_final_state_a_success() {
        let test_case = case(json!({ "path": "README.md", "content": "expected" }));
        let mut transport = InMemoryMcpTransport::new(
            vec![read_tool()],
            McpToolResult {
                is_error: false,
                structured_content: json!({ "content": "transport ok" }),
            },
        );
        let mut adapter = ScriptedAgentAdapter::new(
            "local-scripted-adapter",
            [
                AgentTurn::ToolCall {
                    call: ToolCall {
                        name: "read_file".into(),
                        arguments: json!({ "path": "README.md" }),
                    },
                },
                AgentTurn::ToolCall {
                    call: ToolCall {
                        name: "read_file".into(),
                        arguments: json!({ "path": "README.md" }),
                    },
                },
                AgentTurn::ToolCall {
                    call: ToolCall {
                        name: "read_file".into(),
                        arguments: json!({ "path": "README.md" }),
                    },
                },
            ],
        );
        let probe = StaticFinalStateProbe::new(json!({ "path": "README.md", "content": "wrong" }));
        let report = run_case(&test_case, &mut transport, &mut adapter, &probe).unwrap();
        assert_eq!(report.runtime_call_outcome.as_deref(), Some("success"));
        assert!(!report.task_completed);
        assert_eq!(report.termination_reason, "max_steps");
    }

    #[test]
    fn invalid_call_is_recorded_without_forwarding_and_step_limit_is_enforced() {
        let test_case = case(json!({ "path": "README.md", "content": "fixture" }));
        let mut transport = InMemoryMcpTransport::new(
            vec![read_tool()],
            McpToolResult {
                is_error: false,
                structured_content: json!({}),
            },
        );
        let mut adapter = ScriptedAgentAdapter::new(
            "local-scripted-adapter",
            [
                AgentTurn::ToolCall {
                    call: ToolCall {
                        name: "read_file".into(),
                        arguments: json!({ "unexpected": true }),
                    },
                },
                AgentTurn::ToolCall {
                    call: ToolCall {
                        name: "read_file".into(),
                        arguments: json!({ "path": "README.md" }),
                    },
                },
                AgentTurn::ToolCall {
                    call: ToolCall {
                        name: "read_file".into(),
                        arguments: json!({ "path": "README.md" }),
                    },
                },
            ],
        );
        let probe =
            StaticFinalStateProbe::new(json!({ "path": "README.md", "content": "fixture" }));
        let report = run_case(&test_case, &mut transport, &mut adapter, &probe).unwrap();
        assert_eq!(report.arguments_schema_valid, Some(false));
        assert_eq!(report.actual_tool_calls[0].forwarded_to_runtime, false);
        assert_eq!(report.call_count, 3);
        assert_eq!(transport.calls().len(), 2);
    }

    #[test]
    fn workspace_final_state_probe_is_relative_and_independent() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("README.md"), "fixture").unwrap();
        let test_case = case(json!({ "path": "README.md", "content": "fixture" }));
        let observed = WorkspaceFinalStateProbe::new(directory.path())
            .observe(&test_case)
            .unwrap();
        assert_eq!(observed, test_case.expected_final_state);
        let outside = case(json!({ "path": "../outside.txt", "content": "x" }));
        assert!(WorkspaceFinalStateProbe::new(directory.path())
            .observe(&outside)
            .is_err());
    }
}
