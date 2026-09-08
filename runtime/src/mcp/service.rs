use crate::mcp::context::McpExecutionContext;
use crate::mcp::{glob, read_file};
use crate::tools::{EffectiveTool, ToolAvailability};
use rmcp::handler::server::common::schema_for_input;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ErrorCode, Implementation,
    ListToolsResult, ServerCapabilities, ServerInfo, Tool, ToolAnnotations,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ErrorData;
use serde_json::Value;

const READ_FILE: &str = "read_file";
const GLOB: &str = "glob";

#[derive(Clone)]
pub struct McpService {
    context: McpExecutionContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpProtocolError {
    UnknownTool,
}

impl McpService {
    pub fn new(context: McpExecutionContext) -> Self {
        Self { context }
    }

    /// Lists only currently callable tools from the Runtime effective catalog.
    /// Declared tools that are unavailable by policy, runtime, or disabled state
    /// are intentionally omitted from the MCP exposure boundary.
    pub fn tools(&self) -> Vec<Tool> {
        self.context
            .effective_tools()
            .into_iter()
            .filter(|tool| tool.availability == ToolAvailability::Available)
            .filter_map(tool_from_effective)
            .collect()
    }

    /// Executes one MCP tool adapter without creating a transport.
    pub fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<CallToolResult, McpProtocolError> {
        match name {
            READ_FILE => Ok(read_file::execute(&self.context, arguments)),
            GLOB => Ok(glob::execute(&self.context, arguments)),
            _ => Err(McpProtocolError::UnknownTool),
        }
    }
}

impl ServerHandler for McpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("chatgpt-harness", env!("CARGO_PKG_VERSION")),
        )
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools().into_iter().find(|tool| tool.name == name)
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>>
           + rmcp::service::MaybeSendFuture
           + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(self.tools())))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResponse, ErrorData>>
           + rmcp::service::MaybeSendFuture
           + '_ {
        let result = self
            .call_tool(
                &request.name,
                Value::Object(request.arguments.unwrap_or_default()),
            )
            .map(Into::into)
            .map_err(|McpProtocolError::UnknownTool| {
                ErrorData::new(ErrorCode::METHOD_NOT_FOUND, "unknown tool", None)
            });
        std::future::ready(result)
    }
}

fn tool_from_effective(tool: EffectiveTool) -> Option<Tool> {
    match tool.name.as_str() {
        READ_FILE => Some(read_file_tool_with_metadata(&tool.name, &tool.description)),
        GLOB => Some(glob_tool_with_metadata(&tool.name, &tool.description)),
        _ => None,
    }
}

fn read_file_tool_with_metadata(name: &str, description: &str) -> Tool {
    Tool::new(
        name.to_owned(),
        description.to_owned(),
        schema_for_input::<read_file::ReadFileArguments>()
            .expect("read_file MCP schema is statically valid"),
    )
    .with_annotations(ToolAnnotations::new().read_only(true).destructive(false))
}

fn glob_tool_with_metadata(name: &str, description: &str) -> Tool {
    Tool::new(
        name.to_owned(),
        description.to_owned(),
        schema_for_input::<glob::GlobArguments>().expect("glob MCP schema is statically valid"),
    )
    .with_annotations(ToolAnnotations::new().read_only(true).destructive(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::RuntimeCore;
    use harness_control_protocol::{
        DecimalU64, RuntimeConfigSnapshot, RuntimeLimits, SecurityConfig, SecurityMode,
        WorkspaceConfig,
    };
    use rmcp::handler::server::ServerHandler;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestWorkspace {
        base: PathBuf,
        root: PathBuf,
        outside: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let base =
                std::env::temp_dir().join(format!("chatgpt-harness-mcp-{}", uuid::Uuid::new_v4()));
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

        fn service(&self) -> McpService {
            let mut core = RuntimeCore::new();
            core.initialize(config_for(&self.root))
                .expect("initialize runtime");
            core.acknowledge().expect("acknowledge runtime");
            core.mcp_service().expect("initialized MCP service")
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
        }
    }

    fn call(service: &McpService, name: &str, arguments: Value) -> CallToolResult {
        service.call_tool(name, arguments).expect("known MCP tool")
    }

    fn error_code(result: &CallToolResult) -> &str {
        result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str)
            .expect("structured error code")
    }

    #[test]
    fn context_is_unavailable_until_runtime_initializes() {
        let core = RuntimeCore::new();
        assert!(matches!(
            core.mcp_context(),
            Err(crate::mcp::McpContextError::RuntimeNotInitialized)
        ));
    }

    #[test]
    fn tools_are_not_executable_before_runtime_acknowledgement() {
        let workspace = TestWorkspace::new();
        workspace.write("lines.txt", "one\n");
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        let service = core.mcp_service().expect("MCP service");

        let result = call(&service, READ_FILE, json!({ "path": "lines.txt" }));
        assert_eq!(error_code(&result), "RUNTIME_NOT_READY");
    }

    #[test]
    fn ready_catalog_exposes_read_tools_and_excludes_non_v1_tools() {
        let workspace = TestWorkspace::new();
        let service = workspace.service();
        let names = service
            .tools()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, [READ_FILE, GLOB]);
        for forbidden in [
            "bash",
            "tree",
            "list_files",
            "find_files",
            "git",
            "database",
            "write",
            "edit",
            "runtime.status",
            "diagnostics.run",
        ] {
            assert!(!names.iter().any(|name| name == forbidden));
        }
        let info = service.get_info();
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_none());
        assert!(info.capabilities.prompts.is_none());
        assert_eq!(info.server_info.name, "chatgpt-harness");
    }

    #[test]
    fn read_file_maps_executor_output_to_structured_result() {
        let workspace = TestWorkspace::new();
        workspace.write("lines.txt", "zero\none\ntwo\n");
        let result = call(
            &workspace.service(),
            READ_FILE,
            json!({
                "path": "lines.txt", "offset": 1, "limit": 1
            }),
        );
        let structured = result.structured_content.expect("structured result");
        assert_eq!(structured["content"], "one\n");
        assert_eq!(structured["offset"], 1);
        assert_eq!(structured["linesRead"], 1);
        assert_eq!(structured["bytesRead"], 4);
        assert_eq!(structured["truncated"], true);
        assert_eq!(structured["nextOffset"], 2);
        assert!(structured["normalizedPath"]
            .as_str()
            .unwrap()
            .ends_with("lines.txt"));
        assert_eq!(result.is_error, Some(false));
    }

    #[test]
    fn read_file_security_and_content_failures_are_sanitized_tool_errors() {
        let workspace = TestWorkspace::new();
        let outside_file = workspace.outside.join("secret.txt");
        fs::write(&outside_file, "secret").expect("write outside fixture");
        workspace.write("invalid.bin", [0xff]);
        workspace.write("contains-nul.bin", b"a\0b");
        let service = workspace.service();

        for path in [
            Value::String("../outside/secret.txt".into()),
            Value::String(outside_file.to_string_lossy().into_owned()),
        ] {
            let result = call(&service, READ_FILE, json!({ "path": path }));
            assert_eq!(result.is_error, Some(true));
            assert_eq!(error_code(&result), "WORKSPACE_VIOLATION");
            assert!(!serde_json::to_string(&result)
                .unwrap()
                .contains(&*workspace.outside.to_string_lossy()));
        }
        for path in ["invalid.bin", "contains-nul.bin"] {
            let result = call(&service, READ_FILE, json!({ "path": path }));
            assert_eq!(error_code(&result), "UNSUPPORTED_CONTENT");
        }
    }

    #[test]
    fn glob_maps_base_path_and_structured_result() {
        let workspace = TestWorkspace::new();
        workspace.write("src/main.rs", "fn main() {}\n");
        workspace.write("src/nested/lib.rs", "");
        let result = call(
            &workspace.service(),
            GLOB,
            json!({
                "pattern": "**/*.rs", "basePath": "src"
            }),
        );
        let structured = result.structured_content.expect("structured result");
        assert_eq!(structured["matches"][0]["path"], "src/main.rs");
        assert_eq!(structured["matches"][0]["kind"], "file");
        assert_eq!(structured["matches"][1]["path"], "src/nested/lib.rs");
        assert_eq!(structured["truncated"], false);
        assert_eq!(structured["diagnostics"], json!([]));
        assert_eq!(structured["diagnosticsTruncated"], false);
    }

    #[test]
    fn glob_security_and_malformed_arguments_return_tool_errors() {
        let workspace = TestWorkspace::new();
        let service = workspace.service();
        for base_path in [
            Value::String("../outside".into()),
            Value::String(workspace.outside.to_string_lossy().into_owned()),
        ] {
            let result = call(
                &service,
                GLOB,
                json!({ "pattern": "*", "basePath": base_path }),
            );
            assert_eq!(error_code(&result), "WORKSPACE_VIOLATION");
        }
        let missing_base = call(
            &service,
            GLOB,
            json!({ "pattern": "*", "basePath": "missing" }),
        );
        assert_eq!(error_code(&missing_base), "INVALID_BASE_PATH");
        for (name, arguments) in [
            (READ_FILE, json!({ "offset": 0 })),
            (READ_FILE, json!({ "path": "a.txt", "offset": -1 })),
            (READ_FILE, json!({ "path": 3 })),
            (GLOB, json!({})),
            (GLOB, json!({ "pattern": "*", "basePath": 3 })),
        ] {
            let result = call(&service, name, arguments);
            assert_eq!(error_code(&result), "INVALID_ARGUMENTS");
        }
    }

    #[test]
    fn unknown_tool_is_a_protocol_routing_failure() {
        let workspace = TestWorkspace::new();
        assert!(matches!(
            workspace.service().call_tool("unknown_tool", json!({})),
            Err(McpProtocolError::UnknownTool)
        ));
    }
}
