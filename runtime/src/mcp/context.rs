use crate::workspace::WorkspaceGuard;
use std::sync::Arc;

/// The minimum initialized Runtime state needed by the MCP tool adapters.
///
/// It shares the guard created by Runtime initialization and copies only the
/// immutable configured read limit from that same initialized snapshot.
#[derive(Clone)]
pub struct McpExecutionContext {
    workspace_guard: Arc<WorkspaceGuard>,
    max_read_bytes: u32,
}

impl McpExecutionContext {
    pub(crate) fn new(workspace_guard: Arc<WorkspaceGuard>, max_read_bytes: u32) -> Self {
        Self {
            workspace_guard,
            max_read_bytes,
        }
    }

    pub(crate) fn workspace_guard(&self) -> &WorkspaceGuard {
        &self.workspace_guard
    }

    pub(crate) fn max_read_bytes(&self) -> u32 {
        self.max_read_bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpContextError {
    RuntimeNotInitialized,
}
