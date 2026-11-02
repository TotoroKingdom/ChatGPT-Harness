use super::case::{EvalCaseV1, PolicyExpectationValue};
use super::fixture::{fixture_tree_hash, materialize_fixture};
use super::manifest::{canonical_hash, sha256_file};
use super::{EvalError, EvalResult};
use harness_control_protocol::{
    BashConfig, DecimalU64, PolicyConfig, PolicyDecision, PolicyMatcherConfig,
    RuntimeConfigSnapshot, RuntimeLimits, SecurityConfig, SecurityMode, ToolConfig,
    ToolEnabledConfig, ToolPolicyDefaults, UserPolicyRuleConfig, WorkspaceConfig,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

const CONTROL_TIMEOUT: Duration = Duration::from_secs(30);

#[allow(dead_code)]
pub(crate) struct EvalRuntimeHarness {
    child: Child,
    stdin: ChildStdin,
    messages: mpsc::Receiver<Value>,
    stderr_task: Option<tokio::task::JoinHandle<String>>,
    next_request_id: u64,
    pub(crate) runtime_id: String,
    pub(crate) endpoint_url: String,
    pub(crate) workspace_root: PathBuf,
    pub(crate) evidence_database: PathBuf,
    pub(crate) policy_config_hash: String,
    pub(crate) config_hash: String,
    pub(crate) fixture_hash: String,
    temp_root: tempfile::TempDir,
}

impl EvalRuntimeHarness {
    pub(crate) async fn start(
        binary: &Path,
        case: &EvalCaseV1,
        fixture_source: &Path,
    ) -> EvalResult<Self> {
        let expected_fixture_hash = fixture_tree_hash(fixture_source)?;
        if expected_fixture_hash != case.fixture_hash {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "fixture hash changed before materialization",
            ));
        }
        let temp_root = tempfile::TempDir::new().map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not create eval temp dir: {error}"),
            )
        })?;
        let outside = temp_root.path().join("outside");
        std::fs::create_dir_all(&outside).map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not create outside probe: {error}"),
            )
        })?;
        std::fs::write(outside.join("secret.txt"), "outside-secret\n").map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not write outside probe: {error}"),
            )
        })?;
        let workspace_root = temp_root.path().join("workspace");
        materialize_fixture(fixture_source, &workspace_root)?;
        let materialized_hash = fixture_tree_hash(&workspace_root)?;
        if materialized_hash != expected_fixture_hash {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "materialized fixture hash mismatch",
            ));
        }
        let evidence_dir = temp_root.path().join("evidence");
        std::fs::create_dir_all(&evidence_dir).map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("could not create evidence dir: {error}"),
            )
        })?;
        let evidence_database = evidence_dir.join("evidence.db");
        let config = eval_config(case, &workspace_root)?;
        let config_value = serde_json::to_value(&config)
            .map_err(|error| EvalError::new("HARNESS_CONTRACT_ERROR", error.to_string()))?;

        let mut command = Command::new(binary);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.as_std_mut().creation_flags(0x0800_0000);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                EvalError::new("INFRA_ERROR", format!("could not spawn Runtime: {error}"))
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| EvalError::new("INFRA_ERROR", "Runtime stdin was not piped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| EvalError::new("INFRA_ERROR", "Runtime stdout was not piped"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| EvalError::new("INFRA_ERROR", "Runtime stderr was not piped"))?;
        let (sender, messages) = mpsc::channel(256);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if let Ok(value) = serde_json::from_str::<Value>(line.trim()) {
                            if sender.send(value).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });
        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            let mut output = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if output.len() < 8192 {
                            output.push_str(&line);
                        }
                    }
                }
            }
            output
        });
        let policy_config_hash = policy_profile_hash(&config)?;
        let config_hash = canonical_hash(&config_value)?;
        let mut harness = Self {
            child,
            stdin,
            messages,
            stderr_task: Some(stderr_task),
            next_request_id: 0,
            runtime_id: String::new(),
            endpoint_url: String::new(),
            workspace_root,
            evidence_database,
            policy_config_hash,
            config_hash,
            fixture_hash: materialized_hash,
            temp_root,
        };
        let initialize = harness
            .control_request(
                "initialize",
                json!({
                    "clientInfo": {
                        "name": "m04-eval-runner",
                        "title": "M04 Offline Evaluation",
                        "version": super::manifest::RUNNER_VERSION
                    },
                    "protocolVersion": 1,
                    "capabilities": { "activityReplay": true },
                    "runtimeConfig": config_value,
                    "hostContext": {
                        "hostSessionId": format!("m04-{}", uuid::Uuid::new_v4()),
                        "supervisorGenerationId": format!("m04-generation-{}", uuid::Uuid::new_v4()),
                        "evidenceStore": {
                            "enabled": true,
                            "databasePath": harness.evidence_database.to_string_lossy(),
                            "retentionDays": 30,
                            "maxStorageMiB": 64
                        }
                    }
                }),
            )
            .await?;
        harness.runtime_id = initialize
            .get("runtimeId")
            .and_then(Value::as_str)
            .ok_or_else(|| EvalError::new("INFRA_ERROR", "initialize omitted runtimeId"))?
            .to_owned();
        harness
            .control_notification("initialized", json!({}))
            .await?;
        let endpoint = harness
            .control_request("mcp.endpoint.get", json!({}))
            .await?;
        harness.endpoint_url = endpoint
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| EvalError::new("INFRA_ERROR", "mcp.endpoint.get omitted url"))?
            .to_owned();
        harness.control_request("runtime.status", json!({})).await?;
        Ok(harness)
    }

    pub(crate) async fn control_request(
        &mut self,
        method: &str,
        params: Value,
    ) -> EvalResult<Value> {
        self.next_request_id += 1;
        let id = format!("m04-control-{}", self.next_request_id);
        self.write_json_line(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))
        .await?;
        let deadline = tokio::time::Instant::now() + CONTROL_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(EvalError::new(
                    "INFRA_ERROR",
                    format!("control request {method} timed out"),
                ));
            }
            let message = tokio::time::timeout(remaining, self.messages.recv())
                .await
                .map_err(|_| {
                    EvalError::new("INFRA_ERROR", format!("control request {method} timed out"))
                })?
                .ok_or_else(|| {
                    EvalError::new(
                        "INFRA_ERROR",
                        format!("Runtime stdout closed during {method}"),
                    )
                })?;
            if message.get("id") != Some(&Value::String(id.clone())) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(EvalError::new(
                    "INFRA_ERROR",
                    format!("control request {method} failed: {error}"),
                ));
            }
            return message.get("result").cloned().ok_or_else(|| {
                EvalError::new(
                    "INFRA_ERROR",
                    format!("control request {method} omitted result"),
                )
            });
        }
    }

    pub(crate) async fn control_notification(
        &mut self,
        method: &str,
        params: Value,
    ) -> EvalResult<()> {
        self.write_json_line(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
        .await
    }

    async fn write_json_line(&mut self, value: &Value) -> EvalResult<()> {
        let mut line = serde_json::to_vec(value)
            .map_err(|error| EvalError::new("INTERNAL", error.to_string()))?;
        line.push(b'\n');
        self.stdin.write_all(&line).await.map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("Runtime stdin write failed: {error}"),
            )
        })?;
        self.stdin.flush().await.map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("Runtime stdin flush failed: {error}"),
            )
        })
    }

    pub(crate) async fn shutdown(mut self) -> EvalResult<String> {
        let result = self.control_request("runtime.stop", json!({})).await;
        let _ = self.child.start_kill();
        let wait = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await;
        let mut stderr = String::new();
        if let Some(task) = self.stderr_task.take() {
            stderr = task.await.unwrap_or_default();
        }
        match result {
            Ok(value) if value.get("accepted") == Some(&Value::Bool(true)) => {}
            Ok(_) => {
                return Err(EvalError::new(
                    "INFRA_ERROR",
                    "runtime.stop was not accepted",
                ))
            }
            Err(error) => return Err(error),
        }
        if wait.is_err() {
            return Err(EvalError::new(
                "INFRA_ERROR",
                "Runtime did not exit after stop",
            ));
        }
        if !stderr.trim().is_empty() {
            return Ok("stopped".into());
        }
        Ok("stopped".into())
    }
}

impl Drop for EvalRuntimeHarness {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

pub(crate) fn eval_config(
    case: &EvalCaseV1,
    workspace_root: &Path,
) -> EvalResult<RuntimeConfigSnapshot> {
    let defaults = &case.policy_expectation.defaults;
    let mut user_rules = Vec::new();
    for tool in &case.policy_expectation.deny_tools {
        user_rules.push(UserPolicyRuleConfig {
            id: format!("m04-deny-{tool}"),
            name: format!("M04 deny {tool}"),
            enabled: true,
            decision: PolicyDecision::Deny,
            matcher: PolicyMatcherConfig::Tool { tool: tool.clone() },
            reason: "M04 case explicit policy denial".into(),
        });
    }
    Ok(RuntimeConfigSnapshot {
        config_revision: DecimalU64::new(1),
        workspace: WorkspaceConfig {
            root_path: workspace_root.to_string_lossy().into_owned(),
        },
        security: SecurityConfig {
            mode: SecurityMode::Normal,
            developer_test_mode: false,
        },
        runtime_limits: RuntimeLimits {
            max_concurrent_requests: 1,
            max_activity_events: 256,
            max_read_bytes: 1_048_576,
        },
        tools: ToolConfig {
            read_file: ToolEnabledConfig {
                enabled: defaults.read_file != PolicyExpectationValue::Disabled,
            },
            glob: ToolEnabledConfig {
                enabled: defaults.glob != PolicyExpectationValue::Disabled,
            },
            write: ToolEnabledConfig {
                enabled: defaults.write != PolicyExpectationValue::Disabled,
            },
            edit: ToolEnabledConfig {
                enabled: defaults.edit != PolicyExpectationValue::Disabled,
            },
            bash: ToolEnabledConfig {
                enabled: defaults.bash != PolicyExpectationValue::Disabled,
            },
        },
        bash: BashConfig::default(),
        policies: PolicyConfig {
            tool_defaults: ToolPolicyDefaults {
                read_file: policy_decision(defaults.read_file),
                glob: policy_decision(defaults.glob),
                write: policy_decision(defaults.write),
                edit: policy_decision(defaults.edit),
                bash: policy_decision(defaults.bash),
            },
            user_rules,
        },
    })
}

fn policy_decision(value: PolicyExpectationValue) -> PolicyDecision {
    match value {
        PolicyExpectationValue::Allow => PolicyDecision::Allow,
        PolicyExpectationValue::Ask => PolicyDecision::Ask,
        PolicyExpectationValue::Deny | PolicyExpectationValue::Disabled => PolicyDecision::Deny,
    }
}

pub(crate) fn policy_profile_hash(config: &RuntimeConfigSnapshot) -> EvalResult<String> {
    canonical_hash(&json!({
        "toolDefaults": config.policies.tool_defaults,
        "userRules": config.policies.user_rules,
        "tools": config.tools,
        "bash": "disabled",
    }))
}

pub(crate) fn base_policy_config_hash() -> EvalResult<String> {
    let config = RuntimeConfigSnapshot {
        config_revision: DecimalU64::new(1),
        workspace: WorkspaceConfig {
            root_path: ".".into(),
        },
        security: SecurityConfig {
            mode: SecurityMode::Normal,
            developer_test_mode: false,
        },
        runtime_limits: RuntimeLimits {
            max_concurrent_requests: 1,
            max_activity_events: 256,
            max_read_bytes: 1_048_576,
        },
        tools: ToolConfig {
            read_file: ToolEnabledConfig { enabled: true },
            glob: ToolEnabledConfig { enabled: true },
            write: ToolEnabledConfig { enabled: true },
            edit: ToolEnabledConfig { enabled: true },
            bash: ToolEnabledConfig { enabled: false },
        },
        bash: BashConfig::default(),
        policies: PolicyConfig {
            tool_defaults: ToolPolicyDefaults {
                read_file: PolicyDecision::Allow,
                glob: PolicyDecision::Allow,
                write: PolicyDecision::Ask,
                edit: PolicyDecision::Ask,
                bash: PolicyDecision::Deny,
            },
            user_rules: Vec::new(),
        },
    };
    policy_profile_hash(&config)
}

pub(crate) struct McpClient {
    http: reqwest::Client,
    endpoint_url: String,
    session_id: Option<String>,
    next_request_id: u64,
}

impl McpClient {
    pub(crate) async fn connect(endpoint_url: &str) -> EvalResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| {
                EvalError::new(
                    "INFRA_ERROR",
                    format!("could not create MCP HTTP client: {error}"),
                )
            })?;
        let mut client = Self {
            http,
            endpoint_url: endpoint_url.to_owned(),
            session_id: None,
            next_request_id: 0,
        };
        let (_, headers) = client
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "m04-eval-runner",
                        "version": super::manifest::RUNNER_VERSION
                    }
                }),
            )
            .await?;
        client.session_id = headers
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if client.session_id.is_none() {
            return Err(EvalError::new(
                "INFRA_ERROR",
                "MCP initialize omitted mcp-session-id",
            ));
        }
        Ok(client)
    }

    pub(crate) async fn list_tools(&mut self) -> EvalResult<Value> {
        let (value, _) = self.request("tools/list", json!({})).await?;
        value
            .get("result")
            .cloned()
            .ok_or_else(|| EvalError::new("INFRA_ERROR", "tools/list omitted result"))
    }

    pub(crate) async fn call_tool(&mut self, name: &str, arguments: Value) -> EvalResult<Value> {
        let (value, _) = self
            .request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .await?;
        value.get("result").cloned().ok_or_else(|| {
            EvalError::new("INFRA_ERROR", format!("tools/call {name} omitted result"))
        })
    }

    async fn request(
        &mut self,
        method: &str,
        params: Value,
    ) -> EvalResult<(Value, reqwest::header::HeaderMap)> {
        self.next_request_id += 1;
        let id = self.next_request_id;
        let mut request = self
            .http
            .post(&self.endpoint_url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }));
        if let Some(session_id) = &self.session_id {
            request = request.header("mcp-session-id", session_id);
        }
        let response = request.send().await.map_err(|error| {
            EvalError::new(
                "INFRA_ERROR",
                format!("MCP {method} transport failed: {error}"),
            )
        })?;
        let status = response.status();
        let headers = response.headers().clone();
        let content_type = headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let body = response.text().await.map_err(|error| {
            EvalError::new("INFRA_ERROR", format!("MCP {method} body failed: {error}"))
        })?;
        if !status.is_success() {
            return Err(EvalError::new(
                "INFRA_ERROR",
                format!("MCP {method} returned HTTP {status}"),
            ));
        }
        let value = parse_mcp_body(&body, &content_type)?;
        if let Some(error) = value.get("error") {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("MCP {method} returned protocol error: {error}"),
            ));
        }
        Ok((value, headers))
    }
}

fn parse_mcp_body(body: &str, content_type: &str) -> EvalResult<Value> {
    if content_type.contains("text/event-stream") || body.trim_start().starts_with("event:") {
        let mut last = None;
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if !data.is_empty() && data != "[DONE]" {
                    last = Some(serde_json::from_str::<Value>(data).map_err(|error| {
                        EvalError::new(
                            "HARNESS_CONTRACT_ERROR",
                            format!("invalid MCP SSE JSON: {error}"),
                        )
                    })?);
                }
            }
        }
        return last.ok_or_else(|| {
            EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "MCP SSE response had no JSON data",
            )
        });
    }
    serde_json::from_str(body).map_err(|error| {
        EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("invalid MCP JSON response: {error}"),
        )
    })
}

pub(crate) fn tool_catalog_hash(tools_result: &Value) -> EvalResult<String> {
    let mut tools = tools_result
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "tools/list omitted tools"))?
        .clone();
    tools.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .cmp(&right.get("name").and_then(Value::as_str))
    });
    canonical_hash(&json!({ "tools": tools }))
}

pub(crate) fn exposed_tool_names(tools_result: &Value) -> EvalResult<Vec<String>> {
    let tools = tools_result
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "tools/list omitted tools"))?;
    Ok(tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect())
}

pub(crate) fn tool_schema_map(
    tools_result: &Value,
) -> EvalResult<std::collections::BTreeMap<String, Value>> {
    let tools = tools_result
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "tools/list omitted tools"))?;
    let mut map = std::collections::BTreeMap::new();
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "tool omitted name"))?;
        let schema = tool.get("inputSchema").cloned().ok_or_else(|| {
            EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!("tool {name} omitted inputSchema"),
            )
        })?;
        map.insert(name.to_owned(), schema);
    }
    Ok(map)
}

pub(crate) fn runtime_binary_hash(binary: &Path) -> EvalResult<String> {
    sha256_file(binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_policy_hash_is_stable_and_excludes_workspace() {
        let first = base_policy_config_hash().unwrap();
        let second = base_policy_config_hash().unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn sse_parser_uses_last_data_event() {
        let value = parse_mcp_body(
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n",
            "text/event-stream",
        )
        .unwrap();
        assert_eq!(value["id"], 1);
    }
}
