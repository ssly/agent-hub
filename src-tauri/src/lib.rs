mod commands;
mod config;
mod diff;
mod i18n;
mod mcp;
mod monitor;
mod platform;
mod session;
mod skill;
mod state;
mod sync;
mod trash;

use state::AppState;
use tauri::Manager;
#[cfg(desktop)]
use tauri_plugin_notification::NotificationExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(std::sync::Mutex::new(AppState::new()))
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_process::init())?;
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_notification::init())?;
            // Request macOS notification permission at startup
            #[cfg(desktop)]
            {
                let _ = app.notification().request_permission();
            }

            // Initialize monitor service
            let config = {
                let state = app.state::<std::sync::Mutex<AppState>>();
                let s = state.lock().unwrap();
                s.config.monitor.clone()
            };
            let monitor_service =
                monitor::service::MonitorService::new(app.handle().clone(), config);
            app.manage(std::sync::Arc::new(monitor_service));

            // Start process poll timer (5s interval, only when polling is enabled)
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let state = handle.state::<std::sync::Arc<monitor::service::MonitorService<tauri::Wry>>>();
                    if state.polling_enabled.load(std::sync::atomic::Ordering::Relaxed) {
                        state.poll();
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_platforms,
            commands::get_platform_skills,
            commands::get_skill_detail,
            commands::get_diff_candidates,
            commands::diff_skills_cmd,
            commands::get_sync_targets,
            commands::sync_skill_cmd,
            commands::sync_folder_cmd,
            commands::refresh_platforms,
            commands::get_locale,
            commands::set_locale,
            commands::search_skills,
            commands::read_skill_file,
            commands::delete_skill_cmd,
            commands::list_mcp_platforms,
            commands::get_mcp_servers,
            commands::get_mcp_server,
            commands::save_mcp_server_cmd,
            commands::delete_mcp_server_cmd,
            commands::import_mcp_server_cmd,
            commands::get_mcp_sync_targets,
            commands::preview_mcp_sync_cmd,
            commands::sync_mcp_server_cmd,
            commands::list_trash_cmd,
            commands::restore_trash_item_cmd,
            commands::permanently_delete_trash_item_cmd,
            commands::empty_trash_cmd,
            commands::scan_invalid_skills_cmd,
            commands::get_app_version,
            commands::download_and_install_update_resumable,
            commands::list_session_platforms,
            commands::list_sessions,
            commands::list_session_terminals,
            commands::resume_session,
            commands::get_session_messages,
            commands::delete_session,
            commands::get_active_sessions,
            commands::get_monitor_config,
            commands::set_monitor_config,
            commands::set_monitor_polling,
            commands::force_poll_monitor,
            commands::configure_hooks,
            commands::remove_hooks,
            commands::get_hooks_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running agent-hub");
}
