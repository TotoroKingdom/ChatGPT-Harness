use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// The only two commit semantics needed by the text mutation tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AtomicCommitMode {
    CreateNew,
    ReplaceExisting,
}

/// The internal phase at which an atomic mutation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AtomicPhase {
    TempCreate,
    TempWrite,
    TempFlush,
    Commit,
    Cleanup,
}

/// Stable, Runtime-internal fault points used by unit tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AtomicFault {
    TempCreate,
    TempWrite,
    TempFlush,
    Commit,
}

/// Test-only controls are carried explicitly instead of relying on flaky
/// filesystem conditions such as a full disk or an externally locked file.
#[derive(Clone, Debug, Default)]
pub(crate) struct AtomicMutationOptions {
    pub(crate) fault: Option<AtomicFault>,
    pub(crate) cleanup_fault: bool,
    pub(crate) temp_file_name: Option<OsString>,
}

impl AtomicMutationOptions {
    #[cfg(test)]
    pub(crate) fn with_fault(fault: AtomicFault) -> Self {
        Self {
            fault: Some(fault),
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn with_fault_and_cleanup(fault: AtomicFault) -> Self {
        Self {
            fault: Some(fault),
            cleanup_fault: true,
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn with_temp_file_name(name: impl Into<OsString>) -> Self {
        Self {
            temp_file_name: Some(name.into()),
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AtomicFileError {
    pub(crate) phase: AtomicPhase,
    pub(crate) kind: io::ErrorKind,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AtomicMutationError<E> {
    Revalidation {
        error: E,
        cleanup: Option<AtomicFileError>,
    },
    Io {
        error: AtomicFileError,
        cleanup: Option<AtomicFileError>,
    },
}

/// Stages bytes beside `target`, flushes the staged file, lets the caller
/// revalidate its authorization/version assumptions, and commits in one
/// target-directory operation. The callback is intentionally generic: this
/// primitive is not a second WorkspaceGuard or policy authority.
pub(crate) fn atomic_mutate<E>(
    target: &Path,
    bytes: &[u8],
    mode: AtomicCommitMode,
    revalidate: impl FnOnce() -> Result<(), E>,
    options: AtomicMutationOptions,
) -> Result<(), AtomicMutationError<E>> {
    let Some(parent) = target.parent() else {
        return Err(AtomicMutationError::Io {
            error: AtomicFileError {
                phase: AtomicPhase::TempCreate,
                kind: io::ErrorKind::InvalidInput,
            },
            cleanup: None,
        });
    };
    let Some(target_name) = target.file_name() else {
        return Err(AtomicMutationError::Io {
            error: AtomicFileError {
                phase: AtomicPhase::TempCreate,
                kind: io::ErrorKind::InvalidInput,
            },
            cleanup: None,
        });
    };

    let temp_path = parent.join(
        options
            .temp_file_name
            .clone()
            .unwrap_or_else(|| unique_temp_file_name(target_name)),
    );

    let mut temp = match create_temp_file(&temp_path, &options) {
        Ok(file) => file,
        Err(error) => {
            return Err(AtomicMutationError::Io {
                error,
                cleanup: None,
            })
        }
    };

    if let Err(error) = write_temp_file(&mut temp, bytes, &options) {
        drop(temp);
        return Err(AtomicMutationError::Io {
            error,
            cleanup: cleanup_temp(&temp_path, &options),
        });
    }

    if let Err(error) = flush_temp_file(&mut temp, &options) {
        drop(temp);
        return Err(AtomicMutationError::Io {
            error,
            cleanup: cleanup_temp(&temp_path, &options),
        });
    }
    drop(temp);

    if let Err(error) = revalidate() {
        return Err(AtomicMutationError::Revalidation {
            error,
            cleanup: cleanup_temp(&temp_path, &options),
        });
    }

    if let Err(error) = commit(&temp_path, target, mode, &options) {
        return Err(AtomicMutationError::Io {
            error,
            cleanup: cleanup_temp(&temp_path, &options),
        });
    }

    // ReplaceFile/MoveFileEx and rename consume the source. The non-Windows
    // CreateNew implementation uses a hard link so that an already-created
    // target cannot be silently overwritten, then removes this source name.
    // A post-commit cleanup failure cannot make an already committed operation
    // report failure; it is intentionally an internal best-effort condition.
    let _ = cleanup_temp(&temp_path, &options);
    Ok(())
}

fn unique_temp_file_name(target_name: &std::ffi::OsStr) -> OsString {
    let mut name = OsString::from(".");
    name.push(target_name);
    name.push(".chatgpt-harness-");
    name.push(uuid::Uuid::new_v4().simple().to_string());
    name.push(".tmp");
    name
}

fn create_temp_file(
    temp_path: &Path,
    options: &AtomicMutationOptions,
) -> Result<File, AtomicFileError> {
    if options.fault == Some(AtomicFault::TempCreate) {
        return Err(fault_error(AtomicPhase::TempCreate));
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
        .map_err(|error| io_error(AtomicPhase::TempCreate, error))
}

fn write_temp_file(
    temp: &mut File,
    bytes: &[u8],
    options: &AtomicMutationOptions,
) -> Result<(), AtomicFileError> {
    if options.fault == Some(AtomicFault::TempWrite) {
        return Err(fault_error(AtomicPhase::TempWrite));
    }
    temp.write_all(bytes)
        .map_err(|error| io_error(AtomicPhase::TempWrite, error))
}

fn flush_temp_file(
    temp: &mut File,
    options: &AtomicMutationOptions,
) -> Result<(), AtomicFileError> {
    if options.fault == Some(AtomicFault::TempFlush) {
        return Err(fault_error(AtomicPhase::TempFlush));
    }
    temp.flush()
        .map_err(|error| io_error(AtomicPhase::TempFlush, error))
}

fn commit(
    temp_path: &Path,
    target: &Path,
    mode: AtomicCommitMode,
    options: &AtomicMutationOptions,
) -> Result<(), AtomicFileError> {
    if options.fault == Some(AtomicFault::Commit) {
        return Err(fault_error(AtomicPhase::Commit));
    }

    let result = match mode {
        AtomicCommitMode::CreateNew => commit_create_new(temp_path, target),
        AtomicCommitMode::ReplaceExisting => commit_replace_existing(temp_path, target),
    };
    result.map_err(|error| io_error(AtomicPhase::Commit, error))
}

fn cleanup_temp(temp_path: &Path, options: &AtomicMutationOptions) -> Option<AtomicFileError> {
    if options.cleanup_fault {
        return Some(fault_error(AtomicPhase::Cleanup));
    }
    match fs::remove_file(temp_path) {
        Ok(()) => None,
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => Some(io_error(AtomicPhase::Cleanup, error)),
    }
}

fn fault_error(phase: AtomicPhase) -> AtomicFileError {
    AtomicFileError {
        phase,
        kind: io::ErrorKind::Other,
    }
}

fn io_error(phase: AtomicPhase, error: io::Error) -> AtomicFileError {
    AtomicFileError {
        phase,
        kind: error.kind(),
    }
}

#[cfg(windows)]
fn commit_create_new(temp_path: &Path, target: &Path) -> io::Result<()> {
    windows_move_file(temp_path, target)
}

#[cfg(not(windows))]
fn commit_create_new(temp_path: &Path, target: &Path) -> io::Result<()> {
    // POSIX rename replaces an existing destination. A hard link gives this
    // mode create-new semantics even if another actor creates target after
    // the caller's revalidation and before commit.
    fs::hard_link(temp_path, target)
}

#[cfg(windows)]
fn commit_replace_existing(temp_path: &Path, target: &Path) -> io::Result<()> {
    windows_replace_file(temp_path, target)
}

#[cfg(not(windows))]
fn commit_replace_existing(temp_path: &Path, target: &Path) -> io::Result<()> {
    fs::rename(temp_path, target)
}

#[cfg(windows)]
fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn windows_move_file(source: &Path, destination: &Path) -> io::Result<()> {
    let source = wide_path(source);
    let destination = wide_path(destination);
    let success = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING_DISABLED,
        )
    };
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn windows_replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    let source = wide_path(source);
    let destination = wide_path(destination);
    let success = unsafe {
        ReplaceFileW(
            destination.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
const MOVEFILE_REPLACE_EXISTING_DISABLED: u32 = 0;

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn MoveFileExW(existing_file_name: *const u16, new_file_name: *const u16, flags: u32) -> i32;
    fn ReplaceFileW(
        replaced_file_name: *const u16,
        replacement_file_name: *const u16,
        backup_file_name: *const u16,
        replace_flags: u32,
        exclude: *const std::ffi::c_void,
        reserved: *const std::ffi::c_void,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Fixture {
        base: PathBuf,
        target: PathBuf,
    }

    impl Fixture {
        fn new(existing: Option<&[u8]>) -> Self {
            let base = std::env::temp_dir()
                .join(format!("chatgpt-harness-atomic-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&base).expect("create fixture directory");
            let target = base.join("target.txt");
            if let Some(bytes) = existing {
                fs::write(&target, bytes).expect("write fixture target");
            }
            Self { base, target }
        }

        fn assert_no_temp_residue(&self) {
            let residue = fs::read_dir(&self.base)
                .expect("read fixture directory")
                .map(|entry| entry.expect("read fixture entry").file_name())
                .filter(|name| name.to_string_lossy().contains("chatgpt-harness-"))
                .collect::<Vec<_>>();
            assert!(residue.is_empty(), "temporary residue: {residue:?}");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn ok_revalidation() -> Result<(), ()> {
        Ok(())
    }

    #[test]
    fn create_new_stages_and_commits_complete_bytes_without_residue() {
        let fixture = Fixture::new(None);
        atomic_mutate(
            &fixture.target,
            "新内容\n".as_bytes(),
            AtomicCommitMode::CreateNew,
            ok_revalidation,
            AtomicMutationOptions::default(),
        )
        .expect("atomic create");

        assert_eq!(fs::read(&fixture.target).unwrap(), "新内容\n".as_bytes());
        fixture.assert_no_temp_residue();
    }

    #[test]
    fn replace_existing_uses_atomic_replacement_and_preserves_complete_bytes() {
        let fixture = Fixture::new(Some(b"old"));
        atomic_mutate(
            &fixture.target,
            b"new complete content",
            AtomicCommitMode::ReplaceExisting,
            ok_revalidation,
            AtomicMutationOptions::default(),
        )
        .expect("atomic replace");

        assert_eq!(fs::read(&fixture.target).unwrap(), b"new complete content");
        fixture.assert_no_temp_residue();
    }

    #[test]
    fn precommit_revalidation_failure_preserves_target_and_cleans_temp() {
        let fixture = Fixture::new(Some(b"old"));
        let result = atomic_mutate(
            &fixture.target,
            b"new",
            AtomicCommitMode::ReplaceExisting,
            || Err("changed"),
            AtomicMutationOptions::default(),
        );

        assert_eq!(
            result,
            Err(AtomicMutationError::Revalidation {
                error: "changed",
                cleanup: None,
            })
        );
        assert_eq!(fs::read(&fixture.target).unwrap(), b"old");
        fixture.assert_no_temp_residue();
    }

    #[test]
    fn each_staging_fault_preserves_existing_target_and_cleans_temp() {
        for fault in [
            AtomicFault::TempCreate,
            AtomicFault::TempWrite,
            AtomicFault::TempFlush,
            AtomicFault::Commit,
        ] {
            let fixture = Fixture::new(Some(b"old"));
            let result = atomic_mutate(
                &fixture.target,
                b"new",
                AtomicCommitMode::ReplaceExisting,
                ok_revalidation,
                AtomicMutationOptions::with_fault(fault),
            );
            assert!(matches!(
                result,
                Err(AtomicMutationError::Io {
                    error: AtomicFileError { .. },
                    cleanup: _
                })
            ));
            assert_eq!(fs::read(&fixture.target).unwrap(), b"old");
            fixture.assert_no_temp_residue();
        }
    }

    #[test]
    fn failed_create_never_leaves_a_named_target() {
        let fixture = Fixture::new(None);
        let result = atomic_mutate(
            &fixture.target,
            b"new",
            AtomicCommitMode::CreateNew,
            ok_revalidation,
            AtomicMutationOptions::with_fault(AtomicFault::Commit),
        );

        assert!(result.is_err());
        assert!(!fixture.target.exists());
        fixture.assert_no_temp_residue();
    }

    #[test]
    fn cleanup_failure_is_diagnostic_and_does_not_replace_primary_error() {
        let fixture = Fixture::new(Some(b"old"));
        let result = atomic_mutate(
            &fixture.target,
            b"new",
            AtomicCommitMode::ReplaceExisting,
            ok_revalidation,
            AtomicMutationOptions::with_fault_and_cleanup(AtomicFault::TempWrite),
        );

        assert!(matches!(
            result,
            Err(AtomicMutationError::Io {
                error: AtomicFileError {
                    phase: AtomicPhase::TempWrite,
                    ..
                },
                cleanup: Some(AtomicFileError {
                    phase: AtomicPhase::Cleanup,
                    ..
                })
            })
        ));
        assert_eq!(fs::read(&fixture.target).unwrap(), b"old");
    }

    #[test]
    fn temporary_file_is_created_in_target_directory_and_exclusive() {
        let fixture = Fixture::new(Some(b"old"));
        let name = OsString::from(".known-atomic-temp");
        let temp = fixture.base.join(&name);
        fs::write(&temp, b"sentinel").expect("write collision fixture");
        let result = atomic_mutate(
            &fixture.target,
            b"new",
            AtomicCommitMode::ReplaceExisting,
            || {
                assert_eq!(temp.parent(), fixture.target.parent());
                Ok::<(), ()>(())
            },
            AtomicMutationOptions::with_temp_file_name(name),
        );

        assert!(matches!(
            result,
            Err(AtomicMutationError::Io {
                error: AtomicFileError {
                    phase: AtomicPhase::TempCreate,
                    kind: io::ErrorKind::AlreadyExists,
                },
                cleanup: None
            })
        ));
        assert_eq!(fs::read(&fixture.target).unwrap(), b"old");
        assert_eq!(fs::read(temp).unwrap(), b"sentinel");
    }
}
