//! Windows console-window hygiene for short-lived child processes.
//!
//! Agent Hub is a GUI app. Spawning `cmd`, `where`, `sh` (from Git for Windows),
//! or any console-subsystem binary without `CREATE_NO_WINDOW` flashes a black
//! terminal window for a frame — users hit this when opening the resume modal
//! (PATH probes) or checking Cursor CLI version.
//!
//! Use [`suppress_console`] on every background probe. Never apply it to
//! intentional terminal launches (session resume into WT / PowerShell / cmd).

use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Win32 process-creation flag: do not allocate a console for the child.
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Mark `cmd` so it never flashes a console on Windows. No-op elsewhere.
#[inline]
pub fn suppress_console(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
