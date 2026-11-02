//! MCP service adapter for the Runtime's existing workspace capabilities.
//!
//! This module defines metadata, argument/result mapping, and error mapping.
//! It deliberately owns no transport and performs no filesystem access.

mod bash;
mod context;
mod edit;
mod errors;
mod glob;
mod grep;
mod http;
mod read_file;
mod runtime;
mod service;
mod write;

pub use context::{McpContextError, McpExecutionContext};
#[allow(unused_imports)] // Public API for M5-D3 lifecycle integration.
pub use http::{start_mcp_http_endpoint, McpHttpEndpointHandle, McpHttpError};
pub use runtime::{McpRuntimeService, McpRuntimeServiceError};
pub use service::McpService;
