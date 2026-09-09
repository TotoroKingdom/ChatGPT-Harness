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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(std::sync::Mutex::new(crate::commands::BackendState {
            secrets,
        }))
        .manage(crate::runtime::RuntimeSupervisor::new())
        .manage(crate::tunnel::TunnelProcessManager::new())
        .invoke_handler(tauri::generate_handler![
            crate::commands::config_get,
            crate::commands::config_metadata,
            crate::commands::config_save,
            crate::commands::config_reset,
            crate::commands::secret_api_key_status,
            crate::commands::secret_api_key_delete,
            crate::commands::workspace_choose,
            crate::commands::tunnel_client_choose,
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
            crate::commands::runtime_diagnostics_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running ChatGPT-Harness application");
}

mod commands;
mod config;
mod platform;
mod runtime;
mod secrets;
// M7-B is intentionally internal until a later connection orchestration slice.
#[allow(dead_code)]
mod tunnel;
