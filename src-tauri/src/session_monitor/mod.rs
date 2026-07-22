mod capture;
mod hooks;
mod service;
mod types;

use std::sync::Arc;

pub use hooks::{apply_hook_change, get_hook_status, preview_hook_change, HookAction};
pub use service::CodexSessionMonitorService;
pub use types::{CodexHookChangePreview, CodexHookStatus, CodexMonitorSnapshot};

pub type ServiceHandle = Arc<CodexSessionMonitorService<tauri::Wry>>;

pub fn try_capture_codex_hook_event() -> bool {
    capture::try_capture_codex_hook_event()
}
