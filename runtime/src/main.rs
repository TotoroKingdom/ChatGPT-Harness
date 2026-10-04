mod activity;
mod approval;
mod audit;
mod bash;
mod control_read;
mod diagnostics;
mod events;
mod execution;
#[allow(dead_code)] // The M5-D1 adapter is constructed by the future MCP transport.
mod mcp;
mod policy;
mod server;
mod session_grant;
mod state;
mod tools;
mod workspace;

#[cfg(test)]
mod m3_integration;
#[cfg(test)]
mod m5_analyzer_security;
#[cfg(all(test, windows))]
mod m5_bash_pipeline_tests;
#[cfg(test)]
mod m5_output_security;
#[cfg(test)]
mod m7_qa;

fn main() {
    if let Err(error) = server::run() {
        eprintln!("runtime server stopped: {error}");
        std::process::exit(1);
    }
}
mod handlers;
