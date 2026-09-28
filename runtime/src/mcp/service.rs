use crate::mcp::context::McpExecutionContext;
use crate::mcp::{edit, glob, read_file, write};
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
const WRITE: &str = "write";
const EDIT: &str = "edit";

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
            WRITE => Ok(write::execute(&self.context, arguments)),
            EDIT => Ok(edit::execute(&self.context, arguments)),
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
        WRITE => Some(write_tool_with_metadata(&tool.name, &tool.description)),
        EDIT => Some(edit_tool_with_metadata(&tool.name, &tool.description)),
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

fn write_tool_with_metadata(name: &str, description: &str) -> Tool {
    Tool::new(
        name.to_owned(),
        description.to_owned(),
        schema_for_input::<write::WriteArguments>().expect("write MCP schema is statically valid"),
    )
    .with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(true)
            .open_world(false),
    )
}

fn edit_tool_with_metadata(name: &str, description: &str) -> Tool {
    Tool::new(
        name.to_owned(),
        description.to_owned(),
        schema_for_input::<edit::EditArguments>().expect("edit MCP schema is statically valid"),
    )
    .with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(true)
            .open_world(false),
    )
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
    fn ready_catalog_exposes_read_write_tools_and_excludes_unsupported_tools() {
        let workspace = TestWorkspace::new();
        let service = workspace.service();
        let names = service
            .tools()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, [READ_FILE, GLOB, WRITE, EDIT]);
        for forbidden in [
            "bash",
            "tree",
            "list_files",
            "find_files",
            "git",
            "database",
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

    #[test]
    fn write_and_edit_builders_expose_the_declared_schema_and_mutation_annotations() {
        let workspace = TestWorkspace::new();
        let service = workspace.service();
        for (name, required) in [
            (WRITE, ["path", "content"].as_slice()),
            (EDIT, ["path", "oldText", "newText"].as_slice()),
        ] {
            let tool = service.get_tool(name).expect("write/edit tool");
            let schema = serde_json::to_value(tool.input_schema.as_ref()).unwrap();
            let properties = schema["properties"].as_object().expect("properties");
            for field in required {
                assert!(properties.contains_key(*field), "{name} missing {field}");
            }
            assert_eq!(schema["additionalProperties"], false);
            let annotations = tool.annotations.expect("annotations");
            assert_eq!(annotations.read_only_hint, Some(false));
            assert_eq!(annotations.destructive_hint, Some(true));
            assert_eq!(annotations.open_world_hint, Some(false));
        }
    }

    #[test]
    fn write_and_edit_route_through_structured_success_results() {
        let workspace = TestWorkspace::new();
        workspace.write("src/placeholder.txt", "placeholder");
        let service = workspace.service();

        let write = call(
            &service,
            WRITE,
            json!({ "path": "src/generated.txt", "content": "hello from ChatGPT" }),
        );
        assert_eq!(write.is_error, Some(false));
        assert_eq!(
            write.structured_content.as_ref().unwrap(),
            &json!({
                "normalizedPath": "src/generated.txt",
                "bytesWritten": "hello from ChatGPT".len(),
                "created": true,
                "overwritten": false,
            })
        );
        assert_eq!(
            fs::read_to_string(workspace.root.join("src/generated.txt")).unwrap(),
            "hello from ChatGPT"
        );

        let edit = call(
            &service,
            EDIT,
            json!({
                "path": "src/generated.txt",
                "oldText": "hello from ChatGPT",
                "newText": "hello from V3"
            }),
        );
        assert_eq!(edit.is_error, Some(false));
        assert_eq!(
            edit.structured_content.as_ref().unwrap()["normalizedPath"],
            "src/generated.txt"
        );
        assert_eq!(edit.structured_content.as_ref().unwrap()["replacements"], 1);
        assert_eq!(
            fs::read_to_string(workspace.root.join("src/generated.txt")).unwrap(),
            "hello from V3"
        );
    }

    #[test]
    fn write_and_edit_failures_are_sanitized_and_deterministic() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "one two one");
        let outside = workspace.outside.join("secret.txt");
        fs::write(&outside, "outside secret").expect("outside fixture");
        let service = workspace.service();

        for arguments in [
            json!({ "path": "file.txt" }),
            json!({ "path": "file.txt", "content": "x", "extra": true }),
        ] {
            assert_eq!(
                error_code(&call(&service, WRITE, arguments)),
                "INVALID_ARGUMENTS"
            );
        }
        let outside_result = call(
            &service,
            WRITE,
            json!({ "path": outside.to_string_lossy(), "content": "blocked" }),
        );
        assert_eq!(error_code(&outside_result), "WORKSPACE_VIOLATION");
        assert!(!serde_json::to_string(&outside_result)
            .unwrap()
            .contains(&*outside.to_string_lossy()));

        let no_match = call(
            &service,
            EDIT,
            json!({ "path": "file.txt", "oldText": "missing", "newText": "new" }),
        );
        assert_eq!(error_code(&no_match), "EDIT_MATCH_NOT_FOUND");
        let non_unique = call(
            &service,
            EDIT,
            json!({ "path": "file.txt", "oldText": "one", "newText": "new" }),
        );
        assert_eq!(error_code(&non_unique), "EDIT_MATCH_NOT_UNIQUE");
        let empty_old = call(
            &service,
            EDIT,
            json!({ "path": "file.txt", "oldText": "", "newText": "new" }),
        );
        assert_eq!(error_code(&empty_old), "INVALID_ARGUMENTS");
        assert_eq!(
            fs::read_to_string(workspace.root.join("file.txt")).unwrap(),
            "one two one"
        );
    }

    #[test]
    fn write_and_edit_use_existing_audit_lifecycle_and_boundary_rejection_has_no_start() {
        use crate::audit::AuditEventType;
        use crate::state::RuntimeCore;

        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");

        let success = call(
            &service,
            WRITE,
            json!({ "path": "created.txt", "content": "created" }),
        );
        assert_eq!(success.is_error, Some(false));
        let entries = core.activity_snapshot(20);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.event_type)
                .collect::<Vec<_>>(),
            [
                AuditEventType::RequestReceived,
                AuditEventType::WorkspaceValidated,
                AuditEventType::PolicyEvaluated,
                AuditEventType::ExecutionStarted,
                AuditEventType::ExecutionCompleted,
            ]
        );
        assert!(entries
            .iter()
            .all(|entry| entry.tool_name.as_deref() == Some(WRITE)));

        let outside = workspace.outside.join("outside.txt");
        fs::write(&outside, "outside").expect("outside fixture");
        let rejected = call(
            &service,
            EDIT,
            json!({
                "path": outside.to_string_lossy(),
                "oldText": "outside",
                "newText": "blocked"
            }),
        );
        assert_eq!(error_code(&rejected), "WORKSPACE_VIOLATION");
        let entries = core.activity_snapshot(20);
        let rejected_entries = &entries[5..];
        assert_eq!(
            rejected_entries
                .iter()
                .map(|entry| entry.event_type)
                .collect::<Vec<_>>(),
            [
                AuditEventType::RequestReceived,
                AuditEventType::WorkspaceValidated,
            ]
        );
        assert!(rejected_entries
            .iter()
            .all(|entry| entry.tool_name.as_deref() == Some(EDIT)));
        assert!(!rejected_entries
            .iter()
            .any(|entry| entry.event_type == AuditEventType::ExecutionStarted));
        assert!(!serde_json::to_string(&entries)
            .unwrap()
            .contains(&*workspace.root.to_string_lossy()));
    }
}
