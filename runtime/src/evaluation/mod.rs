//! M04 offline controlled Agent evaluation.
//!
//! This module is a development/QA harness. It never participates in the
//! production Runtime request path and never owns authorization decisions.

pub(crate) mod adapter;
pub(crate) mod agent_loop;
pub(crate) mod aggregate;
pub(crate) mod approvals;
pub(crate) mod case;
pub(crate) mod correlation;
pub(crate) mod dataset;
pub(crate) mod fixture;
pub(crate) mod m05;
pub(crate) mod m09;
pub(crate) mod m09_fixture;
pub(crate) mod m09_gate;
pub(crate) mod m09_handoff;
pub(crate) mod m09_runner;
pub(crate) mod m09_scoring;
pub(crate) mod manifest;
pub(crate) mod mcp_client;
pub(crate) mod openai_responses;
pub(crate) mod output;
pub(crate) mod privacy;
pub(crate) mod profile;
pub(crate) mod scoring;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod formal_integration_tests;

use std::fmt;

#[derive(Debug)]
pub(crate) struct EvalError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl EvalError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EvalError {}

pub(crate) type EvalResult<T> = Result<T, EvalError>;

pub(crate) const DATASET_ID: &str = "m04-core-v1";
pub(crate) const DATASET_VERSION: u32 = 1;
pub(crate) const CASE_VERSION: u32 = 1;
pub(crate) const CASE_COUNT: usize = 24;
pub(crate) const CATEGORY_COUNT: usize = 6;
pub(crate) const CASES_PER_CATEGORY: usize = 4;
pub(crate) const REPETITIONS: u32 = 3;
pub(crate) const LOGICAL_RUNS: usize = CASE_COUNT * REPETITIONS as usize;
pub(crate) const MAX_MODEL_TURNS: u32 = 8;
pub(crate) const MAX_TOOL_CALLS: u32 = 12;
pub(crate) const MAX_OUTPUT_TOKENS_PER_TURN: u32 = 2048;
pub(crate) const RUN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
pub(crate) const ORDER_SEED: u64 = 0x4D3034;
pub(crate) const RUN_SPEND_CAP_USD: f64 = 0.25;
pub(crate) const SUITE_SPEND_CAP_USD: f64 = 15.00;
pub(crate) const BASELINE_TOOLS: [&str; 4] = ["read_file", "glob", "write", "edit"];
