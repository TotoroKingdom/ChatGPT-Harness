use crate::{
    commands::BackendState,
    harness::{HarnessController, HarnessLifecycle},
};
use harness_control_protocol::StartupAttempt;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const ONCE_ARG: &str = "--startup-benchmark-once";
const WARM_ARG: &str = "--startup-benchmark-warm";
const OUTPUT_ARG: &str = "--startup-benchmark-output";
const WARM_ATTEMPTS_ARG: &str = "--startup-benchmark-warm-attempts";
const READY_TIMEOUT: Duration = Duration::from_secs(120);
const STOP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BenchmarkMode {
    Once,
    Warm,
}

#[derive(Debug)]
struct BenchmarkRequest {
    mode: BenchmarkMode,
    output: PathBuf,
    warm_attempts: usize,
}

pub(crate) fn maybe_spawn(app: &AppHandle) -> bool {
    let args: Vec<String> = env::args().collect();
    let Some(request) = parse_request(&args) else {
        return false;
    };
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        let result = run(app.clone(), request);
        if let Err(error) = result {
            eprintln!("startup benchmark failed: {error}");
            app.exit(1);
        } else {
            app.exit(0);
        }
    });
    true
}

fn parse_request(args: &[String]) -> Option<BenchmarkRequest> {
    let mode = if args.iter().any(|arg| arg == ONCE_ARG) {
        BenchmarkMode::Once
    } else if args.iter().any(|arg| arg == WARM_ARG) {
        BenchmarkMode::Warm
    } else {
        return None;
    };
    let output = argument_value(args, OUTPUT_ARG)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::temp_dir().join(format!(
                "chatgpt-harness-startup-benchmark-{}.ndjson",
                Uuid::new_v4()
            ))
        });
    let warm_attempts = argument_value(args, WARM_ATTEMPTS_ARG)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(32);
    Some(BenchmarkRequest {
        mode,
        output,
        warm_attempts,
    })
}

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|values| values[0] == name)
        .map(|values| values[1].clone())
}

fn run(app: AppHandle, request: BenchmarkRequest) -> Result<(), String> {
    validate_output_path(&request.output)?;
    if let Some(parent) = request.output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("benchmark output directory: {error}"))?;
    }
    match request.mode {
        BenchmarkMode::Once => run_once(&app, &request.output),
        BenchmarkMode::Warm => run_warm(&app, &request.output, request.warm_attempts),
    }
}

fn run_once(app: &AppHandle, output: &Path) -> Result<(), String> {
    let harness = app.state::<HarnessController>().inner().clone();
    let backend = app.state::<Mutex<BackendState>>();
    let _ = harness.start(app, backend.inner());
    let _ = wait_for_ready(&harness);
    append_latest(&harness, output)?;
    stop_and_wait(app, &harness)
}

fn run_warm(app: &AppHandle, output: &Path, warm_attempts: usize) -> Result<(), String> {
    if warm_attempts == 0 {
        return Err("warm_attempts must be positive".into());
    }
    let harness = app.state::<HarnessController>().inner().clone();
    let backend = app.state::<Mutex<BackendState>>();

    // The first attempt bootstraps the app process and is cold. It is retained
    // so the analyzer can explicitly exclude it from the warm counted cohort.
    let _ = harness.start(app, backend.inner());
    let _ = wait_for_ready(&harness);
    append_latest(&harness, output)?;
    stop_and_wait(app, &harness)?;

    for index in 0..warm_attempts {
        thread::sleep(Duration::from_secs(2));
        let _ = harness.start(app, backend.inner());
        let _ = wait_for_ready(&harness);
        append_latest(&harness, output)?;
        stop_and_wait(app, &harness)?;
        if index + 1 == warm_attempts {
            break;
        }
    }
    Ok(())
}

fn wait_for_ready(harness: &HarnessController) -> bool {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        let status = harness.status();
        if status.lifecycle == HarnessLifecycle::Ready {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn stop_and_wait(app: &AppHandle, harness: &HarnessController) -> Result<(), String> {
    if harness.status().lifecycle != HarnessLifecycle::Stopped {
        let _ = harness.stop(app);
    }
    let deadline = Instant::now() + STOP_TIMEOUT;
    while harness.status().lifecycle != HarnessLifecycle::Stopped && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    if harness.status().lifecycle == HarnessLifecycle::Stopped {
        Ok(())
    } else {
        Err("Harness did not reach Stopped before the next benchmark attempt".into())
    }
}

fn append_latest(harness: &HarnessController, output: &Path) -> Result<(), String> {
    let attempt: StartupAttempt = harness
        .latest_startup_attempt()
        .ok_or_else(|| "Host did not expose a latest StartupAttempt".to_owned())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(output)
        .map_err(|error| format!("benchmark output: {error}"))?;
    serde_json::to_writer(&mut file, &attempt)
        .map_err(|error| format!("serialize StartupAttempt: {error}"))?;
    file.write_all(b"\n")
        .map_err(|error| format!("write StartupAttempt: {error}"))?;
    file.flush()
        .map_err(|error| format!("flush StartupAttempt: {error}"))
}

fn validate_output_path(path: &Path) -> Result<(), String> {
    let temp_root = fs::canonicalize(env::temp_dir())
        .map_err(|error| format!("benchmark temp root: {error}"))?;
    let candidate_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        temp_root.join(path)
    };
    let parent = candidate_path
        .parent()
        .ok_or_else(|| "benchmark output must name a file".to_owned())?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|error| format!("benchmark output directory is unavailable: {error}"))?;
    let file_name = candidate_path
        .file_name()
        .ok_or_else(|| "benchmark output must name a file".to_owned())?;
    let candidate = canonical_parent.join(file_name);
    if !candidate.starts_with(&temp_root) {
        return Err("benchmark output must remain under the system temp directory".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_once_mode_with_default_warm_count() {
        let request = parse_request(&[
            "chatgpt-harness-ui.exe".into(),
            ONCE_ARG.into(),
            OUTPUT_ARG.into(),
            env::temp_dir()
                .join("startup-once.ndjson")
                .to_string_lossy()
                .into_owned(),
        ])
        .expect("benchmark request");

        assert_eq!(request.mode, BenchmarkMode::Once);
        assert_eq!(request.warm_attempts, 32);
    }

    #[test]
    fn parses_warm_mode_count_without_changing_the_cohort_contract() {
        let request = parse_request(&[
            "chatgpt-harness-ui.exe".into(),
            WARM_ARG.into(),
            WARM_ATTEMPTS_ARG.into(),
            "30".into(),
        ])
        .expect("benchmark request");

        assert_eq!(request.mode, BenchmarkMode::Warm);
        assert_eq!(request.warm_attempts, 30);
    }

    #[test]
    fn accepts_only_existing_temp_output_directories() {
        let directory = env::temp_dir();
        let output = directory.join(format!("startup-benchmark-{}.ndjson", Uuid::new_v4()));
        assert!(validate_output_path(&output).is_ok());
    }

    #[test]
    fn rejects_output_outside_temp_root() {
        let outside = if cfg!(windows) {
            PathBuf::from(r"C:\Windows\Temp\startup-benchmark.ndjson")
        } else {
            PathBuf::from("/var/tmp/startup-benchmark.ndjson")
        };
        if outside.starts_with(env::temp_dir()) {
            return;
        }
        assert!(validate_output_path(&outside).is_err());
    }
}
