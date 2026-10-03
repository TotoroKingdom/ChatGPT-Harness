//! Runtime-owned Git Bash discovery and executable validation.
//!
//! This module deliberately treats a configured path as a desired value, not
//! as proof that the executable is usable.  The final candidate is checked by
//! Runtime and, when possible, started with a bounded `--version` probe.

use super::process_tree::ProcessTreeOwner;
use super::runtime::neutralize_startup_environment;
use harness_control_protocol::BashConfig;
use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
    process::Stdio,
    thread,
    time::Duration,
};
use tokio::process::Command;

/// The probe must not be allowed to hold Runtime initialization indefinitely.
pub(crate) const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BashExecutableSource {
    Manual,
    AutoDetected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BashExecutableState {
    Ready {
        canonical_path: PathBuf,
        source: BashExecutableSource,
    },
    Unavailable {
        reason: BashUnavailableReason,
    },
}

impl BashExecutableState {
    pub(crate) fn unavailable(reason: BashUnavailableReason) -> Self {
        Self::Unavailable { reason }
    }

    pub(crate) fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub(crate) fn canonical_path(&self) -> Option<&Path> {
        match self {
            Self::Ready { canonical_path, .. } => Some(canonical_path),
            Self::Unavailable { .. } => None,
        }
    }

    pub(crate) fn source(&self) -> Option<&BashExecutableSource> {
        match self {
            Self::Ready { source, .. } => Some(source),
            Self::Unavailable { .. } => None,
        }
    }

    /// Stable, remediation-oriented text for effective tool state.  It does
    /// not include a raw command line, environment, or probe output.
    pub(crate) fn reason_message(&self) -> String {
        match self {
            Self::Ready { .. } => "Git Bash executable is ready.".into(),
            Self::Unavailable { reason } => reason.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BashUnavailableReason {
    ManualPathMissing,
    ManualPathInvalid,
    ManualPathUnsupported,
    ManualProbeFailed,
    AutoDetectDisabled,
    NoSupportedCandidate,
    CandidateProbeFailed,
}

impl fmt::Display for BashUnavailableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ManualPathMissing => {
                "Configured Git Bash path is empty; choose a supported Git-for-Windows bash.exe."
            }
            Self::ManualPathInvalid => {
                "Configured Git Bash path is not an accessible regular file."
            }
            Self::ManualPathUnsupported => {
                "Configured executable is not in a supported Git-for-Windows Bash layout."
            }
            Self::ManualProbeFailed => {
                "Configured Git Bash executable failed the bounded --version probe."
            }
            Self::AutoDetectDisabled => {
                "Git Bash auto-detection is disabled and no manual executable was configured."
            }
            Self::NoSupportedCandidate => {
                "No supported Git-for-Windows bash.exe was found; configure a manual path."
            }
            Self::CandidateProbeFailed => {
                "Detected Git-for-Windows Bash candidates failed Runtime validation."
            }
        };
        f.write_str(message)
    }
}

/// Resolve the executable for one immutable Runtime generation.
pub(crate) fn resolve_bash(config: &BashConfig) -> BashExecutableState {
    resolve_bash_with(config, auto_detect_candidates(), |candidate| {
        probe_version(candidate)
    })
}

/// Testable resolver with injected deterministic candidates and probe.  The
/// production resolver above is the only path that performs process startup.
pub(crate) fn resolve_bash_with<I, F>(
    config: &BashConfig,
    candidates: I,
    mut probe: F,
) -> BashExecutableState
where
    I: IntoIterator<Item = PathBuf>,
    F: FnMut(&Path) -> Result<(), ProbeFailure>,
{
    if let Some(path) = config.executable_path.as_deref() {
        let path = PathBuf::from(path);
        if path.as_os_str().is_empty() {
            return BashExecutableState::unavailable(BashUnavailableReason::ManualPathMissing);
        }
        return match validate_candidate_with_probe(&path, &mut probe) {
            Ok(canonical_path) => BashExecutableState::Ready {
                canonical_path,
                source: BashExecutableSource::Manual,
            },
            Err(error) => BashExecutableState::unavailable(error.manual_reason()),
        };
    }

    if !config.auto_detect {
        return BashExecutableState::unavailable(BashUnavailableReason::AutoDetectDisabled);
    }

    let mut saw_candidate = false;
    for candidate in candidates {
        saw_candidate = true;
        if let Ok(canonical_path) = validate_candidate_with_probe(&candidate, &mut probe) {
            return BashExecutableState::Ready {
                canonical_path,
                source: BashExecutableSource::AutoDetected,
            };
        }
    }
    BashExecutableState::unavailable(if saw_candidate {
        BashUnavailableReason::CandidateProbeFailed
    } else {
        BashUnavailableReason::NoSupportedCandidate
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProbeFailure {
    Spawn,
    NonZero,
    Timeout,
    Io,
}

impl ProbeFailure {
    fn manual_reason(&self) -> BashUnavailableReason {
        match self {
            Self::Spawn | Self::Io => BashUnavailableReason::ManualPathInvalid,
            Self::NonZero | Self::Timeout => BashUnavailableReason::ManualProbeFailed,
        }
    }
}

/// Validate a candidate and run an injected probe.  Kept crate-visible so
/// unit tests can prove selection and precedence with fixture files without
/// pretending those files are executable Git Bash instances.
pub(crate) fn validate_candidate_with_probe<F>(
    candidate: &Path,
    probe: &mut F,
) -> Result<PathBuf, CandidateValidationError>
where
    F: FnMut(&Path) -> Result<(), ProbeFailure>,
{
    if !candidate.is_absolute() || is_unc_or_network_path(candidate) {
        return Err(CandidateValidationError::NotAbsolute);
    }
    let metadata =
        fs::symlink_metadata(candidate).map_err(|_| CandidateValidationError::NotRegularFile)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(CandidateValidationError::NotRegularFile);
    }
    let canonical =
        fs::canonicalize(candidate).map_err(|_| CandidateValidationError::CannotCanonicalize)?;
    if !canonical.is_absolute() || is_unc_or_network_path(&canonical) {
        return Err(CandidateValidationError::NotAbsolute);
    }
    if !supported_git_bash_layout(&canonical) {
        return Err(CandidateValidationError::UnsupportedLayout);
    }
    probe(&canonical).map_err(CandidateValidationError::ProbeFailed)?;
    Ok(canonical)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CandidateValidationError {
    NotAbsolute,
    NotRegularFile,
    CannotCanonicalize,
    UnsupportedLayout,
    ProbeFailed(ProbeFailure),
}

impl CandidateValidationError {
    fn manual_reason(&self) -> BashUnavailableReason {
        match self {
            Self::NotAbsolute | Self::NotRegularFile | Self::CannotCanonicalize => {
                BashUnavailableReason::ManualPathInvalid
            }
            Self::UnsupportedLayout => BashUnavailableReason::ManualPathUnsupported,
            Self::ProbeFailed(failure) => failure.manual_reason(),
        }
    }
}

fn supported_git_bash_layout(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !name.eq_ignore_ascii_case("bash.exe") {
        return false;
    }

    // Supported Git-for-Windows layouts place bash.exe in bin or usr/bin and
    // include Git's own command/runtime markers.  We intentionally do not
    // require the installation root to be named "Git": custom install roots
    // are supported, while arbitrary WSL/Cygwin/MSYS bash.exe files are not.
    let mut ancestors = path.ancestors();
    let Some(parent) = ancestors.nth(1) else {
        return false;
    };
    let parent_name = parent.file_name().and_then(|name| name.to_str());
    let parent_is_bin = parent_name.is_some_and(|name| name.eq_ignore_ascii_case("bin"));
    let Some(root_candidate) = (if parent_is_bin { parent.parent() } else { None }) else {
        return false;
    };
    let root = if root_candidate
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("usr"))
    {
        root_candidate.parent()
    } else {
        Some(root_candidate)
    };
    root.is_some_and(git_layout_markers_present)
}

#[cfg(windows)]
fn is_unc_or_network_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};

    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(
                prefix.kind(),
                Prefix::UNC(..)
                    | Prefix::VerbatimUNC(..)
                    | Prefix::DeviceNS(..)
                    | Prefix::Verbatim(_)
            )
    )
}

#[cfg(not(windows))]
fn is_unc_or_network_path(path: &Path) -> bool {
    path.as_os_str().to_string_lossy().starts_with("//")
}

fn git_layout_markers_present(root: &Path) -> bool {
    // Git for Windows has both a command shim and a bundled MSYS runtime.
    // Requiring one of the Git executable markers plus the runtime marker
    // avoids accepting a random executable merely because it is named bash.
    let command_marker = [
        root.join("cmd").join("git.exe"),
        root.join("mingw64").join("bin").join("git.exe"),
        root.join("mingw32").join("bin").join("git.exe"),
    ]
    .into_iter()
    .any(|path| is_regular_file(&path));
    let runtime_marker = is_regular_file(&root.join("usr").join("bin").join("msys-2.0.dll"));
    command_marker && runtime_marker
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn auto_detect_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // Keep this order stable.  The explicit Git-for-Windows locations are
    // checked before PATH-derived locations so unrelated PATH changes cannot
    // silently change the selected installation.
    for variable in ["ProgramW6432", "ProgramFiles", "LocalAppData"] {
        if let Some(root) = env::var_os(variable).map(PathBuf::from) {
            let base = if variable == "LocalAppData" {
                root.join("Programs")
            } else {
                root
            };
            push_candidate_pair(&mut candidates, base.join("Git"));
        }
    }

    if let Some(git) = find_on_path("git.exe") {
        if let Some(root) = find_git_install_root(&git) {
            push_candidate_pair(&mut candidates, root);
        }
    }

    candidates
}

fn push_candidate_pair(candidates: &mut Vec<PathBuf>, git_root: PathBuf) {
    candidates.push(git_root.join("bin").join("bash.exe"));
    candidates.push(git_root.join("usr").join("bin").join("bash.exe"));
}

fn find_on_path(file_name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(file_name))
        .find_map(|candidate| {
            let metadata = fs::symlink_metadata(&candidate).ok()?;
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                return None;
            }
            fs::canonicalize(candidate).ok()
        })
}

fn find_git_install_root(git: &Path) -> Option<PathBuf> {
    git.parent()?
        .ancestors()
        .take(6)
        .find(|root| git_layout_markers_present(root))
        .map(Path::to_path_buf)
}

fn probe_version(path: &Path) -> Result<(), ProbeFailure> {
    // Runtime initialization is synchronous, while ProcessTreeOwner uses
    // Tokio for non-blocking process waits. Run the bounded probe in a
    // dedicated current-thread runtime and join that worker before returning;
    // this prevents a probe timeout from leaving a detached shell behind.
    let path = path.to_path_buf();
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(|_| ProbeFailure::Io)?;
        runtime.block_on(async move {
            let mut command = Command::new(&path);
            command
                .arg("--version")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            neutralize_startup_environment(&mut command);
            let owner = ProcessTreeOwner::spawn(command)
                .await
                .map_err(|_| ProbeFailure::Spawn)?;
            match tokio::time::timeout(VERSION_PROBE_TIMEOUT, owner.wait()).await {
                Ok(Ok(status)) => {
                    let terminated = owner.terminate().is_ok();
                    let quiescent = owner
                        .wait_for_quiescence(Duration::from_secs(1))
                        .await
                        .is_ok();
                    if !terminated || !quiescent {
                        Err(ProbeFailure::Io)
                    } else if status.success() {
                        Ok(())
                    } else {
                        Err(ProbeFailure::NonZero)
                    }
                }
                Ok(Err(_)) => {
                    if owner.terminate().is_err() {
                        return Err(ProbeFailure::Io);
                    }
                    if owner
                        .wait_for_quiescence(Duration::from_secs(1))
                        .await
                        .is_err()
                    {
                        return Err(ProbeFailure::Io);
                    }
                    Err(ProbeFailure::Io)
                }
                Err(_) => {
                    let terminated = owner.terminate().is_ok();
                    let waited = tokio::time::timeout(Duration::from_secs(1), owner.wait())
                        .await
                        .ok()
                        .and_then(Result::ok)
                        .is_some();
                    let quiescent = owner
                        .wait_for_quiescence(Duration::from_secs(1))
                        .await
                        .is_ok();
                    if terminated && waited && quiescent {
                        Err(ProbeFailure::Timeout)
                    } else {
                        Err(ProbeFailure::Io)
                    }
                }
            }
        })
    })
    .join()
    .unwrap_or(Err(ProbeFailure::Io))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "chatgpt-harness-m5-bash-discovery-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock")
                    .as_nanos()
            ));
            fs::create_dir_all(&path).expect("tempdir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture_git_bash() -> (TempDir, PathBuf, PathBuf) {
        let dir = TempDir::new();
        let git_bin = dir.path().join("Git").join("bin");
        let other_bin = dir.path().join("Other").join("bin");
        let git_cmd = dir.path().join("Git").join("cmd");
        let git_usr = dir.path().join("Git").join("usr").join("bin");
        fs::create_dir_all(&git_bin).expect("Git/bin");
        fs::create_dir_all(&git_cmd).expect("Git/cmd");
        fs::create_dir_all(&git_usr).expect("Git/usr/bin");
        fs::create_dir_all(&other_bin).expect("Other/bin");
        fs::write(git_cmd.join("git.exe"), b"fixture").expect("git marker");
        fs::write(git_usr.join("msys-2.0.dll"), b"fixture").expect("runtime marker");
        let supported = git_bin.join("bash.exe");
        let unsupported = other_bin.join("bash.exe");
        fs::write(&supported, b"fixture").expect("supported fixture");
        fs::write(&unsupported, b"fixture").expect("unsupported fixture");
        (dir, supported, unsupported)
    }

    fn success(_: &Path) -> Result<(), ProbeFailure> {
        Ok(())
    }

    #[test]
    fn manual_path_takes_precedence_and_validates_exact_candidate() {
        let (_dir, supported, _unsupported) = fixture_git_bash();
        let config = BashConfig {
            auto_detect: true,
            executable_path: Some(supported.display().to_string()),
        };
        let state = resolve_bash_with(&config, Vec::new(), success);
        assert!(matches!(
            state,
            BashExecutableState::Ready {
                source: BashExecutableSource::Manual,
                ..
            }
        ));
    }

    #[test]
    fn invalid_manual_path_never_falls_back_to_auto_detected_candidate() {
        let (_dir, supported, _unsupported) = fixture_git_bash();
        let config = BashConfig {
            auto_detect: true,
            executable_path: Some("relative\\bash.exe".into()),
        };
        let state = resolve_bash_with(&config, vec![supported], success);
        assert!(matches!(
            state,
            BashExecutableState::Unavailable {
                reason: BashUnavailableReason::ManualPathInvalid
            }
        ));
    }

    #[test]
    fn auto_detect_selects_first_valid_candidate_deterministically() {
        let (_dir, supported, _unsupported) = fixture_git_bash();
        let second = supported.with_file_name("bash-second.exe");
        fs::write(&second, b"fixture").expect("second fixture");
        // Rename the second fixture to a supported filename in another Git
        // root so it can be used as a later candidate.
        let second_root = second.parent().unwrap().parent().unwrap().join("GitTwo");
        let second_bin = second_root.join("bin");
        let second_cmd = second_root.join("cmd");
        let second_usr = second_root.join("usr").join("bin");
        fs::create_dir_all(&second_bin).expect("GitTwo/bin");
        fs::create_dir_all(&second_cmd).expect("GitTwo/cmd");
        fs::create_dir_all(&second_usr).expect("GitTwo/usr/bin");
        fs::write(second_cmd.join("git.exe"), b"fixture").expect("second git marker");
        fs::write(second_usr.join("msys-2.0.dll"), b"fixture").expect("second runtime marker");
        let second_bash = second_bin.join("bash.exe");
        fs::rename(second, &second_bash).expect("second bash");
        let config = BashConfig {
            auto_detect: true,
            executable_path: None,
        };
        let state = resolve_bash_with(&config, vec![supported.clone(), second_bash], success);
        assert_eq!(
            state.canonical_path(),
            Some(fs::canonicalize(supported).unwrap().as_path())
        );
        assert_eq!(state.source(), Some(&BashExecutableSource::AutoDetected));
    }

    #[test]
    fn arbitrary_bash_layout_is_rejected_even_when_probe_succeeds() {
        let (_dir, _supported, unsupported) = fixture_git_bash();
        let config = BashConfig {
            auto_detect: true,
            executable_path: None,
        };
        let state = resolve_bash_with(&config, vec![unsupported], success);
        assert!(matches!(
            state,
            BashExecutableState::Unavailable {
                reason: BashUnavailableReason::CandidateProbeFailed
            }
        ));
    }

    #[test]
    fn probe_failure_is_unavailable() {
        let (_dir, supported, _unsupported) = fixture_git_bash();
        let config = BashConfig {
            auto_detect: true,
            executable_path: None,
        };
        let state = resolve_bash_with(&config, vec![supported], |_| Err(ProbeFailure::Timeout));
        assert!(matches!(
            state,
            BashExecutableState::Unavailable {
                reason: BashUnavailableReason::CandidateProbeFailed
            }
        ));
    }

    #[test]
    fn auto_detection_can_be_disabled_without_a_manual_path() {
        let config = BashConfig {
            auto_detect: false,
            executable_path: None,
        };
        let state = resolve_bash_with(&config, Vec::new(), success);
        assert!(matches!(
            state,
            BashExecutableState::Unavailable {
                reason: BashUnavailableReason::AutoDetectDisabled
            }
        ));
    }

    #[test]
    fn custom_git_install_root_is_supported_without_a_git_directory_name() {
        let (dir, supported, _) = fixture_git_bash();
        let old_root = supported.parent().unwrap().parent().unwrap();
        let custom_root = dir.path().join("CustomDeveloperTools");
        fs::rename(old_root, &custom_root).unwrap();
        let path = custom_root.join("bin/bash.exe");
        let config = BashConfig {
            auto_detect: false,
            executable_path: Some(path.to_string_lossy().into_owned()),
        };
        assert!(resolve_bash_with(&config, Vec::new(), success).is_ready());
    }

    #[cfg(windows)]
    #[test]
    fn installed_git_bash_version_probe_uses_owned_process_path() {
        let candidates = [
            PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"),
            PathBuf::from(r"C:\Program Files\Git\usr\bin\bash.exe"),
        ];
        let Some(path) = candidates.into_iter().find(|path| path.is_file()) else {
            return;
        };
        assert_eq!(probe_version(&path), Ok(()));
    }

    #[cfg(windows)]
    #[test]
    fn windows_local_disk_prefixes_are_distinct_from_network_and_device_paths() {
        for path in [r"C:\Git\bin\bash.exe", r"\\?\C:\Git\bin\bash.exe"] {
            assert!(!is_unc_or_network_path(Path::new(path)), "{path}");
        }
        for path in [
            r"\\host\share\bash.exe",
            r"\\?\UNC\host\share\bash.exe",
            r"\\.\pipe\bash.exe",
        ] {
            assert!(is_unc_or_network_path(Path::new(path)), "{path}");
        }
    }
}
