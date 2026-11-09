use super::control::{ControlError, SearchControl};
use super::filters::{ignore_overrides, PathFilters};
use crate::workspace::{WorkspaceError, WorkspaceGuard};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub(crate) const MAX_WALK_DIAGNOSTICS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinkPolicy {
    /// Follow reparse points only when their canonical target stays inside.
    FollowAuthorized,
    /// Never descend through or read a reparse point.
    Skip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WalkEntryKind {
    File,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WalkEntry {
    pub(crate) logical: String,
    pub(crate) path: PathBuf,
    pub(crate) kind: WalkEntryKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WalkDiagnosticCode {
    WorkspaceRejected,
    BrokenLink,
    MetadataFailed,
    ReadDirectoryFailed,
    ReparseSkipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WalkDiagnostic {
    pub(crate) logical: String,
    pub(crate) code: WalkDiagnosticCode,
    pub(crate) message: Option<String>,
}

pub(crate) struct WalkRequest {
    /// Workspace-relative logical path of the approved walk root.
    pub(crate) base_logical: String,
    /// Approved physical directory for the walk root.
    pub(crate) base_path: PathBuf,
    /// `None` means unbounded depth; `Some(1)` means direct children only.
    pub(crate) max_depth: Option<usize>,
    pub(crate) links: LinkPolicy,
    pub(crate) max_entries: usize,
    pub(crate) filters: PathFilters,
    /// Explicit prefixes whose ignore rules must not hide the requested tree.
    pub(crate) ignore_overrides: Vec<String>,
}

pub(crate) struct WalkOutput {
    pub(crate) entries: Vec<WalkEntry>,
    pub(crate) diagnostics: Vec<WalkDiagnostic>,
    pub(crate) diagnostics_truncated: bool,
    pub(crate) truncated: bool,
    pub(crate) control_error: Option<ControlError>,
}

pub(crate) fn walk(
    guard: &WorkspaceGuard,
    request: WalkRequest,
    control: Arc<Mutex<SearchControl>>,
) -> WalkOutput {
    let root = Arc::new(guard.clone());
    let base_path = request.base_path;
    let base_logical = request.base_logical;
    let filters = request.filters;
    let links = request.links;
    let diagnostics = Arc::new(Mutex::new(Diagnostics::default()));
    let control_cell = Arc::new(Mutex::new(None::<ControlError>));

    let mut builder = WalkBuilder::new(&base_path);
    builder
        .hidden(false)
        .parents(true)
        .ignore(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .follow_links(links == LinkPolicy::FollowAuthorized);
    if let Some(depth) = request.max_depth {
        builder.max_depth(Some(depth));
    }
    if !request.ignore_overrides.is_empty() {
        let mut overrides = OverrideBuilder::new(root.canonical_workspace_root());
        let mut valid = true;
        for prefix in &request.ignore_overrides {
            for pattern in ignore_overrides(prefix) {
                if overrides.add(&pattern).is_err() {
                    valid = false;
                }
            }
        }
        if valid {
            if let Ok(built) = overrides.build() {
                builder.overrides(built);
            }
        }
    }

    {
        let root_for_filter = Arc::clone(&root);
        let base_for_filter = base_path.clone();
        let logical_for_filter = base_logical.clone();
        let filters_for_filter = filters.clone();
        let diagnostics_for_filter = Arc::clone(&diagnostics);
        let control_for_filter = Arc::clone(&control);
        let control_error_for_filter = Arc::clone(&control_cell);
        builder.filter_entry(move |entry| {
            if lock(&control_error_for_filter).is_some() {
                return false;
            }
            {
                let mut control = lock(&control_for_filter);
                if let Err(error) = control.check() {
                    drop(control);
                    *lock(&control_error_for_filter) = Some(error);
                    return false;
                }
            }
            let logical = logical_path(
                &logical_for_filter,
                entry.path().strip_prefix(&base_for_filter).ok(),
            );
            let file_type = entry.file_type();
            let is_dir = file_type.is_some_and(|kind| kind.is_dir());

            if entry.depth() > 0 && is_dir && entry.file_name() == std::ffi::OsStr::new(".git") {
                return false;
            }
            if is_dir && filters_for_filter.prunes_directory(&logical) {
                return false;
            }
            if file_type.is_some_and(|kind| kind.is_file())
                && filters_for_filter.includes()
                && !filters_for_filter.allows(&logical)
            {
                return false;
            }

            // Ordinary files are validated again at open time, so this check
            // only stats directories (junction detection) and known links.
            let needs_reparse_check = is_dir || entry.path_is_symlink();
            if !needs_reparse_check {
                return true;
            }
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(error) => {
                    lock(&diagnostics_for_filter).push(
                        logical,
                        if error.kind() == io::ErrorKind::NotFound {
                            WalkDiagnosticCode::BrokenLink
                        } else {
                            WalkDiagnosticCode::MetadataFailed
                        },
                        Some(sanitized_io_message(error.kind())),
                    );
                    return false;
                }
            };
            if is_reparse_or_symlink(&metadata) {
                match links {
                    LinkPolicy::Skip => {
                        lock(&diagnostics_for_filter).push(
                            logical,
                            WalkDiagnosticCode::ReparseSkipped,
                            None,
                        );
                        return false;
                    }
                    LinkPolicy::FollowAuthorized => {
                        let canonical = match fs::canonicalize(entry.path()) {
                            Ok(path) => path,
                            Err(error) => {
                                lock(&diagnostics_for_filter).push(
                                    logical,
                                    WalkDiagnosticCode::BrokenLink,
                                    Some(sanitized_io_message(error.kind())),
                                );
                                return false;
                            }
                        };
                        if !root_for_filter.contains_canonical_path(&canonical) {
                            lock(&diagnostics_for_filter).push(
                                logical,
                                WalkDiagnosticCode::WorkspaceRejected,
                                None,
                            );
                            return false;
                        }
                    }
                }
            } else if is_dir {
                // Ordinary directories are approved immediately before
                // descent so a replaced parent cannot expose a new subtree.
                let canonical = match fs::canonicalize(entry.path()) {
                    Ok(path) => path,
                    Err(error) => {
                        lock(&diagnostics_for_filter).push(
                            logical,
                            if error.kind() == io::ErrorKind::NotFound {
                                WalkDiagnosticCode::BrokenLink
                            } else {
                                WalkDiagnosticCode::MetadataFailed
                            },
                            Some(sanitized_io_message(error.kind())),
                        );
                        return false;
                    }
                };
                if !root_for_filter.contains_canonical_path(&canonical) {
                    lock(&diagnostics_for_filter).push(
                        logical,
                        WalkDiagnosticCode::WorkspaceRejected,
                        None,
                    );
                    return false;
                }
            }
            true
        });
    }

    let mut entries = BTreeMap::<String, WalkEntry>::new();
    let mut truncated = false;
    for item in builder.build() {
        if lock(&control_cell).is_some() {
            break;
        }
        let Ok(entry) = item else {
            lock(&diagnostics).push(
                base_logical.clone(),
                WalkDiagnosticCode::ReadDirectoryFailed,
                None,
            );
            continue;
        };
        if entry.depth() == 0 {
            continue;
        }
        if let Err(error) = lock(&control).check() {
            *lock(&control_cell) = Some(error);
            break;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        let kind = if file_type.is_dir() {
            WalkEntryKind::Directory
        } else if file_type.is_file() {
            WalkEntryKind::File
        } else {
            continue;
        };
        let logical = logical_path(&base_logical, entry.path().strip_prefix(&base_path).ok());
        if kind == WalkEntryKind::File && !filters.allows(&logical) {
            continue;
        }
        if entries.len() >= request.max_entries {
            truncated = true;
            break;
        }
        entries.insert(
            logical.clone(),
            WalkEntry {
                logical,
                path: entry.path().to_path_buf(),
                kind,
            },
        );
    }

    let diagnostics = lock(&diagnostics);
    let control_error = lock(&control_cell).take();
    WalkOutput {
        entries: entries.into_values().collect(),
        diagnostics: diagnostics.items.clone(),
        diagnostics_truncated: diagnostics.truncated,
        truncated,
        control_error,
    }
}

pub(crate) fn diagnostic_code(error: &WorkspaceError) -> WalkDiagnosticCode {
    match error {
        WorkspaceError::TargetNotFound { .. } => WalkDiagnosticCode::BrokenLink,
        _ => WalkDiagnosticCode::WorkspaceRejected,
    }
}

pub(crate) fn sanitized_io_message(kind: io::ErrorKind) -> String {
    format!("filesystem error: {kind:?}")
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|error| error.into_inner())
}

fn logical_path(base_logical: &str, relative: Option<&Path>) -> String {
    let Some(relative) = relative else {
        return base_logical.to_owned();
    };
    let relative = relative
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned();
    if relative.is_empty() {
        base_logical.to_owned()
    } else if base_logical.is_empty() {
        relative
    } else {
        format!("{base_logical}/{relative}")
    }
}

fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[derive(Default)]
struct Diagnostics {
    items: Vec<WalkDiagnostic>,
    truncated: bool,
}

impl Diagnostics {
    fn push(&mut self, logical: String, code: WalkDiagnosticCode, message: Option<String>) {
        if self.items.len() < MAX_WALK_DIAGNOSTICS {
            self.items.push(WalkDiagnostic {
                logical,
                code,
                message,
            });
        } else {
            self.truncated = true;
        }
    }
}
