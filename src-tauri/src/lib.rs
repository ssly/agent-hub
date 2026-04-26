mod commands;
mod config;
mod diff;
mod i18n;
mod platform;
mod skill;
mod state;
mod sync;

use state::{AppState, SafeState};

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
            commands::refresh_platforms,
            commands::get_locale,
            commands::set_locale,
            commands::search_skills,
            commands::read_skill_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running agent-hub");
}
