import { ref, type Ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { collapseUsageTray, expandUsageTray } from '@/lib/api'

/**
 * Edge-dock ("吸附") state for the usage tray window, macOS-Dock style:
 * the panel snaps to a monitor's outer left/right edge and collapses into a
 * thin strip; hovering the edge slides the panel back out, moving the cursor
 * away slides it back in. Dragging the expanded panel away from the edge
 * leaves dock mode entirely.
 *
 * All geometry, drag detection and animation live in the backend (`tray.rs`)
 * because the native `WindowEvent::Moved` stream is the only drag signal
 * guaranteed to arrive (Tauri's drag-region script may swallow the webview's
 * mousedown). The backend emits `usage-tray-dock-changed`; this composable
 * mirrors that state and forwards hover expand/collapse intents.
 */

export type DockEdge = 'left' | 'right'

export interface TrayDockOptions {
  /** Invoked when dock mode is exited so the panel can re-measure itself. */
  onUndock?: () => void
}

export function useTrayDock(options: TrayDockOptions = {}) {
  const docked: Ref<DockEdge | null> = ref(null)
  const dockExpanded = ref(false)
  /** True during a dock/undock size tween: the current content fades out and
   *  the new state only renders when the tween lands (dock-changed event). */
  const dockAnimating = ref(false)
  const isWeb = import.meta.env.MODE === 'web'
  let unlisten: UnlistenFn | null = null
  let unlistenAnimating: UnlistenFn | null = null

  async function init() {
    if (isWeb) return
    try {
      unlisten = await listen<{ edge: DockEdge | null; expanded: boolean }>(
        'usage-tray-dock-changed',
        event => {
          dockAnimating.value = false
          docked.value = event.payload.edge
          dockExpanded.value = event.payload.expanded
          if (!event.payload.edge) options.onUndock?.()
        },
      )
      unlistenAnimating = await listen('usage-tray-dock-animating', () => {
        dockAnimating.value = true
      })
    } catch {
      // No native tray window (browser preview): docking stays unavailable.
    }
  }

  /** Cursor entered the strip → ask the backend to slide the panel out. */
  function expand() {
    if (!docked.value || dockExpanded.value) return
    void expandUsageTray().catch(() => {})
  }

  /** Cursor left the panel → ask the backend to slide back into the strip.
   *  The backend ignores this while a drag is in progress. */
  function collapse() {
    if (!docked.value || !dockExpanded.value) return
    void collapseUsageTray().catch(() => {})
  }

  function dispose() {
    unlisten?.()
    unlisten = null
    unlistenAnimating?.()
    unlistenAnimating = null
  }

  return { docked, dockExpanded, dockAnimating, expand, collapse, init, dispose }
}
