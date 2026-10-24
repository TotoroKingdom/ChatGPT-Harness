use super::{routing, DesktopLifecycleCoordinator, DesktopSnapshot};
use crate::harness::HarnessLifecycle;
use std::{
    sync::{Arc, OnceLock},
    thread,
    time::Duration,
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

pub const TRAY_ID: &str = "main";
const SHOW_ID: &str = "tray-show";
const STATUS_ID: &str = "tray-status";
const PENDING_ID: &str = "tray-pending";
const START_ID: &str = "tray-start";
const STOP_ID: &str = "tray-stop";
const QUIT_ID: &str = "tray-quit";
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../../icons/icon.ico");
const FLASH_INTERVAL: Duration = Duration::from_millis(500);

/// MCP execution queues activity inside Runtime, but the current control
/// protocol only drains/replays that queue on activity queries. An activity
/// callback alone therefore cannot wake the reminder for a new Pending item.
/// Query the existing authority from Host even while the WebView is hidden.
pub fn observe_pending(
    app: &AppHandle,
    coordinator: &DesktopLifecycleCoordinator,
    runtime: &crate::runtime::RuntimeSupervisor,
) {
    let app = app.clone();
    let coordinator = coordinator.clone();
    let runtime = runtime.clone();
    thread::spawn(move || loop {
        // Quit can be cancelled if owned-process cleanup fails. Keep this
        // observer alive for that case; a successful Quit exits the process.
        if !coordinator.snapshot().state.app_quitting {
            refresh_pending_from_runtime(&app, &coordinator, &runtime);
        }
        thread::sleep(FLASH_INTERVAL);
    });
}

pub(crate) fn refresh_pending_from_runtime(
    app: &AppHandle,
    coordinator: &DesktopLifecycleCoordinator,
    runtime: &crate::runtime::RuntimeSupervisor,
) {
    let before = coordinator.snapshot();
    if before.status.runtime != crate::harness::HarnessComponentStatus::Ready {
        return;
    }
    let approvals = runtime
        .approvals_list(harness_control_protocol::ApprovalsListParams::default())
        .map(|result| result.items)
        .map_err(|_| ());
    coordinator.bootstrap_approvals(before.status.active_session_id.as_deref(), approvals);
    if before.pending_approvals != coordinator.snapshot().pending_approvals {
        refresh(app, coordinator);
    }
}

struct TrayIcons {
    normal: Image<'static>,
    alert: Image<'static>,
}

static TRAY_ICONS: OnceLock<Arc<TrayIcons>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayEventAction {
    ShowWindow,
    Ignore,
}

fn tray_event_action(button: MouseButton, double_click: bool) -> TrayEventAction {
    if double_click && button == MouseButton::Left {
        TrayEventAction::ShowWindow
    } else {
        TrayEventAction::Ignore
    }
}

fn tray_icons() -> tauri::Result<Arc<TrayIcons>> {
    if let Some(icons) = TRAY_ICONS.get() {
        return Ok(icons.clone());
    }
    let normal = Image::from_bytes(TRAY_ICON_BYTES)?;
    let alert = alert_icon(&normal);
    let icons = Arc::new(TrayIcons { normal, alert });
    let _ = TRAY_ICONS.set(icons.clone());
    Ok(TRAY_ICONS.get().cloned().unwrap_or(icons))
}

fn alert_icon(normal: &Image<'static>) -> Image<'static> {
    let width = normal.width();
    let height = normal.height();
    let mut rgba = normal.rgba().to_vec();
    if width == 0 || height == 0 {
        return Image::new_owned(rgba, width, height);
    }
    let radius = (width.min(height) / 3).max(2);
    let center_x = width.saturating_sub(radius + 1);
    let center_y = radius + 1;
    let radius_squared = radius * radius;
    for y in 0..height {
        for x in 0..width {
            let dx = x.abs_diff(center_x);
            let dy = y.abs_diff(center_y);
            if dx * dx + dy * dy > radius_squared {
                continue;
            }
            let index = ((y * width + x) * 4) as usize;
            if let Some(pixel) = rgba.get_mut(index..index + 4) {
                pixel.copy_from_slice(&[220, 40, 40, 255]);
            }
        }
    }
    Image::new_owned(rgba, width, height)
}

pub fn install(app: &AppHandle, coordinator: DesktopLifecycleCoordinator) -> tauri::Result<()> {
    let icons = tray_icons()?;
    let menu = build_menu(app, &coordinator.snapshot())?;
    let coordinator_for_menu = coordinator.clone();
    let app_for_tray = app.clone();
    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        // Windows shows the context menu on the right button by default. Keep
        // left click free for the explicit double-click Show action below.
        .show_menu_on_left_click(false)
        .icon(icons.normal.clone())
        .on_menu_event(move |app, event| {
            handle_menu_event(app, &coordinator_for_menu, event.id().as_ref());
        })
        .on_tray_icon_event(move |_, event| {
            let action = match event {
                TrayIconEvent::Click { button, .. } => tray_event_action(button, false),
                TrayIconEvent::DoubleClick { button, .. } => tray_event_action(button, true),
                _ => TrayEventAction::Ignore,
            };
            if action == TrayEventAction::ShowWindow {
                let _ = routing::show_main_window(&app_for_tray, None);
            }
        })
        .build(app)?;
    sync_pending_indicator(app, &coordinator, coordinator.snapshot().pending_approvals);
    Ok(())
}

pub fn refresh(app: &AppHandle, coordinator: &DesktopLifecycleCoordinator) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if let Ok(menu) = build_menu(app, &coordinator.snapshot()) {
        let _ = tray.set_menu(Some(menu));
    }
    sync_pending_indicator(app, coordinator, coordinator.snapshot().pending_approvals);
}

pub fn stop_flashing(app: &AppHandle, coordinator: &DesktopLifecycleCoordinator) {
    coordinator.stop_tray_flash();
    if let Ok(icons) = tray_icons() {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_icon(Some(icons.normal.clone()));
        }
    }
}

fn sync_pending_indicator(
    app: &AppHandle,
    coordinator: &DesktopLifecycleCoordinator,
    pending: super::PendingApprovalCount,
) {
    let should_flash = matches!(pending, super::PendingApprovalCount::Known(count) if count > 0)
        && coordinator.approval_reminder_enabled(app);
    if !should_flash {
        stop_flashing(app, coordinator);
        return;
    }
    let Some(generation) = coordinator.start_tray_flash() else {
        return;
    };
    let app = app.clone();
    let coordinator = coordinator.clone();
    thread::spawn(move || {
        let Ok(icons) = tray_icons() else {
            coordinator.stop_tray_flash();
            return;
        };
        let mut alert = false;
        loop {
            thread::sleep(FLASH_INTERVAL);
            if !coordinator.tray_flash_should_continue(generation)
                || !coordinator.approval_reminder_enabled(&app)
            {
                break;
            }
            alert = !alert;
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let icon = if alert {
                    icons.alert.clone()
                } else {
                    icons.normal.clone()
                };
                let _ = tray.set_icon(Some(icon));
            } else {
                break;
            }
        }
        // Do not reset a newer flash generation that started while this worker
        // was stopping. The current generation owns the icon at that point.
        if coordinator.stop_tray_flash_if_current(generation) {
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let _ = tray.set_icon(Some(icons.normal.clone()));
            }
        } else if !coordinator.has_active_tray_flash() {
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let _ = tray.set_icon(Some(icons.normal.clone()));
            }
        }
    });
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
        coordinator_stop_enabled(snapshot),
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

fn coordinator_stop_enabled(snapshot: &DesktopSnapshot) -> bool {
    matches!(
        snapshot.status.lifecycle,
        HarnessLifecycle::Starting
            | HarnessLifecycle::Ready
            | HarnessLifecycle::Degraded
            | HarnessLifecycle::Error
            | HarnessLifecycle::Stopping
    )
}

fn handle_menu_event(app: &AppHandle, coordinator: &DesktopLifecycleCoordinator, id: &str) {
    match id {
        SHOW_ID => {
            let _ = routing::show_main_window(app, None);
            refresh(app, coordinator);
        }
        PENDING_ID => {
            let _ = routing::show_main_window(
                app,
                Some(routing::HostNavigationIntent::Approvals {
                    object_id: String::new(),
                }),
            );
            refresh(app, coordinator);
        }
        START_ID => {
            dispatch_start_in_background(app, coordinator);
        }
        STOP_ID => {
            dispatch_stop_in_background(app, coordinator);
        }
        QUIT_ID => {
            dispatch_quit_in_background(app, coordinator);
        }
        _ => {}
    }
}

fn dispatch_start_in_background(app: &AppHandle, coordinator: &DesktopLifecycleCoordinator) {
    let app = app.clone();
    let coordinator = coordinator.clone();
    thread::spawn(move || {
        let result = app
            .try_state::<std::sync::Mutex<crate::commands::BackendState>>()
            .map(|backend| coordinator.dispatch_start(&app, backend.inner()));
        if let Some(Err(error)) = result {
            eprintln!("Tray Start failed ({}): {}", error.code, error.message);
        }
        refresh(&app, &coordinator);
    });
}

fn dispatch_stop_in_background(app: &AppHandle, coordinator: &DesktopLifecycleCoordinator) {
    let app = app.clone();
    let coordinator = coordinator.clone();
    thread::spawn(move || {
        if let Err(error) = coordinator.dispatch_stop(&app) {
            eprintln!("Tray Stop failed ({}): {}", error.code, error.message);
        }
        refresh(&app, &coordinator);
    });
}

fn dispatch_quit_in_background(app: &AppHandle, coordinator: &DesktopLifecycleCoordinator) {
    let app = app.clone();
    let coordinator = coordinator.clone();
    thread::spawn(move || {
        if let Err(error) = coordinator.quit(&app) {
            eprintln!(
                "explicit desktop quit cleanup reported {}: {}",
                error.code, error.message
            );
            refresh(&app, &coordinator);
        }
    });
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
        assert!(!coordinator_stop_enabled(&snapshot));
        assert_eq!(snapshot.pending_approvals.label(), "2");
        assert!(!matches!(
            snapshot.pending_approvals,
            PendingApprovalCount::Unavailable
        ));
    }

    #[test]
    fn error_state_keeps_stop_retry_available() {
        let snapshot = DesktopSnapshot {
            state: super::super::lifecycle::DesktopLifecycleState::default(),
            status: status(HarnessLifecycle::Error),
            pending_approvals: PendingApprovalCount::Unavailable,
        };
        assert!(coordinator_stop_enabled(&snapshot));
    }

    #[test]
    fn only_left_double_click_shows_window() {
        assert_eq!(
            tray_event_action(MouseButton::Left, false),
            TrayEventAction::Ignore
        );
        assert_eq!(
            tray_event_action(MouseButton::Left, true),
            TrayEventAction::ShowWindow
        );
        assert_eq!(
            tray_event_action(MouseButton::Right, false),
            TrayEventAction::Ignore
        );
        assert_eq!(
            tray_event_action(MouseButton::Right, true),
            TrayEventAction::Ignore
        );
    }

    #[test]
    fn bundled_tray_icon_is_a_real_multi_size_ico() {
        let bytes = TRAY_ICON_BYTES;
        assert!(bytes.len() > 6, "tray icon asset is empty");
        assert_eq!(&bytes[0..4], &[0, 0, 1, 0]);
        let entries = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        assert!(entries >= 6, "tray icon should contain all packaged sizes");
        assert!(bytes.len() >= 6 + entries * 16);
        let sizes: std::collections::HashSet<u16> = (0..entries)
            .map(|index| {
                let offset = 6 + index * 16;
                let image_bytes =
                    u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as usize;
                let image_offset =
                    u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap())
                        as usize;
                assert!(
                    image_offset.saturating_add(image_bytes) <= bytes.len(),
                    "ICO entry {index} escapes the resource"
                );
                let width = if bytes[offset] == 0 {
                    256
                } else {
                    bytes[offset] as u16
                };
                width
            })
            .collect();
        for required in [16, 32, 48, 64, 128, 256] {
            assert!(sizes.contains(&required), "missing {required}px tray icon");
        }
    }

    #[test]
    fn normal_icon_decodes_with_visible_pixels_and_alert_icon_differs() {
        let icons = tray_icons().expect("tray icon should decode");
        assert!(
            icons
                .normal
                .rgba()
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0),
            "normal tray icon is fully transparent"
        );
        assert_ne!(
            icons.normal.rgba(),
            icons.alert.rgba(),
            "reminder icon must differ from normal icon"
        );
    }
}
