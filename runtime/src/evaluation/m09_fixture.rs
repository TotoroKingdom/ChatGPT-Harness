//! M09-only fixture recipes. Empty directories and reparse entries are made
//! only inside a disposable fixture; never in a real user's workspace.
use super::fixture::{materialize_fixture, safe_relative};
use super::manifest::{canonical_hash, sha256_hex};
use super::{EvalError, EvalResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FixtureSetup {
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub reparse_entries: Vec<ReparseEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReparseEntry {
    pub path: String,
    pub target: String,
    pub kind: String,
}

impl FixtureSetup {
    pub(crate) fn validate(&self) -> EvalResult<()> {
        let mut names = std::collections::BTreeSet::new();
        for name in self
            .directories
            .iter()
            .chain(self.reparse_entries.iter().map(|entry| &entry.path))
        {
            safe_relative(name)?;
            if !names.insert(name) {
                return Err(contract("duplicate fixture setup path"));
            }
        }
        for entry in &self.reparse_entries {
            safe_relative(&entry.target)?;
            if !matches!(entry.kind.as_str(), "file" | "directory") || entry.path == entry.target {
                return Err(contract("invalid reparse fixture recipe"));
            }
        }
        Ok(())
    }
}

fn contract(message: &str) -> EvalError {
    EvalError::new("HARNESS_CONTRACT_ERROR", message)
}

/// Unlike the frozen M04 text fixture hash, M09 binds exact binary bytes and
/// the setup recipe. No CRLF normalization can mask a changed binary source.
pub(crate) fn fixture_hash(source: &Path, setup: &FixtureSetup) -> EvalResult<String> {
    setup.validate()?;
    let mut files = Vec::new();
    collect(source, source, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let files: Vec<_> = files
        .into_iter()
        .map(|(path, bytes)| json!({"path":path,"kind":"file","sha256":sha256_hex(&bytes)}))
        .collect();
    canonical_hash(&json!({"files":files,"setup":setup}))
}

fn collect(root: &Path, directory: &Path, files: &mut Vec<(String, Vec<u8>)>) -> EvalResult<()> {
    let entries =
        std::fs::read_dir(directory).map_err(|_| contract("fixture directory unreadable"))?;
    for entry in entries {
        let path = entry
            .map_err(|_| contract("fixture entry unreadable"))?
            .path();
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| contract("fixture metadata unreadable"))?;
        if is_reparse(&metadata) {
            return Err(contract(
                "source fixtures must use reparse recipes, not physical links",
            ));
        }
        if metadata.is_dir() {
            collect(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| contract("fixture escaped source root"))?;
            let name = relative
                .iter()
                .map(|part| {
                    part.to_str()
                        .ok_or_else(|| contract("non UTF-8 fixture name"))
                })
                .collect::<EvalResult<Vec<_>>>()?
                .join("/");
            let bytes = std::fs::read(&path).map_err(|_| contract("fixture bytes unreadable"))?;
            files.push((name, bytes));
        } else {
            return Err(contract("unsupported physical fixture entry"));
        }
    }
    Ok(())
}

pub(crate) struct PreparedFixture {
    _temporary: tempfile::TempDir,
    pub workspace: PathBuf,
}

pub(crate) fn prepare_fixture(source: &Path, setup: &FixtureSetup) -> EvalResult<PreparedFixture> {
    // Validate and hash before copying so a source junction cannot be followed.
    fixture_hash(source, setup)?;
    let temporary = tempfile::Builder::new()
        .prefix("harness-m09-fixture-")
        .tempdir()
        .map_err(|_| EvalError::new("INFRA_ERROR", "could not allocate disposable M09 fixture"))?;
    let workspace = temporary.path().join("workspace");
    materialize_fixture(source, &workspace)
        .map_err(|_| EvalError::new("INFRA_ERROR", "could not copy M09 fixture"))?;
    for name in &setup.directories {
        let destination = workspace.join(safe_relative(name)?);
        if destination.exists() && !destination.is_dir() {
            return Err(contract("setup directory conflicts with a source file"));
        }
        std::fs::create_dir_all(destination).map_err(|_| {
            EvalError::new(
                "INFRA_ERROR",
                "could not create disposable fixture directory",
            )
        })?;
    }
    // All links are created last and point to ordinary entries inside the same
    // TempDir. No subsequent preparation step can traverse a created link.
    for entry in &setup.reparse_entries {
        let path = workspace.join(safe_relative(&entry.path)?);
        let target = workspace.join(safe_relative(&entry.target)?);
        let parent = path
            .parent()
            .ok_or_else(|| contract("missing reparse parent"))?;
        if std::fs::symlink_metadata(&path).is_ok() || !parent.is_dir() || !target.exists() {
            return Err(contract("reparse recipe preconditions are not satisfied"));
        }
        let target_metadata = std::fs::symlink_metadata(&target)
            .map_err(|_| contract("reparse fixture target unavailable"))?;
        if is_reparse(&target_metadata)
            || (entry.kind == "directory" && !target_metadata.is_dir())
            || (entry.kind == "file" && !target_metadata.is_file())
        {
            return Err(contract("reparse recipe target kind does not match"));
        }
        for component_path in [parent, target.as_path()] {
            let canonical = component_path
                .canonicalize()
                .map_err(|_| contract("fixture recipe target unavailable"))?;
            let root = workspace
                .canonicalize()
                .map_err(|_| contract("fixture root unavailable"))?;
            if !canonical.starts_with(root) {
                return Err(contract("fixture recipe escaped temporary workspace"));
            }
        }
        create_link(&target, &path, &entry.kind).map_err(|_| {
            EvalError::new(
                "BLOCKED",
                "environment cannot create the required disposable reparse fixture",
            )
        })?;
    }
    Ok(PreparedFixture {
        _temporary: temporary,
        workspace,
    })
}

fn create_link(target: &Path, path: &Path, kind: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        if kind == "directory" {
            std::os::windows::fs::symlink_dir(target, path)
        } else {
            std::os::windows::fs::symlink_file(target, path)
        }
    }
    #[cfg(unix)]
    {
        let _ = kind;
        std::os::unix::fs::symlink(target, path)
    }
}

fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_directories_are_reproducible_and_temporary() {
        let source = tempfile::tempdir().unwrap();
        let recipe = FixtureSetup {
            directories: vec!["empty".into()],
            ..Default::default()
        };
        let prepared = prepare_fixture(source.path(), &recipe).unwrap();
        let location = prepared.workspace.clone();
        assert!(location.join("empty").is_dir());
        assert_eq!(
            std::fs::read_dir(location.join("empty")).unwrap().count(),
            0
        );
        assert!(!source.path().join("empty").exists());
        drop(prepared);
        assert!(!location.exists());
    }
    #[test]
    fn raw_binary_and_recipes_both_change_fixture_identity() {
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("source.bin"), b"a\r\nb").unwrap();
        let first = fixture_hash(source.path(), &FixtureSetup::default()).unwrap();
        std::fs::write(source.path().join("source.bin"), b"a\nb").unwrap();
        assert_ne!(
            first,
            fixture_hash(source.path(), &FixtureSetup::default()).unwrap()
        );
        assert_ne!(
            fixture_hash(source.path(), &FixtureSetup::default()).unwrap(),
            fixture_hash(
                source.path(),
                &FixtureSetup {
                    directories: vec!["empty".into()],
                    ..Default::default()
                }
            )
            .unwrap()
        );
        assert!(FixtureSetup {
            directories: vec!["../outside".into()],
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}
