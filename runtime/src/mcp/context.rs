use crate::execution::{ExecutionError, ExecutionPipeline, ExecutionReport, ToolExecutor};
use crate::tools::EffectiveTool;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Shares the exact initialized enforcement snapshot, not raw executor access.
#[derive(Clone)]
pub struct McpExecutionContext {
    pipeline: Arc<ExecutionPipeline>,
}

impl McpExecutionContext {
    pub(crate) fn new(pipeline: Arc<ExecutionPipeline>) -> Self {
        Self { pipeline }
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
    pub(crate) fn effective_tools(&self) -> Vec<EffectiveTool> {
        self.pipeline.effective_tools()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpContextError {
    RuntimeNotInitialized,
}
