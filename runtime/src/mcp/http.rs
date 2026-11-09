//! Loopback-only Streamable HTTP transport for the Runtime MCP service.
//!
//! This module deliberately owns TCP, HTTP, and endpoint lifetime only. Tool
//! dispatch remains in [`McpService`], which continues to call the M5-D1
//! adapters and their existing Runtime executors.

use super::{McpExecutionContext, McpService};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Errors raised while starting, serving, or stopping the local MCP endpoint.
#[derive(Debug)]
pub enum McpHttpError {
    BindFailed { source: io::Error },
    LocalAddressFailed { source: io::Error },
    StartupFailed { message: &'static str },
    ServeFailed { source: io::Error },
    ShutdownFailed { source: tokio::task::JoinError },
}

impl fmt::Display for McpHttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindFailed { .. } => formatter.write_str("could not bind the local MCP endpoint"),
            Self::LocalAddressFailed { .. } => {
                formatter.write_str("could not determine the local MCP endpoint address")
            }
            Self::StartupFailed { message } => {
                write!(formatter, "could not start the MCP endpoint: {message}")
            }
            Self::ServeFailed { .. } => {
                formatter.write_str("the local MCP endpoint stopped unexpectedly")
            }
            Self::ShutdownFailed { .. } => {
                formatter.write_str("the local MCP endpoint could not shut down cleanly")
            }
        }
    }
}

impl std::error::Error for McpHttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BindFailed { source }
            | Self::LocalAddressFailed { source }
            | Self::ServeFailed { source } => Some(source),
            Self::ShutdownFailed { source } => Some(source),
            Self::StartupFailed { .. } => None,
        }
    }
}

/// Owns one independently-started, loopback-only MCP HTTP endpoint.
///
/// Call [`Self::shutdown`] before dropping this handle so the server task is
/// joined. Dropping the handle still cancels the transport to avoid leaving a
/// listener running indefinitely, but cannot await that join from `Drop`.
pub struct McpHttpEndpointHandle {
    local_addr: SocketAddr,
    cancellation_token: CancellationToken,
    server_task: Option<JoinHandle<Result<(), io::Error>>>,
}

impl McpHttpEndpointHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns the canonical local endpoint URL, always using `127.0.0.1`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.local_addr.port())
    }

    /// Gracefully cancels active rmcp transport work, stops accepting new
    /// connections, and joins the Axum server task.
    pub async fn shutdown(&mut self) -> Result<(), McpHttpError> {
        self.cancellation_token.cancel();

        let Some(server_task) = self.server_task.take() else {
            return Ok(());
        };

        match server_task.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(source)) => Err(McpHttpError::ServeFailed { source }),
            Err(source) => Err(McpHttpError::ShutdownFailed { source }),
        }
    }
}

impl Drop for McpHttpEndpointHandle {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

/// Starts an independent Streamable HTTP MCP endpoint on an OS-assigned
/// loopback port. It does not alter Runtime main or the control protocol.
pub async fn start_mcp_http_endpoint(
    context: McpExecutionContext,
) -> Result<McpHttpEndpointHandle, McpHttpError> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|source| McpHttpError::BindFailed { source })?;
    let local_addr = listener
        .local_addr()
        .map_err(|source| McpHttpError::LocalAddressFailed { source })?;

    let cancellation_token = CancellationToken::new();
    let origin = format!("http://127.0.0.1:{}", local_addr.port());
    let config = StreamableHttpServerConfig::default()
        .with_allowed_origins([origin])
        .with_cancellation_token(cancellation_token.clone());
    let service = McpService::new(context);
    let transport = StreamableHttpService::new(
        move || Ok::<_, io::Error>(service.clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let router = axum::Router::new().route_service("/mcp", transport);
    let graceful_shutdown = cancellation_token.clone();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { graceful_shutdown.cancelled().await })
            .await
    });

    Ok(McpHttpEndpointHandle {
        local_addr,
        cancellation_token,
        server_task: Some(server_task),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::RuntimeCore;
    use harness_control_protocol::{
        DecimalU64, RuntimeConfigSnapshot, RuntimeLimits, SecurityConfig, SecurityMode, ToolConfig,
        WorkspaceConfig,
    };
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Barrier};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
        outside: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let base = std::env::temp_dir()
                .join(format!("chatgpt-harness-mcp-http-{}", uuid::Uuid::new_v4()));
            let root = base.join("workspace");
            let outside = base.join("outside");
            fs::create_dir_all(&root).expect("create workspace");
            fs::create_dir_all(&outside).expect("create outside");
            Self {
                base,
                root,
                outside,
            }
        }

        fn write(&self, relative: impl AsRef<Path>, content: impl AsRef<[u8]>) {
            let path = self.root.join(relative);
            fs::create_dir_all(path.parent().expect("fixture parent")).expect("create parent");
            fs::write(path, content).expect("write fixture");
        }

        fn context(&self) -> McpExecutionContext {
            let mut core = RuntimeCore::new();
            core.initialize(config_for(&self.root))
                .expect("initialize runtime");
            core.acknowledge().expect("acknowledge runtime");
            core.mcp_context().expect("MCP context")
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn config_for(root: &Path) -> RuntimeConfigSnapshot {
        RuntimeConfigSnapshot {
            config_revision: DecimalU64::new(1),
            workspace: WorkspaceConfig {
                root_path: root.to_string_lossy().into_owned(),
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: 1,
                max_activity_events: 10,
                max_read_bytes: 64,
            },
            tools: ToolConfig::default(),
            bash: harness_control_protocol::BashConfig::default(),
            policies: harness_control_protocol::PolicyConfig::default(),
            search: harness_control_protocol::SearchConfig::default(),
        }
    }

    #[derive(Debug)]
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

        fn json_rpc(&self) -> Value {
            let body = std::str::from_utf8(&self.body).expect("UTF-8 HTTP body");
            if let Ok(value) = serde_json::from_str(body) {
                return value;
            }
            body.lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim)
                .find_map(|line| serde_json::from_str(line).ok())
                .expect("JSON-RPC SSE data")
        }
    }

    fn canonical_tool_payload(message: &Value) -> Value {
        assert!(message["result"].get("structuredContent").is_none());
        let text = message["result"]["content"]
            .as_array()
            .and_then(|content| content.first())
            .and_then(|content| content.get("text"))
            .and_then(Value::as_str)
            .expect("canonical MCP TextContent");
        serde_json::from_str(text).expect("canonical tool payload JSON")
    }

    async fn request(
        address: SocketAddr,
        method: &str,
        path: &str,
        body: &[u8],
        headers: Vec<(&str, String)>,
    ) -> HttpResponse {
        let mut stream = timeout(HTTP_TIMEOUT, TcpStream::connect(address))
            .await
            .expect("connect timeout")
            .expect("connect endpoint");
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in headers {
            request.push_str(name);
            request.push_str(": ");
            request.push_str(&value);
            request.push_str("\r\n");
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write request");
        stream.write_all(body).await.expect("write request body");
        stream.flush().await.expect("flush request");

        let mut response = Vec::new();
        timeout(HTTP_TIMEOUT, stream.read_to_end(&mut response))
            .await
            .expect("read timeout")
            .expect("read response");
        parse_response(&response)
    }

    fn parse_response(response: &[u8]) -> HttpResponse {
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP header end");
        let headers = std::str::from_utf8(&response[..header_end]).expect("ASCII headers");
        let mut lines = headers.lines();
        let status = lines
            .next()
            .expect("HTTP status")
            .split_whitespace()
            .nth(1)
            .expect("status code")
            .parse()
            .expect("numeric status");
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
            .collect::<BTreeMap<_, _>>();
        let body = response[header_end + 4..].to_vec();
        let body = if headers
            .get("transfer-encoding")
            .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
        {
            decode_chunked(&body)
        } else {
            body
        };
        HttpResponse {
            status,
            headers,
            body,
        }
    }

    fn decode_chunked(mut wire: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        loop {
            let line_end = wire
                .windows(2)
                .position(|window| window == b"\r\n")
                .expect("chunk length");
            let length = std::str::from_utf8(&wire[..line_end])
                .expect("chunk length text")
                .split(';')
                .next()
                .expect("chunk length value");
            let length = usize::from_str_radix(length, 16).expect("hex chunk length");
            wire = &wire[line_end + 2..];
            if length == 0 {
                return body;
            }
            body.extend_from_slice(&wire[..length]);
            wire = &wire[length + 2..];
        }
    }

    fn mcp_headers(
        address: SocketAddr,
        session_id: Option<&str>,
        origin: Option<&str>,
    ) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("Host", format!("127.0.0.1:{}", address.port())),
            ("Accept", "application/json, text/event-stream".into()),
            ("Content-Type", "application/json".into()),
        ];
        if let Some(session_id) = session_id {
            headers.push(("Mcp-Session-Id", session_id.into()));
            headers.push(("MCP-Protocol-Version", "2025-03-26".into()));
        }
        if let Some(origin) = origin {
            headers.push(("Origin", origin.into()));
        }
        headers
    }

    fn modern_mcp_headers(
        address: SocketAddr,
        method: &str,
        name: Option<&str>,
    ) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("Host", format!("127.0.0.1:{}", address.port())),
            ("Accept", "application/json, text/event-stream".into()),
            ("Content-Type", "application/json".into()),
            ("MCP-Protocol-Version", "2026-07-28".into()),
            ("Mcp-Method", method.into()),
        ];
        if let Some(name) = name {
            headers.push(("Mcp-Name", name.into()));
        }
        headers
    }

    fn inline_mcp_headers(
        address: SocketAddr,
        method: &str,
        name: Option<&str>,
        protocol_version: &str,
    ) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("Host", format!("127.0.0.1:{}", address.port())),
            ("Accept", "application/json, text/event-stream".into()),
            ("Content-Type", "application/json".into()),
            ("MCP-Protocol-Version", protocol_version.to_owned()),
            ("Mcp-Method", method.into()),
        ];
        if let Some(name) = name {
            headers.push(("Mcp-Name", name.into()));
        }
        headers
    }

    async fn mcp_post(
        address: SocketAddr,
        message: Value,
        session_id: Option<&str>,
        origin: Option<&str>,
    ) -> HttpResponse {
        let body = serde_json::to_string(&message).expect("serialize MCP request");
        request(
            address,
            "POST",
            "/mcp",
            body.as_bytes(),
            mcp_headers(address, session_id, origin),
        )
        .await
    }

    async fn modern_mcp_post(address: SocketAddr, method: &str, message: Value) -> HttpResponse {
        modern_mcp_post_named(address, method, None, message).await
    }

    async fn modern_mcp_post_named(
        address: SocketAddr,
        method: &str,
        name: Option<&str>,
        message: Value,
    ) -> HttpResponse {
        let body = serde_json::to_string(&message).expect("serialize MCP request");
        request(
            address,
            "POST",
            "/mcp",
            body.as_bytes(),
            modern_mcp_headers(address, method, name),
        )
        .await
    }

    async fn inline_mcp_post(
        address: SocketAddr,
        protocol_version: &str,
        method: &str,
        name: Option<&str>,
        message: Value,
    ) -> HttpResponse {
        let body = serde_json::to_string(&message).expect("serialize MCP request");
        request(
            address,
            "POST",
            "/mcp",
            body.as_bytes(),
            inline_mcp_headers(address, method, name, protocol_version),
        )
        .await
    }

    async fn initialize(address: SocketAddr, origin: Option<&str>) -> String {
        let response = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "http-test-client", "version": "1.0" }
                }
            }),
            None,
            origin,
        )
        .await;
        assert_eq!(response.status, 200, "initialize body: {:?}", response.body);
        response
            .header("mcp-session-id")
            .expect("MCP session id")
            .to_owned()
    }

    async fn initialize_response(address: SocketAddr) -> HttpResponse {
        mcp_post(
            address,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "wire-contract-client", "version": "1.0" }
                }
            }),
            None,
            None,
        )
        .await
    }

    #[tokio::test]
    async fn initialize_negotiates_frozen_revision_and_truthful_capabilities() {
        let workspace = TestWorkspace::new();
        let mut endpoint = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start endpoint");
        let response = initialize_response(endpoint.local_addr()).await;
        assert_eq!(response.status, 200);
        let body = response.json_rpc();
        assert_eq!(body["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(body["result"]["serverInfo"]["name"], "chatgpt-harness");
        assert_eq!(
            body["result"]["serverInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );
        assert!(body["result"]["capabilities"]["tools"].is_object());
        for unsupported in ["resources", "prompts", "sampling", "roots", "logging"] {
            assert!(
                body["result"]["capabilities"].get(unsupported).is_none(),
                "unsupported capability was advertised: {unsupported}"
            );
        }
        endpoint.shutdown().await.expect("shutdown endpoint");
    }

    #[tokio::test]
    async fn modern_discover_and_tools_list_are_supported_without_session() {
        let workspace = TestWorkspace::new();
        workspace.write("README.md", "modern protocol\n");
        let mut endpoint = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start endpoint");
        let address = endpoint.local_addr();
        let meta = json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {
                "name": "modern-http-test-client",
                "version": "1.0"
            },
            "io.modelcontextprotocol/clientCapabilities": {}
        });

        let discover = modern_mcp_post(
            address,
            "server/discover",
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "server/discover",
                "params": { "_meta": meta.clone() }
            }),
        )
        .await;
        assert_eq!(discover.status, 200, "discover body: {:?}", discover.body);
        let discover_body = discover.json_rpc();
        let versions = discover_body["result"]["supportedVersions"]
            .as_array()
            .expect("supported versions array")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert!(versions.contains(&"2025-03-26"));
        assert!(versions.contains(&"2026-07-28"));
        assert_eq!(discover_body["result"]["resultType"], "complete");
        assert_eq!(
            discover_body["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "chatgpt-harness"
        );

        let list = modern_mcp_post(
            address,
            "tools/list",
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": { "_meta": meta }
            }),
        )
        .await;
        assert_eq!(list.status, 200, "tools/list body: {:?}", list.body);
        let list_body = list.json_rpc();
        assert_eq!(list_body["result"]["resultType"], "complete");
        let tools = list_body["result"]["tools"].as_array().expect("tool array");
        assert!(tools.iter().any(|tool| tool["name"] == "read_file"));
        assert!(tools.iter().all(|tool| tool.get("outputSchema").is_none()));

        endpoint.shutdown().await.expect("shutdown endpoint");
    }

    #[tokio::test]
    async fn modern_tools_call_read_file_returns_structured_content() {
        let workspace = TestWorkspace::new();
        workspace.write("README.md", "modern protocol\n");
        let mut endpoint = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start endpoint");
        let address = endpoint.local_addr();
        let meta = json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {
                "name": "modern-http-test-client",
                "version": "1.0"
            },
            "io.modelcontextprotocol/clientCapabilities": {}
        });

        let call = modern_mcp_post_named(
            address,
            "tools/call",
            Some("read_file"),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "read_file",
                    "arguments": { "path": "README.md" },
                    "_meta": meta
                }
            }),
        )
        .await;
        assert_eq!(call.status, 200, "tools/call body: {:?}", call.body);
        let body = call.json_rpc();
        assert_eq!(body["result"]["resultType"], "complete");
        assert_eq!(
            body["result"]["structuredContent"]["content"],
            "modern protocol\n"
        );
        assert_eq!(
            body["result"]["structuredContent"]["bytesRead"].as_u64(),
            Some("modern protocol\n".len() as u64)
        );
        let text = body["result"]["content"][0]["text"]
            .as_str()
            .expect("canonical JSON text block");
        assert!(text.contains("\"content\":\"modern protocol\\n\""));

        endpoint.shutdown().await.expect("shutdown endpoint");
    }

    #[tokio::test]
    async fn interoperable_revisions_accept_per_request_metadata() {
        for protocol_version in ["2025-06-18", "2025-11-25"] {
            let workspace = TestWorkspace::new();
            workspace.write("README.md", "tunnel protocol\n");
            let mut endpoint = start_mcp_http_endpoint(workspace.context())
                .await
                .expect("start endpoint");
            let address = endpoint.local_addr();
            let meta = json!({
                "io.modelcontextprotocol/protocolVersion": protocol_version,
                "io.modelcontextprotocol/clientInfo": {
                    "name": "tunnel-client",
                    "version": "0.0.13"
                },
                "io.modelcontextprotocol/clientCapabilities": {}
            });

            let initialize = inline_mcp_post(
                address,
                protocol_version,
                "initialize",
                None,
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": protocol_version,
                        "capabilities": {},
                        "clientInfo": { "name": "tunnel-client", "version": "0.0.13" }
                    }
                }),
            )
            .await;
            assert_eq!(
                initialize.status, 200,
                "initialize body for {protocol_version}: {:?}",
                initialize.body
            );
            assert_eq!(
                initialize.json_rpc()["result"]["protocolVersion"],
                protocol_version,
                "initialize must echo the requested supported revision"
            );

            let list = inline_mcp_post(
                address,
                protocol_version,
                "tools/list",
                None,
                json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/list",
                    "params": { "_meta": meta.clone() }
                }),
            )
            .await;
            assert_eq!(
                list.status, 200,
                "tools/list body for {protocol_version}: {:?}",
                list.body
            );
            assert!(list.json_rpc()["result"]["tools"]
                .as_array()
                .expect("tool array")
                .iter()
                .any(|tool| tool["name"] == "read_file"));

            let call = inline_mcp_post(
                address,
                protocol_version,
                "tools/call",
                Some("read_file"),
                json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": {
                        "name": "read_file",
                        "arguments": { "path": "README.md" },
                        "_meta": meta
                    }
                }),
            )
            .await;
            assert_eq!(
                call.status, 200,
                "tools/call body for {protocol_version}: {:?}",
                call.body
            );
            let body = call.json_rpc();
            assert_eq!(
                body["result"]["structuredContent"]["content"], "tunnel protocol\n",
                "structuredContent must be emitted for {protocol_version}"
            );
            let text = body["result"]["content"][0]["text"]
                .as_str()
                .expect("canonical JSON text block");
            assert!(text.contains("\"content\":\"tunnel protocol\\n\""));

            endpoint.shutdown().await.expect("shutdown endpoint");
        }
    }

    #[tokio::test]
    async fn initialized_notification_and_session_header_failures_remain_bounded() {
        let workspace = TestWorkspace::new();
        let mut first = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start first endpoint");
        let mut second = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start second endpoint");
        let first_address = first.local_addr();
        let second_address = second.local_addr();
        let session_id = initialize(first_address, None).await;

        let initialized = mcp_post(
            first_address,
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            Some(&session_id),
            None,
        )
        .await;
        assert!((200..300).contains(&initialized.status));

        let missing_session = mcp_post(
            first_address,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
            None,
            None,
        )
        .await;
        assert!(missing_session.status >= 400);
        assert!(String::from_utf8_lossy(&missing_session.body).len() < 2048);

        let invalid_session = mcp_post(
            first_address,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {} }),
            Some("invalid-session-id"),
            None,
        )
        .await;
        assert!(invalid_session.status >= 400);
        assert!(String::from_utf8_lossy(&invalid_session.body).len() < 2048);

        let cross_session = mcp_post(
            second_address,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list", "params": {} }),
            Some(&session_id),
            None,
        )
        .await;
        assert!(cross_session.status >= 400);
        assert!(!String::from_utf8_lossy(&cross_session.body).contains(&session_id));

        first.shutdown().await.expect("shutdown first endpoint");
        second.shutdown().await.expect("shutdown second endpoint");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_notification_stops_inflight_grep_without_fabricated_success() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "needle\nneedle\n");
        let mut endpoint = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start endpoint");
        let address = endpoint.local_addr();
        let session_id = initialize(address, None).await;
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let hook_entered = entered.clone();
        let hook_release = release.clone();
        let _hook = crate::tools::install_scan_hook(move |scanned| {
            if scanned.file_name().is_some_and(|name| name == "file.txt") {
                hook_entered.wait();
                hook_release.wait();
            }
        });

        let request_task = tokio::spawn({
            let session_id = session_id.clone();
            async move {
                mcp_post(
                    address,
                    json!({
                        "jsonrpc": "2.0",
                        "id": 55,
                        "method": "tools/call",
                        "params": { "name": "grep", "arguments": { "query": "needle" } }
                    }),
                    Some(&session_id),
                    None,
                )
                .await
            }
        });
        tokio::task::spawn_blocking(move || entered.wait())
            .await
            .expect("scan start waiter");

        let cancelled = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/cancelled",
                "params": { "requestId": 55, "reason": "M10 controlled cancellation" }
            }),
            Some(&session_id),
            None,
        )
        .await;
        assert!((200..300).contains(&cancelled.status));
        tokio::task::spawn_blocking(move || release.wait())
            .await
            .expect("scan release waiter");

        let response = request_task.await.expect("grep request task");
        if response.status == 200 {
            let body = String::from_utf8_lossy(&response.body);
            assert!(!body.contains("\"isError\":false"));
            assert!(!body.contains("\"matches\""));
        }

        let next = mcp_post(
            address,
            json!({ "jsonrpc": "2.0", "id": 56, "method": "tools/list", "params": {} }),
            Some(&session_id),
            None,
        )
        .await;
        assert_eq!(next.status, 200);
        endpoint.shutdown().await.expect("shutdown endpoint");
    }

    #[tokio::test]
    async fn http_transport_exposes_available_tools_and_preserves_workspace_security() {
        let workspace = TestWorkspace::new();
        workspace.write("README.md", "zero\none\ntwo\n");
        workspace.write("src/main.rs", "fn main() {}\n");
        workspace.write("src/nested/lib.rs", "");
        fs::write(workspace.outside.join("outside.txt"), "secret").expect("write outside file");
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        let mut endpoint = start_mcp_http_endpoint(core.mcp_context().expect("MCP context"))
            .await
            .expect("start endpoint");
        let address = endpoint.local_addr();
        assert_eq!(
            endpoint.url(),
            format!("http://127.0.0.1:{}/mcp", address.port())
        );
        let session_id = initialize(address, None).await;

        let pre_ack_list = mcp_post(
            address,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
            Some(&session_id),
            None,
        )
        .await
        .json_rpc();
        assert!(pre_ack_list["result"]["tools"]
            .as_array()
            .expect("tool array")
            .is_empty());

        let pre_ack = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",                "params": { "name": "read_file", "arguments": { "path": "README.md" } }
            }),
            Some(&session_id),
            None,
        )
        .await
        .json_rpc();
        assert_eq!(
            canonical_tool_payload(&pre_ack)["code"],
            "RUNTIME_NOT_READY"
        );
        core.acknowledge().expect("acknowledge runtime");

        let list = mcp_post(
            address,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {} }),
            Some(&session_id),
            None,
        )
        .await;
        let list_body = list.json_rpc();
        let names = list_body["result"]["tools"]
            .as_array()
            .expect("tool array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<Vec<_>>();
        assert_eq!(names, ["read_file", "glob", "grep", "write", "edit"]);

        let read = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": { "name": "read_file", "arguments": { "path": "README.md", "offset": 1, "limit": 1 } }
            }),
            Some(&session_id),
            None,
        )
        .await
        .json_rpc();
        let read_result = canonical_tool_payload(&read);
        assert_eq!(read_result["content"], "one\n");
        assert_eq!(read_result["offset"], 1);
        assert_eq!(read_result["linesRead"], 1);
        assert_eq!(read_result["bytesRead"], 4);
        assert_eq!(read_result["nextOffset"], 2);
        assert!(read_result["normalizedPath"]
            .as_str()
            .expect("normalized path")
            .ends_with("README.md"));

        let glob = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0", "id": 5, "method": "tools/call",
                "params": { "name": "glob", "arguments": { "pattern": "**/*.rs" } }
            }),
            Some(&session_id),
            None,
        )
        .await
        .json_rpc();
        let glob_result = canonical_tool_payload(&glob);
        assert_eq!(glob_result["matches"][0]["path"], "src/main.rs");
        assert_eq!(glob_result["matches"][1]["path"], "src/nested/lib.rs");
        assert!(glob_result.get("diagnostics").is_some());

        let glob_escape = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0", "id": 6, "method": "tools/call",
                "params": { "name": "glob", "arguments": { "pattern": "*", "basePath": "../outside" } }
            }),
            Some(&session_id),
            None,
        )
        .await
        .json_rpc();
        assert_eq!(glob_escape["result"]["isError"], true);
        assert_eq!(
            canonical_tool_payload(&glob_escape)["code"],
            "WORKSPACE_VIOLATION"
        );

        let escape = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0", "id": 7, "method": "tools/call",
                "params": { "name": "read_file", "arguments": { "path": "../outside/outside.txt" } }
            }),
            Some(&session_id),
            None,
        )
        .await
        .json_rpc();
        assert_eq!(escape["result"]["isError"], true);
        assert_eq!(
            canonical_tool_payload(&escape)["code"],
            "WORKSPACE_VIOLATION"
        );
        assert!(!escape
            .to_string()
            .contains(&workspace.outside.to_string_lossy().to_string()));

        endpoint.shutdown().await.expect("shutdown endpoint");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_cancels_active_grep_without_success_response() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "needle\nneedle\n");
        let mut endpoint = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start endpoint");
        let address = endpoint.local_addr();
        let session_id = initialize(address, None).await;
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let hook_entered = entered.clone();
        let hook_release = release.clone();
        let _hook = crate::tools::install_scan_hook(move |scanned| {
            if scanned.file_name().is_some_and(|name| name == "file.txt") {
                hook_entered.wait();
                hook_release.wait();
            }
        });
        let request_task = tokio::spawn(async move {
            mcp_post(
                address,
                json!({
                    "jsonrpc": "2.0",
                    "id": 22,
                    "method": "tools/call",
                    "params": { "name": "grep", "arguments": { "query": "needle" } }
                }),
                Some(&session_id),
                None,
            )
            .await
        });
        tokio::task::spawn_blocking(move || entered.wait())
            .await
            .expect("scan start waiter");
        let shutdown = tokio::spawn(async move { endpoint.shutdown().await });
        tokio::task::spawn_blocking(move || release.wait())
            .await
            .expect("scan release waiter");
        assert!(shutdown.await.expect("shutdown task").is_ok());
        let response = request_task.await;
        if let Ok(response) = response {
            if response.status == 200 {
                let body = String::from_utf8_lossy(&response.body);
                assert!(
                    !body.contains("\"matches\"") && !body.contains("\"isError\":false"),
                    "shutdown returned a successful grep response: {body}"
                );
            }
        }
    }

    #[tokio::test]
    async fn http_transport_rejects_bad_hosts_and_origins_but_allows_loopback_origin() {
        let workspace = TestWorkspace::new();
        let mut endpoint = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start endpoint");
        let address = endpoint.local_addr();
        let origin = format!("http://127.0.0.1:{}", address.port());
        let session_id = initialize(address, Some(&origin)).await;

        let bad_origin = mcp_post(
            address,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
            Some(&session_id),
            Some("https://evil.example"),
        )
        .await;
        assert_eq!(bad_origin.status, 403);

        let bad_host_body = serde_json::to_vec(&json!({
            "jsonrpc": "2.0", "id": 3, "method": "initialize",
            "params": { "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "bad-host", "version": "1.0" } }
        }))
        .expect("serialize request");
        let bad_host = request(
            address,
            "POST",
            "/mcp",
            &bad_host_body,
            vec![
                ("Host", "evil.example".into()),
                ("Accept", "application/json, text/event-stream".into()),
                ("Content-Type", "application/json".into()),
            ],
        )
        .await;
        assert_eq!(bad_host.status, 403);

        endpoint.shutdown().await.expect("shutdown endpoint");
    }

    #[tokio::test]
    async fn malformed_requests_wrong_routes_and_oversized_bodies_do_not_stop_endpoint() {
        let workspace = TestWorkspace::new();
        let mut endpoint = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start endpoint");
        let address = endpoint.local_addr();
        let headers = mcp_headers(address, None, None);

        let malformed = request(address, "POST", "/mcp", b"not-json", headers).await;
        assert!(malformed.status >= 400);
        let wrong_route = request(address, "GET", "/health", b"", vec![]).await;
        assert_eq!(wrong_route.status, 404);
        let wrong_method = request(
            address,
            "PUT",
            "/mcp",
            b"",
            mcp_headers(address, None, None),
        )
        .await;
        assert_eq!(wrong_method.status, 405);

        let oversized = vec![b'x'; 4 * 1024 * 1024 + 1];
        let oversized_response = request(
            address,
            "POST",
            "/mcp",
            &oversized,
            mcp_headers(address, None, None),
        )
        .await;
        assert_eq!(oversized_response.status, 413);

        let session_id = initialize(address, None).await;
        let malformed_arguments = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "read_file", "arguments": { "offset": 0 } }
            }),
            Some(&session_id),
            None,
        )
        .await
        .json_rpc();
        assert_eq!(malformed_arguments["error"]["code"], -32602);
        assert!(malformed_arguments["result"].is_null());

        let unknown_tool = mcp_post(
            address,
            json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "unknown_tool", "arguments": {} }
            }),
            Some(&session_id),
            None,
        )
        .await
        .json_rpc();
        assert_eq!(unknown_tool["error"]["code"], -32602);
        assert_eq!(unknown_tool["error"]["message"], "Tool request is invalid.");
        assert!(unknown_tool.to_string().len() < 512);

        let list = mcp_post(
            address,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list", "params": {} }),
            Some(&session_id),
            None,
        )
        .await;
        assert_eq!(list.status, 200);
        assert!(list.json_rpc().get("result").is_some());

        endpoint.shutdown().await.expect("shutdown endpoint");
    }

    #[tokio::test]
    async fn endpoints_use_independent_ephemeral_ports_and_shutdown_releases_listener() {
        let workspace = TestWorkspace::new();
        let mut first = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start first endpoint");
        let mut second = start_mcp_http_endpoint(workspace.context())
            .await
            .expect("start second endpoint");
        let first_addr = first.local_addr();
        assert_ne!(first_addr.port(), second.local_addr().port());
        assert!(first.url().starts_with("http://127.0.0.1:"));
        assert!(!initialize(first_addr, None).await.is_empty());

        first.shutdown().await.expect("shutdown first endpoint");
        TcpListener::bind(first_addr)
            .await
            .expect("released port can bind again");
        second.shutdown().await.expect("shutdown second endpoint");
    }
}
