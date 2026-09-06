use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::{invalid_arguments, read_file_error};
use crate::tools::{execute_read_file, ReadFileRequest};
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
    let arguments = match serde_json::from_value::<ReadFileArguments>(arguments) {
        Ok(arguments) => arguments,
        Err(_) => return invalid_arguments(),
    };
    let request = ReadFileRequest {
        path: PathBuf::from(arguments.path),
        offset: arguments.offset.unwrap_or(0),
        limit: arguments.limit,
    };

    match execute_read_file(
        context.workspace_guard(),
        context.max_read_bytes(),
        &request,
    ) {
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
        Err(error) => read_file_error(&error),
    }
}
