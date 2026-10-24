pub mod incidents;
pub mod lifecycle;
pub mod notifications;

use crate::{
    commands::{BackendState, ControlError},
    config::{ConfigStore, DesktopPreferencesPatch},
    harness::{HarnessComponentStatus, HarnessController, HarnessStatus},
};
use harness_control_protocol::{ActivityEvent, ActivityEventType, AppConfig, ApprovalRequest};
use incidents::{format_connection_notification, IncidentCoordinator, IncidentTransition};
use lifecycle::{
    close_decision, start_allowed, CloseDecision, DesktopLifecycleState, TrayFlashState,
};
use notifications::{
    route_notification_click, ApprovalClickRoute, ApprovalNotificationCoordinator,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent};
use tauri_plugin_notification::NotificationExt;

pub mod routing;
pub mod tray;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PendingApprovalCount {
    Known(u64),
    Unavailable,
}

impl PendingApprovalCount {
    pub fn label(&self) -> String {
        match self {
            Self::Known(value) => value.to_string(),
            Self::Unavailable => "unavailable".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DesktopSnapshot {
    pub state: DesktopLifecycleState,
    pub status: HarnessStatus,
    pub pending_approvals: PendingApprovalCount,
}

struct DesktopInner {
    lifecycle: DesktopLifecycleState,
    status: HarnessStatus,
    pending_approvals: PendingApprovalCount,
    tray_flash: TrayFlashState,
    approval_notifications: ApprovalNotificationCoordinator,
    incidents: IncidentCoordinator,
}

#[derive(Clone)]
pub struct DesktopLifecycleCoordinator {
    inner: Arc<Mutex<DesktopInner>>,
    harness: HarnessController,
}

impl DesktopLifecycleCoordinator {
    pub fn new(harness: HarnessController) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DesktopInner {
                lifecycle: DesktopLifecycleState::default(),
                status: harness.status(),
                pending_approvals: PendingApprovalCount::Unavailable,
                tray_flash: TrayFlashState::default(),
                approval_notifications: ApprovalNotificationCoordinator::default(),
                incidents: IncidentCoordinator::default(),
            })),
            harness,
        }
    }

    pub fn snapshot(&self) -> DesktopSnapshot {
        self.inner
            .lock()
            .map(|inner| DesktopSnapshot {
                state: inner.lifecycle.clone(),
                status: inner.status.clone(),
                pending_approvals: inner.pending_approvals.clone(),
            })
            .unwrap_or_else(|_| DesktopSnapshot {
                state: DesktopLifecycleState::default(),
                status: self.harness.status(),
                pending_approvals: PendingApprovalCount::Unavailable,
            })
    }

    pub fn set_background_notice_acknowledged(&self, acknowledged: bool) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.lifecycle.background_notice_acknowledged = acknowledged;
            if acknowledged {
                inner.lifecycle.education_visible = false;
            }
        }
    }

    fn observe_status_internal(&self, status: HarnessStatus) -> (bool, IncidentTransition) {
        if let Ok(mut inner) = self.inner.lock() {
            // This is a display projection only; HarnessController remains the
            // lifecycle authority and is queried again for actions.
            let session_changed = inner.status.active_session_id != status.active_session_id;
            let runtime_became_ready = inner.status.runtime != HarnessComponentStatus::Ready
                && status.runtime == HarnessComponentStatus::Ready;
            let app_quitting = inner.lifecycle.app_quitting;
            let transition = inner.incidents.observe(&status, app_quitting);
            if status.runtime != HarnessComponentStatus::Ready {
                // A cached Pending count is not authoritative after Runtime
                // loss or while a generation is changing. Stop the Tray
                // reminder at the same boundary so it cannot outlive truth.
                inner.pending_approvals = PendingApprovalCount::Unavailable;
                inner.tray_flash.stop();
            }
            inner.status = status;
            return (session_changed || runtime_became_ready, transition);
        }
        (false, IncidentTransition::None)
    }

    pub fn observe_status(&self, status: HarnessStatus) -> bool {
        self.observe_status_internal(status).0
    }

    pub fn observe_status_with_app(&self, app: &AppHandle, status: HarnessStatus) -> bool {
        let (session_changed, transition) = self.observe_status_internal(status);
        let app_quitting = self
            .inner
            .lock()
            .map(|inner| inner.lifecycle.app_quitting)
            .unwrap_or(true);
        if !app_quitting && app_desktop_config(app).desktop.connection_notifications {
            if let Some(notification) = format_connection_notification(&transition) {
                let _ = app
                    .notification()
                    .builder()
                    .title(notification.title)
                    .body(notification.body)
                    .extra("kind", "connection")
                    .show();
            }
        }
        session_changed
    }

    pub fn set_pending_approvals(&self, pending: PendingApprovalCount) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.pending_approvals = pending;
        }
    }

    pub(crate) fn approval_reminder_enabled(&self, app: &AppHandle) -> bool {
        app_desktop_config(app).desktop.approval_notifications
    }

    pub(crate) fn start_tray_flash(&self) -> Option<u64> {
        self.inner.lock().ok().and_then(|mut inner| {
            let app_quitting = inner.lifecycle.app_quitting;
            inner.tray_flash.start(app_quitting)
        })
    }

    pub(crate) fn stop_tray_flash(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.tray_flash.stop();
        }
    }

    pub(crate) fn tray_flash_should_continue(&self, generation: u64) -> bool {
        self.inner
            .lock()
            .map(|inner| {
                inner.tray_flash.should_continue(
                    generation,
                    matches!(
                        inner.pending_approvals,
                        PendingApprovalCount::Known(count) if count > 0
                    ),
                )
            })
            .unwrap_or(false)
    }

    pub(crate) fn stop_tray_flash_if_current(&self, generation: u64) -> bool {
        self.inner
            .lock()
            .map(|mut inner| {
                if inner.tray_flash.is_current(generation) {
                    inner.tray_flash.stop();
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false)
    }

    pub(crate) fn has_active_tray_flash(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.tray_flash.is_active())
            .unwrap_or(false)
    }

    pub fn bootstrap_approvals(
        &self,
        session: Option<&str>,
        approvals: Result<Vec<ApprovalRequest>, ()>,
    ) {
        self.apply_approval_refresh(session, approvals, None);
    }

    pub fn handle_activity(
        &self,
        session: Option<&str>,
        event: ActivityEvent,
        approvals: Result<Vec<ApprovalRequest>, ()>,
    ) {
        if matches!(
            event.event_type,
            ActivityEventType::ApprovalRequested
                | ActivityEventType::ApprovalResolved
                | ActivityEventType::ApprovalDenied
                | ActivityEventType::ApprovalExpired
                | ActivityEventType::ApprovalCancelled
        ) {
            self.apply_approval_refresh(session, approvals, Some(&event));
        }
    }

    fn apply_approval_refresh(
        &self,
        session: Option<&str>,
        approvals: Result<Vec<ApprovalRequest>, ()>,
        event: Option<&ActivityEvent>,
    ) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        // A query that started before Restart must not overwrite the new
        // session's projection, including with an old disconnected error.
        if inner.status.active_session_id.as_deref() != session {
            return;
        }
        if inner.status.runtime != HarnessComponentStatus::Ready || approvals.is_err() {
            inner.pending_approvals = PendingApprovalCount::Unavailable;
            inner.tray_flash.stop();
            return;
        }
        let approvals = approvals.expect("successful approval query");
        let result = match event {
            Some(event) => inner
                .approval_notifications
                .observe_activity(event, &approvals),
            None => inner.approval_notifications.bootstrap(&approvals),
        };
        inner.pending_approvals = result.pending_count;
        // Reminders use the existing Tray projection; no approval Toast is sent.
    }
    pub fn handle_notification_click(
        &self,
        app: &AppHandle,
        runtime_id: String,
        request_id: String,
        approvals: Result<Vec<ApprovalRequest>, ()>,
    ) {
        let _ = routing::show_main_window(app, None);
        let route = match approvals {
            Ok(items) => route_notification_click(&runtime_id, &request_id, &items),
            Err(()) => ApprovalClickRoute::Unavailable(request_id.clone()),
        };
        let intent = match route {
            ApprovalClickRoute::Pending(object_id) | ApprovalClickRoute::Terminal(object_id) => {
                routing::HostNavigationIntent::Approvals { object_id }
            }
            ApprovalClickRoute::Unavailable(object_id) => {
                routing::HostNavigationIntent::ApprovalsUnavailable { object_id }
            }
        };
        let _ = app.emit("desktop:navigation", intent);
    }

    pub fn close_decision(&self) -> CloseDecision {
        self.inner
            .lock()
            .map(|inner| {
                close_decision(
                    inner.lifecycle.app_quitting,
                    inner.lifecycle.background_notice_acknowledged,
                )
            })
            .unwrap_or(CloseDecision::RequestEducation)
    }

    pub fn handle_window_event(&self, window: &Window, event: &WindowEvent) {
        if window.label() != "main" {
            return;
        }
        let WindowEvent::CloseRequested { api, .. } = event else {
            return;
        };
        match self.close_decision() {
            CloseDecision::AllowExit => {}
            CloseDecision::Hide => {
                api.prevent_close();
                let _ = window.hide();
            }
            CloseDecision::RequestEducation => {
                api.prevent_close();
                if let Ok(mut inner) = self.inner.lock() {
                    inner.lifecycle.education_visible = true;
                }
                let _ = window.emit("desktop:background-notice", ());
            }
        }
    }

    pub fn start_allowed(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| start_allowed(inner.lifecycle.app_quitting, inner.status.lifecycle))
            .unwrap_or(false)
    }

    pub fn begin_quit(&self) -> bool {
        let began = self
            .inner
            .lock()
            .map(|mut inner| inner.lifecycle.begin_quit())
            .unwrap_or(false);
        if began {
            self.mark_intentional_stop();
        }
        began
    }

    fn cancel_quit(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.lifecycle.cancel_quit();
            inner.incidents.cancel_intentional_stop();
        }
    }

    pub fn mark_intentional_stop(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.incidents.mark_intentional_stop();
        }
    }

    pub fn quit(&self, app: &AppHandle) -> Result<(), ControlError> {
        if !self.begin_quit() {
            return Ok(());
        }
        // The flag is set before entering HarnessController's serialized stop
        // path, so a racing Tray Start cannot create a new owned process.
        if let Err(error) = self.harness.stop(app) {
            // Do not exit while Harness reports a cleanup failure: the bounded
            // stop path may have left an owned child alive. Keep the Host up,
            // release the quit lock, and let the command surface the error so
            // the user can retry without creating an orphan.
            self.cancel_quit();
            tray::refresh(app, self);
            return Err(error);
        }
        tray::stop_flashing(app, self);
        let _ = app.remove_tray_by_id(tray::TRAY_ID);
        app.exit(0);
        Ok(())
    }

    pub fn acknowledge_and_hide(
        &self,
        app: &AppHandle,
        config: &ConfigStore,
    ) -> Result<(), ControlError> {
        config
            .set_desktop_preferences(DesktopPreferencesPatch {
                approval_notifications: None,
                connection_notifications: None,
                background_notice_acknowledged: Some(true),
                auto_start_harness_on_app_launch: None,
            })
            .map_err(ControlError::from)?;
        self.set_background_notice_acknowledged(true);
        let window = app.get_webview_window("main").ok_or_else(|| ControlError {
            code: "WINDOW_UNAVAILABLE".into(),
            message: "Main window is unavailable; Harness was left running.".into(),
        })?;
        window.hide().map_err(|error| ControlError {
            code: "WINDOW_HIDE_FAILED".into(),
            message: error.to_string(),
        })
    }

    pub fn dispatch_start(
        &self,
        app: &AppHandle,
        backend: &Mutex<BackendState>,
    ) -> Result<HarnessStatus, ControlError> {
        if !self.start_allowed() {
            return Err(ControlError {
                code: "LIFECYCLE_CONFLICT".into(),
                message: "Harness is already running, changing state, or quitting.".into(),
            });
        }
        self.harness.start(app, backend)
    }

    pub fn dispatch_stop(&self, app: &AppHandle) -> Result<HarnessStatus, ControlError> {
        self.mark_intentional_stop();
        self.harness.stop(app)
    }
}

fn app_desktop_config(app: &AppHandle) -> AppConfig {
    app.path()
        .app_data_dir()
        .ok()
        .and_then(|directory| ConfigStore::new(directory).load().ok())
        .flatten()
        .unwrap_or_else(ConfigStore::safe_defaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        harness::{HarnessComponentStatus, HarnessErrorSummary},
        tunnel::{TunnelHealth, TunnelProcessManager, TunnelRecoveryStatus},
    };
    fn status(
        lifecycle: crate::harness::HarnessLifecycle,
        runtime: HarnessComponentStatus,
        session: Option<&str>,
    ) -> HarnessStatus {
        HarnessStatus {
            lifecycle,
            runtime,
            mcp: runtime,
            tunnel: runtime,
            active_session_id: session.map(str::to_owned),
            config_revision: Some("1".into()),
            last_error: None::<HarnessErrorSummary>,
            tunnel_health: TunnelHealth::stopped(),
            tunnel_readiness: None,
            tunnel_recovery: TunnelRecoveryStatus::default(),
        }
    }

    fn coordinator() -> DesktopLifecycleCoordinator {
        let harness = HarnessController::new(
            crate::runtime::RuntimeSupervisor::new(),
            TunnelProcessManager::new(),
        );
        DesktopLifecycleCoordinator::new(harness)
    }

    #[test]
    fn late_approval_query_cannot_overwrite_a_new_harness_session() {
        let coordinator = coordinator();
        coordinator.observe_status(status(
            crate::harness::HarnessLifecycle::Ready,
            HarnessComponentStatus::Ready,
            Some("new-session"),
        ));
        coordinator.set_pending_approvals(PendingApprovalCount::Known(3));
        coordinator.bootstrap_approvals(Some("old-session"), Ok(vec![]));
        assert_eq!(
            coordinator.snapshot().pending_approvals,
            PendingApprovalCount::Known(3)
        );
        coordinator.bootstrap_approvals(Some("old-session"), Err(()));
        assert_eq!(
            coordinator.snapshot().pending_approvals,
            PendingApprovalCount::Known(3)
        );
        coordinator.bootstrap_approvals(Some("new-session"), Ok(vec![]));
        assert_eq!(
            coordinator.snapshot().pending_approvals,
            PendingApprovalCount::Known(0)
        );
    }

    #[test]
    fn runtime_loss_invalidates_pending_projection_and_stops_flash_even_with_same_session() {
        let coordinator = coordinator();
        coordinator.observe_status(status(
            crate::harness::HarnessLifecycle::Ready,
            HarnessComponentStatus::Ready,
            Some("session-1"),
        ));
        coordinator.set_pending_approvals(PendingApprovalCount::Known(1));
        let generation = coordinator.start_tray_flash().expect("flash started");

        // The Runtime can report loss while the old session id is still
        // present. The Host must invalidate the cached Pending count anyway.
        coordinator.observe_status(status(
            crate::harness::HarnessLifecycle::Degraded,
            HarnessComponentStatus::Stopped,
            Some("session-1"),
        ));
        let snapshot = coordinator.snapshot();
        assert_eq!(
            snapshot.pending_approvals,
            PendingApprovalCount::Unavailable
        );
        assert!(!coordinator.tray_flash_should_continue(generation));
        assert!(!coordinator.has_active_tray_flash());
    }
}
