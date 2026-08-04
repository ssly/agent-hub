mod claude_plugin;
mod commands;
mod config;
mod diff;
mod i18n;
mod mcp;
#[allow(dead_code)]
mod monitor;
mod paths;
mod platform;
mod session;
mod session_monitor;
mod skill;
mod state;
mod switch;
mod sync;
mod trash;
mod tray;
mod zcode_plugin;

use state::AppState;
use tauri::Manager;

/// Atomic flag used to signal the resumable updater to abort the current
/// download. Set by `cancel_update_download`, checked in the download loop.
pub type UpdateCancelFlag = std::sync::atomic::AtomicBool;

pub fn try_handle_hook_event() -> bool {
    session_monitor::try_capture_hook_event()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(std::sync::Mutex::new(AppState::new()))
        .manage(UpdateCancelFlag::new(false))
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_process::init())?;
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_shell::init())?;
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_dialog::init())?;
            // Watch Agent Hub's own small hook-event inbox. No agent session
            // directories are scanned and no processes are polled.
            app.manage(std::sync::Arc::new(
                session_monitor::SessionMonitorService::new(app.handle().clone()),
            ));

            #[cfg(desktop)]
            tray::setup(app)?;

            // macOS uses the overlay title bar (see tauri.conf.json), so the
            // traffic lights stay native. Windows has no equivalent, so drop
            // the native frame and let the toolbar render its own controls.
            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_decorations(false);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_platforms,
            commands::get_platform_skills,
            commands::get_skill_detail,
            commands::open_skill_folder,
            commands::get_diff_candidates,
            commands::diff_skills_cmd,
            commands::get_sync_targets,
            commands::sync_skill_cmd,
            commands::sync_folder_cmd,
            commands::refresh_platforms,
            commands::refresh_platform_skills,
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
            commands::preview_mcp_change_cmd,
            commands::list_trash_cmd,
            commands::restore_trash_item_cmd,
            commands::permanently_delete_trash_item_cmd,
            commands::empty_trash_cmd,
            commands::get_app_version,
            commands::download_and_install_update_resumable,
            commands::cancel_update_download,
            commands::list_session_platforms,
            commands::list_sessions,
            commands::list_session_terminals,
            commands::resume_session,
            commands::get_session_resume_preview,
            commands::get_session_messages,
            commands::search_session_messages,
            commands::delete_session,
            commands::delete_sessions,
            commands::export_sessions_html,
            commands::get_codex_session_monitor_snapshot,
            commands::delete_codex_session_monitor_session,
            commands::get_codex_hook_status,
            commands::preview_codex_hook_change,
            commands::apply_codex_hook_change,
            commands::get_claude_session_monitor_snapshot,
            commands::delete_claude_session_monitor_session,
            commands::get_claude_hook_status,
            commands::preview_claude_hook_change,
            commands::apply_claude_hook_change,
            commands::get_cursor_session_monitor_snapshot,
            commands::delete_cursor_session_monitor_session,
            commands::get_cursor_hook_status,
            commands::preview_cursor_hook_change,
            commands::apply_cursor_hook_change,
            commands::get_grok_session_monitor_snapshot,
            commands::delete_grok_session_monitor_session,
            commands::get_grok_hook_status,
            commands::preview_grok_hook_change,
            commands::apply_grok_hook_change,
            commands::get_kimi_session_monitor_snapshot,
            commands::delete_kimi_session_monitor_session,
            commands::get_kimi_hook_status,
            commands::preview_kimi_hook_change,
            commands::apply_kimi_hook_change,
            commands::get_zcode_session_monitor_snapshot,
            commands::delete_zcode_session_monitor_session,
            commands::get_zcode_hook_status,
            commands::preview_zcode_hook_change,
            commands::apply_zcode_hook_change,
            claude_plugin::list_claude_plugins,
            claude_plugin::set_claude_plugin_enabled,
            commands::get_zcode_plugins,
            switch::commands::list_switch_profiles,
            switch::commands::save_current_auth_profile,
            switch::commands::add_auth_profile,
            switch::commands::switch_auth_profile,
            switch::commands::update_auth_profile_note,
            switch::commands::delete_auth_profile,
            switch::commands::get_auth_profile_content,
            switch::commands::update_auth_profile_content,
            switch::commands::clear_active_auth,
            switch::commands::delete_active_auth,
            switch::commands::get_codex_usage,
            switch::commands::get_codex_reset_credits,
            switch::commands::get_codex_tray_usage,
            switch::commands::get_grok_usage,
            switch::commands::get_kimi_usage,
            switch::commands::get_claude_usage,
            switch::commands::get_usage_provider_availability,
            tray::resize_usage_tray,
            tray::set_usage_tray_pinned,
            tray::open_usage_tray,
        ])
        .run(tauri::generate_context!())
        .expect("error while running agent-hub");
}
