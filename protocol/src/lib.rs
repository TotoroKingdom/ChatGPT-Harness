//! Canonical UI-to-Runtime control protocol contracts for ChatGPT-Harness.
//!
//! This crate defines contracts only. It does not implement a Runtime, UI, MCP,
//! policy engine, secret store, or transport process.

pub mod export;
pub mod rpc;
pub mod v1;

pub use rpc::*;
pub use v1::*;

pub const PROTOCOL_VERSION: u32 = 1;
