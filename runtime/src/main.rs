mod events;
mod server;
mod state;
mod workspace;

fn main() {
    if let Err(error) = server::run() {
        eprintln!("runtime server stopped: {error}");
        std::process::exit(1);
    }
}
mod handlers;
