use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::{glob_error, invalid_arguments, runtime_not_ready};
use crate::tools::{execute_glob, GlobMatchKind, GlobRequest};
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GlobArguments {
    /// Glob pattern evaluated within the active workspace.
    pub pattern: String,
    /// Optional workspace-relative directory from which to evaluate the pattern.
    #[serde(rename = "basePath")]
    pub base_path: Option<String>,
}

pub(crate) fn execute(context: &McpExecutionContext, arguments: Value) -> CallToolResult {
    if !context.is_runtime_ready() {
        return runtime_not_ready();
    }
    let arguments = match serde_json::from_value::<GlobArguments>(arguments) {
        Ok(arguments) => arguments,
        Err(_) => return invalid_arguments(),
    };
    let mut request = GlobRequest::new(arguments.pattern);
    if let Some(base_path) = arguments.base_path {
        request = request.with_base_path(PathBuf::from(base_path));
    }

    match execute_glob(context.workspace_guard(), &request) {
        Ok(output) => {
            let matches = output
                .matches
                .into_iter()
                .map(|item| {
                    json!({
                        "path": item.path,
                        "kind": match item.kind {
                            GlobMatchKind::File => "file",
                            GlobMatchKind::Directory => "directory",
                        },
                    })
                })
                .collect::<Vec<_>>();
            let diagnostics = output
                .diagnostics
                .into_iter()
                .map(|item| {
                    json!({
                        "path": item.path,
                        "code": format!("{:?}", item.code),
                        "message": item.message,
                    })
                })
                .collect::<Vec<_>>();
            let mut result = CallToolResult::structured(json!({
                "matches": matches,
                "truncated": output.truncated,
                "diagnostics": diagnostics,
                "diagnosticsTruncated": output.diagnostics_truncated,
            }));
            result
                .content
                .push(ContentBlock::text("Listed workspace paths."));
            result
        }
        Err(error) => glob_error(&error),
    }
}
