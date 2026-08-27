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
mod qwen_plugin;
mod session;
mod session_monitor;
mod skill;
mod state;
mod switch;
mod sync;
mod trash;
mod tray;
mod win_console;
mod zcode_plugin;

use state::AppState;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Atomic flag used to signal the resumable updater to abort the current
/// download. Set by `cancel_update_download`, checked in the download loop.
pub type UpdateCancelFlag = std::sync::atomic::AtomicBool;

pub fn try_handle_hook_event() -> bool {
    session_monitor::try_capture_hook_event()
}

/// Red traffic-light / window X must not destroy `main` while the tray keeps
/// the process alive: Dock reopen and tray "Check for Updates" both look up
/// the window by label. Hide instead so Mission Control / Dock can bring it
/// back.
fn attach_main_window_lifecycle(window: &WebviewWindow) {
    let win = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win.hide();
        }
    });
}

/// Build a replacement `main` window matching `tauri.conf.json` when the
/// previous one was destroyed (older builds, or any path that skipped hide).
fn recreate_main_window(app: &AppHandle) -> Option<WebviewWindow> {
    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Agent Hub")
        .inner_size(1024.0, 768.0)
        .min_inner_size(800.0, 600.0)
        .resizable(true)
        .fullscreen(false);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(12.0, 23.0));
    }

    #[cfg(target_os = "windows")]
    {
        builder = builder.decorations(false);
    }

    match builder.build() {
        Ok(window) => {
            attach_main_window_lifecycle(&window);
            Some(window)
        }
        Err(error) => {
            eprintln!("agent-hub: failed to recreate main window: {error}");
            None
        }
    }
}

/// Surface the main UI: unhide the app (macOS Cmd+H), recreate main if it was
/// destroyed, then show / unminimize / focus. Used by Dock reopen and tray
/// menu actions that need the primary window.
pub fn show_main_window(app: &AppHandle) {
    // Cmd+H hides the *application*. Showing a window alone is not enough;
    // NSApp must be unhidden first or Dock clicks appear to do nothing.
    #[cfg(target_os = "macos")]
    {
        let _ = app.show();
    }

    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => match recreate_main_window(app) {
            Some(window) => window,
            None => return,
        },
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
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
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                {
                    let _ = window.set_decorations(false);
                }
                // Tray keeps the process alive after the last UI close; hide
                // main instead of destroying it so Dock / tray can reopen it.
                attach_main_window_lifecycle(&window);
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
            commands::get_qwen_session_monitor_snapshot,
            commands::delete_qwen_session_monitor_session,
            commands::get_qwen_hook_status,
            commands::preview_qwen_hook_change,
            commands::apply_qwen_hook_change,
            commands::get_zcode_session_monitor_snapshot,
            commands::delete_zcode_session_monitor_session,
            commands::get_zcode_hook_status,
            commands::preview_zcode_hook_change,
            commands::apply_zcode_hook_change,
            commands::get_antigravity_session_monitor_snapshot,
            commands::delete_antigravity_session_monitor_session,
            commands::get_antigravity_hook_status,
            commands::preview_antigravity_hook_change,
            commands::apply_antigravity_hook_change,
            commands::get_kiro_session_monitor_snapshot,
            commands::delete_kiro_session_monitor_session,
            commands::get_kiro_hook_status,
            commands::preview_kiro_hook_change,
            commands::apply_kiro_hook_change,
            commands::get_workbuddy_session_monitor_snapshot,
            commands::delete_workbuddy_session_monitor_session,
            commands::get_workbuddy_hook_status,
            commands::preview_workbuddy_hook_change,
            commands::apply_workbuddy_hook_change,
            commands::get_dsh_session_monitor_snapshot,
            commands::delete_dsh_session_monitor_session,
            commands::get_dsh_hook_status,
            commands::preview_dsh_hook_change,
            commands::apply_dsh_hook_change,
            commands::get_dsh_web_status,
            commands::start_dsh_web,
            commands::stop_dsh_web,
            commands::list_available_monitor_agents,
            claude_plugin::list_claude_plugins,
            claude_plugin::set_claude_plugin_enabled,
            commands::get_zcode_plugins,
            commands::get_qwen_plugins,
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
            switch::deepseek::get_deepseek_settings,
            switch::deepseek::get_deepseek_usage,
            switch::monitor_settings::get_usage_monitor_settings,
            switch::monitor_settings::set_usage_refresh_minutes,
            switch::monitor_settings::set_usage_selected_agent,
            switch::monitor_settings::set_usage_agent_listening,
            tray::resize_usage_tray,
            tray::resize_usage_tray_dock,
            tray::close_usage_tray,
            tray::expand_usage_tray,
            tray::collapse_usage_tray,
            tray::set_usage_tray_hovered,
            tray::set_usage_tray_overlay,
            tray::open_usage_tray,
        ])
        .build(tauri::generate_context!())
        .expect("error while building agent-hub")
        .run(|handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                // Dock-icon click. Tray popup / edge strip counts as a
                // "visible window" for hasVisibleWindows, so macOS default
                // reopen is a no-op — always surface main ourselves.
                // Also covers: main was hide-on-close'd, Cmd+H, or destroyed
                // on older builds (recreate path).
                show_main_window(handle);
            }
        });
}
