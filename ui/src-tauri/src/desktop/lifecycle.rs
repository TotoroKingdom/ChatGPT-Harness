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
}
