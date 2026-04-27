mod commands;
mod config;
mod diff;
mod i18n;
mod mcp;
mod platform;
mod skill;
mod state;
mod sync;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(std::sync::Mutex::new(AppState::new()))
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
            commands::list_mcp_platforms,
            commands::get_mcp_servers,
            commands::get_mcp_server,
            commands::save_mcp_server_cmd,
            commands::delete_mcp_server_cmd,
            commands::import_mcp_server_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running agent-hub");
}
