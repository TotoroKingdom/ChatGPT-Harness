use crate::execution::ExecutionError;
use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::execution_error;
use crate::tools::WriteRequest;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WriteArguments {
    /// Workspace-relative path to create or overwrite.
    pub path: String,
    /// Full UTF-8 text content to write.
    pub content: String,
}

pub(crate) fn execute(context: &McpExecutionContext, arguments: Value) -> CallToolResult {
    let report = context.execute(|| {
        let arguments = serde_json::from_value::<WriteArguments>(arguments)
            .map_err(|_| ExecutionError::InvalidArguments)?;
        Ok(WriteRequest {
            path: PathBuf::from(arguments.path),
            content: arguments.content,
        })
    });
    match report.result {
        Ok(output) => {
            let mut result = CallToolResult::structured(json!({
                "normalizedPath": output.normalized_path.to_string_lossy().replace('\\', "/"),
                "bytesWritten": output.bytes_written,
                "created": output.created,
                "overwritten": output.overwritten,
            }));
            result
                .content
                .push(ContentBlock::text("Wrote workspace file."));
            result
        }
        Err(error) => execution_error(&error),
    }
}
