use crate::harness::HarnessLifecycle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseDecision {
    AllowExit,
    Hide,
    RequestEducation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopLifecycleState {
    pub app_quitting: bool,
    pub background_notice_acknowledged: bool,
    pub education_visible: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayFlashState {
    generation: u64,
    active: bool,
}

impl TrayFlashState {
    pub fn start(&mut self, app_quitting: bool) -> Option<u64> {
        if app_quitting || self.active {
            return None;
        }
        self.generation = self.generation.saturating_add(1);
        self.active = true;
        Some(self.generation)
    }

    pub fn stop(&mut self) {
        if self.active {
            self.generation = self.generation.saturating_add(1);
            self.active = false;
        }
    }

    pub fn should_continue(&self, generation: u64, pending: bool) -> bool {
        self.active && self.generation == generation && pending
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.active && self.generation == generation
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl DesktopLifecycleState {
    pub fn begin_quit(&mut self) -> bool {
        if self.app_quitting {
            return false;
        }
        self.app_quitting = true;
        self.education_visible = false;
        true
    }

    pub fn cancel_quit(&mut self) {
        self.app_quitting = false;
    }
}

pub fn close_decision(app_quitting: bool, background_notice_acknowledged: bool) -> CloseDecision {
    if app_quitting {
        CloseDecision::AllowExit
    } else if background_notice_acknowledged {
        CloseDecision::Hide
    } else {
        CloseDecision::RequestEducation
    }
}

pub fn start_allowed(app_quitting: bool, lifecycle: HarnessLifecycle) -> bool {
    !app_quitting
        && matches!(
            lifecycle,
            HarnessLifecycle::Stopped | HarnessLifecycle::Error
        )
}

pub fn status_label(lifecycle: HarnessLifecycle) -> &'static str {
    match lifecycle {
        HarnessLifecycle::Stopped => "Stopped",
        HarnessLifecycle::Starting => "Starting",
        HarnessLifecycle::Ready => "Ready",
        HarnessLifecycle::Degraded => "Degraded",
        HarnessLifecycle::Error => "Error",
        HarnessLifecycle::Stopping => "Stopping",
    }
}

/// Display projection must always return the authoritative lifecycle value.
/// A process/PID observation is intentionally ignored here.
#[cfg(test)]
pub fn lifecycle_from_process_observation(
    _process_exists: bool,
    authoritative: HarnessLifecycle,
) -> HarnessLifecycle {
    authoritative
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_uses_background_notice_until_acknowledged() {
        assert_eq!(
            close_decision(false, false),
            CloseDecision::RequestEducation
        );
        assert_eq!(close_decision(false, true), CloseDecision::Hide);
        assert_eq!(close_decision(true, false), CloseDecision::AllowExit);
    }

    #[test]
    fn quit_wins_race_and_sets_flag_before_cleanup() {
        let mut state = DesktopLifecycleState::default();
        assert!(state.begin_quit());
        assert!(state.app_quitting);
        assert!(!state.begin_quit());
        assert_eq!(
            close_decision(state.app_quitting, false),
            CloseDecision::AllowExit
        );
    }

    #[test]
    fn failed_quit_cleanup_releases_the_quit_lock_for_retry() {
        let mut state = DesktopLifecycleState::default();
        assert!(state.begin_quit());
        state.cancel_quit();
        assert!(!state.app_quitting);
        assert!(state.begin_quit());
    }

    #[test]
    fn start_is_allowed_only_for_stopped_or_error_and_never_after_quit() {
        for lifecycle in [
            HarnessLifecycle::Starting,
            HarnessLifecycle::Ready,
            HarnessLifecycle::Degraded,
            HarnessLifecycle::Stopping,
        ] {
            assert!(!start_allowed(false, lifecycle));
        }
        assert!(start_allowed(false, HarnessLifecycle::Stopped));
        assert!(start_allowed(false, HarnessLifecycle::Error));
        assert!(!start_allowed(true, HarnessLifecycle::Stopped));
    }

    #[test]
    fn status_projection_is_authoritative_and_does_not_use_process_pid() {
        assert_eq!(status_label(HarnessLifecycle::Stopped), "Stopped");
        assert_eq!(status_label(HarnessLifecycle::Starting), "Starting");
        assert_eq!(status_label(HarnessLifecycle::Ready), "Ready");
        assert_eq!(status_label(HarnessLifecycle::Degraded), "Degraded");
        assert_eq!(status_label(HarnessLifecycle::Error), "Error");
        assert_eq!(status_label(HarnessLifecycle::Stopping), "Stopping");
        assert_eq!(
            lifecycle_from_process_observation(true, HarnessLifecycle::Stopped),
            HarnessLifecycle::Stopped
        );
    }

    #[test]
    fn tray_flash_stops_for_zero_or_unavailable_pending_and_quit() {
        let mut state = TrayFlashState::default();
        let generation = state.start(false).expect("flash should start");
        assert!(state.should_continue(generation, true));
        assert!(!state.should_continue(generation, false));
        state.stop();
        assert!(!state.is_active());
        assert!(!state.should_continue(generation, true));
        assert!(state.start(true).is_none());
    }

    #[test]
    fn tray_flash_current_generation_can_be_stopped_after_pending_clears() {
        let mut state = TrayFlashState::default();
        let generation = state.start(false).expect("flash should start");
        assert!(state.is_current(generation));
        assert!(!state.should_continue(generation, false));
        state.stop();
        assert!(!state.is_current(generation));
    }
}
