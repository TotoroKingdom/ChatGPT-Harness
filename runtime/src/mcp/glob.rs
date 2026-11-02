use crate::execution::ExecutionError;
use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::execution_error;
use crate::tools::{GlobMatchKind, GlobRequest};
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
    let report = context.execute(|| {
        let arguments = serde_json::from_value::<GlobArguments>(arguments)
            .map_err(|_| ExecutionError::InvalidArguments)?;
        let mut request = GlobRequest::new(arguments.pattern);
        if let Some(base_path) = arguments.base_path {
            request = request.with_base_path(PathBuf::from(base_path));
        }
        Ok(request)
    });
    let request_id = report.context.request_id;
    match report.result {
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
            context.attach_correlation(&mut result, request_id);
            result
        }
        Err(error) => {
            let mut result = execution_error(&error);
            context.attach_correlation(&mut result, request_id);
            result
        }
    }
}

pub(crate) async fn execute_async(
    context: &McpExecutionContext,
    arguments: Value,
) -> CallToolResult {
    let report = context
        .execute_async(|| {
            let arguments = serde_json::from_value::<GlobArguments>(arguments)
                .map_err(|_| ExecutionError::InvalidArguments)?;
            let mut request = GlobRequest::new(arguments.pattern);
            if let Some(base_path) = arguments.base_path {
                request = request.with_base_path(PathBuf::from(base_path));
            }
            Ok(request)
        })
        .await;
    let request_id = report.context.request_id;
    match report.result {
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
            context.attach_correlation(&mut result, request_id);
            result
        }
        Err(error) => {
            let mut result = execution_error(&error);
            context.attach_correlation(&mut result, request_id);
            result
        }
    }
}
