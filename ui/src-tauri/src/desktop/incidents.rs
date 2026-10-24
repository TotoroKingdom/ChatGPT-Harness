use crate::harness::{HarnessLifecycle, HarnessStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IncidentTransition {
    None,
    Opened { incident_id: String },
    Recovered { incident_id: String },
    ClosedSilently,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionNotification {
    pub title: String,
    pub body: String,
}

#[derive(Default)]
pub struct IncidentCoordinator {
    active_session_id: Option<String>,
    had_ready: bool,
    open_incident: Option<String>,
    sequence: u64,
    intentional_stop_or_quit: bool,
}

impl IncidentCoordinator {
    pub fn mark_intentional_stop(&mut self) {
        self.intentional_stop_or_quit = true;
    }

    pub fn observe(&mut self, status: &HarnessStatus, app_quitting: bool) -> IncidentTransition {
        if status.active_session_id != self.active_session_id {
            let closed = self.open_incident.take().is_some();
            self.active_session_id = status.active_session_id.clone();
            self.had_ready = false;
            self.intentional_stop_or_quit = false;
            if closed {
                return IncidentTransition::ClosedSilently;
            }
        }

        if status.lifecycle == HarnessLifecycle::Stopped {
            let closed = self.open_incident.take().is_some();
            self.had_ready = false;
            self.intentional_stop_or_quit = false;
            return if closed {
                IncidentTransition::ClosedSilently
            } else {
                IncidentTransition::None
            };
        }

        if status.lifecycle == HarnessLifecycle::Ready {
            self.had_ready = true;
            if let Some(incident_id) = self.open_incident.take() {
                if self.intentional_stop_or_quit || app_quitting {
                    return IncidentTransition::ClosedSilently;
                }
                return IncidentTransition::Recovered { incident_id };
            }
            return IncidentTransition::None;
        }

        if matches!(
            status.lifecycle,
            HarnessLifecycle::Degraded | HarnessLifecycle::Error
        ) && self.had_ready
            && self.open_incident.is_none()
            && !self.intentional_stop_or_quit
            && !app_quitting
        {
            self.sequence = self.sequence.saturating_add(1);
            let incident_id = format!(
                "{}-incident-{}",
                self.active_session_id
                    .as_deref()
                    .unwrap_or("unknown-session"),
                self.sequence
            );
            self.open_incident = Some(incident_id.clone());
            return IncidentTransition::Opened { incident_id };
        }

        IncidentTransition::None
    }
}

pub fn format_connection_notification(
    transition: &IncidentTransition,
) -> Option<ConnectionNotification> {
    match transition {
        IncidentTransition::Opened { .. } => Some(ConnectionNotification {
            title: "ChatGPT-Harness — Connection issue".into(),
            body: "Connection lost. Open ChatGPT-Harness to view the current status.".into(),
        }),
        IncidentTransition::Recovered { .. } => Some(ConnectionNotification {
            title: "ChatGPT-Harness — Connection recovered".into(),
            body: "Connection recovered. Open ChatGPT-Harness to view the current status.".into(),
        }),
        IncidentTransition::None | IncidentTransition::ClosedSilently => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::HarnessComponentStatus;
    use crate::tunnel::{TunnelHealth, TunnelRecoveryStatus};

    fn status(lifecycle: HarnessLifecycle, session: Option<&str>) -> HarnessStatus {
        HarnessStatus {
            lifecycle,
            runtime: match lifecycle {
                HarnessLifecycle::Ready => HarnessComponentStatus::Ready,
                HarnessLifecycle::Degraded => HarnessComponentStatus::Ready,
                HarnessLifecycle::Error => HarnessComponentStatus::Error,
                HarnessLifecycle::Starting => HarnessComponentStatus::Starting,
                HarnessLifecycle::Stopping => HarnessComponentStatus::Stopping,
                HarnessLifecycle::Stopped => HarnessComponentStatus::Stopped,
            },
            mcp: HarnessComponentStatus::Ready,
            tunnel: HarnessComponentStatus::Ready,
            active_session_id: session.map(str::to_owned),
            config_revision: Some("1".into()),
            last_error: None,
            tunnel_health: TunnelHealth::stopped(),
            tunnel_readiness: None,
            tunnel_recovery: TunnelRecoveryStatus::default(),
        }
    }

    #[test]
    fn table_driven_incident_transitions_match_contract() {
        let cases = [
            (
                "startup",
                vec![
                    HarnessLifecycle::Stopped,
                    HarnessLifecycle::Starting,
                    HarnessLifecycle::Ready,
                ],
                0,
                0,
            ),
            (
                "loss",
                vec![HarnessLifecycle::Ready, HarnessLifecycle::Degraded],
                1,
                0,
            ),
            (
                "repeat",
                vec![
                    HarnessLifecycle::Ready,
                    HarnessLifecycle::Degraded,
                    HarnessLifecycle::Degraded,
                ],
                1,
                0,
            ),
            (
                "error_same",
                vec![
                    HarnessLifecycle::Ready,
                    HarnessLifecycle::Degraded,
                    HarnessLifecycle::Error,
                ],
                1,
                0,
            ),
            (
                "recovery",
                vec![
                    HarnessLifecycle::Ready,
                    HarnessLifecycle::Degraded,
                    HarnessLifecycle::Ready,
                ],
                1,
                1,
            ),
            (
                "second_loss",
                vec![
                    HarnessLifecycle::Ready,
                    HarnessLifecycle::Degraded,
                    HarnessLifecycle::Ready,
                    HarnessLifecycle::Degraded,
                ],
                2,
                1,
            ),
        ];
        for (name, transitions, expected_open, expected_recovery) in cases {
            let mut coordinator = IncidentCoordinator::default();
            let mut opened = 0;
            let mut recovered = 0;
            for lifecycle in transitions {
                match coordinator.observe(&status(lifecycle, Some("session-1")), false) {
                    IncidentTransition::Opened { .. } => opened += 1,
                    IncidentTransition::Recovered { .. } => recovered += 1,
                    _ => {}
                }
            }
            assert_eq!(
                (opened, recovered),
                (expected_open, expected_recovery),
                "{name}"
            );
        }
    }

    #[test]
    fn stop_and_quit_close_incident_without_fake_recovery() {
        let mut coordinator = IncidentCoordinator::default();
        assert!(matches!(
            coordinator.observe(&status(HarnessLifecycle::Ready, Some("session-1")), false),
            IncidentTransition::None
        ));
        assert!(matches!(
            coordinator.observe(
                &status(HarnessLifecycle::Degraded, Some("session-1")),
                false
            ),
            IncidentTransition::Opened { .. }
        ));
        coordinator.mark_intentional_stop();
        assert!(matches!(
            coordinator.observe(
                &status(HarnessLifecycle::Stopping, Some("session-1")),
                false
            ),
            IncidentTransition::None
        ));
        assert!(matches!(
            coordinator.observe(&status(HarnessLifecycle::Stopped, None), false),
            IncidentTransition::ClosedSilently
        ));
        assert!(matches!(
            coordinator.observe(
                &status(HarnessLifecycle::Starting, Some("session-2")),
                false
            ),
            IncidentTransition::None
        ));
    }

    #[test]
    fn notification_disabled_still_tracks_source_incident_and_rollover_resets_keys() {
        let mut coordinator = IncidentCoordinator::default();
        // Notification preferences are applied by the Host delivery adapter;
        // source incident tracking remains active when delivery is disabled.
        assert!(matches!(
            coordinator.observe(&status(HarnessLifecycle::Ready, Some("session-1")), false),
            IncidentTransition::None
        ));
        assert!(matches!(
            coordinator.observe(
                &status(HarnessLifecycle::Degraded, Some("session-1")),
                false
            ),
            IncidentTransition::Opened { .. }
        ));
        assert!(matches!(
            coordinator.observe(&status(HarnessLifecycle::Ready, Some("session-1")), false),
            IncidentTransition::Recovered { .. }
        ));
        assert!(matches!(
            coordinator.observe(
                &status(HarnessLifecycle::Starting, Some("session-2")),
                false
            ),
            IncidentTransition::None
        ));
        assert!(matches!(
            coordinator.observe(&status(HarnessLifecycle::Ready, Some("session-2")), false),
            IncidentTransition::None
        ));
        assert!(matches!(
            coordinator.observe(
                &status(HarnessLifecycle::Degraded, Some("session-2")),
                false
            ),
            IncidentTransition::Opened { .. }
        ));
    }
}
