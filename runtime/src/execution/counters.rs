use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

/// Runtime-owned observations. Never authorization or concurrency limits.
#[derive(Default)]
pub(crate) struct RequestCounters {
    active: Arc<AtomicU32>,
    running: Arc<AtomicU32>,
}
pub(crate) struct CounterGuard(Arc<AtomicU32>);
impl CounterGuard {
    fn enter(counter: &Arc<AtomicU32>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self(Arc::clone(counter))
    }
}
impl Drop for CounterGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
impl RequestCounters {
    pub(crate) fn active_guard(&self) -> CounterGuard {
        CounterGuard::enter(&self.active)
    }
    pub(crate) fn running_guard(&self) -> CounterGuard {
        CounterGuard::enter(&self.running)
    }
    pub(crate) fn snapshot(&self) -> (u32, u32) {
        (
            self.active.load(Ordering::Acquire),
            self.running.load(Ordering::Acquire),
        )
    }
}
