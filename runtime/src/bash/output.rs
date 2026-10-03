//! Bounded concurrent stdout/stderr capture for one Bash invocation.
//!
//! The limit is enforced on raw bytes before UTF-8 decoding.  The two pipe
//! readers share one atomic counter, so a producer cannot bypass the limit by
//! writing to the other stream.

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::sync::CancellationToken;

/// M5 hard cap for stdout plus stderr, in raw bytes.
pub(crate) const MAX_CAPTURED_OUTPUT_BYTES: usize = 262_144;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
struct OutputState {
    total: AtomicUsize,
    limited: AtomicBool,
    limit: usize,
    limit_cancel: CancellationToken,
}

/// Shared state used by both pipe readers.
#[derive(Clone, Debug)]
pub(crate) struct OutputLimiter {
    state: Arc<OutputState>,
}

impl OutputLimiter {
    pub(crate) fn new(limit: usize, limit_cancel: CancellationToken) -> Self {
        Self {
            state: Arc::new(OutputState {
                total: AtomicUsize::new(0),
                limited: AtomicBool::new(false),
                limit,
                limit_cancel,
            }),
        }
    }

    pub(crate) fn is_limited(&self) -> bool {
        self.state.limited.load(Ordering::Acquire)
    }

    pub(crate) fn total_bytes(&self) -> usize {
        self.state.total.load(Ordering::Acquire)
    }

    /// Drain one stream until EOF or until the combined raw-byte limit is
    /// reached.  The caller terminates the owned process tree after the
    /// cancellation token is observed.
    pub(crate) async fn drain<R>(
        &self,
        mut reader: R,
        _stream: OutputStream,
    ) -> std::io::Result<Vec<u8>>
    where
        R: AsyncRead + Unpin,
    {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let count = reader.read(&mut buffer).await?;
            if count == 0 {
                return Ok(output);
            }

            let previous = self.state.total.fetch_add(count, Ordering::AcqRel);
            let allowed = self.state.limit.saturating_sub(previous).min(count);
            if allowed != 0 {
                output.extend_from_slice(&buffer[..allowed]);
            }
            if previous.saturating_add(count) > self.state.limit {
                self.state.limited.store(true, Ordering::Release);
                self.state.limit_cancel.cancel();
                return Ok(output);
            }
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct CapturedOutput {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

impl CapturedOutput {
    pub(crate) fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub(crate) fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::BufReader;

    struct FailingReader;

    impl AsyncRead for FailingReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            _buffer: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let _ = self;
            Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "test reader failure",
            )))
        }
    }

    #[tokio::test]
    async fn combined_limit_is_shared_by_stdout_and_stderr() {
        let cancel = CancellationToken::new();
        let limiter = OutputLimiter::new(5, cancel.clone());
        let stdout = limiter
            .drain(BufReader::new(&b"abc"[..]), OutputStream::Stdout)
            .await
            .expect("stdout drain");
        let stderr = limiter
            .drain(BufReader::new(&b"def"[..]), OutputStream::Stderr)
            .await
            .expect("stderr drain");

        assert_eq!(stdout, b"abc");
        assert_eq!(stderr, b"de");
        assert_eq!(limiter.total_bytes(), 6);
        assert!(limiter.is_limited());
        assert!(cancel.is_cancelled());
    }

    #[test]
    fn invalid_utf8_uses_stable_lossy_replacement() {
        let output = CapturedOutput {
            stdout: vec![b'a', 0xff, b'b'],
            stderr: vec![0xfe],
        };
        assert_eq!(output.stdout_text(), "a�b");
        assert_eq!(output.stderr_text(), "�");
    }

    #[tokio::test]
    async fn exact_limit_does_not_trigger_overflow() {
        let cancel = CancellationToken::new();
        let limiter = OutputLimiter::new(3, cancel.clone());
        let output = limiter
            .drain(BufReader::new(&b"abc"[..]), OutputStream::Stdout)
            .await
            .expect("drain");
        assert_eq!(output, b"abc");
        assert!(!limiter.is_limited());
        assert!(!cancel.is_cancelled());
    }

    #[tokio::test]
    async fn reader_errors_are_returned_without_waiting_for_process_exit() {
        let limiter = OutputLimiter::new(32, CancellationToken::new());
        let error = limiter
            .drain(FailingReader, OutputStream::Stdout)
            .await
            .expect_err("reader failure must be visible to runner");
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    }
}
