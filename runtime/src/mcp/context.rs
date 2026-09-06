use crate::workspace::WorkspaceGuard;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// The minimum initialized Runtime state needed by the MCP tool adapters.
///
/// It shares the guard created by Runtime initialization and copies only the
/// immutable configured read limit from that same initialized snapshot.
#[derive(Clone)]
pub struct McpExecutionContext {
    workspace_guard: Arc<WorkspaceGuard>,
    max_read_bytes: u32,
    readiness: Arc<AtomicBool>,
}

impl McpExecutionContext {
    pub(crate) fn new(
        workspace_guard: Arc<WorkspaceGuard>,
        max_read_bytes: u32,
        readiness: Arc<AtomicBool>,
    ) -> Self {
        Self {
            workspace_guard,
            max_read_bytes,
            readiness,
        }
    }

    pub(crate) fn workspace_guard(&self) -> &WorkspaceGuard {
        &self.workspace_guard
    }

    pub(crate) fn max_read_bytes(&self) -> u32 {
        self.max_read_bytes
    }

    /// A read-only cross-thread projection of the Runtime lifecycle. The
    /// RuntimeCore phase remains authoritative and is its only writer.
    pub(crate) fn is_runtime_ready(&self) -> bool {
        self.readiness.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpContextError {
    RuntimeNotInitialized,
}
