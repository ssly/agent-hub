mod commands;
mod config;
mod diff;
mod i18n;
mod mcp;
#[allow(dead_code)]
mod monitor;
mod platform;
mod session;
mod skill;
mod state;
mod switch;
mod sync;
mod trash;

use state::AppState;

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
            // Monitor / completion hooks are currently disabled. Do not request
            // notification permission, start filesystem watchers, or spawn the
            // process polling thread while the feature is hidden.

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
            switch::commands::list_switch_profiles,
            switch::commands::save_current_auth_profile,
            switch::commands::add_auth_profile,
            switch::commands::switch_auth_profile,
            switch::commands::update_auth_profile_note,
            switch::commands::delete_auth_profile,
            switch::commands::get_auth_profile_content,
            switch::commands::update_auth_profile_content,
            switch::commands::clear_active_auth,
        ])
        .run(tauri::generate_context!())
        .expect("error while running agent-hub");
}
