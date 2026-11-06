use crate::mcp::context::McpExecutionContext;
use crate::mcp::{bash, edit, glob, grep, read_file, write};
use crate::tools::{EffectiveTool, ToolAvailability};
use rmcp::handler::server::common::schema_for_input;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorCode,
    Implementation, ListToolsResult, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
    ToolAnnotations,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ErrorData;
use serde_json::Value;
use std::borrow::Cow;
use tokio_util::sync::CancellationToken;

const READ_FILE: &str = "read_file";
const GLOB: &str = "glob";
const GREP: &str = "grep";
const WRITE: &str = "write";
const EDIT: &str = "edit";
const BASH: &str = "bash";

#[derive(Clone)]
pub struct McpService {
    context: McpExecutionContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpProtocolError {
    UnknownTool,
    AsyncOnly,
}

impl McpService {
    pub fn new(context: McpExecutionContext) -> Self {
        Self { context }
    }

    /// Lists tools that the current Runtime can call directly or can call
    /// after a human approval. Disabled, denied, and unavailable tools remain
    /// hidden from the MCP exposure boundary.
    pub fn tools(&self) -> Vec<Tool> {
        self.context
            .effective_tools()
            .into_iter()
            .filter(|tool| {
                matches!(
                    tool.availability,
                    ToolAvailability::Available | ToolAvailability::AvailableWithApproval
                ) || (self.context.exposes_policy_denied_tools()
                    && tool.availability == ToolAvailability::UnavailableByPolicy)
            })
            .filter_map(tool_from_effective)
            .collect()
    }

    fn wire_tools(&self) -> Vec<Tool> {
        self.tools()
            .into_iter()
            .map(strip_later_revision_tool_fields)
            .collect()
    }

    fn is_known_tool(&self, name: &str) -> bool {
        matches!(name, READ_FILE | GLOB | GREP | WRITE | EDIT | BASH)
    }

    fn tool_schema(&self, name: &str) -> Option<Value> {
        let tool = match name {
            READ_FILE => read_file_tool_with_metadata(READ_FILE, ""),
            GLOB => glob_tool_with_metadata(GLOB, ""),
            GREP => grep_tool_with_metadata(GREP, ""),
            WRITE => write_tool_with_metadata(WRITE, ""),
            EDIT => edit_tool_with_metadata(EDIT, ""),
            BASH => bash_tool_with_metadata(BASH, ""),
            _ => return None,
        };
        serde_json::to_value(tool.input_schema.as_ref()).ok()
    }

    fn validate_tool_arguments(&self, name: &str, arguments: &Value) -> Result<(), ErrorData> {
        let Some(schema) = self.tool_schema(name) else {
            return Err(ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                "Tool request is invalid.",
                None,
            ));
        };
        let validator = jsonschema::validator_for(&schema).map_err(|_| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                "Tool schema is unavailable.",
                None,
            )
        })?;
        if validator.validate(arguments).is_err() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                "Tool arguments are invalid.",
                None,
            ));
        }
        Ok(())
    }

    fn observe_unknown_tool(&self) {
        self.context.observe_unknown_tool();
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
            GREP => Ok(grep::execute(&self.context, arguments)),
            WRITE => Ok(write::execute(&self.context, arguments)),
            EDIT => Ok(edit::execute(&self.context, arguments)),
            BASH => Err(McpProtocolError::AsyncOnly),
            _ => {
                self.context.observe_unknown_tool();
                Err(McpProtocolError::UnknownTool)
            }
        }
    }

    pub async fn call_tool_async(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<CallToolResult, McpProtocolError> {
        self.call_tool_async_with_cancel(name, arguments, CancellationToken::new())
            .await
    }

    pub async fn call_tool_async_with_cancel(
        &self,
        name: &str,
        arguments: Value,
        request_cancel: CancellationToken,
    ) -> Result<CallToolResult, McpProtocolError> {
        match name {
            READ_FILE => Ok(read_file::execute_async(&self.context, arguments).await),
            GLOB => Ok(glob::execute_async(&self.context, arguments).await),
            GREP => Ok(grep::execute_async(&self.context, arguments, request_cancel).await),
            WRITE => Ok(write::execute_async(&self.context, arguments).await),
            EDIT => Ok(edit::execute_async(&self.context, arguments).await),
            BASH => Ok(bash::execute(&self.context, arguments, request_cancel).await),
            _ => {
                self.context.observe_unknown_tool();
                Err(McpProtocolError::UnknownTool)
            }
        }
    }
}

impl ServerHandler for McpService {
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2025_03_26])
    }

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
        std::future::ready(Ok(ListToolsResult::with_all_items(self.wire_tools())))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResponse, ErrorData>>
           + rmcp::service::MaybeSendFuture
           + '_ {
        let service = self.clone();
        async move {
            if !service.is_known_tool(&request.name) {
                service.observe_unknown_tool();
                return Err(ErrorData::new(
                    ErrorCode::INVALID_PARAMS,
                    "Tool request is invalid.",
                    None,
                ));
            }
            let arguments = Value::Object(request.arguments.unwrap_or_default());
            service.validate_tool_arguments(&request.name, &arguments)?;
            service
                .call_tool_async_with_cancel(&request.name, arguments, context.ct.clone())
                .await
                .map(project_call_tool_result)
                .map(Into::into)
                .map_err(|error| match error {
                    McpProtocolError::UnknownTool => {
                        ErrorData::new(ErrorCode::INVALID_PARAMS, "Tool request is invalid.", None)
                    }
                    McpProtocolError::AsyncOnly => {
                        ErrorData::new(ErrorCode::INVALID_PARAMS, "Tool request is invalid.", None)
                    }
                })
        }
    }
}

fn strip_later_revision_tool_fields(mut tool: Tool) -> Tool {
    tool.output_schema = None;
    tool
}

/// Projects the Runtime's typed safe result into the frozen MCP 2025-03-26
/// CallToolResult shape. Internal callers continue to receive the structured
/// result; only the protocol boundary drops `structuredContent`.
fn project_call_tool_result(mut result: CallToolResult) -> CallToolResult {
    let canonical_json = result
        .structured_content
        .take()
        .and_then(|value| serde_json::to_string(&value).ok());
    let existing_content = std::mem::take(&mut result.content);
    let mut content = Vec::new();
    if let Some(canonical_json) = canonical_json {
        content.push(ContentBlock::text(canonical_json));
    }
    for block in existing_content {
        let Some(text) = block.as_text() else {
            content.push(block);
            continue;
        };
        let duplicate = content.iter().any(|existing| {
            existing
                .as_text()
                .is_some_and(|existing| existing.text == text.text)
        });
        if !duplicate {
            content.push(block);
        }
    }
    result.content = content;
    result
}

fn tool_from_effective(tool: EffectiveTool) -> Option<Tool> {
    match tool.name.as_str() {
        READ_FILE => Some(read_file_tool_with_metadata(&tool.name, &tool.description)),
        GLOB => Some(glob_tool_with_metadata(&tool.name, &tool.description)),
        GREP => Some(grep_tool_with_metadata(&tool.name, &tool.description)),
        WRITE => Some(write_tool_with_metadata(&tool.name, &tool.description)),
        EDIT => Some(edit_tool_with_metadata(&tool.name, &tool.description)),
        BASH => Some(bash_tool_with_metadata(&tool.name, &tool.description)),
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

fn grep_tool_with_metadata(name: &str, description: &str) -> Tool {
    Tool::new(
        name.to_owned(),
        description.to_owned(),
        schema_for_input::<grep::GrepArguments>().expect("grep MCP schema is statically valid"),
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

fn bash_tool_with_metadata(name: &str, description: &str) -> Tool {
    Tool::new(
        name.to_owned(),
        description.to_owned(),
        schema_for_input::<bash::BashArguments>().expect("bash MCP schema is statically valid"),
    )
    .with_output_schema::<bash::BashOutputSchema>()
    .with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(true)
            .open_world(true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::RuntimeCore;
    use harness_control_protocol::{
        DecimalU64, PolicyDecision, RuntimeConfigSnapshot, RuntimeLimits, SecurityConfig,
        SecurityMode, ToolConfig, UserPolicyRuleConfig, WorkspaceConfig,
    };
    use rmcp::handler::server::ServerHandler;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tokio_util::sync::CancellationToken;

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

        fn assert_no_atomic_temp_residue(&self) {
            let residue = fs::read_dir(&self.root)
                .expect("read workspace directory")
                .map(|entry| entry.expect("read workspace entry").file_name())
                .filter(|name| name.to_string_lossy().contains(".chatgpt-harness-"))
                .collect::<Vec<_>>();
            assert!(residue.is_empty(), "atomic temporary residue: {residue:?}");
        }

        fn service(&self) -> McpService {
            self.service_with_config(config_for(&self.root))
        }

        fn service_with_config(&self, config: RuntimeConfigSnapshot) -> McpService {
            let mut core = RuntimeCore::new();
            core.initialize(config).expect("initialize runtime");
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
            tools: ToolConfig::default(),
            bash: harness_control_protocol::BashConfig::default(),
            policies: harness_control_protocol::PolicyConfig::default(),
        }
    }

    #[cfg(windows)]
    fn bash_config_for(
        root: &Path,
        decision: harness_control_protocol::PolicyDecision,
    ) -> RuntimeConfigSnapshot {
        let mut config = config_for(root);
        let program_files = std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .expect("ProgramFiles for Git Bash fixture");
        let bash = program_files.join("Git").join("bin").join("bash.exe");
        assert!(
            bash.is_file(),
            "Git Bash fixture is required: {}",
            bash.display()
        );
        config.security = harness_control_protocol::SecurityConfig {
            mode: harness_control_protocol::SecurityMode::DeveloperTest,
            developer_test_mode: true,
        };
        config.tools.bash.enabled = true;
        config.bash = harness_control_protocol::BashConfig {
            auto_detect: false,
            executable_path: Some(bash.to_string_lossy().into_owned()),
        };
        config.policies.tool_defaults.bash = decision;
        config
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

    async fn wait_for_pending(core: &RuntimeCore, count: usize) {
        for _ in 0..100 {
            if core.approval_manager().pending_count() == count {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        panic!("timed out waiting for {count} pending approvals");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn bash_allow_once_resumes_same_request_with_structured_output() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        let mut config = bash_config_for(
            &workspace.root,
            harness_control_protocol::PolicyDecision::Ask,
        );
        config.policies.tool_defaults.bash = harness_control_protocol::PolicyDecision::Ask;
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(BASH, json!({ "command": "printf bash-ok" }))
                    .await
                    .expect("known Bash tool")
            }
        });
        wait_for_pending(&core, 1).await;
        let pending = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        assert_eq!(pending[0].tool, BASH);
        core.approval_manager()
            .resolve(
                &pending[0].request_id,
                harness_control_protocol::ApprovalDecision::AllowOnce,
            )
            .expect("allow Bash once");
        let result = call.await.expect("Bash call task");
        assert_eq!(result.is_error, Some(false));
        assert_eq!(result.structured_content.as_ref().unwrap()["exitCode"], 0);
        assert_eq!(
            result.structured_content.as_ref().unwrap()["stdout"],
            "bash-ok"
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn bash_request_cancellation_cancels_pending_approval_without_spawn() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(bash_config_for(
            &workspace.root,
            harness_control_protocol::PolicyDecision::Ask,
        ))
        .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let cancellation = CancellationToken::new();
        let call = tokio::spawn({
            let service = service.clone();
            let cancellation = cancellation.clone();
            async move {
                service
                    .call_tool_async_with_cancel(
                        BASH,
                        json!({ "command": "sleep 60" }),
                        cancellation,
                    )
                    .await
                    .expect("known Bash tool")
            }
        });
        wait_for_pending(&core, 1).await;
        cancellation.cancel();
        let result = call.await.expect("cancelled Bash call task");
        assert_eq!(error_code(&result), "APPROVAL_CANCELLED");
        assert_eq!(core.approval_manager().pending_count(), 0);
        assert!(!workspace.root.join("should-not-run").exists());
        core.drain_activity_events();
        let activity = core.activity_snapshot(100);
        let cancelled = activity
            .iter()
            .find(|entry| entry.event_type == crate::audit::AuditEventType::ApprovalCancelled)
            .expect("approval cancellation evidence");
        assert_eq!(cancelled.tool_name.as_deref(), Some(BASH));
        assert!(cancelled.duration_ms.is_some());
        assert!(cancelled.bash.as_ref().unwrap().duration_ms.is_none());
        assert_eq!(
            cancelled.failure_category,
            Some(crate::audit::AuditFailureCategory::ApprovalCancelled)
        );
        assert_eq!(
            cancelled.bash.as_ref().unwrap().cwd,
            harness_control_protocol::BashWorkingDirectory::ActiveWorkspace
        );
        let serialized = serde_json::to_string(&activity).unwrap();
        assert!(!serialized.contains("stdout"));
        assert!(!serialized.contains("stderr"));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn bash_request_cancellation_stops_allowed_process() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(bash_config_for(
            &workspace.root,
            harness_control_protocol::PolicyDecision::Allow,
        ))
        .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let cancellation = CancellationToken::new();
        let call = tokio::spawn({
            let service = service.clone();
            let cancellation = cancellation.clone();
            async move {
                service
                    .call_tool_async_with_cancel(
                        BASH,
                        json!({ "command": "sleep 60" }),
                        cancellation,
                    )
                    .await
                    .expect("known Bash tool")
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(core.request_counts(), (1, 1));
        cancellation.cancel();
        let result = call.await.expect("cancelled Bash call task");
        assert_eq!(error_code(&result), "BASH_CANCELLED");
        assert_eq!(core.request_counts(), (0, 0));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn runtime_stop_cancels_allowed_bash_process() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(bash_config_for(
            &workspace.root,
            harness_control_protocol::PolicyDecision::Allow,
        ))
        .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(BASH, json!({ "command": "sleep 60" }))
                    .await
                    .expect("known Bash tool")
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        core.stop();
        let result = call.await.expect("runtime-stopped Bash call task");
        assert_eq!(error_code(&result), "BASH_CANCELLED");
        assert_eq!(core.request_counts(), (0, 0));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn post_approval_revalidation_rejects_stopped_bash_generation() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(bash_config_for(
            &workspace.root,
            harness_control_protocol::PolicyDecision::Ask,
        ))
        .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(BASH, json!({ "command": "printf should-not-run" }))
                    .await
                    .expect("known Bash tool")
            }
        });
        wait_for_pending(&core, 1).await;
        let pending = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        core.bash_runtime().expect("Bash Runtime").stop();
        core.approval_manager()
            .resolve(
                &pending[0].request_id,
                harness_control_protocol::ApprovalDecision::AllowOnce,
            )
            .expect("resolve approval");
        let result = call.await.expect("post-approval task");
        assert_eq!(error_code(&result), "BASH_CANCELLED");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn normal_mode_bash_stale_call_is_unavailable_by_mode_not_policy_denied() {
        let workspace = TestWorkspace::new();
        let mut config = bash_config_for(
            &workspace.root,
            harness_control_protocol::PolicyDecision::Allow,
        );
        config.security.mode = harness_control_protocol::SecurityMode::Normal;
        config.security.developer_test_mode = true;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let result = service
            .call_tool_async(BASH, json!({ "command": "printf should-not-run" }))
            .await
            .expect("known Bash tool");
        assert_eq!(error_code(&result), "BASH_UNAVAILABLE");
    }

    #[test]
    fn context_is_unavailable_until_runtime_initializes() {
        let core = RuntimeCore::new();
        assert!(matches!(
            core.mcp_context(),
            Err(crate::mcp::McpContextError::RuntimeNotInitialized)
        ));
    }

    #[tokio::test]
    async fn ask_call_stays_pending_and_allow_once_resumes_the_same_invocation() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        assert!(service.tools().iter().any(|tool| tool.name == WRITE));

        let pending_call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(
                        WRITE,
                        json!({ "path": "approved.txt", "content": "approved" }),
                    )
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 1).await;
        let pending = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        assert_eq!(pending.len(), 1);
        let status = core.status().unwrap();
        assert_eq!(
            (
                status.active_requests,
                status.pending_approvals,
                status.running_executions
            ),
            (1, 1, 0)
        );
        assert_eq!(pending[0].tool, WRITE);
        assert_eq!(pending[0].scope_summary, "approved.txt");
        assert_eq!(
            pending[0].sanitized_arguments["content"],
            "<redacted content>"
        );

        core.approval_manager()
            .resolve(
                &pending[0].request_id,
                harness_control_protocol::ApprovalDecision::AllowOnce,
            )
            .expect("resolve pending approval");
        let result = pending_call.await.expect("same invocation task");
        assert_eq!(result.is_error, Some(false));
        assert_eq!(
            fs::read_to_string(workspace.root.join("approved.txt")).unwrap(),
            "approved"
        );
        let entries = core.activity_snapshot(256);
        let lifecycle: Vec<_> = entries
            .iter()
            .filter(|e| e.request_id.to_string() == pending[0].request_id)
            .collect();
        for entry in lifecycle.iter().filter(|e| {
            matches!(
                e.event_type,
                crate::audit::AuditEventType::ApprovalRequired
                    | crate::audit::AuditEventType::ApprovalResolved
                    | crate::audit::AuditEventType::ExecutionCompleted
            )
        }) {
            assert_eq!(
                entry.approval_id.as_deref(),
                Some(pending[0].request_id.as_str())
            );
        }
        let terminal = lifecycle.last().unwrap();
        assert!(terminal.duration_ms.is_some());
        assert_eq!(
            terminal.approval_decision,
            Some(harness_control_protocol::ApprovalDecision::AllowOnce)
        );
        assert!(terminal.result_evidence.is_some());

        assert_eq!(core.request_counts(), (0, 0));
    }

    #[tokio::test]
    async fn deny_wakes_exact_call_without_a_filesystem_side_effect() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let pending_call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(WRITE, json!({ "path": "denied.txt", "content": "secret" }))
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 1).await;
        let pending = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        core.approval_manager()
            .resolve(
                &pending[0].request_id,
                harness_control_protocol::ApprovalDecision::Deny,
            )
            .expect("deny pending approval");
        let result = pending_call.await.expect("same invocation task");
        assert_eq!(error_code(&result), "APPROVAL_DENIED");
        assert!(!workspace.root.join("denied.txt").exists());
        assert_eq!(core.request_counts(), (0, 0));
    }

    #[tokio::test]
    async fn post_approval_workspace_revalidation_rejects_a_deleted_target() {
        let workspace = TestWorkspace::new();
        workspace.write("vanishing.txt", "content");
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.read_file = harness_control_protocol::PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(READ_FILE, json!({ "path": "vanishing.txt" }))
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 1).await;
        fs::remove_file(workspace.root.join("vanishing.txt")).expect("remove target");
        let pending = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        core.approval_manager()
            .resolve(
                &pending[0].request_id,
                harness_control_protocol::ApprovalDecision::AllowOnce,
            )
            .expect("allow original request");
        let result = call.await.expect("call task");
        assert_eq!(error_code(&result), "TARGET_NOT_FOUND");
        assert!(!workspace.root.join("vanishing.txt").exists());
    }

    #[tokio::test]
    async fn runtime_stop_cancels_pending_call_and_clears_generation_grants() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(
                        WRITE,
                        json!({ "path": "stopped.txt", "content": "blocked" }),
                    )
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 1).await;
        core.stop().expect("stop runtime");
        assert_eq!(core.approval_manager().pending_count(), 0);
        assert_eq!(core.session_grants().len(), 0);
        let result = call.await.expect("call task");
        assert_eq!(error_code(&result), "APPROVAL_CANCELLED");
        assert!(!workspace.root.join("stopped.txt").exists());
        assert_eq!(core.request_counts(), (0, 0));
    }

    #[tokio::test]
    async fn dropped_mcp_call_cancels_its_pending_approval() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(
                        WRITE,
                        json!({ "path": "disconnected.txt", "content": "blocked" }),
                    )
                    .await
            }
        });
        wait_for_pending(&core, 1).await;
        call.abort();
        let _ = call.await;
        for _ in 0..20 {
            if core.approval_manager().pending_count() == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(core.approval_manager().pending_count(), 0);
        assert_eq!(
            core.approval_manager()
                .list(Some(harness_control_protocol::ApprovalStatus::Cancelled))
                .len(),
            1
        );
        assert!(!workspace.root.join("disconnected.txt").exists());
        let terminal = core.activity_snapshot(1).pop().unwrap();
        assert_eq!(
            terminal.event_type,
            crate::audit::AuditEventType::ApprovalCancelled
        );
        assert_eq!(terminal.approval_id, Some(terminal.request_id.to_string()));
        assert!(terminal.duration_ms.is_some());

        assert_eq!(core.request_counts(), (0, 0));
    }

    #[tokio::test]
    async fn approval_for_request_a_cannot_wake_request_b() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let first = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(WRITE, json!({ "path": "first.txt", "content": "first" }))
                    .await
                    .expect("known tool")
            }
        });
        let second = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(WRITE, json!({ "path": "second.txt", "content": "second" }))
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 2).await;
        let pending = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        let first_request = pending
            .iter()
            .find(|request| request.scope_summary == "first.txt")
            .expect("first request")
            .request_id
            .clone();
        core.approval_manager()
            .resolve(
                &first_request,
                harness_control_protocol::ApprovalDecision::AllowOnce,
            )
            .expect("resolve first request");
        let first_result = first.await.expect("first task");
        assert_eq!(first_result.is_error, Some(false));
        assert!(workspace.root.join("first.txt").exists());
        assert!(!workspace.root.join("second.txt").exists());
        assert_eq!(core.approval_manager().pending_count(), 1);

        let second_request = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending))[0]
            .request_id
            .clone();
        core.approval_manager()
            .resolve(
                &second_request,
                harness_control_protocol::ApprovalDecision::Deny,
            )
            .expect("resolve second request");
        assert_eq!(
            error_code(&second.await.expect("second task")),
            "APPROVAL_DENIED"
        );
    }

    #[tokio::test]
    async fn allow_for_session_creates_a_narrow_grant_for_later_same_scope_calls() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = harness_control_protocol::PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let first = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(WRITE, json!({ "path": "session.txt", "content": "first" }))
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 1).await;
        let pending = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        let request_id = pending[0].request_id.clone();
        let manager = core.approval_manager();
        let grants = core.session_grants();
        let decision = harness_control_protocol::ApprovalDecision::AllowForSession;
        manager
            .resolve_with(&request_id, decision, |approval, scope| {
                let scope = scope.expect("classified file approval has grant scope");
                grants.insert_from_approved_scope(
                    &approval.generation_id,
                    &approval.request_id,
                    &approval.tool,
                    scope,
                    chrono::Utc::now(),
                );
            })
            .expect("resolve for session");
        assert_eq!(first.await.expect("first task").is_error, Some(false));
        assert!(grants.matches(core.runtime_id.as_str(), WRITE, "session.txt"));

        let second = service
            .call_tool_async(WRITE, json!({ "path": "session.txt", "content": "second" }))
            .await
            .expect("known tool");
        assert_eq!(second.is_error, Some(false));
        assert_eq!(
            fs::read_to_string(workspace.root.join("session.txt")).unwrap(),
            "second"
        );
        assert_eq!(core.approval_manager().pending_count(), 0);

        let different_scope = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(WRITE, json!({ "path": "other.txt", "content": "other" }))
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 1).await;
        let other = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        core.approval_manager()
            .resolve(
                &other[0].request_id,
                harness_control_protocol::ApprovalDecision::Deny,
            )
            .expect("deny different scope");
        assert_eq!(
            error_code(&different_scope.await.expect("different scope task")),
            "APPROVAL_DENIED"
        );
        assert!(!workspace.root.join("other.txt").exists());
        assert_eq!(core.request_counts(), (0, 0));
        let entries = core.activity_snapshot(256);
        let grant = entries
            .iter()
            .find(|e| e.event_type == crate::audit::AuditEventType::SessionGrantMatched)
            .unwrap();
        assert_ne!(grant.request_id.to_string(), request_id);
        assert_eq!(
            grant.decision_summary,
            Some(harness_control_protocol::PolicyDecision::Ask)
        );
        assert!(grant.duration_ms.is_none());
        assert!(grant.result_evidence.is_none());
        let terminal = entries
            .iter()
            .find(|e| {
                e.request_id == grant.request_id
                    && e.event_type == crate::audit::AuditEventType::ExecutionCompleted
            })
            .unwrap();
        assert!(terminal.duration_ms.is_some());
        assert!(terminal.result_evidence.is_some());
    }

    #[tokio::test]
    async fn explicit_deny_precedes_an_existing_session_grant() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = PolicyDecision::Ask;
        config.policies.user_rules.push(UserPolicyRuleConfig::new(
            "deny-writes",
            "Deny writes",
            PolicyDecision::Deny,
            WRITE,
            "Integration explicit deny fixture.",
        ));
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        core.session_grants().insert_from_approved_scope(
            &core.runtime_id,
            "source-request",
            WRITE,
            "denied-by-policy.txt",
            chrono::Utc::now(),
        );
        let service = core.mcp_service().expect("MCP service");
        let result = service
            .call_tool_async(
                WRITE,
                json!({ "path": "denied-by-policy.txt", "content": "blocked" }),
            )
            .await
            .expect("known tool");
        assert_eq!(error_code(&result), "POLICY_DENIED");
        assert_eq!(core.approval_manager().pending_count(), 0);
        assert!(!workspace.root.join("denied-by-policy.txt").exists());
    }

    #[tokio::test]
    async fn system_workspace_guardrail_precedes_an_existing_session_grant() {
        let workspace = TestWorkspace::new();
        let outside_target = workspace.outside.join("outside.txt");
        fs::write(&outside_target, "outside").expect("write outside fixture");
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        core.session_grants().insert_from_approved_scope(
            &core.runtime_id,
            "source-request",
            WRITE,
            &outside_target.to_string_lossy(),
            chrono::Utc::now(),
        );
        let service = core.mcp_service().expect("MCP service");
        let result = service
            .call_tool_async(
                WRITE,
                json!({
                    "path": outside_target.to_string_lossy(),
                    "content": "must stay outside"
                }),
            )
            .await
            .expect("known tool");
        assert_eq!(error_code(&result), "WORKSPACE_VIOLATION");
        assert_eq!(core.approval_manager().pending_count(), 0);
        assert_eq!(fs::read(&outside_target).unwrap(), b"outside");
    }

    #[tokio::test]
    async fn post_approval_runtime_revalidation_rejects_when_readiness_changes() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(
                        WRITE,
                        json!({ "path": "runtime-revalidated.txt", "content": "blocked" }),
                    )
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 1).await;
        core.set_mcp_readiness_for_test(false);
        let pending = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending));
        core.approval_manager()
            .resolve(
                &pending[0].request_id,
                harness_control_protocol::ApprovalDecision::AllowOnce,
            )
            .expect("approve original request");
        let result = call.await.expect("same request task");
        assert_eq!(error_code(&result), "RUNTIME_NOT_READY");
        assert!(!workspace.root.join("runtime-revalidated.txt").exists());
        let approved = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Approved));
        assert_eq!(approved.len(), 1);
        assert_eq!(
            approved[0].decision,
            Some(harness_control_protocol::ApprovalDecision::AllowOnce)
        );
    }

    #[tokio::test]
    async fn runtime_generation_reset_drops_old_pending_and_grants() {
        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.policies.tool_defaults.write = PolicyDecision::Ask;
        let mut core = RuntimeCore::new();
        core.initialize(config.clone()).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let call = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .call_tool_async(
                        WRITE,
                        json!({ "path": "generation-reset.txt", "content": "blocked" }),
                    )
                    .await
                    .expect("known tool")
            }
        });
        wait_for_pending(&core, 1).await;
        let old_request_id = core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending))[0]
            .request_id
            .clone();
        core.stop().expect("stop old generation");
        assert_eq!(
            error_code(&call.await.expect("cancelled old call")),
            "APPROVAL_CANCELLED"
        );
        core.initialize(config).expect("initialize new generation");
        core.acknowledge().expect("acknowledge new generation");
        assert!(core
            .approval_manager()
            .list(Some(harness_control_protocol::ApprovalStatus::Pending))
            .is_empty());
        assert_eq!(
            core.approval_manager().resolve(
                &old_request_id,
                harness_control_protocol::ApprovalDecision::AllowOnce
            ),
            Err(crate::approval::ApprovalManagerError::ApprovalNotFound)
        );
        assert_eq!(core.session_grants().len(), 0);
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
    fn write_is_not_executable_before_runtime_acknowledgement() {
        use crate::audit::AuditEventType;

        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        let service = core.mcp_service().expect("MCP service");

        let result = call(
            &service,
            WRITE,
            json!({ "path": "not-ready.txt", "content": "blocked" }),
        );
        assert_eq!(error_code(&result), "RUNTIME_NOT_READY");
        assert!(!workspace.root.join("not-ready.txt").exists());

        let activity = core.activity_snapshot(20);
        assert!(!activity.iter().any(|entry| {
            matches!(
                entry.event_type,
                AuditEventType::ExecutionStarted | AuditEventType::ExecutionCompleted
            )
        }));
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
        assert_eq!(names, [READ_FILE, GLOB, GREP, WRITE, EDIT]);
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
    fn disabled_tools_are_absent_from_mcp_tools_list_but_enabled_siblings_remain() {
        for disabled in [READ_FILE, GLOB, GREP, WRITE, EDIT] {
            let workspace = TestWorkspace::new();
            let mut config = config_for(&workspace.root);
            match disabled {
                READ_FILE => config.tools.read_file.enabled = false,
                GLOB => config.tools.glob.enabled = false,
                GREP => config.tools.grep.enabled = false,
                WRITE => config.tools.write.enabled = false,
                EDIT => config.tools.edit.enabled = false,
                _ => unreachable!(),
            }
            let names = workspace
                .service_with_config(config)
                .tools()
                .into_iter()
                .map(|tool| tool.name.into_owned())
                .collect::<Vec<_>>();

            assert!(!names.iter().any(|name| name == disabled));
            assert_eq!(names.len(), 4);
        }
    }

    #[test]
    fn stale_disabled_write_call_is_rejected_before_executor_and_audit_marks_disabled() {
        use crate::audit::{AuditEventType, AuditFailureCategory};

        let workspace = TestWorkspace::new();
        let mut config = config_for(&workspace.root);
        config.tools.write.enabled = false;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");

        let result = call(
            &service,
            WRITE,
            json!({ "path": "stale.txt", "content": "must not write" }),
        );
        assert_eq!(error_code(&result), "TOOL_DISABLED");
        assert!(!workspace.root.join("stale.txt").exists());

        let activity = core.activity_snapshot(20);
        assert_eq!(
            activity
                .iter()
                .map(|entry| entry.event_type)
                .collect::<Vec<_>>(),
            [
                AuditEventType::RequestReceived,
                AuditEventType::ExecutionFailed
            ]
        );
        assert_eq!(
            activity[1].failure_category,
            Some(AuditFailureCategory::ToolDisabled)
        );
        assert!(!activity.iter().any(|entry| {
            matches!(
                entry.event_type,
                AuditEventType::ExecutionStarted | AuditEventType::ExecutionCompleted
            )
        }));
    }

    #[test]
    fn stale_disabled_read_call_is_rejected_without_read_success_evidence() {
        use crate::audit::{AuditEventType, AuditFailureCategory};

        let workspace = TestWorkspace::new();
        workspace.write("secret.txt", "must not be read");
        let mut config = config_for(&workspace.root);
        config.tools.read_file.enabled = false;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");

        let result = call(&service, READ_FILE, json!({ "path": "secret.txt" }));
        assert_eq!(error_code(&result), "TOOL_DISABLED");

        let activity = core.activity_snapshot(20);
        assert_eq!(activity.len(), 2);
        assert_eq!(activity[0].event_type, AuditEventType::RequestReceived);
        assert_eq!(activity[1].event_type, AuditEventType::ExecutionFailed);
        assert_eq!(
            activity[1].failure_category,
            Some(AuditFailureCategory::ToolDisabled)
        );
        assert!(!activity
            .iter()
            .any(|entry| entry.event_type == AuditEventType::ExecutionCompleted));
    }

    #[test]
    fn stale_disabled_edit_call_does_not_mutate_existing_file() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "unchanged");
        let mut config = config_for(&workspace.root);
        config.tools.edit.enabled = false;
        let mut core = RuntimeCore::new();
        core.initialize(config).expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");

        let result = call(
            &service,
            EDIT,
            json!({ "path": "file.txt", "oldText": "unchanged", "newText": "mutated" }),
        );
        assert_eq!(error_code(&result), "TOOL_DISABLED");
        assert_eq!(
            fs::read_to_string(workspace.root.join("file.txt")).unwrap(),
            "unchanged"
        );
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
    fn grep_exposes_exact_schema_and_routes_literal_regex_filters() {
        let workspace = TestWorkspace::new();
        workspace.write("docs/a.md", "GREP_MATCH_SENTINEL 4317\nalpha\n");
        workspace.write("docs/b.md", "4317\n");
        workspace.write("docs/skip.md", "4317\n");
        workspace.write("src/main.rs", "4317\n");
        let service = workspace.service();

        let tool = service.get_tool(GREP).expect("grep tool");
        let schema = serde_json::to_value(tool.input_schema.as_ref()).unwrap();
        let properties = schema["properties"].as_object().expect("properties");
        for field in [
            "query",
            "mode",
            "scope",
            "include",
            "exclude",
            "maxResults",
            "maxOutputBytes",
        ] {
            assert!(properties.contains_key(field), "grep missing {field}");
        }
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["query"]["minLength"], 1);
        assert_eq!(schema["properties"]["query"]["maxLength"], 4096);
        assert_eq!(schema["properties"]["mode"]["default"], "literal");
        assert_eq!(schema["properties"]["scope"]["default"], ".");
        assert_eq!(schema["properties"]["scope"]["minLength"], 1);
        assert_eq!(schema["properties"]["scope"]["maxLength"], 1024);
        assert_eq!(schema["properties"]["include"]["maxItems"], 32);
        assert_eq!(schema["properties"]["include"]["items"]["minLength"], 1);
        assert_eq!(schema["properties"]["include"]["items"]["maxLength"], 512);
        assert_eq!(schema["properties"]["exclude"]["maxItems"], 32);
        assert_eq!(schema["properties"]["exclude"]["items"]["maxLength"], 512);
        assert_eq!(schema["properties"]["maxResults"]["minimum"], 1);
        assert_eq!(schema["properties"]["maxResults"]["maximum"], 1000);
        assert_eq!(schema["properties"]["maxResults"]["default"], 100);
        assert_eq!(schema["properties"]["maxOutputBytes"]["minimum"], 1024);
        assert_eq!(schema["properties"]["maxOutputBytes"]["maximum"], 262_144);
        assert_eq!(schema["properties"]["maxOutputBytes"]["default"], 65_536);
        let validator = jsonschema::validator_for(&schema).expect("grep schema is valid");
        for invalid in [
            json!({ "query": "" }),
            json!({ "query": "x", "maxResults": 0 }),
            json!({ "query": "x", "maxResults": 1001 }),
            json!({ "query": "x", "maxOutputBytes": 1023 }),
            json!({ "query": "x", "maxOutputBytes": 262_145 }),
            json!({ "query": "x", "scope": "a".repeat(1025) }),
            json!({ "query": "x", "include": vec!["*"; 33] }),
            json!({ "query": "x", "include": ["a".repeat(513)] }),
            json!({ "query": "x", "exclude": vec!["*"; 33] }),
        ] {
            assert!(
                validator.validate(&invalid).is_err(),
                "grep schema accepted invalid arguments: {invalid}"
            );
        }
        let annotations = tool.annotations.expect("grep annotations");
        assert_eq!(annotations.read_only_hint, Some(true));
        assert_eq!(annotations.destructive_hint, Some(false));

        let literal = call(
            &service,
            GREP,
            json!({
                "query": "GREP_MATCH_SENTINEL",
                "scope": "docs",
                "include": ["docs/*.md"],
                "exclude": ["docs/skip.*"]
            }),
        );
        assert_eq!(literal.is_error, Some(false));
        let structured = literal.structured_content.as_ref().unwrap();
        assert_eq!(structured["matches"][0]["path"], "docs/a.md");
        assert_eq!(structured["matches"][0]["lineNumber"], 1);
        assert_eq!(structured["matches"][0]["previewStartByte"], 0);
        assert_eq!(structured["matches"][0]["matchStartByte"], 0);
        assert_eq!(structured["truncated"], false);
        assert_eq!(structured["truncationReasons"], json!([]));
        assert_eq!(structured["diagnostics"], json!([]));

        let regex = call(
            &service,
            GREP,
            json!({
                "query": "(?m)^4\\d{3}$",
                "mode": "regex",
                "scope": "docs"
            }),
        );
        assert_eq!(regex.is_error, Some(false));
        let regex = regex.structured_content.unwrap();
        assert_eq!(regex["matches"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn grep_argument_scope_filter_and_disabled_errors_are_typed_and_sanitized() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "needle\n");
        let service = workspace.service();
        for (arguments, code) in [
            (json!({}), "INVALID_ARGUMENTS"),
            (json!({"query": ""}), "INVALID_ARGUMENTS"),
            (
                json!({"query": "needle", "maxResults": 0}),
                "INVALID_ARGUMENTS",
            ),
            (
                json!({"query": "needle", "maxOutputBytes": 1023}),
                "INVALID_ARGUMENTS",
            ),
            (
                json!({"query": "needle", "unknown": true}),
                "INVALID_ARGUMENTS",
            ),
            (json!({"query": "(", "mode": "regex"}), "INVALID_REGEX"),
            (
                json!({"query": "needle", "include": ["["]}),
                "INVALID_FILTER_PATTERN",
            ),
            (
                json!({"query": "needle", "scope": "file.txt"}),
                "INVALID_SCOPE",
            ),
            (
                json!({"query": "needle", "scope": "../outside"}),
                "INVALID_SCOPE",
            ),
        ] {
            let result = call(&service, GREP, arguments);
            assert_eq!(error_code(&result), code);
            assert!(!serde_json::to_string(&result)
                .unwrap()
                .contains(&*workspace.root.to_string_lossy()));
        }

        let mut config = config_for(&workspace.root);
        config.tools.grep.enabled = false;
        let disabled = call(
            &workspace.service_with_config(config),
            GREP,
            json!({"query": "needle"}),
        );
        assert_eq!(error_code(&disabled), "TOOL_DISABLED");
    }

    #[test]
    fn grep_policy_deny_and_ask_deny_do_not_scan_and_allow_once_resumes_exact_request() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "needle\n");

        let mut deny_config = config_for(&workspace.root);
        deny_config.policies.tool_defaults.grep = harness_control_protocol::PolicyDecision::Deny;
        let mut deny_core = RuntimeCore::new();
        deny_core.initialize(deny_config).unwrap();
        deny_core.acknowledge().unwrap();
        let deny_service = deny_core.mcp_service().unwrap();
        let denied = call(&deny_service, GREP, json!({"query": "GREP_QUERY_SENTINEL"}));
        assert_eq!(error_code(&denied), "POLICY_DENIED");
        assert!(!deny_core
            .activity_snapshot(100)
            .iter()
            .any(|entry| entry.event_type == crate::audit::AuditEventType::ExecutionStarted));

        let mut ask_config = config_for(&workspace.root);
        ask_config.policies.tool_defaults.grep = harness_control_protocol::PolicyDecision::Ask;
        let mut ask_core = RuntimeCore::new();
        ask_core.initialize(ask_config).unwrap();
        ask_core.acknowledge().unwrap();
        let ask_service = ask_core.mcp_service().unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let allow = tokio::spawn({
                let service = ask_service.clone();
                async move {
                    service
                        .call_tool_async(
                            GREP,
                            json!({"query": "needle", "scope": ".", "include": []}),
                        )
                        .await
                        .unwrap()
                }
            });
            wait_for_pending(&ask_core, 1).await;
            let pending = ask_core
                .approval_manager()
                .list(Some(harness_control_protocol::ApprovalStatus::Pending));
            assert_eq!(pending[0].tool, GREP);
            assert_eq!(pending[0].sanitized_arguments["queryBytes"], "6");
            assert!(!pending[0].sanitized_arguments.contains_key("query"));
            assert!(!serde_json::to_string(&pending[0])
                .unwrap()
                .contains("needle"));
            assert!(!ask_core
                .activity_snapshot(100)
                .iter()
                .any(|entry| entry.event_type == crate::audit::AuditEventType::ExecutionStarted));
            ask_core
                .approval_manager()
                .resolve(
                    &pending[0].request_id,
                    harness_control_protocol::ApprovalDecision::AllowOnce,
                )
                .unwrap();
            let result = allow.await.unwrap();
            assert_eq!(result.is_error, Some(false));
            assert_eq!(
                result.structured_content.unwrap()["matches"][0]["path"],
                "file.txt"
            );

            let deny = tokio::spawn({
                let service = ask_service.clone();
                async move {
                    service
                        .call_tool_async(GREP, json!({"query": "needle"}))
                        .await
                        .unwrap()
                }
            });
            wait_for_pending(&ask_core, 1).await;
            let pending = ask_core
                .approval_manager()
                .list(Some(harness_control_protocol::ApprovalStatus::Pending));
            ask_core
                .approval_manager()
                .resolve(
                    &pending[0].request_id,
                    harness_control_protocol::ApprovalDecision::Deny,
                )
                .unwrap();
            assert_eq!(error_code(&deny.await.unwrap()), "APPROVAL_DENIED");
        });
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn grep_mcp_cancellation_stops_active_scan_before_partial_success() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "needle\nneedle\n");
        let service = workspace.service();
        let entered = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        let hook_entered = entered.clone();
        let hook_release = release.clone();
        let _hook = crate::tools::install_scan_hook(move |scanned| {
            if scanned.file_name().is_some_and(|name| name == "file.txt") {
                hook_entered.wait();
                hook_release.wait();
            }
        });
        let cancel = CancellationToken::new();
        let task = tokio::spawn({
            let service = service.clone();
            let cancel = cancel.clone();
            async move {
                service
                    .call_tool_async_with_cancel(GREP, json!({ "query": "needle" }), cancel)
                    .await
                    .expect("known MCP tool")
            }
        });
        tokio::task::spawn_blocking(move || entered.wait())
            .await
            .expect("scan start waiter");
        cancel.cancel();
        tokio::task::spawn_blocking(move || release.wait())
            .await
            .expect("scan release waiter");
        let result = task.await.expect("MCP task");
        assert_eq!(error_code(&result), "CANCELLED");
    }

    #[test]
    fn grep_activity_uses_typed_scalars_without_query_filter_or_preview_text() {
        let workspace = TestWorkspace::new();
        workspace.write("docs/file.txt", "GREP_MATCH_SENTINEL needle needle\n");
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&workspace.root)).unwrap();
        core.acknowledge().unwrap();
        let service = core.mcp_service().unwrap();
        let result = call(
            &service,
            GREP,
            json!({
                "query": "GREP_QUERY_SENTINEL",
                "include": ["docs/GREP_FILTER_SENTINEL*.txt"]
            }),
        );
        assert_eq!(result.is_error, Some(false));
        let activity = serde_json::to_string(&core.activity_snapshot(100)).unwrap();
        for sentinel in [
            "GREP_QUERY_SENTINEL",
            "GREP_FILTER_SENTINEL",
            "GREP_MATCH_SENTINEL",
        ] {
            assert!(!activity.contains(sentinel), "activity leaked {sentinel}");
        }
        assert!(activity.contains("\"tool\":\"grep\""));
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
    fn unknown_tool_dispatch_creates_safe_history_trace_before_method_not_found() {
        use chatgpt_harness_evidence_store::EvidenceReader;
        use harness_control_protocol::{EvidenceStoreLaunchConfig, RuntimeHostContext};

        let workspace = TestWorkspace::new();
        let database = workspace.base.join("history").join("history.sqlite3");
        let mut core = RuntimeCore::new();
        core.initialize_with_host(
            config_for(&workspace.root),
            Some(RuntimeHostContext {
                host_session_id: Some("m08-test-host".into()),
                supervisor_generation_id: Some("m08-test-generation".into()),
                evidence_store: Some(EvidenceStoreLaunchConfig {
                    enabled: true,
                    database_path: database.to_string_lossy().into_owned(),
                    retention_days: 30,
                    max_storage_mib: 32,
                }),
            }),
        )
        .expect("initialize runtime with evidence");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let requested_name = "PRIVATE_UNKNOWN_TOOL_NAME";
        assert!(matches!(
            service.call_tool(requested_name, json!({ "secret": "PRIVATE_PAYLOAD" })),
            Err(McpProtocolError::UnknownTool)
        ));
        let activity = core.activity_snapshot(10);
        assert_eq!(activity.len(), 2);
        assert_eq!(
            activity[0].event_type,
            crate::audit::AuditEventType::RequestReceived
        );
        assert_eq!(
            activity[1].event_type,
            crate::audit::AuditEventType::ExecutionFailed
        );
        // Activity sanitizes names through the configured registry and hides
        // an unresolved caller-supplied name; the durable Trace uses the
        // bounded canonical label `unknown`.
        assert_eq!(activity[0].tool_name, None);
        assert_eq!(activity[1].tool_name, None);
        let request_id = activity[0].request_id.to_string();
        core.stop_evidence();

        let reader = EvidenceReader::open(&database).expect("open evidence database");
        let trace = reader
            .trace_get(&harness_control_protocol::TraceGetParams {
                runtime_id: core.runtime_id.clone(),
                request_id,
            })
            .expect("query unknown tool trace");
        assert!(trace.trace_id.is_some());
        assert_eq!(trace.tool_name, "unknown");
        assert!(!trace.executor_started);
        assert_eq!(
            trace.availability,
            harness_control_protocol::TraceAvailability::Available
        );
        assert!(trace.spans.iter().any(|span| span.span_name
            == harness_control_protocol::TraceSpanName::ToolResolution
            && span.status == harness_control_protocol::TraceSpanStatus::Failed));
        assert_eq!(
            trace.failure_stage,
            Some(harness_control_protocol::TraceSpanName::ToolResolution)
        );
        let serialized = serde_json::to_string(&trace).expect("serialize trace");
        assert!(!serialized.contains(requested_name));
        assert!(!serialized.contains("PRIVATE_PAYLOAD"));
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
    fn bash_builder_exposes_input_output_schema_and_truthful_annotations() {
        let tool = bash_tool_with_metadata("bash", "controlled Bash");
        let input = serde_json::to_value(tool.input_schema.as_ref()).unwrap();
        assert_eq!(input["properties"]["command"]["type"], "string");
        assert_eq!(input["additionalProperties"], false);
        let output = serde_json::to_value(tool.output_schema.expect("output schema")).unwrap();
        assert_eq!(output["properties"]["exitCode"]["type"], "integer");
        assert_eq!(output["properties"]["stdout"]["type"], "string");
        assert_eq!(output["properties"]["stderr"]["type"], "string");
        assert_eq!(output["properties"]["durationMs"]["type"], "integer");
        let annotations = tool.annotations.expect("Bash annotations");
        assert_eq!(annotations.read_only_hint, Some(false));
        assert_eq!(annotations.destructive_hint, Some(true));
        assert_eq!(annotations.open_world_hint, Some(true));
    }

    #[test]
    fn frozen_wire_projector_moves_typed_payload_to_canonical_text() {
        let mut result = CallToolResult::structured(json!({
            "code": "OK",
            "value": 7,
        }));
        result
            .content
            .push(ContentBlock::text("deterministic summary"));

        let projected = project_call_tool_result(result);
        let wire = serde_json::to_value(&projected).expect("serialize projected result");
        assert!(wire.get("structuredContent").is_none());
        assert_eq!(wire["isError"], false);
        assert_eq!(wire["content"][0]["text"], "{\"code\":\"OK\",\"value\":7}");
        assert_eq!(wire["content"][1]["text"], "deterministic summary");
    }

    #[test]
    fn frozen_wire_projector_preserves_safe_tool_error_without_structured_content() {
        let projected = project_call_tool_result(crate::mcp::errors::read_file_error(
            &crate::tools::ReadFileError::InvalidArguments {
                message: "caller input is invalid".into(),
            },
        ));
        let wire = serde_json::to_value(&projected).expect("serialize projected error");
        assert!(wire.get("structuredContent").is_none());
        assert_eq!(wire["isError"], true);
        let text = wire["content"][0]["text"]
            .as_str()
            .expect("error JSON text");
        assert!(text.starts_with('{'));
        assert!(text.contains("INVALID_ARGUMENTS"));
    }

    #[test]
    fn server_advertises_only_the_frozen_mcp_revision_and_no_output_schema() {
        let workspace = TestWorkspace::new();
        let service = workspace.service();
        assert_eq!(
            service.supported_protocol_versions().as_ref(),
            &[ProtocolVersion::V_2025_03_26]
        );
        assert!(service
            .wire_tools()
            .into_iter()
            .all(|tool| tool.output_schema.is_none()));
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
    fn write_overwrite_routes_through_mcp_and_reports_structured_flags() {
        let workspace = TestWorkspace::new();
        workspace.write("existing.txt", "old content");
        let result = call(
            &workspace.service(),
            WRITE,
            json!({ "path": "existing.txt", "content": "new content" }),
        );

        assert_eq!(result.is_error, Some(false));
        assert_eq!(
            result.structured_content.as_ref().unwrap(),
            &json!({
                "normalizedPath": "existing.txt",
                "bytesWritten": "new content".len(),
                "created": false,
                "overwritten": true,
            })
        );
        assert_eq!(
            fs::read(workspace.root.join("existing.txt")).unwrap(),
            b"new content"
        );
    }

    #[test]
    fn m1_integration_write_preserves_exact_utf8_bytes_and_rejects_binary() {
        let workspace = TestWorkspace::new();
        let service = workspace.service();
        let cases = [
            ("multibyte.txt", "你好, ChatGPT 👋"),
            ("empty.txt", ""),
            ("no-trailing-newline.txt", "no trailing newline"),
            ("lf.txt", "line one\nline two\n"),
            ("crlf.txt", "line one\r\nline two\r\n"),
            ("bom.txt", "\u{FEFF}BOM supplied explicitly"),
        ];

        for (path, content) in cases {
            let result = call(&service, WRITE, json!({ "path": path, "content": content }));
            assert_eq!(result.is_error, Some(false), "write failed for {path}");
            assert_eq!(
                result.structured_content.as_ref().unwrap()["bytesWritten"],
                content.as_bytes().len()
            );
            assert_eq!(
                fs::read(workspace.root.join(path)).unwrap(),
                content.as_bytes()
            );
            workspace.assert_no_atomic_temp_residue();
        }

        let binary_path = workspace.root.join("binary.bin");
        let binary_before = [0xff, 0xfe, 0x00, 0x80];
        fs::write(&binary_path, binary_before).expect("binary fixture");
        let rejected = call(
            &service,
            WRITE,
            json!({ "path": "binary.bin", "content": "replacement" }),
        );
        assert_eq!(error_code(&rejected), "UNSUPPORTED_CONTENT");
        assert_eq!(fs::read(&binary_path).unwrap(), binary_before);
        workspace.assert_no_atomic_temp_residue();
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
    fn mcp_write_boundary_rejections_preserve_filesystem_and_audit_boundary() {
        use crate::audit::AuditEventType;

        let workspace = TestWorkspace::new();
        let outside = workspace.outside.join("outside.txt");
        fs::write(&outside, b"outside before").expect("outside fixture");
        let outside_before = fs::read(&outside).expect("outside bytes");
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");

        for path in [
            Value::String("../outside/escape.txt".into()),
            Value::String(outside.to_string_lossy().into_owned()),
        ] {
            let before = core.activity_snapshot(100).len();
            let result = call(
                &service,
                WRITE,
                json!({ "path": path, "content": "blocked" }),
            );
            assert_eq!(error_code(&result), "WORKSPACE_VIOLATION");
            assert!(!serde_json::to_string(&result)
                .unwrap()
                .contains(&*outside.to_string_lossy()));
            let entries = core.activity_snapshot(100);
            let request_entries = &entries[before..];
            assert_eq!(
                request_entries
                    .iter()
                    .map(|entry| entry.event_type)
                    .collect::<Vec<_>>(),
                [
                    AuditEventType::RequestReceived,
                    AuditEventType::WorkspaceValidated,
                ]
            );
            assert!(request_entries.iter().all(|entry| {
                !matches!(
                    entry.event_type,
                    AuditEventType::ExecutionStarted | AuditEventType::ExecutionCompleted
                )
            }));
        }

        let before = core.activity_snapshot(100).len();
        let result = call(
            &service,
            WRITE,
            json!({
                "path": "missing-parent/file.txt",
                "content": "blocked"
            }),
        );
        assert_eq!(error_code(&result), "TARGET_NOT_FOUND");
        assert!(!workspace.root.join("missing-parent").exists());
        let entries = core.activity_snapshot(100);
        assert_eq!(
            entries[before..]
                .iter()
                .map(|entry| entry.event_type)
                .collect::<Vec<_>>(),
            [
                AuditEventType::RequestReceived,
                AuditEventType::WorkspaceValidated,
            ]
        );
        assert_eq!(fs::read(&outside).unwrap(), outside_before);
    }

    #[test]
    fn mcp_malformed_mutations_and_unsupported_edit_content_do_not_mutate() {
        let workspace = TestWorkspace::new();
        workspace.write("file.txt", "old content");
        workspace.write("invalid.bin", [0xff, 0xfe]);
        let before = fs::read(workspace.root.join("file.txt")).expect("file bytes");
        let service = workspace.service();

        let malformed = [
            (WRITE, json!({ "content": "x" })),
            (WRITE, json!({ "path": "file.txt" })),
            (WRITE, json!({ "path": 3, "content": "x" })),
            (WRITE, json!({ "path": "file.txt", "content": 3 })),
            (
                WRITE,
                json!({ "path": "file.txt", "content": "x", "unknown": true }),
            ),
            (EDIT, json!({ "path": "file.txt", "oldText": "old" })),
            (EDIT, json!({ "path": "file.txt", "newText": "new" })),
            (
                EDIT,
                json!({ "path": 3, "oldText": "old", "newText": "new" }),
            ),
            (
                EDIT,
                json!({ "path": "file.txt", "oldText": 3, "newText": "new" }),
            ),
            (
                EDIT,
                json!({ "path": "file.txt", "oldText": "old", "newText": 3 }),
            ),
            (
                EDIT,
                json!({ "path": "file.txt", "oldText": "old", "newText": "new", "unknown": true }),
            ),
            (
                EDIT,
                json!({ "path": "file.txt", "oldText": "", "newText": "new" }),
            ),
        ];
        for (tool, arguments) in malformed {
            assert_eq!(
                error_code(&call(&service, tool, arguments)),
                "INVALID_ARGUMENTS"
            );
        }
        assert_eq!(fs::read(workspace.root.join("file.txt")).unwrap(), before);

        let invalid_utf8_before = fs::read(workspace.root.join("invalid.bin")).unwrap();
        let result = call(
            &service,
            EDIT,
            json!({
                "path": "invalid.bin",
                "oldText": "old",
                "newText": "new"
            }),
        );
        assert_eq!(error_code(&result), "UNSUPPORTED_CONTENT");
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("invalid.bin"));
        assert_eq!(
            fs::read(workspace.root.join("invalid.bin")).unwrap(),
            invalid_utf8_before
        );
    }

    #[test]
    fn mutation_audit_projection_excludes_raw_write_and_edit_payloads() {
        let workspace = TestWorkspace::new();
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&workspace.root))
            .expect("initialize runtime");
        core.acknowledge().expect("acknowledge runtime");
        let service = core.mcp_service().expect("MCP service");
        let marker = "M0_SECRET_SENTINEL_DO_NOT_AUDIT";
        let replacement = "M0_SECRET_REPLACEMENT_DO_NOT_AUDIT";

        let write = call(
            &service,
            WRITE,
            json!({ "path": "audit.txt", "content": marker }),
        );
        assert_eq!(write.is_error, Some(false));
        let edit = call(
            &service,
            EDIT,
            json!({
                "path": "audit.txt",
                "oldText": marker,
                "newText": replacement
            }),
        );
        assert_eq!(edit.is_error, Some(false));
        assert_eq!(
            fs::read_to_string(workspace.root.join("audit.txt")).unwrap(),
            replacement
        );

        let activity = serde_json::to_string(&core.activity_snapshot(100)).unwrap();
        assert!(!activity.contains(marker));
        assert!(!activity.contains(replacement));
        assert!(!activity.contains(&*workspace.root.to_string_lossy()));
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
