mod capture;
mod hooks;
mod kiro;
mod service;
mod types;

use std::sync::Arc;

pub use hooks::{apply_hook_change, get_hook_status, preview_hook_change, HookAction};
pub use service::SessionMonitorService;
pub use types::{AgentKind, HookChangePreview, HookStatus, KiroMonitorStatus, MonitorSnapshot};

pub type ServiceHandle = Arc<SessionMonitorService<tauri::Wry>>;

pub fn try_capture_hook_event() -> bool {
    capture::try_capture_hook_event()
}
