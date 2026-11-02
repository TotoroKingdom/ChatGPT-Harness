use crate::execution::ExecutionError;
use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::execution_error;
use crate::tools::{GrepRequest, DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_RESULTS};
use harness_control_protocol::GrepMode;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GrepModeArgument {
    Literal,
    Regex,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GrepArguments {
    /// Non-empty text or Rust-regex pattern searched line by line.
    pub query: String,
    /// Matching mode. Defaults to literal.
    pub mode: Option<GrepModeArgument>,
    /// Workspace-relative directory scope. Defaults to the Workspace root.
    pub scope: Option<String>,
    /// Workspace-relative include globs evaluated against normalized paths.
    pub include: Option<Vec<String>>,
    /// Workspace-relative exclude globs; exclude wins over include.
    pub exclude: Option<Vec<String>>,
    /// Maximum matching logical lines. Defaults to 100 and is capped at 1000.
    #[serde(rename = "maxResults")]
    pub max_results: Option<u32>,
    /// Maximum path+preview payload bytes. Defaults to 65536 and is capped at 262144.
    #[serde(rename = "maxOutputBytes")]
    pub max_output_bytes: Option<u32>,
}

impl GrepArguments {
    fn request(self) -> GrepRequest {
        GrepRequest {
            query: self.query,
            mode: match self.mode.unwrap_or(GrepModeArgument::Literal) {
                GrepModeArgument::Literal => GrepMode::Literal,
                GrepModeArgument::Regex => GrepMode::Regex,
            },
            scope: PathBuf::from(self.scope.unwrap_or_else(|| ".".into())),
            include: self.include.unwrap_or_default(),
            exclude: self.exclude.unwrap_or_default(),
            max_results: self.max_results.unwrap_or(DEFAULT_MAX_RESULTS),
            max_output_bytes: self.max_output_bytes.unwrap_or(DEFAULT_MAX_OUTPUT_BYTES),
        }
    }
}

pub(crate) fn execute(context: &McpExecutionContext, arguments: Value) -> CallToolResult {
    let report = context.execute(|| {
        let arguments = serde_json::from_value::<GrepArguments>(arguments)
            .map_err(|_| ExecutionError::InvalidArguments)?;
        Ok(arguments.request())
    });
    finish(context, report)
}

pub(crate) async fn execute_async(
    context: &McpExecutionContext,
    arguments: Value,
    cancel: tokio_util::sync::CancellationToken,
) -> CallToolResult {
    let report = context
        .execute_async_cancellable(
            || {
                let arguments = serde_json::from_value::<GrepArguments>(arguments)
                    .map_err(|_| ExecutionError::InvalidArguments)?;
                Ok(arguments.request())
            },
            cancel,
        )
        .await;
    finish(context, report)
}

fn finish(
    context: &McpExecutionContext,
    report: crate::execution::ExecutionReport<crate::tools::GrepOutput>,
) -> CallToolResult {
    let request_id = report.context.request_id;
    match report.result {
        Ok(output) => {
            let matches = output
                .matches
                .into_iter()
                .map(|item| {
                    json!({
                        "path": item.path,
                        "lineNumber": item.line_number,
                        "preview": item.preview,
                        "previewStartByte": item.preview_start_byte,
                        "matchStartByte": item.match_start_byte,
                        "matchEndByte": item.match_end_byte,
                        "previewTruncated": item.preview_truncated,
                    })
                })
                .collect::<Vec<_>>();
            let truncation_reasons = output
                .truncation_reasons
                .into_iter()
                .map(|reason| reason.as_str())
                .collect::<Vec<_>>();
            let diagnostics = output
                .diagnostics
                .into_iter()
                .map(|item| {
                    json!({
                        "path": item.path,
                        "code": item.code.as_str(),
                    })
                })
                .collect::<Vec<_>>();
            let mut result = CallToolResult::structured(json!({
                "matches": matches,
                "scannedFiles": output.scanned_files,
                "scannedBytes": output.scanned_bytes,
                "skippedFiles": output.skipped_files,
                "truncated": output.truncated,
                "truncationReasons": truncation_reasons,
                "diagnostics": diagnostics,
                "diagnosticsTruncated": output.diagnostics_truncated,
            }));
            result
                .content
                .push(ContentBlock::text("Searched Workspace text."));
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
