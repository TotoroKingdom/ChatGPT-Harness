//! Async, bounded Bash execution owned by one Runtime generation.
//!
//! This module only runs a command after the caller has obtained an
//! `ExecutionPermit` from the ExecutionPipeline.  It does not expose cwd,
//! environment, timeout, or output-limit overrides to the MCP request.

use super::output::{OutputLimiter, OutputStream, MAX_CAPTURED_OUTPUT_BYTES};
use super::process_tree::ProcessTreeOwner;
use crate::execution::ExecutionPermit;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::{oneshot, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// M5 hard limits.  These are intentionally not persisted configuration.
pub(crate) const MAX_COMMAND_BYTES: usize = 8_192;
pub(crate) const EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const MAX_CONCURRENT_BASH: usize = 1;
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug)]
pub(crate) struct BashLimits {
    pub(crate) timeout: Duration,
    pub(crate) output_bytes: usize,
    pub(crate) max_concurrent: usize,
}

impl BashLimits {
    pub(crate) const fn production() -> Self {
        Self {
            timeout: EXECUTION_TIMEOUT,
            output_bytes: MAX_CAPTURED_OUTPUT_BYTES,
            max_concurrent: MAX_CONCURRENT_BASH,
        }
    }

    /// Test-only shortening of the frozen production values.  This function
    /// is not compiled into the Runtime product and cannot be reached through
    /// config or MCP.
    #[cfg(test)]
    pub(crate) const fn for_test(timeout: Duration, output_bytes: usize) -> Self {
        Self {
            timeout,
            output_bytes,
            max_concurrent: MAX_CONCURRENT_BASH,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BashRequest {
    pub(crate) command: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BashOutput {
    pub(crate) exit_code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) duration_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BashRuntimeError {
    Unavailable,
    InvalidCommand,
    CommandTooLong,
    Timeout,
    OutputLimit,
    ConcurrencyLimit,
    SpawnFailed,
    Cancelled,
    ProcessFailed,
}

impl BashRuntimeError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "BASH_UNAVAILABLE",
            Self::InvalidCommand => "BASH_INVALID_COMMAND",
            Self::CommandTooLong => "BASH_COMMAND_TOO_LONG",
            Self::Timeout => "BASH_TIMEOUT",
            Self::OutputLimit => "BASH_OUTPUT_LIMIT",
            Self::ConcurrencyLimit => "BASH_CONCURRENCY_LIMIT",
            Self::SpawnFailed => "BASH_SPAWN_FAILED",
            Self::Cancelled => "BASH_CANCELLED",
            Self::ProcessFailed => "BASH_PROCESS_FAILED",
        }
    }
}

struct ActiveInvocations {
    entries: Mutex<HashMap<u64, Arc<super::process_tree::ProcessTreeControl>>>,
}

impl ActiveInvocations {
    fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn lock(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<u64, Arc<super::process_tree::ProcessTreeControl>>> {
        match self.entries.lock() {
            Ok(entries) => entries,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn insert(&self, id: u64, control: Arc<super::process_tree::ProcessTreeControl>) {
        self.lock().insert(id, control);
    }

    fn remove(&self, id: u64) {
        self.lock().remove(&id);
    }

    fn terminate_all(&self) {
        let controls = self.lock().values().cloned().collect::<Vec<_>>();
        for control in controls {
            let _ = control.terminate();
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.lock().len()
    }
}

struct ActiveRegistration {
    active: Arc<ActiveInvocations>,
    id: u64,
}

impl Drop for ActiveRegistration {
    fn drop(&mut self) {
        self.active.remove(self.id);
    }
}

/// Cancels an invocation-local token when its request future is dropped.
struct CancelOnDrop(CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

struct ReaderTaskGuard {
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for ReaderTaskGuard {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[derive(Debug)]
struct CapturedOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

async fn cleanup_after_termination(
    owner: &ProcessTreeOwner,
    stdout_rx: &mut oneshot::Receiver<io::Result<Vec<u8>>>,
    stderr_rx: &mut oneshot::Receiver<io::Result<Vec<u8>>>,
) -> Result<CapturedOutput, ()> {
    let capture_result = tokio::time::timeout(CLEANUP_TIMEOUT, async {
        owner.wait().await.map_err(|_| ())?;
        let stdout = (&mut *stdout_rx).await.map_err(|_| ())?.map_err(|_| ())?;
        let stderr = (&mut *stderr_rx).await.map_err(|_| ())?.map_err(|_| ())?;
        Ok::<CapturedOutput, ()>(CapturedOutput { stdout, stderr })
    })
    .await;
    let quiescence_result = owner.wait_for_quiescence(CLEANUP_TIMEOUT).await;
    if quiescence_result.is_err() {
        return Err(());
    }
    match capture_result {
        Ok(result) => result,
        Err(_) => Err(()),
    }
}

async fn cleanup_owner_without_readers(owner: &ProcessTreeOwner) -> Result<(), ()> {
    if owner.terminate().is_err() {
        return Err(());
    }
    tokio::time::timeout(CLEANUP_TIMEOUT, owner.wait())
        .await
        .map_err(|_| ())
        .and_then(|result| result.map_err(|_| ()))?;
    owner
        .wait_for_quiescence(CLEANUP_TIMEOUT)
        .await
        .map_err(|_| ())
}

/// Validate the user command against the frozen M5 hard boundary.  This free
/// function is intentionally independent of Runtime readiness so Pipeline can
/// reject malformed requests before mode/capability evaluation.
pub(crate) fn validate_command(command: &str) -> Result<(), BashRuntimeError> {
    if command.trim().is_empty() || command.contains('\0') {
        return Err(BashRuntimeError::InvalidCommand);
    }
    if command.as_bytes().len() > MAX_COMMAND_BYTES {
        return Err(BashRuntimeError::CommandTooLong);
    }
    Ok(())
}

/// One immutable generation-owned Bash executor.
pub(crate) struct BashRuntime {
    executable: Option<PathBuf>,
    workspace_root: PathBuf,
    runtime_cancel: CancellationToken,
    semaphore: Arc<Semaphore>,
    active: Arc<ActiveInvocations>,
    next_invocation: AtomicU64,
    limits: BashLimits,
}

impl BashRuntime {
    pub(crate) fn new(executable: Option<PathBuf>, workspace_root: PathBuf) -> Self {
        Self::with_limits(executable, workspace_root, BashLimits::production())
    }

    #[cfg(test)]
    pub(crate) fn with_test_limits(
        executable: Option<PathBuf>,
        workspace_root: PathBuf,
        limits: BashLimits,
    ) -> Self {
        Self::with_limits(executable, workspace_root, limits)
    }

    fn with_limits(
        executable: Option<PathBuf>,
        workspace_root: PathBuf,
        limits: BashLimits,
    ) -> Self {
        Self {
            executable,
            workspace_root,
            runtime_cancel: CancellationToken::new(),
            semaphore: Arc::new(Semaphore::new(limits.max_concurrent)),
            active: Arc::new(ActiveInvocations::new()),
            next_invocation: AtomicU64::new(1),
            limits,
        }
    }

    /// Runtime stop is synchronous at the control boundary: signal all
    /// waiters and terminate every currently owned Job immediately.  The
    /// MCP thread then awaits each invocation's bounded cleanup path.
    pub(crate) fn stop(&self) {
        self.runtime_cancel.cancel();
        self.active.terminate_all();
    }

    #[cfg(test)]
    pub(crate) fn active_count(&self) -> usize {
        self.active.len()
    }

    #[cfg(test)]
    pub(crate) fn active_job_pids(&self) -> Vec<Vec<u32>> {
        self.active
            .lock()
            .values()
            .filter_map(|control| control.process_ids().ok())
            .collect()
    }

    pub(crate) fn generation_readiness(&self) -> Result<(), BashRuntimeError> {
        let executable = self
            .executable
            .as_ref()
            .ok_or(BashRuntimeError::Unavailable)?;
        let metadata =
            std::fs::symlink_metadata(executable).map_err(|_| BashRuntimeError::Unavailable)?;
        if !metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || is_reparse_point(&metadata)
        {
            return Err(BashRuntimeError::Unavailable);
        }
        let canonical =
            std::fs::canonicalize(executable).map_err(|_| BashRuntimeError::Unavailable)?;
        if !same_path_identity(&canonical, executable) {
            return Err(BashRuntimeError::Unavailable);
        }
        let workspace_metadata = std::fs::symlink_metadata(&self.workspace_root)
            .map_err(|_| BashRuntimeError::Unavailable)?;
        if !workspace_metadata.is_dir()
            || workspace_metadata.file_type().is_symlink()
            || is_reparse_point(&workspace_metadata)
        {
            return Err(BashRuntimeError::Unavailable);
        }
        let canonical_workspace = std::fs::canonicalize(&self.workspace_root)
            .map_err(|_| BashRuntimeError::Unavailable)?;
        if !same_path_identity(&canonical_workspace, &self.workspace_root) {
            return Err(BashRuntimeError::Unavailable);
        }
        if self.runtime_cancel.is_cancelled() {
            return Err(BashRuntimeError::Cancelled);
        }
        Ok(())
    }

    /// Execute the original command with generation-owned cwd and executable.
    /// The permit has no public constructor; accepting it here preserves the
    /// Pipeline-only execution boundary for Bash.
    pub(crate) async fn execute(
        &self,
        permit: &ExecutionPermit,
        request: BashRequest,
        request_cancel: CancellationToken,
    ) -> Result<BashOutput, BashRuntimeError> {
        let _permit = permit;
        validate_command(&request.command)?;
        self.generation_readiness()?;
        let executable = self
            .executable
            .as_deref()
            .ok_or(BashRuntimeError::Unavailable)?;
        if self.runtime_cancel.is_cancelled() || request_cancel.is_cancelled() {
            return Err(BashRuntimeError::Cancelled);
        }

        let _concurrency = self
            .semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| BashRuntimeError::ConcurrencyLimit)?;

        let command = self.command(executable, &request.command);
        let mut owner = ProcessTreeOwner::spawn_suspended(command)
            .await
            .map_err(|error| {
                let _ = error;
                BashRuntimeError::SpawnFailed
            })?;
        let invocation_id = self.next_invocation.fetch_add(1, Ordering::Relaxed);
        self.active.insert(invocation_id, owner.control());
        let _active_registration = ActiveRegistration {
            active: Arc::clone(&self.active),
            id: invocation_id,
        };

        if self.runtime_cancel.is_cancelled() || request_cancel.is_cancelled() {
            if cleanup_owner_without_readers(&owner).await.is_err() {
                return Err(BashRuntimeError::ProcessFailed);
            }
            return Err(BashRuntimeError::Cancelled);
        }
        if owner.resume().is_err() {
            if cleanup_owner_without_readers(&owner).await.is_err() {
                return Err(BashRuntimeError::ProcessFailed);
            }
            return Err(BashRuntimeError::SpawnFailed);
        }
        if self.runtime_cancel.is_cancelled() || request_cancel.is_cancelled() {
            if cleanup_owner_without_readers(&owner).await.is_err() {
                return Err(BashRuntimeError::ProcessFailed);
            }
            return Err(BashRuntimeError::Cancelled);
        }

        let stdout = owner.take_stdout().ok_or(BashRuntimeError::ProcessFailed)?;
        let stderr = owner.take_stderr().ok_or(BashRuntimeError::ProcessFailed)?;
        let output_cancel = CancellationToken::new();
        let limiter = OutputLimiter::new(self.limits.output_bytes, output_cancel.clone());
        let reader_error_cancel = CancellationToken::new();
        let reader_error = Arc::new(Mutex::new(None::<io::ErrorKind>));
        let (stdout_tx, mut stdout_rx) = oneshot::channel();
        let (stderr_tx, mut stderr_rx) = oneshot::channel();
        let stdout_error = Arc::clone(&reader_error);
        let stdout_error_cancel = reader_error_cancel.clone();
        let stdout_limiter = limiter.clone();
        let stdout_task = tokio::spawn(async move {
            let result = stdout_limiter.drain(stdout, OutputStream::Stdout).await;
            if let Err(error) = &result {
                if let Ok(mut slot) = stdout_error.lock() {
                    *slot = Some(error.kind());
                }
                stdout_error_cancel.cancel();
            }
            let _ = stdout_tx.send(result);
        });
        let stderr_error = Arc::clone(&reader_error);
        let stderr_error_cancel = reader_error_cancel.clone();
        let stderr_limiter = limiter.clone();
        let stderr_task = tokio::spawn(async move {
            let result = stderr_limiter.drain(stderr, OutputStream::Stderr).await;
            if let Err(error) = &result {
                if let Ok(mut slot) = stderr_error.lock() {
                    *slot = Some(error.kind());
                }
                stderr_error_cancel.cancel();
            }
            let _ = stderr_tx.send(result);
        });
        let _reader_tasks = ReaderTaskGuard {
            tasks: vec![stdout_task, stderr_task],
        };
        let invocation_cancel = CancellationToken::new();
        let _cancel_on_drop = CancelOnDrop(invocation_cancel.clone());
        let started = std::time::Instant::now();
        let timeout = tokio::time::sleep(self.limits.timeout);
        tokio::pin!(timeout);

        enum Completion {
            Parent(Result<ExitStatus, super::process_tree::ProcessTreeError>),
            Abort(BashRuntimeError),
        }
        let completion = {
            let parent_wait = owner.wait();
            tokio::pin!(parent_wait);
            tokio::select! {
                result = &mut parent_wait => Completion::Parent(result),
                _ = &mut timeout => Completion::Abort(BashRuntimeError::Timeout),
                _ = output_cancel.cancelled() => Completion::Abort(BashRuntimeError::OutputLimit),
                _ = reader_error_cancel.cancelled() => Completion::Abort(BashRuntimeError::ProcessFailed),
                _ = self.runtime_cancel.cancelled() => Completion::Abort(BashRuntimeError::Cancelled),
                _ = request_cancel.cancelled() => Completion::Abort(BashRuntimeError::Cancelled),
                _ = invocation_cancel.cancelled() => Completion::Abort(BashRuntimeError::Cancelled),
            }
        };

        let termination_error = owner.terminate().is_err();
        let captured = cleanup_after_termination(&owner, &mut stdout_rx, &mut stderr_rx).await;
        if termination_error || captured.is_err() {
            return Err(BashRuntimeError::ProcessFailed);
        }
        let captured = captured.expect("cleanup result checked");
        match completion {
            Completion::Abort(error) => Err(error),
            Completion::Parent(status) => {
                // A cancellation can make both parent-wait and token futures
                // ready in the same poll. Do not report a killed parent as a
                // normal nonzero shell exit merely because select chose it.
                if self.runtime_cancel.is_cancelled() || request_cancel.is_cancelled() {
                    return Err(BashRuntimeError::Cancelled);
                }
                let status = status.map_err(|_| BashRuntimeError::ProcessFailed)?;
                if limiter.is_limited() {
                    return Err(BashRuntimeError::OutputLimit);
                }
                let exit_code = status.code().ok_or(BashRuntimeError::ProcessFailed)?;
                Ok(BashOutput {
                    exit_code,
                    stdout: String::from_utf8_lossy(&captured.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&captured.stderr).into_owned(),
                    duration_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
                })
            }
        }
    }

    fn command(&self, executable: &Path, command: &str) -> Command {
        let mut process = Command::new(executable);
        process
            .args(["--noprofile", "--norc", "-c"])
            .arg(command)
            .current_dir(&self.workspace_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        neutralize_startup_environment(&mut process);
        process
    }
}

fn same_path_identity(actual: &Path, expected: &Path) -> bool {
    #[cfg(windows)]
    {
        fn display_path(path: &Path) -> String {
            let value = path.to_string_lossy();
            value
                .strip_prefix("\\\\?\\")
                .unwrap_or(&value)
                .replace('/', "\\")
                .to_ascii_lowercase()
        }
        display_path(actual) == display_path(expected)
    }
    #[cfg(not(windows))]
    {
        actual == expected
    }
}

fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

pub(crate) fn neutralize_startup_environment(process: &mut Command) {
    for key in startup_injection_env_keys(std::env::vars_os()) {
        process.env_remove(key);
    }
}

pub(crate) fn startup_injection_env_keys<I>(vars: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
{
    vars.into_iter()
        .filter_map(|(key, value)| {
            let key_text = key.to_string_lossy();
            let value_text = value.to_string_lossy();
            let old_style_function = value_text.trim_start().starts_with("() {");
            let key_is_function = key_text
                .get(..9)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("BASH_FUNC_"));
            let key_is_new_style_function = key_is_function && key_text.ends_with("%%");
            let startup = [
                "BASH_ENV",
                "ENV",
                "SHELLOPTS",
                "BASHOPTS",
                "PS4",
                "PROMPT_COMMAND",
                "BASH_XTRACEFD",
            ]
            .iter()
            .any(|name| key_text.eq_ignore_ascii_case(name));
            if startup || key_is_function || key_is_new_style_function || old_style_function {
                Some(key)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::ExecutionPermit;
    use harness_control_protocol::BashConfig;
    use std::fs;
    use std::time::Instant;

    struct TestWorkspace {
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "chatgpt-harness-m5-bash-runtime-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock")
                    .as_nanos()
            ));
            fs::create_dir_all(&root).expect("workspace");
            Self { root }
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn validated_git_bash() -> PathBuf {
        match super::super::discovery::resolve_bash(&BashConfig {
            auto_detect: true,
            executable_path: None,
        }) {
            super::super::discovery::BashExecutableState::Ready { canonical_path, .. } => {
                canonical_path
            }
            super::super::discovery::BashExecutableState::Unavailable { reason } => {
                panic!("validated Git Bash is required for Runtime tests: {reason}")
            }
        }
    }

    fn runtime(limits: BashLimits) -> (TestWorkspace, BashRuntime) {
        let executable = validated_git_bash();
        let workspace = TestWorkspace::new();
        let root = fs::canonicalize(&workspace.root).expect("canonical workspace");
        (
            workspace,
            BashRuntime::with_test_limits(Some(executable), root, limits),
        )
    }

    async fn execute(runtime: &BashRuntime, command: &str) -> Result<BashOutput, BashRuntimeError> {
        let permit = ExecutionPermit::for_executor_test();
        runtime
            .execute(
                &permit,
                BashRequest {
                    command: command.into(),
                },
                CancellationToken::new(),
            )
            .await
    }

    #[cfg(windows)]
    fn open_process_for_test(pid: u32) -> Option<windows_sys::Win32::Foundation::HANDLE> {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_INVALID_PARAMETER};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        };
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if handle.is_null() {
            assert_eq!(
                unsafe { GetLastError() },
                ERROR_INVALID_PARAMETER,
                "OpenProcess failed for active Bash PID {pid}"
            );
            None
        } else {
            Some(handle)
        }
    }

    #[cfg(windows)]
    async fn hold_active_job_handles(
        runtime: &BashRuntime,
        minimum: usize,
    ) -> Vec<(u32, windows_sys::Win32::Foundation::HANDLE)> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let mut pids = runtime
                .active_job_pids()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            pids.sort_unstable();
            pids.dedup();
            let mut handles = pids
                .into_iter()
                .filter_map(|pid| open_process_for_test(pid).map(|handle| (pid, handle)))
                .collect::<Vec<_>>();
            if handles.len() >= minimum {
                return handles;
            }
            close_process_handles(&mut handles);
            assert!(
                Instant::now() < deadline,
                "active Bash Job did not expose parent/child/grandchild PIDs"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[cfg(windows)]
    fn close_process_handles(handles: &mut Vec<(u32, windows_sys::Win32::Foundation::HANDLE)>) {
        use windows_sys::Win32::Foundation::CloseHandle;
        for (_, handle) in handles.drain(..) {
            unsafe { CloseHandle(handle) };
        }
    }

    #[cfg(windows)]
    fn assert_process_handles_exited(handles: Vec<(u32, windows_sys::Win32::Foundation::HANDLE)>) {
        use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
        for (pid, handle) in handles {
            assert_eq!(
                unsafe { WaitForSingleObject(handle, 3_000) },
                0,
                "Bash-owned PID {pid} remained alive"
            );
            let mut exit_code = 0_u32;
            assert_ne!(
                unsafe { GetExitCodeProcess(handle, &mut exit_code) },
                0,
                "could not query Bash-owned PID {pid}"
            );
            unsafe { CloseHandle(handle) };
            assert_ne!(
                exit_code, STILL_ACTIVE as u32,
                "Bash-owned PID {pid} remained active"
            );
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn simple_stdout_stderr_and_nonzero_exit_are_structured() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(2),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let output = execute(&runtime, "printf out; printf err >&2; exit 7")
            .await
            .expect("structured shell result");
        assert_eq!(output.exit_code, 7);
        assert_eq!(output.stdout, "out");
        assert_eq!(output.stderr, "err");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn cwd_is_the_generation_workspace() {
        let (workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(2),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let output = execute(&runtime, "pwd -W").await.expect("pwd");
        let actual = output.stdout.trim().replace('/', "\\");
        let expected = fs::canonicalize(workspace.root.clone())
            .expect("canonical workspace")
            .to_string_lossy()
            .replace('/', "\\");
        let expected = expected
            .strip_prefix("\\\\?\\")
            .unwrap_or(&expected)
            .to_owned();
        assert_eq!(actual.to_ascii_lowercase(), expected.to_ascii_lowercase());
    }

    #[tokio::test]
    async fn invalid_requests_are_rejected_before_spawn() {
        let workspace = TestWorkspace::new();
        let runtime = BashRuntime::with_test_limits(
            None,
            workspace.root.clone(),
            BashLimits::for_test(Duration::from_secs(1), 32),
        );
        assert_eq!(
            execute(&runtime, " ").await,
            Err(BashRuntimeError::InvalidCommand)
        );
        assert_eq!(
            execute(&runtime, "echo\0unsafe").await,
            Err(BashRuntimeError::InvalidCommand)
        );
        assert_eq!(
            execute(&runtime, &"x".repeat(MAX_COMMAND_BYTES + 1)).await,
            Err(BashRuntimeError::CommandTooLong)
        );
        let exact_utf8 = "é".repeat(MAX_COMMAND_BYTES / 'é'.len_utf8());
        assert_eq!(exact_utf8.as_bytes().len(), MAX_COMMAND_BYTES);
        assert_eq!(
            execute(&runtime, &exact_utf8).await,
            Err(BashRuntimeError::Unavailable)
        );
        let over_utf8 = format!("{exact_utf8}é");
        assert_eq!(
            execute(&runtime, &over_utf8).await,
            Err(BashRuntimeError::CommandTooLong)
        );
        assert_eq!(runtime.active_count(), 0);
    }

    #[test]
    fn frozen_runtime_limits_are_not_relaxed() {
        assert_eq!(MAX_COMMAND_BYTES, 8_192);
        assert_eq!(EXECUTION_TIMEOUT, Duration::from_secs(30));
        assert_eq!(MAX_CAPTURED_OUTPUT_BYTES, 262_144);
        assert_eq!(MAX_CONCURRENT_BASH, 1);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn timeout_terminates_invocation() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_millis(500),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let runtime = Arc::new(runtime);
        let task = tokio::spawn({
            let runtime = Arc::clone(&runtime);
            async move { execute(&runtime, "bash -c 'sleep 60' & wait").await }
        });
        let handles = hold_active_job_handles(&runtime, 3).await;
        let result = task.await.expect("timeout task");
        assert_eq!(result, Err(BashRuntimeError::Timeout));
        assert_process_handles_exited(handles);
        assert_eq!(runtime.active_count(), 0);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn output_limit_terminates_unbounded_writer() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(Duration::from_secs(2), 32));
        let runtime = Arc::new(runtime);
        let task = tokio::spawn({
            let runtime = Arc::clone(&runtime);
            async move { execute(&runtime, "bash -c 'sleep 60' & sleep 0.2; yes output").await }
        });
        let handles = hold_active_job_handles(&runtime, 3).await;
        let result = task.await.expect("output-limit task");
        assert_eq!(result, Err(BashRuntimeError::OutputLimit));
        assert_process_handles_exited(handles);
        assert_eq!(runtime.active_count(), 0);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn production_output_cap_terminates_tree() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(3),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let runtime = Arc::new(runtime);
        let task = tokio::spawn({
            let runtime = Arc::clone(&runtime);
            async move { execute(&runtime, "bash -c 'sleep 60' & sleep 0.2; yes output").await }
        });
        let handles = hold_active_job_handles(&runtime, 3).await;
        let result = task.await.expect("production output-limit task");
        assert_eq!(result, Err(BashRuntimeError::OutputLimit));
        assert_process_handles_exited(handles);
        assert_eq!(runtime.active_count(), 0);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn request_cancellation_terminates_process() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(2),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let cancellation = CancellationToken::new();
        let runtime = Arc::new(runtime);
        let task = tokio::spawn({
            let cancellation = cancellation.clone();
            let runtime = Arc::clone(&runtime);
            async move {
                let permit = ExecutionPermit::for_executor_test();
                runtime
                    .execute(
                        &permit,
                        BashRequest {
                            command: "bash -c 'sleep 60' & wait".into(),
                        },
                        cancellation,
                    )
                    .await
            }
        });
        let handles = hold_active_job_handles(&runtime, 3).await;
        cancellation.cancel();
        assert_eq!(
            task.await.expect("cancellation task"),
            Err(BashRuntimeError::Cancelled)
        );
        assert_process_handles_exited(handles);
        assert_eq!(runtime.active_count(), 0);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn second_invocation_fails_immediately_at_concurrency_limit() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(2),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let runtime = Arc::new(runtime);
        let first = tokio::spawn({
            let runtime = Arc::clone(&runtime);
            async move { execute(&runtime, "sleep 1").await }
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while runtime.active_count() == 0 {
            assert!(Instant::now() < deadline);
            tokio::task::yield_now().await;
        }
        let started = Instant::now();
        let second = execute(&runtime, "echo second").await;
        assert!(started.elapsed() < Duration::from_millis(200));
        assert_eq!(second, Err(BashRuntimeError::ConcurrencyLimit));
        runtime.stop();
        assert_eq!(
            first.await.expect("first task"),
            Err(BashRuntimeError::Cancelled)
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn invalid_utf8_is_lossy_and_output_streams_drain_concurrently() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(3),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let output = execute(
            &runtime,
            "printf '\\377'; head -c 100000 /dev/zero; head -c 100000 /dev/zero >&2",
        )
        .await
        .expect("bounded concurrent output");
        assert!(output.stdout.starts_with('�'));
        assert_eq!(output.stdout.chars().count(), 100001);
        assert_eq!(output.stderr.chars().count(), 100000);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn dropping_execution_future_removes_active_owner() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(10),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let runtime = Arc::new(runtime);
        let task = tokio::spawn({
            let runtime = Arc::clone(&runtime);
            async move { execute(&runtime, "bash -c 'sleep 60' & wait").await }
        });
        let handles = hold_active_job_handles(&runtime, 3).await;
        task.abort();
        let _ = task.await;
        assert_process_handles_exited(handles);
        let deadline = Instant::now() + Duration::from_secs(1);
        while runtime.active_count() != 0 {
            assert!(Instant::now() < deadline);
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn spawn_failure_is_sanitized() {
        let workspace = TestWorkspace::new();
        let fake_executable = workspace.root.join("fake-bash.exe");
        fs::write(&fake_executable, b"not a Windows executable").expect("fake executable");
        let runtime = BashRuntime::with_test_limits(
            Some(fake_executable),
            workspace.root.clone(),
            BashLimits::for_test(Duration::from_secs(1), 32),
        );
        assert_eq!(
            execute(&runtime, "echo secret").await,
            Err(BashRuntimeError::SpawnFailed)
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn normal_parent_exit_kills_background_tree_before_returning() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(3),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let runtime = Arc::new(runtime);
        let task = tokio::spawn({
            let runtime = Arc::clone(&runtime);
            async move { execute(&runtime, "bash -c 'sleep 60' & sleep 0.2; exit 0").await }
        });
        let handles = hold_active_job_handles(&runtime, 3).await;
        let output = task
            .await
            .expect("normal-exit task")
            .expect("normal result");
        assert_eq!(output.exit_code, 0);
        assert_process_handles_exited(handles);
        assert_eq!(runtime.active_count(), 0);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn runtime_stop_terminates_parent_child_and_grandchild() {
        let (_workspace, runtime) = runtime(BashLimits::for_test(
            Duration::from_secs(10),
            MAX_CAPTURED_OUTPUT_BYTES,
        ));
        let runtime = Arc::new(runtime);
        let task = tokio::spawn({
            let runtime = Arc::clone(&runtime);
            async move { execute(&runtime, "bash -c 'sleep 60' & wait").await }
        });
        let handles = hold_active_job_handles(&runtime, 3).await;
        runtime.stop();
        assert_eq!(
            task.await.expect("runtime-stop task"),
            Err(BashRuntimeError::Cancelled)
        );
        assert_process_handles_exited(handles);
        assert_eq!(runtime.active_count(), 0);
    }

    #[test]
    fn startup_environment_projection_removes_known_injection_entries_only() {
        let keys = startup_injection_env_keys([
            ("BASH_ENV".into(), "profile".into()),
            ("ENV".into(), "profile".into()),
            ("SHELLOPTS".into(), "xtrace".into()),
            ("BASHOPTS".into(), "expand_aliases".into()),
            ("PS4".into(), "$(whoami)".into()),
            ("PROMPT_COMMAND".into(), "echo bad".into()),
            ("BASH_XTRACEFD".into(), "2".into()),
            ("BASH_FUNC_bad%%".into(), "() { echo bad; }".into()),
            ("legacy".into(), "() { echo bad; }".into()),
            ("PATH".into(), "safe".into()),
            ("SAFE_VALUE".into(), "() text".into()),
        ]);
        let keys = keys
            .into_iter()
            .map(|key| key.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(keys.contains(&"BASH_ENV".into()));
        assert!(keys.contains(&"ENV".into()));
        assert!(keys.contains(&"SHELLOPTS".into()));
        assert!(keys.contains(&"BASHOPTS".into()));
        assert!(keys.contains(&"PS4".into()));
        assert!(keys.contains(&"PROMPT_COMMAND".into()));
        assert!(keys.contains(&"BASH_XTRACEFD".into()));
        assert!(keys.contains(&"BASH_FUNC_bad%%".into()));
        assert!(keys.contains(&"legacy".into()));
        assert!(!keys.contains(&"PATH".into()));
        assert!(!keys.contains(&"SAFE_VALUE".into()));
    }
}
