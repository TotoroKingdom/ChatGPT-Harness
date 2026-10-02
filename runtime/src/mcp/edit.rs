use crate::execution::ExecutionError;
use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::execution_error;
use crate::tools::EditRequest;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditArguments {
    /// Workspace-relative path to an existing UTF-8 text file.
    pub path: String,
    /// Exact non-empty text to replace once.
    #[serde(rename = "oldText")]
    pub old_text: String,
    /// Replacement text; it may be empty.
    #[serde(rename = "newText")]
    pub new_text: String,
}

pub(crate) fn execute(context: &McpExecutionContext, arguments: Value) -> CallToolResult {
    let report = context.execute(|| {
        let arguments = serde_json::from_value::<EditArguments>(arguments)
            .map_err(|_| ExecutionError::InvalidArguments)?;
        Ok(EditRequest {
            path: PathBuf::from(arguments.path),
            old_text: arguments.old_text,
            new_text: arguments.new_text,
        })
    });
    match report.result {
        Ok(output) => {
            let mut result = CallToolResult::structured(json!({
                "normalizedPath": output.normalized_path.to_string_lossy().replace('\\', "/"),
                "replacements": output.replacements,
                "bytesWritten": output.bytes_written,
            }));
            result
                .content
                .push(ContentBlock::text("Edited workspace file."));
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
            let arguments = serde_json::from_value::<EditArguments>(arguments)
                .map_err(|_| ExecutionError::InvalidArguments)?;
            Ok(EditRequest {
                path: PathBuf::from(arguments.path),
                old_text: arguments.old_text,
                new_text: arguments.new_text,
            })
        })
        .await;
    match report.result {
        Ok(output) => {
            let mut result = CallToolResult::structured(json!({
                "normalizedPath": output.normalized_path.to_string_lossy().replace('\\', "/"),
                "replacements": output.replacements,
                "bytesWritten": output.bytes_written,
            }));
            result
                .content
                .push(ContentBlock::text("Edited workspace file."));
            result
        }
        Err(error) => execution_error(&error),
    }
}
