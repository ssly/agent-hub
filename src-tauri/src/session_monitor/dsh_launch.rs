//! Start / stop DeepSeek Harness Web (`npx @deepseek-ai/dsh web`).
//!
//! Agent Hub does not bundle Node or dsh. The user's `npx` runs the package
//! from the npm cache. `--yes` skips the interactive install prompt that
//! would hang a GUI-spawned process.

use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

const NPX_MISSING: &str = "本机没有找到 npx。请先安装 Node.js（需包含 npm/npx），然后再启动 DeepSeek Harness。";
const DSH_WEB_PORT: u16 = 3080;
const DSH_WEB_URL: &str = "http://127.0.0.1:3080";

struct Launcher {
    child: Option<Child>,
    pid: Option<u32>,
    last_error: Option<String>,
}

impl Launcher {
    const fn new() -> Self {
        Self {
            child: None,
            pid: None,
            last_error: None,
        }
    }
}

static LAUNCHER: Mutex<Launcher> = Mutex::new(Launcher::new());

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshWebStatus {
    pub state: String,
    pub url: Option<String>,
    pub error: Option<String>,
}

fn monitor_dir() -> PathBuf {
    crate::paths::home_dir()
        .join(".agent-hub")
        .join("session-monitor")
}

fn pid_path() -> PathBuf {
    monitor_dir().join("dsh-web.pid")
}

fn log_path() -> PathBuf {
    monitor_dir().join("dsh-web.log")
}

fn web_port_open() -> bool {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), DSH_WEB_PORT);
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

fn running_status() -> DshWebStatus {
    DshWebStatus {
        state: "running".into(),
        url: Some(DSH_WEB_URL.into()),
        error: None,
    }
}

fn starting_status() -> DshWebStatus {
    DshWebStatus {
        state: "starting".into(),
        url: None,
        error: None,
    }
}

fn stopped_status(error: Option<String>) -> DshWebStatus {
    DshWebStatus {
        state: "stopped".into(),
        url: None,
        error,
    }
}

fn write_pid(pid: u32) {
    let dir = monitor_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(pid_path(), format!("{pid}\n"));
}

fn read_pid_file() -> Option<u32> {
    let text = fs::read_to_string(pid_path()).ok()?;
    text.trim().parse().ok()
}

fn clear_pid_file() {
    let _ = fs::remove_file(pid_path());
}

fn pid_alive(pid: u32) -> bool {
    let mut sys = sysinfo::System::new();
    let sys_pid = sysinfo::Pid::from_u32(pid);
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sys_pid]), true);
    sys.process(sys_pid).is_some()
}

fn open_log(truncate: bool) -> Result<File, String> {
    let dir = monitor_dir();
    fs::create_dir_all(&dir).map_err(|error| format!("unable to create {}: {error}", dir.display()))?;
    let mut opts = OpenOptions::new();
    opts.write(true).create(true);
    if truncate {
        opts.truncate(true);
    } else {
        opts.append(true);
    }
    opts.open(log_path())
        .map_err(|error| format!("unable to open dsh-web.log: {error}"))
}

fn log_tail() -> Option<String> {
    let mut file = File::open(log_path()).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(2048);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn path_separator() -> char {
    if cfg!(windows) { ';' } else { ':' }
}

fn npx_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["npx.cmd", "npx.exe", "npx"]
    } else {
        &["npx"]
    }
}

fn is_npx_file(path: &Path) -> bool {
    path.is_file()
}

fn find_npx_in_dir(dir: &Path) -> Option<PathBuf> {
    for name in npx_names() {
        let candidate = dir.join(name);
        if is_npx_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn find_npx_on_path(path_var: &str) -> Option<PathBuf> {
    for dir in path_var.split(path_separator()) {
        if dir.is_empty() {
            continue;
        }
        if let Some(found) = find_npx_in_dir(Path::new(dir)) {
            return Some(found);
        }
    }
    None
}

fn extra_bin_dirs() -> Vec<PathBuf> {
    let home = crate::paths::home_dir();
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        home.join(".local").join("bin"),
        home.join(".volta").join("bin"),
        home.join(".fnm").join("aliases").join("default").join("bin"),
        home.join(".local")
            .join("share")
            .join("fnm")
            .join("aliases")
            .join("default")
            .join("bin"),
    ];
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs.push(PathBuf::from(r"C:\Program Files\nodejs"));
    if let Ok(entries) = fs::read_dir(home.join(".nvm").join("versions").join("node")) {
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path().join("bin"))
            .filter(|path| path.is_dir())
            .collect();
        versions.sort();
        dirs.extend(versions);
    }
    dirs
}

fn cheap_find_npx() -> Option<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        if let Some(found) = find_npx_on_path(&path_var) {
            return Some(found);
        }
    }
    extra_bin_dirs().iter().find_map(|dir| find_npx_in_dir(dir))
}

fn login_shell_npx() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let output = Command::new(shell)
            .arg("-lic")
            .arg("command -v npx")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| {
                line.ends_with("/npx") || line.ends_with("\\npx") || line.ends_with("\\npx.cmd")
            })
            .map(PathBuf::from)
            .find(|path| is_npx_file(path))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn resolve_npx() -> Result<PathBuf, String> {
    cheap_find_npx()
        .or_else(login_shell_npx)
        .ok_or_else(|| NPX_MISSING.to_string())
}

fn spawn_path_env(npx: &Path) -> String {
    let mut parts = Vec::new();
    if let Some(dir) = npx.parent() {
        parts.push(dir.to_string_lossy().into_owned());
    }
    for dir in extra_bin_dirs() {
        parts.push(dir.to_string_lossy().into_owned());
    }
    if let Ok(existing) = std::env::var("PATH") {
        parts.push(existing);
    }
    parts.join(&path_separator().to_string())
}

pub(crate) fn cmdline_looks_like_dsh_web(cmd: &[String]) -> bool {
    let joined = cmd.join(" ").to_ascii_lowercase();
    if joined.contains("agent-hub") {
        return false;
    }
    let has_pkg = joined.contains("@deepseek-ai/dsh")
        || joined.contains("/@deepseek-ai/dsh/")
        || joined.contains("\\@deepseek-ai\\dsh\\");
    has_pkg && joined.split_whitespace().any(|part| part == "web")
}

fn kill_pid_tree(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status();
        std::thread::sleep(Duration::from_millis(400));
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{pid}")])
            .status();
        if pid_alive(pid) {
            let _ = Command::new("kill").args(["-KILL", &pid.to_string()]).status();
        }
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        crate::win_console::suppress_console(&mut cmd);
        let _ = cmd.status();
    }
}

fn kill_matching_processes() {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for process in sys.processes().values() {
        let cmd: Vec<String> = process
            .cmd()
            .iter()
            .map(|part| part.to_string_lossy().into_owned())
            .collect();
        if cmdline_looks_like_dsh_web(&cmd) {
            process.kill();
        }
    }
}

fn forget_child(launcher: &mut Launcher) {
    launcher.child = None;
    launcher.pid = None;
    clear_pid_file();
}

fn reap_child(launcher: &mut Launcher) {
    let Some(child) = launcher.child.as_mut() else {
        if let Some(pid) = launcher.pid.or_else(read_pid_file) {
            if !pid_alive(pid) {
                forget_child(launcher);
            }
        }
        return;
    };
    match child.try_wait() {
        Ok(Some(status)) => {
            if !status.success() && !web_port_open() {
                let detail = log_tail().unwrap_or_else(|| status.to_string());
                launcher.last_error = Some(format!("DeepSeek Harness 启动失败：{detail}"));
            }
            forget_child(launcher);
        }
        Ok(None) => {}
        Err(_) => forget_child(launcher),
    }
}

fn adopt_pid_file(launcher: &mut Launcher) {
    if launcher.pid.is_some() {
        return;
    }
    if let Some(pid) = read_pid_file() {
        if pid_alive(pid) {
            launcher.pid = Some(pid);
        } else {
            clear_pid_file();
        }
    }
}

pub fn dsh_web_status() -> DshWebStatus {
    let Ok(mut launcher) = LAUNCHER.lock() else {
        return stopped_status(Some("dsh launcher is unavailable".into()));
    };
    adopt_pid_file(&mut launcher);
    reap_child(&mut launcher);
    if web_port_open() {
        launcher.last_error = None;
        return running_status();
    }
    if launcher.pid.is_some() || launcher.child.is_some() {
        return starting_status();
    }
    stopped_status(launcher.last_error.clone())
}

pub fn dsh_web_start() -> Result<DshWebStatus, String> {
    {
        let current = dsh_web_status();
        if current.state == "running" || current.state == "starting" {
            return Ok(current);
        }
    }
    let npx = resolve_npx()?;
    let home = crate::paths::home_dir();
    let log = open_log(true)?;
    let log_err = log
        .try_clone()
        .map_err(|error| format!("unable to clone dsh-web.log: {error}"))?;

    let mut cmd = Command::new(&npx);
    cmd.args(["--yes", "@deepseek-ai/dsh", "web"])
        .current_dir(&home)
        .env("PATH", spawn_path_env(&npx))
        .env("NPM_CONFIG_YES", "true")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(
            crate::win_console::CREATE_NO_WINDOW | 0x0000_0200, // CREATE_NEW_PROCESS_GROUP
        );
    }

    let child = cmd
        .spawn()
        .map_err(|error| format!("无法启动 npx：{error}"))?;
    let pid = child.id();
    write_pid(pid);

    let Ok(mut launcher) = LAUNCHER.lock() else {
        return Err("dsh launcher is unavailable".into());
    };
    launcher.child = Some(child);
    launcher.pid = Some(pid);
    launcher.last_error = None;
    if web_port_open() {
        Ok(running_status())
    } else {
        Ok(starting_status())
    }
}

pub fn dsh_web_stop() -> Result<DshWebStatus, String> {
    let pid = {
        let Ok(mut launcher) = LAUNCHER.lock() else {
            return Err("dsh launcher is unavailable".into());
        };
        adopt_pid_file(&mut launcher);
        launcher.pid.or_else(read_pid_file)
    };
    if let Some(pid) = pid {
        kill_pid_tree(pid);
    }
    kill_matching_processes();

    if let Ok(mut launcher) = LAUNCHER.lock() {
        if let Some(mut child) = launcher.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        forget_child(&mut launcher);
        launcher.last_error = None;
    }

    for _ in 0..10 {
        if !web_port_open() {
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
        kill_matching_processes();
    }

    Ok(dsh_web_status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_npx_and_node_dsh_web() {
        assert!(cmdline_looks_like_dsh_web(&[
            "npx".into(),
            "--yes".into(),
            "@deepseek-ai/dsh".into(),
            "web".into(),
        ]));
        assert!(cmdline_looks_like_dsh_web(&[
            "node".into(),
            "/Users/x/.npm/_npx/abc/node_modules/@deepseek-ai/dsh/lib/bin.js".into(),
            "web".into(),
        ]));
    }

    #[test]
    fn ignores_unrelated_and_our_own_process() {
        assert!(!cmdline_looks_like_dsh_web(&[
            "node".into(),
            "vite".into(),
        ]));
        assert!(!cmdline_looks_like_dsh_web(&[
            "agent-hub".into(),
            "--agent-hub-dsh-hook".into(),
        ]));
        assert!(!cmdline_looks_like_dsh_web(&[
            "npx".into(),
            "@deepseek-ai/dsh".into(),
            "plugin".into(),
        ]));
    }
}
