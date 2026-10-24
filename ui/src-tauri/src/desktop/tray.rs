use super::{routing, DesktopLifecycleCoordinator, DesktopSnapshot};
use crate::harness::HarnessLifecycle;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

pub const TRAY_ID: &str = "main";
const SHOW_ID: &str = "tray-show";
const STATUS_ID: &str = "tray-status";
const PENDING_ID: &str = "tray-pending";
const START_ID: &str = "tray-start";
const STOP_ID: &str = "tray-stop";
const QUIT_ID: &str = "tray-quit";

pub fn install(app: &AppHandle, coordinator: DesktopLifecycleCoordinator) -> tauri::Result<()> {
    let menu = build_menu(app, &coordinator.snapshot())?;
    let coordinator_for_menu = coordinator.clone();
    let app_for_tray = app.clone();
    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .icon(Image::from_bytes(include_bytes!("../../icons/icon.ico"))?)
        .on_menu_event(move |app, event| {
            handle_menu_event(app, &coordinator_for_menu, event.id().as_ref());
        })
        .on_tray_icon_event(move |_, event| {
            if matches!(
                event,
                TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
            ) {
                let _ = routing::show_main_window(&app_for_tray, None);
            }
        })
        .build(app)?;
    Ok(())
}

pub fn refresh(app: &AppHandle, coordinator: &DesktopLifecycleCoordinator) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if let Ok(menu) = build_menu(app, &coordinator.snapshot()) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn build_menu(app: &AppHandle, snapshot: &DesktopSnapshot) -> tauri::Result<Menu<Wry>> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show ChatGPT-Harness", true, None::<&str>)?;
    let status = MenuItem::with_id(
        app,
        STATUS_ID,
        format!(
            "Status: {}",
            super::lifecycle::status_label(snapshot.status.lifecycle)
        ),
        false,
        None::<&str>,
    )?;
    let pending = MenuItem::with_id(
        app,
        PENDING_ID,
        format!("Pending approvals: {}", snapshot.pending_approvals.label()),
        true,
        None::<&str>,
    )?;
    let start = MenuItem::with_id(
        app,
        START_ID,
        "Start Harness",
        coordinator_start_enabled(snapshot),
        None::<&str>,
    )?;
    let stop = MenuItem::with_id(
        app,
        STOP_ID,
        "Stop Harness",
        matches!(
            snapshot.status.lifecycle,
            HarnessLifecycle::Starting
                | HarnessLifecycle::Ready
                | HarnessLifecycle::Degraded
                | HarnessLifecycle::Stopping
        ),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &show,
            &status,
            &pending,
            &start,
            &stop,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

fn coordinator_start_enabled(snapshot: &DesktopSnapshot) -> bool {
    super::lifecycle::start_allowed(snapshot.state.app_quitting, snapshot.status.lifecycle)
}

fn handle_menu_event(app: &AppHandle, coordinator: &DesktopLifecycleCoordinator, id: &str) {
    match id {
        SHOW_ID => {
            let _ = routing::show_main_window(app, None);
        }
        PENDING_ID => {
            let _ = routing::show_main_window(
                app,
                Some(routing::HostNavigationIntent::Approvals {
                    object_id: String::new(),
                }),
            );
        }
        START_ID => {
            if let Some(backend) =
                app.try_state::<std::sync::Mutex<crate::commands::BackendState>>()
            {
                let _ = coordinator.dispatch_start(app, backend.inner());
            }
        }
        STOP_ID => {
            let _ = coordinator.dispatch_stop(app);
        }
        QUIT_ID => coordinator.quit(app),
        _ => {}
    }
    refresh(app, coordinator);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::PendingApprovalCount;
    use crate::harness::{HarnessComponentStatus, HarnessErrorSummary, HarnessStatus};
    use crate::tunnel::{TunnelHealth, TunnelRecoveryStatus};

    fn status(lifecycle: HarnessLifecycle) -> HarnessStatus {
        HarnessStatus {
            lifecycle,
            runtime: HarnessComponentStatus::Stopped,
            mcp: HarnessComponentStatus::Stopped,
            tunnel: HarnessComponentStatus::Stopped,
            active_session_id: None,
            config_revision: None,
            last_error: None::<HarnessErrorSummary>,
            tunnel_health: TunnelHealth::stopped(),
            tunnel_readiness: None,
            tunnel_recovery: TunnelRecoveryStatus::default(),
        }
    }

    #[test]
    fn menu_projection_uses_authoritative_lifecycle_and_pending_count() {
        let snapshot = DesktopSnapshot {
            state: super::super::lifecycle::DesktopLifecycleState::default(),
            status: status(HarnessLifecycle::Stopped),
            pending_approvals: PendingApprovalCount::Known(2),
        };
        assert!(coordinator_start_enabled(&snapshot));
        assert_eq!(snapshot.pending_approvals.label(), "2");
        assert!(!matches!(
            snapshot.pending_approvals,
            PendingApprovalCount::Unavailable
        ));
    }
}
