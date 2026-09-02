mod events;
mod server;
mod state;

fn main() {
    if let Err(error) = server::run() {
        eprintln!("runtime server stopped: {error}");
    }
}
mod handlers;
