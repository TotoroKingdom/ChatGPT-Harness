mod audit;
mod events;
mod execution;
#[allow(dead_code)] // The M5-D1 adapter is constructed by the future MCP transport.
mod mcp;
mod policy;
mod server;
mod state;
mod tools;
mod workspace;

fn main() {
    if let Err(error) = server::run() {
        eprintln!("runtime server stopped: {error}");
        std::process::exit(1);
    }
}
mod handlers;
