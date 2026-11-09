use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Runtime-configurable wall-clock budget for one search invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SearchBudget {
    pub(crate) timeout: Duration,
}

impl SearchBudget {
    pub(crate) const DEFAULT: Self = Self {
        timeout: Duration::from_secs(30),
    };

    pub(crate) fn from_seconds(seconds: u32) -> Self {
        Self {
            timeout: Duration::from_secs(u64::from(seconds)),
        }
    }
}

impl Default for SearchBudget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The two independent configured budgets carried by one Runtime generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SearchBudgets {
    pub(crate) glob: SearchBudget,
    pub(crate) grep: SearchBudget,
}

impl Default for SearchBudgets {
    fn default() -> Self {
        Self {
            glob: SearchBudget::default(),
            grep: SearchBudget::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlError {
    Timeout,
    Cancelled,
}

/// Deadline and cancellation checks shared by traversal and content scanning.
pub(crate) struct SearchControl {
    cancel: CancellationToken,
    deadline: Instant,
    checks: u32,
    last_error: Option<ControlError>,
}

impl SearchControl {
    pub(crate) fn new(budget: SearchBudget, cancel: CancellationToken) -> Self {
        Self {
            cancel,
            deadline: Instant::now() + budget.timeout,
            checks: 0,
            last_error: None,
        }
    }

    /// Both signals are checked on every call so a blocking gap between two
    /// checks cannot hide an expired deadline.
    pub(crate) fn check(&mut self) -> Result<(), ControlError> {
        if self.cancel.is_cancelled() {
            self.last_error = Some(ControlError::Cancelled);
            return Err(ControlError::Cancelled);
        }
        self.checks = self.checks.wrapping_add(1);
        if Instant::now() >= self.deadline {
            self.last_error = Some(ControlError::Timeout);
            return Err(ControlError::Timeout);
        }
        Ok(())
    }

    pub(crate) fn last_error(&self) -> Option<ControlError> {
        self.last_error
    }

    pub(crate) fn record_error(&mut self, error: ControlError) {
        self.last_error = Some(error);
    }
}
