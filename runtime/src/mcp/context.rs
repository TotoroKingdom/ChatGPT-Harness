use crate::execution::{ExecutionError, ExecutionPipeline, ExecutionReport, ToolExecutor};
use crate::tools::EffectiveTool;
use std::sync::Arc;

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
    pub(crate) fn effective_tools(&self) -> Vec<EffectiveTool> {
        self.pipeline.effective_tools()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpContextError {
    RuntimeNotInitialized,
}
