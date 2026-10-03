use crate::execution::{BashToolRequest, ExecutionError};
use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::execution_error;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BashArguments {
    /// Original shell command. Runtime owns cwd, environment and hard limits.
    pub command: String,
}

#[derive(JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BashOutputSchema {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

pub(crate) async fn execute(
    context: &McpExecutionContext,
    arguments: Value,
    request_cancel: CancellationToken,
) -> CallToolResult {
    let report = context
        .execute_async_cancellable(
            || {
                let arguments = serde_json::from_value::<BashArguments>(arguments)
                    .map_err(|_| ExecutionError::InvalidArguments)?;
                Ok(BashToolRequest::new(arguments.command))
            },
            request_cancel,
        )
        .await;
    match report.result {
        Ok(output) => {
            let mut result = CallToolResult::structured(json!({
                "exitCode": output.exit_code,
                "stdout": output.stdout,
                "stderr": output.stderr,
                "durationMs": output.duration_ms,
            }));
            result
                .content
                .push(ContentBlock::text("Executed controlled Bash command."));
            result
        }
        Err(error) => execution_error(&error),
    }
}
