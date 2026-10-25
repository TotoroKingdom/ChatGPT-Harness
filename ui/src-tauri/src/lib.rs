#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let secrets: Box<dyn crate::secrets::SecretStore> = if cfg!(windows) {
        #[cfg(windows)]
        {
            Box::new(crate::secrets::WindowsCredentialStore)
        }
        #[cfg(not(windows))]
        {
            Box::new(crate::secrets::MemorySecretStore::default())
        }
    } else {
        Box::new(crate::secrets::MemorySecretStore::default())
    };
    let runtime = crate::runtime::RuntimeSupervisor::new();
    let tunnel = crate::tunnel::TunnelProcessManager::new();
    let harness = crate::harness::HarnessController::new(runtime.clone(), tunnel.clone());
    runtime.set_event_sink(std::sync::Arc::new(harness.clone()));
    runtime.set_startup_sink(std::sync::Arc::new(harness.clone()));
    tunnel.set_event_sink(std::sync::Arc::new(harness.clone()));
    let desktop = crate::desktop::DesktopLifecycleCoordinator::new(harness.clone());
    let desktop_for_window = desktop.clone();
    let desktop_for_setup = desktop.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(desktop)
        .on_window_event(move |window, event| {
            desktop_for_window.handle_window_event(window, event);
        })
        .setup(move |app| {
            use tauri::{Emitter, Manager};
            use tauri_plugin_notification::NotificationExt;
            let controller = app.state::<crate::harness::HarnessController>();
            let runtime = app
                .state::<crate::runtime::RuntimeSupervisor>()
                .inner()
                .clone();
            let handle = app.handle().clone();
            let desktop = desktop_for_setup.clone();
            let acknowledged = app
                .path()
                .app_data_dir()
                .ok()
                .and_then(|directory| crate::config::ConfigStore::new(directory).load().ok())
                .flatten()
                .map(|config| config.desktop.background_notice_acknowledged)
                .unwrap_or(false);
            desktop.set_background_notice_acknowledged(acknowledged);
            desktop.observe_status(controller.status());
            desktop.bootstrap_approvals(
                desktop.snapshot().status.active_session_id.as_deref(),
                runtime
                    .approvals_list(harness_control_protocol::ApprovalsListParams::default())
                    .map(|result| result.items)
                    .map_err(|_| ()),
            );

            let desktop_for_activity = desktop.clone();
            let handle_for_activity = handle.clone();
            let runtime_for_activity = runtime.clone();
            runtime.set_activity_sink(std::sync::Arc::new(move |event| {
                if !matches!(
                    event.event_type,
                    harness_control_protocol::ActivityEventType::ApprovalRequested
                        | harness_control_protocol::ActivityEventType::ApprovalResolved
                        | harness_control_protocol::ActivityEventType::ApprovalDenied
                        | harness_control_protocol::ActivityEventType::ApprovalExpired
                        | harness_control_protocol::ActivityEventType::ApprovalCancelled
                ) {
                    return;
                }
                let session = desktop_for_activity.snapshot().status.active_session_id;
                let approvals = runtime_for_activity
                    .approvals_list(harness_control_protocol::ApprovalsListParams::default())
                    .map(|result| result.items)
                    .map_err(|_| ());
                desktop_for_activity.handle_activity(session.as_deref(), event, approvals);
                // Approval activity changes the authoritative Pending
                // projection without necessarily changing Harness lifecycle.
                // Refresh the Tray here so its count cannot remain stale.
                crate::desktop::tray::refresh(&handle_for_activity, &desktop_for_activity);
            }));

            let desktop_for_click = desktop.clone();
            let handle_for_click = handle.clone();
            let runtime_for_click = runtime.clone();
            handle.notification().on_action(move |action| {
                let Some(notification) = action.notification() else {
                    return;
                };
                if notification
                    .extra()
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("connection")
                {
                    let _ = crate::desktop::routing::show_main_window(
                        &handle_for_click,
                        Some(crate::desktop::routing::HostNavigationIntent::Overview),
                    );
                    return;
                }
                let Some(runtime_id) = notification
                    .extra()
                    .get("runtimeId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                else {
                    return;
                };
                let Some(request_id) = notification
                    .extra()
                    .get("requestId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                else {
                    return;
                };
                let approvals = runtime_for_click
                    .approvals_list(harness_control_protocol::ApprovalsListParams::default())
                    .map(|result| result.items)
                    .map_err(|_| ());
                desktop_for_click.handle_notification_click(
                    &handle_for_click,
                    runtime_id,
                    request_id,
                    approvals,
                );
            })?;
            crate::desktop::tray::install(&handle, desktop.clone())?;
            crate::desktop::tray::observe_pending(&handle, &desktop, &runtime);
            let desktop_for_status = desktop.clone();
            let runtime_for_status = runtime.clone();
            controller.set_status_sink(std::sync::Arc::new(move |status| {
                let _ = handle.emit("harness:status", status);
                if desktop_for_status.observe_status_with_app(&handle, status.clone()) {
                    let approvals = runtime_for_status
                        .approvals_list(harness_control_protocol::ApprovalsListParams::default())
                        .map(|result| result.items)
                        .map_err(|_| ());
                    desktop_for_status
                        .bootstrap_approvals(status.active_session_id.as_deref(), approvals);
                }
                crate::desktop::tray::refresh(&handle, &desktop_for_status);
            }));
            Ok(())
        })
        .manage(std::sync::Mutex::new(crate::commands::BackendState {
            secrets,
        }))
        .manage(runtime)
        .manage(tunnel)
        .manage(harness)
        .invoke_handler(tauri::generate_handler![
            crate::commands::config_get,
            crate::commands::config_metadata,
            crate::commands::config_save,
            crate::commands::ui_language_set,
            crate::commands::desktop_preferences_set,
            crate::commands::desktop_continue_in_background,
            crate::commands::desktop_quit,
            crate::commands::config_reset,
            crate::commands::secret_api_key_status,
            crate::commands::secret_api_key_delete,
            crate::commands::workspace_choose,
            crate::commands::tunnel_client_choose,
            crate::commands::bash_executable_choose,
            crate::commands::tunnel_client_detect,
            crate::commands::validate_path,
            crate::commands::local_diagnostics_run,
            crate::commands::runtime_start,
            crate::commands::runtime_start_eligibility,
            crate::commands::runtime_status,
            crate::commands::runtime_process_status,
            crate::commands::runtime_stop,
            crate::commands::runtime_tools_list,
            crate::commands::runtime_policies_list,
            crate::commands::runtime_activity_list,
            crate::commands::runtime_activity_subscribe,
            crate::commands::runtime_approvals_list,
            crate::commands::runtime_approvals_resolve,
            crate::commands::runtime_get_diagnostics,
            crate::commands::runtime_get_activity_snapshot,
            crate::commands::runtime_get_effective_tools,
            crate::commands::runtime_diagnostics_run,
            crate::commands::connection_diagnostics_run,
            crate::commands::harness_start_eligibility,
            crate::commands::harness_start,
            crate::commands::harness_status,
            crate::commands::harness_startup_latest,
            crate::commands::harness_stop,
            crate::commands::harness_restart
        ])
        .run(tauri::generate_context!())
        .expect("error while running ChatGPT-Harness application");
}

mod commands;
mod config;
mod desktop;
mod diagnostics;
mod harness;
mod platform;
mod runtime;
mod secrets;
mod startup;
// M7-B is intentionally internal until a later connection orchestration slice.
#[allow(dead_code)]
mod tunnel;
