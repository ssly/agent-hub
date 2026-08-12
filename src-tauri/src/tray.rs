#[cfg(any(target_os = "macos", target_os = "windows"))]
use tauri::Manager;
use tauri::{App, AppHandle};

#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_ID: &str = "codex-usage-tray";
const TRAY_WINDOW_WIDTH: f64 = 400.0;
/// Mini mode floor: one usage-orb wide (orb 132 + same panel/shell padding as normal).
const TRAY_MINI_WIDTH: f64 = 160.0;
const TRAY_LOADING_HEIGHT: f64 = 120.0;
const TRAY_MAX_HEIGHT: f64 = 620.0;

#[cfg(any(target_os = "macos", target_os = "windows"))]
const MENU_CHECK_UPDATE: &str = "tray-check-update";
#[cfg(any(target_os = "macos", target_os = "windows"))]
const MENU_QUIT: &str = "tray-quit";

/// Docked (edge-snapped) mode, kept in memory only. While docked the popup is
/// a persistent edge indicator: focus loss must not hide it, and reopening
/// must not reset its size/position. Dock mode has two visual states: the
/// collapsed strip at the screen edge, and the panel slid out on hover.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_DOCKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Docked sub-state: false = strip at the edge, true = panel slid out.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_EXPANDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Which screen edge the dock lives on.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static DOCK_EDGE: std::sync::Mutex<Option<&'static str>> = std::sync::Mutex::new(None);
/// True while a dock/undock tween runs; move events from our own animation
/// must not retrigger snap detection.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_ANIMATING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Timestamp of the last programmatic geometry change (set_size / set_position
/// from dock expand/collapse/snap/resize). Windows synthesizes Moved + a
/// spurious Focused(false) for those; treating them as user drags expands the
/// strip and can collapse again before outer_size catches up, poisoning
/// DOCK_PANEL with strip dimensions so the next hover expands to a tiny panel.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static LAST_OWNED_GEOMETRY: std::sync::Mutex<Option<std::time::Instant>> =
    std::sync::Mutex::new(None);
/// How long after our own set_size/set_position we ignore Moved / focus-collapse.
#[cfg(any(target_os = "macos", target_os = "windows"))]
const OWNED_GEOMETRY_GUARD_MS: u128 = 250;
/// Floating rect (physical) before docking; its size is reused for hover-expand.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static PRE_DOCK: std::sync::Mutex<Option<(i32, i32, u32, u32)>> = std::sync::Mutex::new(None);
/// The slid-out panel's rect, captured on every collapse so the next expand
/// returns to the exact same spot instead of drifting down each cycle.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static DOCK_PANEL: std::sync::Mutex<Option<(i32, i32, u32, u32)>> = std::sync::Mutex::new(None);
/// (generation, burst): generation bumps on every move so a settled drag can
/// tell it is the last one; burst counts consecutive moves to tell user drags
/// (many events) apart from programmatic repositions (one or two).
#[cfg(any(target_os = "macos", target_os = "windows"))]
static MOVE_STATE: std::sync::Mutex<(u64, u32)> = std::sync::Mutex::new((0, 0));
/// Last move time; a hover-leave collapse arriving right after move events
/// means the user is mid-drag, not hovering away.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static LAST_MOVE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
/// A panel popover (opacity / refresh-interval slider) is open: the cursor
/// watcher must not collapse the expanded panel while the user interacts
/// with controls that overflow the panel bounds.
#[cfg(target_os = "macos")]
static TRAY_OVERLAY_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Last physical window position, kept in memory only (per user request:
/// remembered while the app lives, forgotten on exit). Updated by both user
/// drags and programmatic centering, so "remembered" means "where it last was".
#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_POSITION: std::sync::Mutex<Option<(i32, i32)>> = std::sync::Mutex::new(None);

/// Docked strip size / snap trigger distance, in logical px (scaled per monitor).
#[cfg(any(target_os = "macos", target_os = "windows"))]
const DOCK_STRIP_WIDTH: f64 = 20.0;
#[cfg(any(target_os = "macos", target_os = "windows"))]
const DOCK_STRIP_HEIGHT: f64 = 72.0;
#[cfg(any(target_os = "macos", target_os = "windows"))]
const SNAP_DISTANCE: f64 = 28.0;
/// Two monitor edges closer than this (with vertical overlap) form a seam
/// between screens; seams never snap.
#[cfg(any(target_os = "macos", target_os = "windows"))]
const SEAM_GAP: i32 = 8;
#[cfg(any(target_os = "macos", target_os = "windows"))]
const MOVE_SETTLE_MS: u64 = 190;
#[cfg(any(target_os = "macos", target_os = "windows"))]
const MIN_DRAG_MOVES: u32 = 3;

/// Emitted to the tray window when edge-dock state changes; the frontend
/// swaps between the full panel and the docked strip on this event.
#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_DOCK_CHANGED_EVENT: &str = "usage-tray-dock-changed";
/// Emitted right before a dock/undock size tween starts so the frontend can
/// fade the current content out; the new state (and its content) only
/// appears with the dock-changed event after the tween completes.
#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_DOCK_ANIMATING_EVENT: &str = "usage-tray-dock-animating";

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DockChangedPayload {
    edge: Option<&'static str>,
    expanded: bool,
}

/// The panel's close button (✕): drop any dock state and hide the window.
/// The next tray-icon click is always a fresh, centered, undocked open.
#[tauri::command]
pub fn close_usage_tray(app: AppHandle) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(window) = app.get_webview_window("codex-usage") {
        clear_dock_state(&window);
        let _ = window.hide();
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

/// Frontend hover state for the tray window. Edge-snapping ONLY happens when
/// the cursor has been outside the panel for SNAP_DELAY_MS — never during a
/// drag, never while the cursor rests on the panel.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_HOVERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Generation counter cancelling pending edge-snap schedules.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static SNAP_GEN: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
/// How long the cursor must stay outside the panel before an edge snap fires.
#[cfg(any(target_os = "macos", target_os = "windows"))]
const SNAP_DELAY_MS: u64 = 200;

/// The cursor left the floating panel: if it is resting against a snappable
/// edge, dock it right away. Suppressed mid-drag (the cursor can outrun the
/// window during a native drag, which also fires mouseleave).
#[tauri::command]
pub fn set_usage_tray_hovered(app: AppHandle, hovered: bool) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use std::sync::atomic::Ordering;
        TRAY_HOVERED.store(hovered, Ordering::Relaxed);
        if let Some(window) = app.get_webview_window("codex-usage") {
            if hovered {
                cancel_edge_snap();
            } else {
                // Cursor left the panel: start the out-of-panel countdown.
                schedule_edge_snap(&window);
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (app, hovered);
}

/// Schedule an edge snap SNAP_DELAY_MS out. At fire time every condition is
/// rechecked: still not hovered, no recent movement (not mid-drag), not
/// docked/animating, and actually touching an edge.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn schedule_edge_snap(window: &tauri::WebviewWindow) {
    if tray_docked() {
        return;
    }
    let generation = match SNAP_GEN.lock() {
        Ok(mut gen) => {
            *gen += 1;
            *gen
        }
        Err(_) => return,
    };
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(SNAP_DELAY_MS));
        {
            let Ok(gen) = SNAP_GEN.lock() else {
                return;
            };
            if *gen != generation {
                return; // re-scheduled or cancelled since
            }
        }
        use std::sync::atomic::Ordering;
        if tray_docked() || TRAY_ANIMATING.load(Ordering::Relaxed) || TRAY_HOVERED.load(Ordering::Relaxed) {
            return;
        }
        let recent_move = LAST_MOVE
            .lock()
            .ok()
            .and_then(|last| *last)
            .is_some_and(|instant| instant.elapsed().as_millis() < SNAP_DELAY_MS as u128);
        if recent_move {
            return; // still dragging (moves within the delay window)
        }
        try_snap_tray(&window);
    });
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn cancel_edge_snap() {
    if let Ok(mut gen) = SNAP_GEN.lock() {
        *gen += 1;
    }
}

/// Keep the panel fully inside the union of all monitor work areas. Only
/// safe to call once a drag has SETTLED — calling set_position mid-drag
/// fights the OS-native drag session and shoves the window sideways.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn clamp_tray_into_monitors(window: &tauri::WebviewWindow) {
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let monitors = window.available_monitors().unwrap_or_default();
    if monitors.is_empty() {
        return;
    }
    let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
    let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
    for monitor in &monitors {
        // X: full frame — the panel may rest flush against the physical
        // screen edge. Y: work area — never under the menu bar.
        min_x = min_x.min(monitor.position().x);
        max_x = max_x.max(monitor.position().x + monitor.size().width as i32);
        let work = monitor.work_area();
        min_y = min_y.min(work.position.y);
        max_y = max_y.max(work.position.y + work.size.height as i32);
    }
    let max_pos_x = max_x - size.width as i32;
    let max_pos_y = max_y - size.height as i32;
    if max_pos_x < min_x || max_pos_y < min_y {
        return; // window larger than the desktop; nothing sensible to do
    }
    let clamped_x = pos.x.clamp(min_x, max_pos_x);
    let clamped_y = pos.y.clamp(min_y, max_pos_y);
    if clamped_x != pos.x || clamped_y != pos.y {
        mark_owned_geometry();
        let _ = window.set_position(tauri::PhysicalPosition::new(clamped_x, clamped_y));
        mark_owned_geometry();
    }
}

/// Frontend reports whether a panel popover (opacity / refresh-interval
/// slider) is open; the cursor watcher suspends auto-collapse meanwhile.
#[tauri::command]
pub fn set_usage_tray_overlay(open: bool) {
    #[cfg(target_os = "macos")]
    TRAY_OVERLAY_OPEN.store(open, std::sync::atomic::Ordering::Relaxed);

    #[cfg(not(target_os = "macos"))]
    let _ = open;
}

/// The docked strip shows one status dot per monitored session, so its
/// height follows the dot count; the frontend measures and pushes it here.
/// Only applies while collapsed into the strip; width stays the strip width.
#[tauri::command]
pub fn resize_usage_tray_dock(app: AppHandle, height: f64) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(window) = app.get_webview_window("codex-usage") {
        if tray_docked()
            && !tray_expanded()
            && !TRAY_ANIMATING.load(std::sync::atomic::Ordering::Relaxed)
        {
            let scale = window.scale_factor().unwrap_or(1.0);
            let h = (height.clamp(24.0, 400.0) * scale).round().max(1.0) as u32;
            let w = (DOCK_STRIP_WIDTH * scale).round().max(1.0) as u32;
            // Keep X anchored to the dock edge while height changes.
            if let Ok(pos) = window.outer_position() {
                let x = match dock_edge() {
                    Some("right") => {
                        if let Some(ctx) = snap_context(&window) {
                            ctx.fr - w as i32
                        } else {
                            pos.x
                        }
                    }
                    _ => pos.x,
                };
                set_tray_physical_frame(&window, x, pos.y, w, h);
            } else {
                mark_owned_geometry();
                let _ = window.set_size(tauri::PhysicalSize::new(w, h));
                mark_owned_geometry();
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (app, height);
}

/// Hovering the docked strip slides the panel out (frontend mouseenter).
#[tauri::command]
pub fn expand_usage_tray(app: AppHandle) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(window) = app.get_webview_window("codex-usage") {
        if !TRAY_ANIMATING.load(std::sync::atomic::Ordering::Relaxed) {
            expand_dock(&window, true);
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

/// The cursor left the slid-out panel: slide back into the strip. Ignored
/// while a drag is in progress (moving the panel fires mouseleave too).
#[tauri::command]
pub fn collapse_usage_tray(app: AppHandle) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(window) = app.get_webview_window("codex-usage") {
        let dragging = LAST_MOVE
            .lock()
            .ok()
            .and_then(|last| *last)
            .is_some_and(|instant| instant.elapsed().as_millis() < 600);
        if !dragging && !TRAY_ANIMATING.load(std::sync::atomic::Ordering::Relaxed) {
            collapse_dock(&window, true);
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn tray_docked() -> bool {
    TRAY_DOCKED.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn tray_expanded() -> bool {
    TRAY_EXPANDED.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn dock_edge() -> Option<&'static str> {
    DOCK_EDGE.lock().ok().and_then(|edge| *edge)
}

/// Mark that we just changed the tray frame ourselves so the resulting
/// Moved / Focused(false) events are not treated as user interaction.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn mark_owned_geometry() {
    if let Ok(mut last) = LAST_OWNED_GEOMETRY.lock() {
        *last = Some(std::time::Instant::now());
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn geometry_recently_owned() -> bool {
    LAST_OWNED_GEOMETRY
        .lock()
        .ok()
        .and_then(|last| *last)
        .is_some_and(|instant| instant.elapsed().as_millis() < OWNED_GEOMETRY_GUARD_MS)
}

/// Physical width of a real slid-out panel (never the dock strip).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn is_panel_physical_width(width: u32, scale: f64) -> bool {
    // Mini mode floor is TRAY_MINI_WIDTH; strip is ~20 logical px. Anything
    // near strip width is a poisoned remember and must not be reused.
    let min = (TRAY_MINI_WIDTH * scale * 0.85).round().max(1.0) as u32;
    width >= min
}

/// Apply a physical frame and suppress the synthetic Moved/focus events it
/// produces on Windows (and occasionally macOS).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn set_tray_physical_frame(window: &tauri::WebviewWindow, x: i32, y: i32, w: u32, h: u32) {
    mark_owned_geometry();
    let _ = window.set_size(tauri::PhysicalSize::new(w.max(1), h.max(1)));
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    mark_owned_geometry();
}

/// Remember the slid-out panel rect only when it is a plausible full panel
/// (not the dock strip or a half-applied intermediate frame).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn store_dock_panel_rect(scale: f64, rect: (i32, i32, u32, u32)) {
    if !is_panel_physical_width(rect.2, scale) {
        return;
    }
    if let Ok(mut stored) = DOCK_PANEL.lock() {
        *stored = Some(rect);
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn emit_dock_changed(window: &tauri::WebviewWindow) {
    use tauri::Emitter;
    let payload = if tray_docked() {
        DockChangedPayload {
            edge: dock_edge(),
            expanded: tray_expanded(),
        }
    } else {
        DockChangedPayload {
            edge: None,
            expanded: false,
        }
    };
    let _ = window.emit(TRAY_DOCK_CHANGED_EVENT, payload);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn emit_dock_animating(window: &tauri::WebviewWindow) {
    use tauri::Emitter;
    let _ = window.emit(TRAY_DOCK_ANIMATING_EVENT, ());
}

/// Which snappable edge (if any) the window currently touches, with the
/// monitor context needed to place the strip/panel. X anchors to the full
/// monitor frame (the strip must hug the physical screen edge); Y stays
/// inside the work area so the menu bar is never covered.
#[cfg(any(target_os = "macos", target_os = "windows"))]
struct SnapCtx {
    edge: &'static str,
    /// Full-frame left/right X (physical screen edges).
    fx: i32,
    fr: i32,
    /// Work-area vertical bounds.
    wy: i32,
    wh: i32,
    scale: f64,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn snap_context(window: &tauri::WebviewWindow) -> Option<SnapCtx> {
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return None;
    };
    let center_x = pos.x + size.width as i32 / 2;
    let center_y = pos.y + size.height as i32 / 2;
    let monitor = window
        .monitor_from_point(center_x as f64, center_y as f64)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())?;
    let monitors = window.available_monitors().unwrap_or_default();
    let work = monitor.work_area();
    let (wy, wh) = (work.position.y, work.size.height as i32);
    // Full frame: work_area can be inset by the Dock / Stage Manager, which
    // would leave a visible gap between the strip and the screen edge.
    let fx = monitor.position().x;
    let fr = fx + monitor.size().width as i32;
    let scale = monitor.scale_factor();
    let threshold = (SNAP_DISTANCE * scale).round() as i32;

    let is_seam = |edge_x: i32| -> bool {
        monitors.iter().any(|other| {
            let op = other.position();
            let os = other.size();
            let is_self = op.x == monitor.position().x
                && op.y == monitor.position().y
                && os.width == monitor.size().width
                && os.height == monitor.size().height;
            if is_self {
                return false;
            }
            let (ox, ow, oy, oh) = (op.x, os.width as i32, op.y, os.height as i32);
            let abuts = (ox + ow - edge_x).abs() < SEAM_GAP || (ox - edge_x).abs() < SEAM_GAP;
            let overlaps = oy < wy + wh && wy < oy + oh;
            abuts && overlaps
        })
    };

    let mut candidate: Option<(&'static str, i32)> = None;
    if !is_seam(fx) {
        let distance = (pos.x - fx).abs();
        if distance <= threshold {
            candidate = Some(("left", distance));
        }
    }
    if !is_seam(fr) {
        let distance = (pos.x + size.width as i32 - fr).abs();
        if distance <= threshold && candidate.is_none_or(|(_, best)| distance < best) {
            candidate = Some(("right", distance));
        }
    }
    candidate.map(|(edge, _)| SnapCtx {
        edge,
        fx,
        fr,
        wy,
        wh,
        scale,
    })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn strip_rect(ctx: &SnapCtx, center_y: i32) -> (i32, i32, u32, u32) {
    let strip_w = (DOCK_STRIP_WIDTH * ctx.scale).round().max(1.0) as u32;
    let strip_h = (DOCK_STRIP_HEIGHT * ctx.scale).round().max(1.0) as u32;
    (
        if ctx.edge == "left" {
            ctx.fx
        } else {
            ctx.fr - strip_w as i32
        },
        (center_y - strip_h as i32 / 2).clamp(ctx.wy, ctx.wy + ctx.wh - strip_h as i32),
        strip_w,
        strip_h,
    )
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn panel_rect(ctx: &SnapCtx, strip_y: i32) -> (i32, i32, u32, u32) {
    // The last slid-out rect wins (collapse remembers it); before the first
    // collapse, the pre-dock floating rect is the best guess. Only as a last
    // resort fall back to a default size aligned with the strip.
    // Reject strip-sized "memories" (Windows race can poison DOCK_PANEL).
    let pick = |rect: (i32, i32, u32, u32)| -> Option<(u32, u32, i32)> {
        if is_panel_physical_width(rect.2, ctx.scale) {
            Some((rect.2, rect.3, rect.1))
        } else {
            None
        }
    };
    let remembered = DOCK_PANEL
        .lock()
        .ok()
        .and_then(|stored| *stored)
        .and_then(pick)
        .or_else(|| {
            PRE_DOCK
                .lock()
                .ok()
                .and_then(|stored| *stored)
                .and_then(pick)
        });
    let (width, height, top) = match remembered {
        Some(rect) => rect,
        None => (
            (TRAY_WINDOW_WIDTH * ctx.scale).round().max(1.0) as u32,
            (320.0 * ctx.scale).round().max(1.0) as u32,
            strip_y,
        ),
    };
    (
        if ctx.edge == "left" {
            ctx.fx
        } else {
            ctx.fr - width as i32
        },
        top.clamp(ctx.wy, ctx.wy + ctx.wh - height as i32),
        width,
        height,
    )
}

/// Current cursor position in logical points (top-left origin, matching the
/// window rect divided by the scale factor).
#[cfg(target_os = "macos")]
fn cursor_position() -> Option<(f64, f64)> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let point = event.location();
    Some((point.x, point.y))
}

/// WKWebView hover is unreliable while another app is focused even with
/// acceptsMouseMovedEvents, so dock-mode hover runs on raw cursor polling:
/// over the strip → slide the panel out; away from the panel for 350ms →
/// slide back in. Works in any focus state, anytime.
#[cfg(target_os = "macos")]
fn spawn_dock_hover_watcher(window: tauri::WebviewWindow) {
    use std::sync::atomic::Ordering;
    std::thread::spawn(move || {
        let mut outside_since: Option<std::time::Instant> = None;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(120));
            // A hidden window must never be expanded by stale cursor matches
            // (its frame still reports the old rect while hidden).
            let visible = window.is_visible().unwrap_or(false);
            if !visible
                || !tray_docked()
                || TRAY_ANIMATING.load(Ordering::Relaxed)
                || TRAY_OVERLAY_OPEN.load(Ordering::Relaxed)
            {
                outside_since = None;
                continue;
            }
            let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
                continue;
            };
            let scale = window.scale_factor().unwrap_or(1.0);
            let Some((cursor_x, cursor_y)) = cursor_position() else {
                continue;
            };
            let px = (cursor_x * scale) as i32;
            let py = (cursor_y * scale) as i32;
            let inside = px >= pos.x - 4
                && px < pos.x + size.width as i32 + 4
                && py >= pos.y - 4
                && py < pos.y + size.height as i32 + 4;
            if !tray_expanded() {
                if inside {
                    expand_dock(&window, true);
                }
            } else if inside {
                outside_since = None;
            } else {
                let since = outside_since.get_or_insert_with(std::time::Instant::now);
                if since.elapsed().as_millis() >= 350 {
                    outside_since = None;
                    collapse_dock(&window, true);
                }
            }
        }
    });
}

/// Leave dock mode entirely. A collapsed strip first grows back into the
/// panel at the edge (instant, no tween) so no state is left stranded.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn exit_dock(window: &tauri::WebviewWindow) {
    if !tray_docked() {
        return;
    }
    if !tray_expanded() {
        if let Some(ctx) = snap_context(window) {
            if let Ok(pos) = window.outer_position() {
                let target = panel_rect(&ctx, pos.y);
                set_tray_physical_frame(window, target.0, target.1, target.2, target.3);
            }
        }
    }
    clear_dock_state(window);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn clear_dock_state(window: &tauri::WebviewWindow) {
    use std::sync::atomic::Ordering;
    if !tray_docked() {
        return;
    }
    TRAY_DOCKED.store(false, Ordering::Relaxed);
    TRAY_EXPANDED.store(false, Ordering::Relaxed);
    if let Ok(mut edge) = DOCK_EDGE.lock() {
        *edge = None;
    }
    if let Ok(mut panel) = DOCK_PANEL.lock() {
        *panel = None;
    }
    emit_dock_changed(window);
}

/// A tray-icon / sidebar open is a FRESH open: any dock or pin state is
/// dropped, and the panel always reappears centered at loading height.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn reopen_usage_tray(
    window: &tauri::WebviewWindow,
    monitor: Option<tauri::Monitor>,
) {
    use tauri::{Emitter, LogicalSize};

    clear_dock_state(window);
    let _ = window.set_size(LogicalSize::new(TRAY_WINDOW_WIDTH, TRAY_LOADING_HEIGHT));
    if let Some(monitor) = monitor {
        position_on_monitor(window, &monitor, TRAY_WINDOW_WIDTH, TRAY_LOADING_HEIGHT);
    }
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("usage-tray-opened", ());
}

/// Every Moved event (user drag or programmatic) funnels here.
/// - Docked strip moved → the user grabbed it: grow into the panel instantly
///   and let the drag continue with the panel.
/// - Moves settle while docked → near an edge: slide back into the strip;
///   away from every edge: leave dock mode, the panel stays where dropped.
/// - Moves settle while floating (and the burst looked like a real drag) →
///   near an edge: dock into the strip.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn handle_tray_moved(window: &tauri::WebviewWindow) {
    use std::sync::atomic::Ordering;
    // Ignore moves produced by our own set_size/set_position (dock tween,
    // strip height push, expand/collapse). On Windows those fire Moved and
    // were previously treated as "user grabbed the strip" → expand → collapse
    // with a strip-sized outer_size, poisoning the next hover expand.
    if TRAY_ANIMATING.load(Ordering::Relaxed) || geometry_recently_owned() {
        return;
    }
    if let Ok(mut last) = LAST_MOVE.lock() {
        *last = Some(std::time::Instant::now());
    }
    if tray_docked() && !tray_expanded() {
        expand_dock(window, false);
    }
    let generation = match MOVE_STATE.lock() {
        Ok(mut state) => {
            state.0 += 1;
            state.1 += 1;
            state.0
        }
        Err(_) => return,
    };
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(MOVE_SETTLE_MS));
        let burst = {
            let Ok(mut state) = MOVE_STATE.lock() else {
                return;
            };
            if state.0 != generation {
                return; // still moving
            }
            let burst = state.1;
            state.1 = 0;
            burst
        };
        // Drag ended. Pull the window back inside the desktop first (it may
        // have been dragged off-screen — mid-drag clamping is impossible
        // without fighting the native drag session), then decide.
        clamp_tray_into_monitors(&window);
        if tray_docked() {
            match snap_context(&window) {
                Some(_) => collapse_dock(&window, true),
                None => exit_dock(&window),
            }
        } else if burst >= MIN_DRAG_MOVES {
            // Drag ended: snap only once the cursor has stayed outside the
            // panel for SNAP_DELAY_MS (hovered drags never snap).
            schedule_edge_snap(&window);
        }
    });
}

/// Floating panel released at an edge → tween into the docked strip.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn try_snap_tray(window: &tauri::WebviewWindow) {
    use std::sync::atomic::Ordering;
    if tray_docked() || TRAY_ANIMATING.load(Ordering::Relaxed) {
        return;
    }
    let Some(ctx) = snap_context(window) else {
        return;
    };
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    // Only remember a real floating-panel size for later hover-expand.
    if is_panel_physical_width(size.width, ctx.scale) {
        if let Ok(mut pre) = PRE_DOCK.lock() {
            *pre = Some((pos.x, pos.y, size.width, size.height));
        }
    }
    TRAY_DOCKED.store(true, Ordering::Relaxed);
    TRAY_EXPANDED.store(false, Ordering::Relaxed);
    if let Ok(mut edge) = DOCK_EDGE.lock() {
        *edge = Some(ctx.edge);
    }
    let center_y = pos.y + size.height as i32 / 2;
    let target = strip_rect(&ctx, center_y);
    emit_dock_animating(window);
    animate_tray_window(window, (pos.x, pos.y, size.width, size.height), target, |win| {
        emit_dock_changed(win);
    });
}

/// Strip → panel. Animated on hover; instant when the user grabs the strip
/// and drags (the native drag keeps steering the now-full-size window).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn expand_dock(window: &tauri::WebviewWindow, animate: bool) {
    use std::sync::atomic::Ordering;
    if !tray_docked() || tray_expanded() {
        return;
    }
    let Some(ctx) = snap_context(window) else {
        return;
    };
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let target = panel_rect(&ctx, pos.y);
    TRAY_EXPANDED.store(true, Ordering::Relaxed);
    if animate {
        // Fade the strip out first; the panel state (and content) is only
        // published when the tween lands.
        emit_dock_animating(window);
        animate_tray_window(window, (pos.x, pos.y, size.width, size.height), target, |win| {
            emit_dock_changed(win);
        });
    } else {
        set_tray_physical_frame(window, target.0, target.1, target.2, target.3);
        emit_dock_changed(window);
    }
}

/// Panel → strip. The settle path may re-dock to a different edge (dragging
/// the expanded panel from one side of the screen to the other).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn collapse_dock(window: &tauri::WebviewWindow, animate: bool) {
    use std::sync::atomic::Ordering;
    if !tray_docked() || !tray_expanded() {
        return;
    }
    // Never collapse while a tween is still applying frames — the outer_size
    // mid-tween is not a valid panel memory.
    if TRAY_ANIMATING.load(Ordering::Relaxed) {
        return;
    }
    if let Some(ctx) = snap_context(window) {
        if let Ok(mut edge) = DOCK_EDGE.lock() {
            *edge = Some(ctx.edge);
        }
    }
    let Some(ctx) = snap_context(window) else {
        return;
    };
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    // Remember where the panel was so the next hover-expand returns here.
    // Skip strip-sized / half-applied frames (Windows async set_size race).
    store_dock_panel_rect(ctx.scale, (pos.x, pos.y, size.width, size.height));
    let center_y = pos.y + size.height as i32 / 2;
    let target = strip_rect(&ctx, center_y);
    if animate {
        emit_dock_animating(window);
        animate_tray_window(window, (pos.x, pos.y, size.width, size.height), target, |win| {
            TRAY_EXPANDED.store(false, Ordering::Relaxed);
            emit_dock_changed(win);
        });
    } else {
        TRAY_EXPANDED.store(false, Ordering::Relaxed);
        set_tray_physical_frame(window, target.0, target.1, target.2, target.3);
        emit_dock_changed(window);
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn animate_tray_window(
    window: &tauri::WebviewWindow,
    from: (i32, i32, u32, u32),
    to: (i32, i32, u32, u32),
    done: impl FnOnce(&tauri::WebviewWindow) + Send + 'static,
) {
    use std::sync::atomic::Ordering;
    TRAY_ANIMATING.store(true, Ordering::Relaxed);
    mark_owned_geometry();
    // NOTE: NSWindow.setFrame:display:animate: was tried here and reverted —
    // on a transparent, shadowless borderless window the system frame
    // animation visibly flickers. The per-frame IPC tween stays.
    let window = window.clone();
    std::thread::spawn(move || {
        const STEPS: u32 = 18;
        for i in 1..=STEPS {
            let t = i as f64 / STEPS as f64;
            let k = 1.0 - (1.0 - t).powi(3); // easeOutCubic
            let lerp = |a: f64, b: f64| a + (b - a) * k;
            mark_owned_geometry();
            let _ = window.set_size(tauri::PhysicalSize::new(
                lerp(from.2 as f64, to.2 as f64).round().max(1.0) as u32,
                lerp(from.3 as f64, to.3 as f64).round().max(1.0) as u32,
            ));
            let _ = window.set_position(tauri::PhysicalPosition::new(
                lerp(from.0 as f64, to.0 as f64).round() as i32,
                lerp(from.1 as f64, to.1 as f64).round() as i32,
            ));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // Hold the owned-geometry guard past the last frame so residual
        // Moved / Focused(false) from the final set_size are ignored.
        mark_owned_geometry();
        TRAY_ANIMATING.store(false, Ordering::Relaxed);
        done(&window);
        mark_owned_geometry();
    });
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn remembered_position() -> Option<(i32, i32)> {
    TRAY_POSITION.lock().ok().and_then(|stored| *stored)
}

/// Event emitted to the main window when the tray menu "Check for Updates"
/// item is clicked. The frontend opens About and runs the existing updater flow.
pub const TRAY_CHECK_UPDATES_EVENT: &str = "tray-check-updates";

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct TrayLabels {
    tooltip: &'static str,
    check_update: &'static str,
    quit: &'static str,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn tray_labels(locale: &str) -> TrayLabels {
    if locale.to_ascii_lowercase().starts_with("zh") {
        TrayLabels {
            tooltip: "监控面板",
            check_update: "检查更新",
            quit: "退出",
        }
    } else {
        TrayLabels {
            tooltip: "Monitor Panel",
            check_update: "Check for Updates",
            quit: "Quit",
        }
    }
}

/// Owned tray menu items so locale switches can rewrite their titles in place.
#[cfg(any(target_os = "macos", target_os = "windows"))]
struct TrayMenuHandles {
    check_update: tauri::menu::MenuItem<tauri::Wry>,
    quit: tauri::menu::MenuItem<tauri::Wry>,
}

#[tauri::command]
pub fn resize_usage_tray(app: AppHandle, height: f64, width: Option<f64>) {
    let height = height.clamp(TRAY_LOADING_HEIGHT, TRAY_MAX_HEIGHT);
    // None → normal 400; mini mode passes ~orb width. Never exceed normal width.
    let width = width
        .unwrap_or(TRAY_WINDOW_WIDTH)
        .clamp(TRAY_MINI_WIDTH, TRAY_WINDOW_WIDTH);

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        // Collapsed strip owns its size exclusively. Fighting it with a full
        // panel resize mid-dock was one path to a strip-sized DOCK_PANEL.
        if tray_docked() && !tray_expanded() {
            return;
        }
        if tray_docked()
            && tray_expanded()
            && !TRAY_ANIMATING.load(std::sync::atomic::Ordering::Relaxed)
        {
            // Content remeasure while slid out: keep the edge anchor and
            // refresh the remembered panel size for the next expand cycle.
            if let Some(window) = app.get_webview_window("codex-usage") {
                if let Some(ctx) = snap_context(&window) {
                    let scale = ctx.scale;
                    let w = (width * scale).round().max(1.0) as u32;
                    let h = (height * scale).round().max(1.0) as u32;
                    let top = window
                        .outer_position()
                        .map(|p| p.y)
                        .unwrap_or(ctx.wy);
                    let x = if ctx.edge == "left" {
                        ctx.fx
                    } else {
                        ctx.fr - w as i32
                    };
                    let y = top.clamp(ctx.wy, ctx.wy + ctx.wh - h as i32);
                    set_tray_physical_frame(&window, x, y, w, h);
                    store_dock_panel_rect(scale, (x, y, w, h));
                    return;
                }
            }
        }
        resize_centered_on_current_monitor(&app, width, height);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (app, width);
}

/// Open the usage popup from the main window (sidebar button). Same window,
/// same behavior as a tray-icon click: a fresh open — undocked, unpinned,
/// centered on the main window's monitor at compact loading height.
#[tauri::command]
pub fn open_usage_tray(app: AppHandle) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        if let Some(window) = app.get_webview_window("codex-usage") {
            let monitor = app
                .get_webview_window("main")
                .and_then(|main| main.current_monitor().ok().flatten())
                .or_else(|| window.current_monitor().ok().flatten());
            reopen_usage_tray(&window, monitor);
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

/// Refresh native tray menu / tooltip strings after the user switches language.
pub fn apply_locale(app: &AppHandle, locale: &str) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let labels = tray_labels(locale);
        if let Some(handles) = app.try_state::<TrayMenuHandles>() {
            let _ = handles.check_update.set_text(labels.check_update);
            let _ = handles.quit.set_text(labels.quit);
        }
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_tooltip(Some(labels.tooltip));
        }
        if let Some(window) = app.get_webview_window("codex-usage") {
            let _ = window.set_title(labels.tooltip);
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app, locale);
    }
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    setup_desktop(app)?;

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn current_locale_tag(app: &App) -> String {
    app.try_state::<std::sync::Mutex<crate::state::AppState>>()
        .and_then(|state| state.lock().ok().map(|s| s.locale.tag().to_string()))
        .unwrap_or_else(|| crate::i18n::Locale::detect().tag().to_string())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn setup_desktop(app: &mut App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder, WindowEvent};

    let labels = tray_labels(&current_locale_tag(app));

    let mut window_builder = WebviewWindowBuilder::new(
        app,
        "codex-usage",
        WebviewUrl::App("index.html?view=codex-usage".into()),
    )
    .title(labels.tooltip)
    .inner_size(TRAY_WINDOW_WIDTH, TRAY_LOADING_HEIGHT)
    .center()
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .transparent(true)
    .visible(false);

    #[cfg(target_os = "macos")]
    {
        window_builder = window_builder.visible_on_all_workspaces(true);
    }

    let window = window_builder.build()?;

    // Borderless background windows get no hover events by default, so the
    // docked strip could never slide open while another app is focused.
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWindow;
        if let Ok(ns_window) = window.ns_window() {
            unsafe { &*(ns_window as *const NSWindow) }.setAcceptsMouseMovedEvents(true);
        }
        spawn_dock_hover_watcher(window.clone());
    }

    let window_to_hide = window.clone();
    let window_for_moves = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            // Windows: tao's drag-region implementation fakes an HTCAPTION
            // click, which fires a spurious Focused(false) immediately
            // followed by Focused(true) on every mousedown (tauri#10767,
            // unfixed upstream). Hiding here would make the popup vanish as
            // soon as the user tries to drag it. Recheck the live focus state
            // after a short delay — the fake blur has recovered by then,
            // while a real click-away stays unfocused and still hides.
            let window = window_to_hide.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(80));
                // On query failure, err on the side of staying visible.
                if window.is_focused().unwrap_or(true) {
                    return;
                }
                // Docked mode never hides on blur; a real click-away instead
                // slides the expanded panel back into the strip so it does
                // not linger open while the user works elsewhere.
                // Skip when the blur is the synthetic one Windows fires after
                // our own set_size (expand/collapse/strip resize) — collapsing
                // then can read a still-strip outer_size and poison DOCK_PANEL.
                if tray_docked() {
                    if tray_expanded()
                        && !TRAY_ANIMATING.load(std::sync::atomic::Ordering::Relaxed)
                        && !geometry_recently_owned()
                    {
                        collapse_dock(&window, true);
                    }
                    return;
                }
                let _ = window.hide();
            });
        }
        WindowEvent::Moved(position) => {
            if let Ok(mut stored) = TRAY_POSITION.lock() {
                *stored = Some((position.x, position.y));
            }
            handle_tray_moved(&window_for_moves);
        }
        _ => {}
    });

    // Right-click context menu: Check for Updates + Quit.
    // Left click remains reserved for the usage popup (show_menu_on_left_click=false).
    let check_update =
        MenuItem::with_id(app, MENU_CHECK_UPDATE, labels.check_update, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&check_update, &quit])?;
    app.manage(TrayMenuHandles {
        check_update: check_update.clone(),
        quit: quit.clone(),
    });

    // Tray icon: monochrome cutout logo (transparent gaps), embedded as raw
    // RGBA to avoid the image-png feature. Rendered as a macOS template so the
    // silhouette adapts to light/dark menu bars.
    let icon = tauri::image::Image::new(include_bytes!("../icons/tray-icon.rgba"), 128, 128);

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID).icon(icon);
    #[cfg(target_os = "macos")]
    {
        tray_builder = tray_builder.icon_as_template(true);
    }

    tray_builder
        .menu(&menu)
        .tooltip(labels.tooltip)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                MENU_CHECK_UPDATE => {
                    // Hide the usage popup if open, then hand off to the main
                    // window's existing About / updater UI.
                    if let Some(usage) = app.get_webview_window("codex-usage") {
                        let _ = usage.hide();
                    }
                    if let Some(main) = app.get_webview_window("main") {
                        let _ = main.show();
                        let _ = main.unminimize();
                        let _ = main.set_focus();
                        let _ = main.emit(TRAY_CHECK_UPDATES_EVENT, ());
                    }
                }
                MENU_QUIT => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            else {
                return;
            };

            let app = tray.app_handle().clone();
            if let Some(window) = app.get_webview_window("codex-usage") {
                // Fresh open on the monitor that holds the tray icon.
                let window_scale = window.scale_factor().unwrap_or(1.0);
                let tray_position = rect.position.to_physical::<f64>(window_scale);
                let tray_size = rect.size.to_physical::<f64>(window_scale);
                let center_x = tray_position.x + tray_size.width / 2.0;
                let center_y = tray_position.y + tray_size.height / 2.0;
                let monitor = app
                    .monitor_from_point(center_x, center_y)
                    .ok()
                    .flatten()
                    .or_else(|| window.current_monitor().ok().flatten());
                reopen_usage_tray(&window, monitor);
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn resize_centered_on_current_monitor(app: &AppHandle, width: f64, height: f64) {
    use tauri::LogicalSize;

    let Some(window) = app.get_webview_window("codex-usage") else {
        return;
    };

    // Apply the requested content size first and unconditionally. Re-centering
    // is secondary and may legitimately be unavailable during a window resize.
    // A remembered position (dragged or previously centered) wins over
    // re-centering so resizing never yanks the popup back to screen center.
    mark_owned_geometry();
    let _ = window.set_size(LogicalSize::new(width, height));
    mark_owned_geometry();
    if remembered_position().is_none() {
        if let Ok(Some(monitor)) = window.current_monitor() {
            position_on_monitor(&window, &monitor, width, height);
            mark_owned_geometry();
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn position_on_monitor(
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
    width: f64,
    height: f64,
) {
    use tauri::{PhysicalPosition, Position};

    let scale = monitor.scale_factor();
    let window_width = width * scale;
    let window_height = height * scale;
    let (x, y) = centered_window_position(
        window_width,
        window_height,
        PhysicalBounds {
            x: monitor.position().x as f64,
            y: monitor.position().y as f64,
            width: monitor.size().width as f64,
            height: monitor.size().height as f64,
        },
    );

    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
#[derive(Clone, Copy)]
struct PhysicalBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn centered_window_position(
    window_width: f64,
    window_height: f64,
    screen: PhysicalBounds,
) -> (i32, i32) {
    let x = screen.x + (screen.width - window_width).max(0.0) / 2.0;
    let y = screen.y + (screen.height - window_height).max(0.0) / 2.0;
    (x.round() as i32, y.round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_window_is_centered_on_the_active_monitor() {
        let screen = PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: 3_024.0,
            height: 1_964.0,
        };
        let position = centered_window_position(800.0, 1_040.0, screen);
        assert_eq!(position, (1_112, 462));
    }

    #[test]
    fn tray_window_centers_on_a_monitor_with_a_nonzero_origin() {
        let screen = PhysicalBounds {
            x: 3_024.0,
            y: -120.0,
            width: 1_920.0,
            height: 1_080.0,
        };
        let position = centered_window_position(800.0, 1_040.0, screen);
        assert_eq!(position, (3_584, -100));
    }
}
