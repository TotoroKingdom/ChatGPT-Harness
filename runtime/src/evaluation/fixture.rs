use super::manifest::{canonical_hash, normalize_lf, sha256_hex};
use super::{EvalError, EvalResult};
use serde_json::json;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) fn safe_relative(value: &str) -> EvalResult<PathBuf> {
    if value.trim().is_empty() {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "empty relative path",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "absolute path is not allowed",
        ));
    }
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    "parent traversal is not allowed",
                ))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    "rooted path is not allowed",
                ))
            }
        }
    }
    if result.as_os_str().is_empty() {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "path resolves to the fixture root",
        ));
    }
    Ok(result)
}

pub(crate) fn fixture_tree_hash(root: &Path) -> EvalResult<String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let entries = files
        .into_iter()
        .map(|(relative, bytes)| {
            json!({
                "path": relative,
                "kind": "text",
                "sha256": sha256_hex(&normalize_lf(&bytes)),
            })
        })
        .collect::<Vec<_>>();
    canonical_hash(&json!({ "files": entries }))
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> EvalResult<()> {
    let entries = fs::read_dir(current).map_err(|error| {
        EvalError::new(
            "INFRA_ERROR",
            format!("could not read fixture directory: {error}"),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            EvalError::new("INFRA_ERROR", format!("fixture entry error: {error}"))
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            EvalError::new("INFRA_ERROR", format!("fixture metadata error: {error}"))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "fixture symlinks are not allowed",
            ));
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| EvalError::new("HARNESS_CONTRACT_ERROR", "fixture path escaped root"))?
                .components()
                .filter_map(|component| match component {
                    Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            let bytes = fs::read(&path).map_err(|error| {
                EvalError::new(
                    "INFRA_ERROR",
                    format!("could not read fixture file: {error}"),
                )
            })?;
            files.push((relative, bytes));
        }
    }
    Ok(())
}

pub(crate) fn materialize_fixture(source: &Path, destination: &Path) -> EvalResult<()> {
    if destination.exists() {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            "fixture destination already exists",
        ));
    }
    fs::create_dir_all(destination).map_err(|error| {
        EvalError::new(
            "INFRA_ERROR",
            format!("could not create fixture destination: {error}"),
        )
    })?;
    copy_tree(source, destination)
}

fn copy_tree(source: &Path, destination: &Path) -> EvalResult<()> {
    for entry in fs::read_dir(source)
        .map_err(|error| EvalError::new("INFRA_ERROR", format!("fixture read error: {error}")))?
    {
        let entry = entry.map_err(|error| {
            EvalError::new("INFRA_ERROR", format!("fixture entry error: {error}"))
        })?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&from).map_err(|error| {
            EvalError::new("INFRA_ERROR", format!("fixture metadata error: {error}"))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                "fixture symlinks are not allowed",
            ));
        }
        if metadata.is_dir() {
            fs::create_dir(&to).map_err(|error| {
                EvalError::new("INFRA_ERROR", format!("fixture mkdir error: {error}"))
            })?;
            copy_tree(&from, &to)?;
        } else if metadata.is_file() {
            let bytes = fs::read(&from).map_err(|error| {
                EvalError::new("INFRA_ERROR", format!("fixture copy read error: {error}"))
            })?;
            fs::write(&to, bytes).map_err(|error| {
                EvalError::new("INFRA_ERROR", format!("fixture copy write error: {error}"))
            })?;
        }
    }
    Ok(())
}

pub(crate) fn read_workspace_file(root: &Path, relative: &str) -> EvalResult<Vec<u8>> {
    let relative = safe_relative(relative)?;
    let path = root.join(relative);
    fs::read(&path).map_err(|error| {
        EvalError::new(
            "INFRA_ERROR",
            format!("could not read workspace file for scoring: {error}"),
        )
    })
}

pub(crate) fn workspace_file_sha256(root: &Path, relative: &str) -> EvalResult<String> {
    Ok(sha256_hex(&normalize_lf(&read_workspace_file(
        root, relative,
    )?)))
}

pub(crate) fn workspace_file_exists(root: &Path, relative: &str) -> EvalResult<bool> {
    let relative = safe_relative(relative)?;
    Ok(root.join(relative).is_file())
}
