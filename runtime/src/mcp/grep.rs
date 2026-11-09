use crate::execution::ExecutionError;
use crate::mcp::context::McpExecutionContext;
use crate::mcp::errors::execution_error;
use crate::tools::{
    GrepError, GrepOutput, GrepRequest, DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_RESULTS,
};
use harness_control_protocol::GrepMode;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GrepModeArgument {
    #[default]
    Literal,
    Regex,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GrepArguments {
    /// Non-empty text or Rust-regex pattern searched line by line.
    #[schemars(length(min = 1, max = 4096))]
    pub query: String,
    /// Matching mode. Defaults to literal.
    #[serde(default)]
    pub mode: GrepModeArgument,
    /// Workspace-relative directory scope. Defaults to the Workspace root.
    #[serde(default = "default_scope")]
    #[schemars(length(min = 1, max = 1024))]
    pub scope: String,
    /// Workspace-relative include globs evaluated against normalized paths.
    #[serde(default)]
    #[schemars(length(max = 32), inner(length(min = 1, max = 512)))]
    pub include: Vec<String>,
    /// Workspace-relative exclude globs; exclude wins over include.
    #[serde(default)]
    #[schemars(length(max = 32), inner(length(min = 1, max = 512)))]
    pub exclude: Vec<String>,
    /// Maximum matching logical lines. Defaults to 100 and is capped at 1000.
    #[serde(rename = "maxResults", default = "default_max_results")]
    #[schemars(range(min = 1, max = 1000))]
    pub max_results: u32,
    /// Maximum path+preview payload bytes. Defaults to 65536 and is capped at 262144.
    #[serde(rename = "maxOutputBytes", default = "default_max_output_bytes")]
    #[schemars(range(min = 1024, max = 262144))]
    pub max_output_bytes: u32,
}

impl GrepArguments {
    fn request(self) -> GrepRequest {
        GrepRequest {
            query: self.query,
            mode: match self.mode {
                GrepModeArgument::Literal => GrepMode::Literal,
                GrepModeArgument::Regex => GrepMode::Regex,
            },
            scope: PathBuf::from(self.scope),
            include: self.include,
            exclude: self.exclude,
            max_results: self.max_results,
            max_output_bytes: self.max_output_bytes,
        }
    }
}

fn default_scope() -> String {
    ".".into()
}

fn default_max_results() -> u32 {
    DEFAULT_MAX_RESULTS
}

fn default_max_output_bytes() -> u32 {
    DEFAULT_MAX_OUTPUT_BYTES
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
            let mut result = CallToolResult::structured(output_json(&output));
            result
                .content
                .push(ContentBlock::text("Searched Workspace text."));
            context.attach_correlation(&mut result, request_id);
            result
        }
        Err(crate::execution::ExecutionError::Grep(GrepError::SearchTimeout {
            partial: Some(output),
        })) => {
            let mut value = output_json(&output);
            value["code"] = json!("SEARCH_TIMEOUT");
            value["message"] = json!("Search exceeded the Runtime wall-clock limit.");
            value["truncated"] = json!(true);
            let mut result = CallToolResult::structured_error(value);
            result.content.push(ContentBlock::text(
                "Search timed out; returned matches are incomplete.",
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

fn output_json(output: &GrepOutput) -> Value {
    let matches = output
        .matches
        .iter()
        .map(|item| {
            json!({
                "path": &item.path,
                "lineNumber": item.line_number,
                "preview": &item.preview,
                "previewStartByte": item.preview_start_byte,
                "matchStartByte": item.match_start_byte,
                "matchEndByte": item.match_end_byte,
                "previewTruncated": item.preview_truncated,
            })
        })
        .collect::<Vec<_>>();
    let truncation_reasons = output
        .truncation_reasons
        .iter()
        .map(|reason| reason.as_str())
        .collect::<Vec<_>>();
    let diagnostics = output
        .diagnostics
        .iter()
        .map(|item| {
            json!({
                "path": &item.path,
                "code": item.code.as_str(),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "matches": matches,
        "scannedFiles": output.scanned_files,
        "scannedBytes": output.scanned_bytes,
        "skippedFiles": output.skipped_files,
        "truncated": output.truncated,
        "truncationReasons": truncation_reasons,
        "diagnostics": diagnostics,
        "diagnosticsTruncated": output.diagnostics_truncated,
    })
}
