#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if agent_hub::try_handle_hook_event() {
        return;
    }
    agent_hub::run()
}
