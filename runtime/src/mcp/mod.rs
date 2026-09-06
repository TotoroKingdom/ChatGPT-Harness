//! MCP service adapter for the Runtime's existing read-only capabilities.
//!
//! This module defines metadata, argument/result mapping, and error mapping.
//! It deliberately owns no transport and performs no filesystem access.

mod context;
mod errors;
mod glob;
mod read_file;
mod service;

pub use context::{McpContextError, McpExecutionContext};
pub use service::McpService;
