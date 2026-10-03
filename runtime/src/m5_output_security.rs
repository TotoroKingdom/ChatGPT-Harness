use crate::bash::output::{OutputLimiter, OutputStream};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simultaneous_pipe_readers_share_one_raw_byte_budget() {
    let cancel = CancellationToken::new();
    let limiter = OutputLimiter::new(4096, cancel.clone());
    let (stdout_read, mut stdout_write) = tokio::io::duplex(128);
    let (stderr_read, mut stderr_write) = tokio::io::duplex(128);
    let stdout_limiter = limiter.clone();
    let stderr_limiter = limiter.clone();
    let stdout = tokio::spawn(async move {
        stdout_limiter
            .drain(stdout_read, OutputStream::Stdout)
            .await
            .unwrap()
    });
    let stderr = tokio::spawn(async move {
        stderr_limiter
            .drain(stderr_read, OutputStream::Stderr)
            .await
            .unwrap()
    });
    let stdout_producer = tokio::spawn(async move {
        for _ in 0..100 {
            if stdout_write.write_all(&[0xff; 127]).await.is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    });
    let stderr_producer = tokio::spawn(async move {
        for _ in 0..100 {
            if stderr_write.write_all(&[0xfe; 127]).await.is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    });
    let (stdout_bytes, stderr_bytes) =
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            let stdout_bytes = stdout.await.unwrap();
            let stderr_bytes = stderr.await.unwrap();
            stdout_producer.await.unwrap();
            stderr_producer.await.unwrap();
            (stdout_bytes, stderr_bytes)
        })
        .await
        .expect("combined output readers deadlocked");
    assert_eq!(stdout_bytes.len() + stderr_bytes.len(), 4096);
    assert!(limiter.total_bytes() > 4096);
    assert!(limiter.is_limited());
    assert!(cancel.is_cancelled());
}

#[tokio::test]
async fn raw_limit_is_enforced_before_lossy_text_expansion() {
    let cancel = CancellationToken::new();
    let limiter = OutputLimiter::new(3, cancel.clone());
    let raw = limiter
        .drain(&[0xff, 0xfe, 0xfd][..], OutputStream::Stdout)
        .await
        .unwrap();
    assert_eq!(raw.len(), 3);
    assert_eq!(String::from_utf8_lossy(&raw), "���");
    assert_eq!(limiter.total_bytes(), 3);
    assert!(!cancel.is_cancelled());
}
