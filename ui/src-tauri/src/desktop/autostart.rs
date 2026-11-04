use crate::{commands::ControlError, config::ConfigError};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Outcome {
    #[default]
    NotEvaluated,
    Evaluating,
    Disabled,
    Pending,
    Attempting,
    Succeeded,
    Failed,
    Skipped,
}

/// Process-local observation only. HarnessController remains the lifecycle authority.
#[derive(Clone, Default)]
pub struct AutoStartCoordinator {
    outcome: Arc<Mutex<Outcome>>,
}

impl AutoStartCoordinator {
    #[cfg(test)]
    pub(super) fn outcome(&self) -> Outcome {
        *self.outcome.lock().unwrap()
    }
    /// Claim the one launch evaluation before touching the configuration.
    pub fn evaluate(&self, read: impl FnOnce() -> Result<Option<bool>, ConfigError>) -> bool {
        {
            let Ok(mut outcome) = self.outcome.lock() else {
                return false;
            };
            if *outcome != Outcome::NotEvaluated {
                return false;
            }
            *outcome = Outcome::Evaluating;
        }
        let next = match read() {
            Ok(Some(true)) => Outcome::Pending,
            Ok(Some(false) | None) => Outcome::Disabled,
            Err(_) => Outcome::Failed,
        };
        if let Ok(mut outcome) = self.outcome.lock() {
            *outcome = next;
            next == Outcome::Pending
        } else {
            false
        }
    }

    /// Execute at most one standard Host request, including when called concurrently.
    pub fn attempt<T>(&self, request: impl FnOnce() -> Result<T, ControlError>) {
        {
            let Ok(mut outcome) = self.outcome.lock() else {
                return;
            };
            if *outcome != Outcome::Pending {
                return;
            }
            *outcome = Outcome::Attempting;
        }
        let terminal = match request() {
            Ok(_) => Outcome::Succeeded,
            Err(error) if error.code == "LIFECYCLE_CONFLICT" => Outcome::Skipped,
            Err(_) => Outcome::Failed,
        };
        if let Ok(mut outcome) = self.outcome.lock() {
            *outcome = terminal;
        }
    }
}

/// Called once after the Host's managed states and status sink are installed.
pub fn schedule(app: &AppHandle, coordinator: &AutoStartCoordinator) {
    if coordinator.evaluate(|| {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| ConfigError::Io("configuration location unavailable".into()))?;
        crate::config::ConfigStore::new(directory)
            .inspect_read_only()
            .map(|config| config.map(|config| config.desktop.auto_start_harness_on_app_launch))
    }) {
        let app = app.clone();
        let coordinator = coordinator.clone();
        std::thread::spawn(move || {
            coordinator.attempt(|| {
                app.state::<super::DesktopLifecycleCoordinator>()
                    .dispatch_automatic_start(
                        &app,
                        &app.state::<Mutex<crate::commands::BackendState>>(),
                    )
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Barrier,
    };

    #[test]
    fn disabled_missing_and_invalid_config_never_request_start() {
        for input in [
            Ok(None),
            Ok(Some(false)),
            Err(ConfigError::Invalid("invalid".into())),
        ] {
            let coordinator = AutoStartCoordinator::default();
            assert!(!coordinator.evaluate(|| input));
            coordinator.attempt(|| -> Result<(), ControlError> { panic!("unexpected Start") });
            assert!(!coordinator.evaluate(|| Ok(Some(true))));
        }
    }

    #[test]
    fn enabled_is_claimed_once_even_with_concurrent_workers() {
        let coordinator = AutoStartCoordinator::default();
        assert!(coordinator.evaluate(|| Ok(Some(true))));
        assert!(!coordinator.evaluate(|| panic!("preference read twice")));
        let count = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(3));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let coordinator = coordinator.clone();
                let count = count.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    coordinator.attempt(|| {
                        count.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    });
                })
            })
            .collect();
        barrier.wait();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(*coordinator.outcome.lock().unwrap(), Outcome::Succeeded);
    }

    #[test]
    fn failure_and_conflict_are_terminal_without_retry() {
        for (code, expected) in [
            ("LIFECYCLE_CONFLICT", Outcome::Skipped),
            ("CONFIG_INVALID", Outcome::Failed),
        ] {
            let coordinator = AutoStartCoordinator::default();
            assert!(coordinator.evaluate(|| Ok(Some(true))));
            coordinator.attempt(|| -> Result<(), ControlError> {
                Err(ControlError {
                    code: code.into(),
                    message: "ignored".into(),
                })
            });
            assert_eq!(*coordinator.outcome.lock().unwrap(), expected);
            coordinator.attempt(|| -> Result<(), ControlError> { panic!("retried") });
        }
    }
}
