use crate::execution::{ExecutionError, ExecutionPipeline, ExecutionReport, ToolExecutor};
use crate::tools::EffectiveTool;
use rmcp::model::{CallToolResult, MetaObject};
use serde_json::json;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Shares the exact initialized enforcement snapshot, not raw executor access.
#[derive(Clone)]
pub struct McpExecutionContext {
    pipeline: Arc<ExecutionPipeline>,
    runtime_id: Option<String>,
    expose_policy_denied_tools: bool,
}

impl McpExecutionContext {
    pub(crate) fn new(pipeline: Arc<ExecutionPipeline>) -> Self {
        Self {
            pipeline,
            runtime_id: None,
            expose_policy_denied_tools: false,
        }
    }

    pub(crate) fn with_runtime_id(runtime_id: String, pipeline: Arc<ExecutionPipeline>) -> Self {
        Self::with_runtime_id_for_evaluation(runtime_id, pipeline, false)
    }

    pub(crate) fn with_runtime_id_for_evaluation(
        runtime_id: String,
        pipeline: Arc<ExecutionPipeline>,
        expose_policy_denied_tools: bool,
    ) -> Self {
        Self {
            pipeline,
            runtime_id: Some(runtime_id),
            expose_policy_denied_tools,
        }
    }

    pub(crate) fn exposes_policy_denied_tools(&self) -> bool {
        self.expose_policy_denied_tools
    }

    /// Additive, observational MCP result metadata. The business
    /// `structuredContent` is never modified.
    pub(crate) fn attach_correlation(&self, result: &mut CallToolResult, request_id: uuid::Uuid) {
        let mut value = json!({
            "chatgptHarness": {
                "requestId": request_id.to_string(),
            }
        });
        if let Some(runtime_id) = &self.runtime_id {
            value["chatgptHarness"]["runtimeId"] = json!(runtime_id);
        }
        let Some(correlation) = value.as_object() else {
            return;
        };
        let mut meta = result
            .meta
            .take()
            .map(|existing| existing.0)
            .unwrap_or_default();
        if let Some(correlation) = correlation.get("chatgptHarness") {
            meta.insert("chatgptHarness".into(), correlation.clone());
        }
        result.meta = Some(MetaObject(meta));
    }

    pub(crate) fn execute<E: ToolExecutor>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
    ) -> ExecutionReport<E::Output> {
        self.pipeline.execute(prepare)
    }

    pub(crate) async fn execute_async<E: ToolExecutor + Send + Sync>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
    ) -> ExecutionReport<E::Output>
    where
        E::Output: Send,
    {
        self.pipeline.execute_async(prepare).await
    }

    pub(crate) async fn execute_async_cancellable<E: ToolExecutor + Send + Sync>(
        &self,
        prepare: impl FnOnce() -> Result<E, ExecutionError>,
        request_cancel: CancellationToken,
    ) -> ExecutionReport<E::Output>
    where
        E::Output: Send,
    {
        self.pipeline
            .execute_async_cancellable(prepare, request_cancel)
            .await
    }

    pub(crate) fn observe_unknown_tool(&self) -> uuid::Uuid {
        self.pipeline.observe_unknown_tool()
    }

    pub(crate) fn effective_tools(&self) -> Vec<EffectiveTool> {
        self.pipeline.effective_tools()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpContextError {
    RuntimeNotInitialized,
}
