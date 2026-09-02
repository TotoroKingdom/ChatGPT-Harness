pub fn is_valid_directory(path: &str) -> bool {
    let p = std::path::Path::new(path);
    p.is_dir()
}
pub fn is_valid_file(path: &str) -> bool {
    let p = std::path::Path::new(path);
    p.is_file()
}
pub fn find_tunnel_client() -> Option<String> {
    let candidates = ["tunnel-client.exe", "tunnel-client"];
    let mut paths = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for n in candidates {
                paths.push(parent.join(n));
            }
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            for n in candidates {
                paths.push(dir.join(n));
            }
        }
    }
    paths
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}
