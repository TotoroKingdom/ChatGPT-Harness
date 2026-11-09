use crate::execution::{ExecutionError, ExecutionReport};
use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::execution_error;
use crate::tools::{GlobError, GlobMatchKind, GlobOutput, GlobRequest};
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

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
    finish(context, report)
}

pub(crate) async fn execute_async(
    context: &McpExecutionContext,
    arguments: Value,
    cancel: CancellationToken,
) -> CallToolResult {
    let report = context
        .execute_async_cancellable(
            || {
                let arguments = serde_json::from_value::<GlobArguments>(arguments)
                    .map_err(|_| ExecutionError::InvalidArguments)?;
                let mut request = GlobRequest::new(arguments.pattern);
                if let Some(base_path) = arguments.base_path {
                    request = request.with_base_path(PathBuf::from(base_path));
                }
                Ok(request)
            },
            cancel,
        )
        .await;
    finish(context, report)
}

fn finish(
    context: &McpExecutionContext,
    report: ExecutionReport<crate::tools::GlobOutput>,
) -> CallToolResult {
    let request_id = report.context.request_id;
    match report.result {
        Ok(output) => {
            let mut result = success_result(output);
            context.attach_correlation(&mut result, request_id);
            result
        }
        Err(ExecutionError::Glob(GlobError::SearchTimeout {
            partial: Some(output),
        })) => {
            let mut value = output_json(&output);
            value["code"] = json!("SEARCH_TIMEOUT");
            value["message"] = json!("Glob search exceeded the Runtime wall-clock limit.");
            value["truncated"] = json!(true);
            let mut result = CallToolResult::structured_error(value);
            result.content.push(ContentBlock::text(
                "Glob search timed out; returned paths are incomplete.",
            ));
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

fn success_result(output: GlobOutput) -> CallToolResult {
    let mut result = CallToolResult::structured(output_json(&output));
    result
        .content
        .push(ContentBlock::text("Listed workspace paths."));
    result
}

fn output_json(output: &GlobOutput) -> Value {
    let matches = output
        .matches
        .iter()
        .map(|item| {
            json!({
                "path": &item.path,
                "kind": match item.kind {
                    GlobMatchKind::File => "file",
                    GlobMatchKind::Directory => "directory",
                },
            })
        })
        .collect::<Vec<_>>();
    let diagnostics = output
        .diagnostics
        .iter()
        .map(|item| {
            json!({
                "path": &item.path,
                "code": item.code.as_str(),
                "message": &item.message,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "matches": matches,
        "truncated": output.truncated,
        "truncationReasons": output
            .truncation_reasons
            .iter()
            .map(|reason| reason.as_str())
            .collect::<Vec<_>>(),
        "diagnostics": diagnostics,
        "diagnosticsTruncated": output.diagnostics_truncated,
    })
}
