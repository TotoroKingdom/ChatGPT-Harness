//! Runtime-owned process tree ownership.
//!
//! On Windows every invocation is created suspended, assigned to its own Job
//! Object, and only then resumed.  This closes the spawn/assign window in
//! which a shell could otherwise create an unowned descendant.

use std::fmt;
use std::io;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::Mutex;

#[derive(Debug)]
pub(crate) enum ProcessTreeError {
    Spawn(io::Error),
    Ownership(io::Error),
    Resume(io::Error),
    Wait(io::Error),
    Cleanup(io::Error),
}

impl fmt::Display for ProcessTreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(_) => formatter.write_str("process could not be started"),
            Self::Ownership(_) => formatter.write_str("process ownership could not be established"),
            Self::Resume(_) => formatter.write_str("owned process could not be resumed"),
            Self::Wait(_) => formatter.write_str("owned process wait failed"),
            Self::Cleanup(_) => formatter.write_str("owned process tree cleanup failed"),
        }
    }
}

impl std::error::Error for ProcessTreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error)
            | Self::Ownership(error)
            | Self::Resume(error)
            | Self::Wait(error)
            | Self::Cleanup(error) => Some(error),
        }
    }
}

/// A process-tree cancellation handle retained by the Runtime active set.
///
/// The concrete Windows handle is intentionally private.  Callers can only
/// terminate the entire invocation-owned tree; they cannot target an
/// arbitrary PID through this type.
#[derive(Debug)]
pub(crate) struct ProcessTreeControl {
    #[cfg(windows)]
    job: WindowsJob,
}

impl ProcessTreeControl {
    #[cfg(windows)]
    fn new() -> Result<Self, ProcessTreeError> {
        Ok(Self {
            job: WindowsJob::new().map_err(ProcessTreeError::Ownership)?,
        })
    }

    #[cfg(not(windows))]
    fn new() -> Result<Self, ProcessTreeError> {
        Err(ProcessTreeError::Ownership(io::Error::new(
            io::ErrorKind::Unsupported,
            "Bash process-tree ownership is only supported on Windows",
        )))
    }

    #[cfg(windows)]
    fn assign(&self, process: std::os::windows::io::RawHandle) -> io::Result<()> {
        self.job.assign(process)
    }

    pub(crate) fn terminate(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            self.job.terminate()
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    }

    #[cfg(all(windows, test))]
    pub(crate) fn process_ids(&self) -> io::Result<Vec<u32>> {
        self.job.process_ids()
    }

    #[cfg(all(not(windows), test))]
    pub(crate) fn process_ids(&self) -> io::Result<Vec<u32>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Bash process-tree ownership is only supported on Windows",
        ))
    }

    fn active_process_count(&self) -> io::Result<usize> {
        #[cfg(windows)]
        {
            self.job.active_process_count()
        }
        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Bash process-tree ownership is only supported on Windows",
            ))
        }
    }
}

/// One invocation and its owned process-tree control.
pub(crate) struct ProcessTreeOwner {
    child: Arc<Mutex<Child>>,
    control: Arc<ProcessTreeControl>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    pid: u32,
    resumed: bool,
    exit_status: Arc<Mutex<Option<ExitStatus>>>,
}

impl fmt::Debug for ProcessTreeOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessTreeOwner")
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

impl ProcessTreeOwner {
    /// Spawn, establish ownership, and resume the child.  Runtime execution
    /// uses [`Self::spawn_suspended`] so the owner can be registered and
    /// cancellation can be rechecked before user shell code starts.
    pub(crate) async fn spawn(command: Command) -> Result<Self, ProcessTreeError> {
        let mut owner = Self::spawn_suspended(command).await?;
        if let Err(error) = owner.resume() {
            let terminated = owner.terminate().is_ok();
            let waited = tokio::time::timeout(Duration::from_secs(2), owner.wait())
                .await
                .ok()
                .and_then(Result::ok)
                .is_some();
            let quiescent = owner
                .wait_for_quiescence(Duration::from_secs(2))
                .await
                .is_ok();
            if !terminated || !waited || !quiescent {
                return Err(ProcessTreeError::Cleanup(io::Error::new(
                    io::ErrorKind::Other,
                    "failed to clean up suspended process after resume failure",
                )));
            }
            return Err(error);
        }
        Ok(owner)
    }

    /// Spawn and assign a suspended child without running user shell code.
    /// The caller must register the returned Job control and call `resume`.
    pub(crate) async fn spawn_suspended(mut command: Command) -> Result<Self, ProcessTreeError> {
        command.kill_on_drop(true);
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};
            command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
        }

        // Creating the Job first makes the ownership transition explicit.  A
        // suspended child is still safe if assignment fails, but it is always
        // terminated on the error path below before the owner is returned.
        let control = Arc::new(ProcessTreeControl::new()?);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return Err(ProcessTreeError::Spawn(error)),
        };
        let pid = child.id().ok_or_else(|| {
            let error = io::Error::new(io::ErrorKind::Other, "spawned child has no process id");
            ProcessTreeError::Spawn(error)
        })?;

        #[cfg(windows)]
        {
            let assignment = {
                let raw = child.raw_handle().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::Other, "spawned child handle is unavailable")
                });
                raw.and_then(|raw| control.assign(raw))
            };
            if let Err(error) = assignment {
                let terminated = control.terminate().is_ok();
                let killed = child.kill().await.is_ok();
                let waited = child.wait().await.is_ok();
                if !terminated || !killed || !waited {
                    return Err(ProcessTreeError::Cleanup(io::Error::new(
                        io::ErrorKind::Other,
                        "failed to clean up unassigned suspended process",
                    )));
                }
                return Err(ProcessTreeError::Ownership(error));
            }
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            control,
            stdout,
            stderr,
            pid,
            resumed: false,
            exit_status: Arc::new(Mutex::new(None)),
        })
    }

    pub(crate) fn resume(&mut self) -> Result<(), ProcessTreeError> {
        if self.resumed {
            return Ok(());
        }
        #[cfg(windows)]
        resume_suspended_process(self.pid).map_err(ProcessTreeError::Resume)?;
        #[cfg(not(windows))]
        return Err(ProcessTreeError::Resume(io::Error::new(
            io::ErrorKind::Unsupported,
            "Bash process-tree ownership is only supported on Windows",
        )));
        self.resumed = true;
        Ok(())
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn control(&self) -> Arc<ProcessTreeControl> {
        Arc::clone(&self.control)
    }

    pub(crate) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.stdout.take()
    }

    pub(crate) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.stderr.take()
    }

    pub(crate) async fn wait(&self) -> Result<ExitStatus, ProcessTreeError> {
        let mut cached = self.exit_status.lock().await;
        if let Some(status) = cached.as_ref() {
            return Ok(status.clone());
        }
        let status = self
            .child
            .lock()
            .await
            .wait()
            .await
            .map_err(ProcessTreeError::Wait)?;
        *cached = Some(status.clone());
        Ok(status)
    }

    pub(crate) fn terminate(&self) -> io::Result<()> {
        self.control.terminate()
    }

    pub(crate) async fn wait_for_quiescence(
        &self,
        timeout: Duration,
    ) -> Result<(), ProcessTreeError> {
        let deadline = Instant::now() + timeout;
        loop {
            let active = self
                .control
                .active_process_count()
                .map_err(ProcessTreeError::Cleanup)?;
            if active == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ProcessTreeError::Cleanup(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "owned process Job remained active after termination",
                )));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

impl Drop for ProcessTreeOwner {
    fn drop(&mut self) {
        let _ = self.control.terminate();
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct WindowsJob {
    handle: windows_sys::Win32::Foundation::HANDLE,
    terminating: std::sync::atomic::AtomicBool,
}

#[cfg(windows)]
// SAFETY: the wrapper owns one kernel Job Object handle. Windows kernel
// handles are process-wide references and Job Object operations are safe from
// multiple threads; all mutation is serialized by the atomic termination
// guard. Drop closes this owned handle exactly once.
unsafe impl Send for WindowsJob {}
#[cfg(windows)]
unsafe impl Sync for WindowsJob {}

#[cfg(windows)]
impl WindowsJob {
    fn new() -> io::Result<Self> {
        use std::ptr::null;
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        let handle = unsafe { CreateJobObjectW(null(), null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) != 0
        };
        if !configured {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            handle,
            terminating: std::sync::atomic::AtomicBool::new(false),
        })
    }

    fn assign(&self, process: std::os::windows::io::RawHandle) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        let assigned = unsafe {
            AssignProcessToJobObject(
                self.handle,
                process as windows_sys::Win32::Foundation::HANDLE,
            ) != 0
        };
        if assigned {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn terminate(&self) -> io::Result<()> {
        use std::sync::atomic::Ordering;
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        if self.terminating.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let terminated = unsafe { TerminateJobObject(self.handle, 1) != 0 };
        if terminated {
            Ok(())
        } else {
            self.terminating.store(false, Ordering::Release);
            Err(io::Error::last_os_error())
        }
    }

    fn active_process_count(&self) -> io::Result<usize> {
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicAccountingInformation, QueryInformationJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        };
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let queried = unsafe {
            QueryInformationJobObject(
                self.handle,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            ) != 0
        };
        if queried {
            Ok(accounting.ActiveProcesses as usize)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(test)]
    fn process_ids(&self) -> io::Result<Vec<u32>> {
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicProcessIdList, QueryInformationJobObject, JOBOBJECT_BASIC_PROCESS_ID_LIST,
        };
        let capacity = 128usize;
        // Use usize storage so the cast is correctly aligned for the struct's
        // trailing usize process-id array.  A Vec<u8> would be an alignment
        // violation even when its byte length happened to be sufficient.
        let words = (std::mem::size_of::<JOBOBJECT_BASIC_PROCESS_ID_LIST>()
            + capacity * std::mem::size_of::<usize>())
        .div_ceil(std::mem::size_of::<usize>());
        let mut buffer = vec![0_usize; words];
        let mut returned = 0_u32;
        let queried = unsafe {
            QueryInformationJobObject(
                self.handle,
                JobObjectBasicProcessIdList,
                buffer.as_mut_ptr().cast(),
                (buffer.len() * std::mem::size_of::<usize>()) as u32,
                &mut returned,
            ) != 0
        };
        if !queried {
            return Err(io::Error::last_os_error());
        }
        let list = unsafe { &*(buffer.as_ptr() as *const JOBOBJECT_BASIC_PROCESS_ID_LIST) };
        let count = (list.NumberOfProcessIdsInList as usize).min(capacity);
        let ids = unsafe { std::slice::from_raw_parts(list.ProcessIdList.as_ptr(), count) };
        Ok(ids.iter().map(|id| *id as u32).collect())
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        let _ = self.terminate();
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
fn resume_suspended_process(pid: u32) -> io::Result<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut found = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    let mut resumed = false;
    while found {
        if entry.th32OwnerProcessID == pid {
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if !thread.is_null() {
                let result = unsafe { ResumeThread(thread) };
                unsafe { CloseHandle(thread) };
                if result != u32::MAX {
                    resumed = true;
                    break;
                }
            }
        }
        found = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    if resumed {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "suspended process thread was not found",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[cfg(windows)]
    fn cmd_path() -> PathBuf {
        std::env::var_os("ComSpec")
            .map(PathBuf::from)
            .expect("Windows command processor must be available")
    }

    #[cfg(windows)]
    fn descendant_command() -> Command {
        let mut command = Command::new(cmd_path());
        command.args([
            "/C",
            r#"start "" /B ping 127.0.0.1 -n 30 > NUL & ping 127.0.0.1 -n 30 > NUL"#,
        ]);
        command
    }

    #[cfg(windows)]
    async fn wait_for_descendant(control: &ProcessTreeControl) -> Vec<u32> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let ids = control.process_ids().expect("query Job process IDs");
            if ids.len() >= 2 {
                return ids;
            }
            assert!(
                Instant::now() < deadline,
                "descendant was not assigned to Job"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[cfg(windows)]
    fn open_pid(pid: u32) -> Option<windows_sys::Win32::Foundation::HANDLE> {
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
                "OpenProcess failed for PID {pid} while it may still be alive"
            );
            None
        } else {
            Some(handle)
        }
    }

    #[cfg(windows)]
    fn assert_pid_exited(pid: u32) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Foundation::STILL_ACTIVE;
        use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
        let Some(handle) = open_pid(pid) else {
            return;
        };
        let wait = unsafe { WaitForSingleObject(handle, 2_000) };
        assert_eq!(wait, 0, "owned descendant PID {pid} remained alive");
        let mut code = 0_u32;
        let queried = unsafe { GetExitCodeProcess(handle, &mut code) } != 0;
        unsafe { CloseHandle(handle) };
        assert!(queried);
        assert_ne!(code, STILL_ACTIVE as u32, "owned PID {pid} remained active");
    }

    #[cfg(windows)]
    fn assert_pid_handles_exited(handles: &[(u32, windows_sys::Win32::Foundation::HANDLE)]) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Foundation::STILL_ACTIVE;
        use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
        for (pid, handle) in handles {
            let wait = unsafe { WaitForSingleObject(*handle, 2_000) };
            assert_eq!(wait, 0, "owned descendant PID {pid} remained alive");
            let mut code = 0_u32;
            let queried = unsafe { GetExitCodeProcess(*handle, &mut code) } != 0;
            unsafe { CloseHandle(*handle) };
            assert!(queried);
            assert_ne!(code, STILL_ACTIVE as u32, "owned PID {pid} remained active");
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn job_termination_reaches_real_descendant_pids() {
        let owner = ProcessTreeOwner::spawn(descendant_command())
            .await
            .expect("spawn owned process");
        let control = owner.control();
        let pids = wait_for_descendant(&control).await;
        let handles = pids
            .iter()
            .filter_map(|pid| open_pid(*pid).map(|handle| (*pid, handle)))
            .collect::<Vec<_>>();
        owner.terminate().expect("terminate owned Job");
        owner.terminate().expect("idempotent terminate owned Job");
        let _ = owner.wait().await.expect("wait parent");
        owner
            .wait_for_quiescence(Duration::from_secs(2))
            .await
            .expect("owned Job quiescence");
        assert_pid_handles_exited(&handles);
        for pid in pids {
            assert_pid_exited(pid);
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn dropping_owner_kills_tree_and_does_not_touch_unrelated_process() {
        let owner = ProcessTreeOwner::spawn(descendant_command())
            .await
            .expect("spawn owned process");
        let control = owner.control();
        let pids = wait_for_descendant(&control).await;

        let unrelated = ProcessTreeOwner::spawn({
            let mut command = Command::new(cmd_path());
            command.args(["/C", "ping 127.0.0.1 -n 30 > NUL"]);
            command
        })
        .await
        .expect("spawn unrelated owned process");
        let unrelated_pid = unrelated.pid();
        let unrelated_control = unrelated.control();
        drop(owner);
        for pid in pids {
            assert_pid_exited(pid);
        }
        assert!(!unrelated_control
            .process_ids()
            .expect("query unrelated Job")
            .is_empty());
        drop(unrelated);
        assert_pid_exited(unrelated_pid);
    }
}
