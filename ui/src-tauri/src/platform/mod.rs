pub fn is_valid_directory(path: &str) -> bool {
    let p = std::path::Path::new(path);
    p.is_dir()
}

pub fn is_valid_file(path: &str) -> bool {
    let p = std::path::Path::new(path);
    p.is_file()
}

const TUNNEL_CLIENT_EXECUTABLE: &str = "tunnel-client.exe";

pub fn resolve_tunnel_client_executable(
    directory: &std::path::Path,
) -> Result<std::path::PathBuf, &'static str> {
    if !directory.is_dir() {
        return Err("Tunnel client directory is missing or inaccessible.");
    }
    let executable = directory.join(TUNNEL_CLIENT_EXECUTABLE);
    if !executable.is_file() {
        return Err("This folder does not contain tunnel-client.exe.");
    }
    Ok(executable)
}

pub fn normalize_tunnel_client_directory(path: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(path);
    if path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(TUNNEL_CLIENT_EXECUTABLE))
    {
        if let Some(parent) = path.parent() {
            return parent.to_path_buf();
        }
    }
    path.to_path_buf()
}

fn find_tunnel_client_in(paths: impl IntoIterator<Item = std::path::PathBuf>) -> Option<String> {
    paths
        .into_iter()
        .find(|p| p.is_file())
        .and_then(|p| p.parent().map(|parent| parent.display().to_string()))
}

pub fn find_tunnel_client() -> Option<String> {
    let mut paths = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            paths.push(parent.join(TUNNEL_CLIENT_EXECUTABLE));
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            paths.push(dir.join(TUNNEL_CLIENT_EXECUTABLE));
        }
    }
    find_tunnel_client_in(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_executable_from_valid_directory() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join(TUNNEL_CLIENT_EXECUTABLE);
        std::fs::write(&executable, b"mock").unwrap();

        assert_eq!(
            resolve_tunnel_client_executable(directory.path()).unwrap(),
            executable
        );
    }

    #[test]
    fn rejects_directory_without_executable() {
        let directory = tempfile::tempdir().unwrap();

        assert_eq!(
            resolve_tunnel_client_executable(directory.path()),
            Err("This folder does not contain tunnel-client.exe.")
        );
    }

    #[test]
    fn normalizes_existing_executable_path_to_parent_directory() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join(TUNNEL_CLIENT_EXECUTABLE);

        assert_eq!(
            normalize_tunnel_client_directory(&executable.display().to_string()),
            directory.path()
        );
    }

    #[test]
    fn auto_detect_returns_executable_parent_directory() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join(TUNNEL_CLIENT_EXECUTABLE);
        std::fs::write(&executable, b"mock").unwrap();

        assert_eq!(
            find_tunnel_client_in([executable]),
            Some(directory.path().display().to_string())
        );
    }
}
