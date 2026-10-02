use crate::execution::ExecutionError;
use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::execution_error;
use crate::tools::ReadFileRequest;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadFileArguments {
    /// Workspace-relative path to a UTF-8 text file.
    pub path: String,
    /// Zero-based line offset.
    pub offset: Option<u64>,
    /// Maximum number of lines to return.
    pub limit: Option<u64>,
}

pub(crate) fn execute(context: &McpExecutionContext, arguments: Value) -> CallToolResult {
    let report = context.execute(|| {
        let arguments = serde_json::from_value::<ReadFileArguments>(arguments)
            .map_err(|_| ExecutionError::InvalidArguments)?;
        Ok(ReadFileRequest {
            path: PathBuf::from(arguments.path),
            offset: arguments.offset.unwrap_or(0),
            limit: arguments.limit,
        })
    });
    match report.result {
        Ok(output) => {
            let structured = json!({
                "normalizedPath": output.normalized_path.to_string_lossy(),
                "content": output.content,
                "offset": output.offset,
                "linesRead": output.lines_read,
                "bytesRead": output.bytes_read,
                "truncated": output.truncated,
                "nextOffset": output.next_offset,
            });
            let mut result = CallToolResult::structured(structured);
            result
                .content
                .push(ContentBlock::text("Read workspace file."));
            result
        }
        Err(error) => execution_error(&error),
    }
}

pub(crate) async fn execute_async(
    context: &McpExecutionContext,
    arguments: Value,
) -> CallToolResult {
    let report = context
        .execute_async(|| {
            let arguments = serde_json::from_value::<ReadFileArguments>(arguments)
                .map_err(|_| ExecutionError::InvalidArguments)?;
            Ok(ReadFileRequest {
                path: PathBuf::from(arguments.path),
                offset: arguments.offset.unwrap_or(0),
                limit: arguments.limit,
            })
        })
        .await;
    match report.result {
        Ok(output) => {
            let structured = json!({
                "normalizedPath": output.normalized_path.to_string_lossy(),
                "content": output.content,
                "offset": output.offset,
                "linesRead": output.lines_read,
                "bytesRead": output.bytes_read,
                "truncated": output.truncated,
                "nextOffset": output.next_offset,
            });
            let mut result = CallToolResult::structured(structured);
            result
                .content
                .push(ContentBlock::text("Read workspace file."));
            result
        }
        Err(error) => execution_error(&error),
    }
}
