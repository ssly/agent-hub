<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Activity, BarChart3, Blend, CreditCard, Maximize2, Minimize2, RefreshCw, Timer, X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useToast } from '@/composables/useToast'
import AppToast from '@/components/layout/AppToast.vue'
import {
  getAntigravitySessionMonitorSnapshot,
  getClaudeSessionMonitorSnapshot,
  getClaudeUsage,
  getCodexSessionMonitorSnapshot,
  getCodexTrayUsage,
  getCursorSessionMonitorSnapshot,
  getGrokSessionMonitorSnapshot,
  getOmpSessionMonitorSnapshot,
  getKimiSessionMonitorSnapshot,
  getKiroSessionMonitorSnapshot,
  getKiroUsage,
  markSessionMonitorSessionRead,
  getQwenSessionMonitorSnapshot,
  getWorkbuddySessionMonitorSnapshot,
  getDshSessionMonitorSnapshot,
  getOpencodeSessionMonitorSnapshot,
  getUsageProviderAvailability,
  locateMonitorSession,
  getUsageMonitorSettings,
  getZCodeSessionMonitorSnapshot,
  listAvailableMonitorAgents,
  resizeUsageTray,
  resizeUsageTrayDock,
  closeUsageTray,
  setUsageRefreshMinutes,
  setUsageSelectedAgent,
  setUsageTrayHovered,
  setUsageTrayOverlay,
} from '@/lib/api'
import type {
  ClaudeUsage,
  CodexTraySnapshot,
  KiroUsage,
  ResetCreditEntry,
  UsageMonitorSettings,
  UsageProviderAvailability,
  UsageWindow,
} from '@/lib/api'
import { monitorStatusRank, type AgentSessionState, type MonitorAgent, type MonitorSnapshot, type RuntimeStatus } from '@/stores/session-monitor'
import AgentIcon from '@/components/agents/AgentIcon.vue'
import SessionClientIcon from '@/components/sessions/SessionClientIcon.vue'
import UsageOrb, { type OrbTone, type OrbWindow } from './UsageOrb.vue'
import UsageOrbPlaceholder from './UsageOrbPlaceholder.vue'
import TrayWaveLoader from './TrayWaveLoader.vue'
import { useTrayDock } from './useTrayDock'
import { compactSessionPreview } from '@/lib/session-display'

const { t, locale } = useI18n()
const { showToast } = useToast()
type UsageProvider = 'codex' | 'claude-code' | 'kiro'
const selectedProvider = ref<UsageProvider>(preferredProviderFromAccounts())
const availability = ref<UsageProviderAvailability | null>(null)
const snapshot = ref<CodexTraySnapshot | null>(null)
const claudeUsage = ref<ClaudeUsage | null>(null)
const kiroUsage = ref<KiroUsage | null>(null)
// The real tray window starts hidden and compact. Defaulting to loading avoids
// a flash of the empty-state layout before the first tray-click event arrives.
const providerLoading = ref<Record<UsageProvider, boolean>>({
  codex: false,
  'claude-code': false,
  kiro: false,
})
const loading = computed(() => providerLoading.value[selectedProvider.value])
const compactLoading = ref(import.meta.env.MODE !== 'web')
const providerErrors = ref<Record<UsageProvider, string | null>>({
  codex: null,
  'claude-code': null,
  kiro: null,
})
const error = computed(() => providerErrors.value[selectedProvider.value])
const errorMessage = computed(() => error.value?.trim() || t('tray.failed_hint'))
// Last successful query time for the visible provider, shown in the top band
// next to the refresh/pin buttons (no footer row needed).
const lastQueryAt = computed(() => {
  if (selectedProvider.value === 'codex') return snapshot.value?.last_query_at ?? null
  if (selectedProvider.value === 'claude-code') return claudeUsage.value?.fetched_at ?? null
  if (selectedProvider.value === 'kiro') return kiroUsage.value?.fetched_at ?? null
  return null
})
const loginUnavailable = ref(false)
const queriedProviders = ref<Record<UsageProvider, boolean>>({
  codex: false,
  'claude-code': false,
  kiro: false,
})
const unlisteners: UnlistenFn[] = []
// Compact tray layout: ring-only usage, short labels, no opacity control.
const MINI_STORAGE_KEY = 'ah-tray-mini'
const miniMode = ref(localStorage.getItem(MINI_STORAGE_KEY) === '1')
const opacityOpen = ref(false)
function setMiniMode(next: boolean) {
  miniMode.value = next
  localStorage.setItem(MINI_STORAGE_KEY, next ? '1' : '0')
  opacityOpen.value = false
}
// The mini toggle is hidden from the UI (the code path stays intact for a
// future re-enable). Never leave a stored mini=1 behind with no way out.
const SHOW_MINI_TOGGLE = false
if (!SHOW_MINI_TOGGLE && miniMode.value) setMiniMode(false)
const refreshSequences: Record<UsageProvider, number> = {
  codex: 0,
  'claude-code': 0,
  kiro: 0,
}
let resizeSequence = 0
// Keep in sync with the .reset-credit-popover transition duration: the native
// window only shrinks again once the close fade has finished.
const RESET_CREDIT_FADE_MS = 160

// --- Mini monitor strip ---------------------------------------------------
// Same data the Monitor tab shows (backend snapshots + change events), but
// reduced to one line per session: status dot + agent + user question.
// Same order as MONITOR_AGENTS / platform registry (monitor subset).
const MONITOR_AGENTS_LIST: MonitorAgent[] = ['codex', 'claude', 'cursor', 'antigravity', 'grok', 'kimi', 'qwen', 'zcode', 'workbuddy', 'kiro', 'dsh', 'omp', 'opencode']
const MONITOR_CHANGED_EVENTS: Record<MonitorAgent, string> = {
  codex: 'session-monitor:codex-changed',
  claude: 'session-monitor:claude-changed',
  cursor: 'session-monitor:cursor-changed',
  antigravity: 'session-monitor:antigravity-changed',
  grok: 'session-monitor:grok-changed',
  kimi: 'session-monitor:kimi-changed',
  qwen: 'session-monitor:qwen-changed',
  zcode: 'session-monitor:zcode-changed',
  workbuddy: 'session-monitor:workbuddy-changed',
  kiro: 'session-monitor:kiro-changed',
  dsh: 'session-monitor:dsh-changed',
  omp: 'session-monitor:omp-changed',
  opencode: 'session-monitor:opencode-changed',
}
const MONITOR_SNAPSHOT_API: Record<MonitorAgent, () => Promise<MonitorSnapshot>> = {
  codex: getCodexSessionMonitorSnapshot,
  claude: getClaudeSessionMonitorSnapshot,
  cursor: getCursorSessionMonitorSnapshot,
  antigravity: getAntigravitySessionMonitorSnapshot,
  grok: getGrokSessionMonitorSnapshot,
  kimi: getKimiSessionMonitorSnapshot,
  qwen: getQwenSessionMonitorSnapshot,
  zcode: getZCodeSessionMonitorSnapshot,
  workbuddy: getWorkbuddySessionMonitorSnapshot,
  kiro: getKiroSessionMonitorSnapshot,
  dsh: getDshSessionMonitorSnapshot,
  omp: getOmpSessionMonitorSnapshot,
  opencode: getOpencodeSessionMonitorSnapshot,
}
const monitorSnapshots = ref<Record<MonitorAgent, MonitorSnapshot>>({
  codex: { revision: 0, sessions: [] },
  claude: { revision: 0, sessions: [] },
  cursor: { revision: 0, sessions: [] },
  antigravity: { revision: 0, sessions: [] },
  grok: { revision: 0, sessions: [] },
  kimi: { revision: 0, sessions: [] },
  qwen: { revision: 0, sessions: [] },
  zcode: { revision: 0, sessions: [] },
  workbuddy: { revision: 0, sessions: [] },
  kiro: { revision: 0, sessions: [] },
  dsh: { revision: 0, sessions: [] },
  omp: { revision: 0, sessions: [] },
  opencode: { revision: 0, sessions: [] },
})

// A snapshot request may finish after a newer monitor event. Revisions are
// persisted by the backend, so never let that older response undo unread
// state already received by this tray window.
function applyMonitorSnapshot(agent: MonitorAgent, next: MonitorSnapshot) {
  if (next.revision < monitorSnapshots.value[agent].revision) return
  monitorSnapshots.value[agent] = next
}

// Agents whose platform presence directory exists (backend probe). null =
// not loaded yet / probe failed → show every agent, matching the Monitor tab.
const availableMonitorAgents = ref<MonitorAgent[] | null>(null)
const visibleMonitorAgents = computed<MonitorAgent[]>(() =>
  availableMonitorAgents.value === null
    ? MONITOR_AGENTS_LIST
    : MONITOR_AGENTS_LIST.filter(agent => availableMonitorAgents.value!.includes(agent)),
)

async function loadMonitorAvailability() {
  try {
    const ids = await listAvailableMonitorAgents()
    availableMonitorAgents.value = MONITOR_AGENTS_LIST.filter(agent => ids.includes(agent))
  } catch {
    // Degrade to the full list — availability must never blank the strip.
  }
}

// --- Shared usage-monitor settings (backend file + event) ------------------
// The same snapshot drives the Accounts view and its sidebar settings modal;
// `usage-monitor-settings-changed` keeps this popup in sync live.
const monitorSettings = ref<UsageMonitorSettings | null>(null)
const refreshMinutes = computed(() => monitorSettings.value?.refreshMinutes ?? 5)
const monitorLimit = computed(() => monitorSettings.value?.monitorLimit ?? 6)

// Merged like the Monitor tab's "all" view: unread/newest first, then waiting,
// then running — shared by the capped panel list and the uncapped strip counts.
function compareMonitorRows(a: AgentSessionState, b: AgentSessionState): number {
  const rank = monitorStatusRank(a.status, a.unread) - monitorStatusRank(b.status, b.unread)
  if (rank !== 0) return rank
  return b.updatedAt - a.updatedAt
}

/** Tray strip shows at most monitorLimit (6-12) sessions; tip placement splits at the midpoint. */
const monitorRows = computed<AgentSessionState[]>(() =>
  visibleMonitorAgents.value
    .flatMap(agent => monitorSnapshots.value[agent].sessions.map(session => ({ ...session, agent })))
    .sort(compareMonitorRows)
    .slice(0, monitorLimit.value),
)

// --- Edge dock (吸附) -------------------------------------------------------
// macOS-Dock style: drag the panel to a monitor's outer left/right edge and
// the backend (tray.rs) tweens it into a thin strip with a tiny traffic
// light; hovering the strip slides the panel back out, moving the cursor
// away slides it in again; dragging the expanded panel away from the edge
// leaves dock mode. The strip never auto-hides on blur and dock state is
// backend-memory-only (same "forgotten on exit" semantics as the position).
const { docked, dockExpanded, dockAnimating, expand: expandDock, collapse: collapseDock, init: initDock, dispose: disposeDock } =
  useTrayDock({
    onUndock: () => {
      void applyContentHeight()
    },
  })

const trayVisible = ref(import.meta.env.MODE === 'web')
const relativeNow = ref(Date.now())
let relativeClock: ReturnType<typeof window.setInterval> | undefined

function stopRelativeClock() {
  if (relativeClock != null) {
    window.clearInterval(relativeClock)
    relativeClock = undefined
  }
}

function syncRelativeClock(recalibrate = false) {
  const shouldRun = trayVisible.value
    && document.visibilityState === 'visible'
    && (!docked.value || dockExpanded.value)
  if (!shouldRun) {
    stopRelativeClock()
    return
  }
  if (recalibrate) relativeNow.value = Date.now()
  if (relativeClock == null) {
    relativeClock = window.setInterval(() => {
      relativeNow.value = Date.now()
    }, 60_000)
  }
}

function handleTrayClosed() {
  trayVisible.value = false
  stopRelativeClock()
  clearNativeMonitorHover()
  collapseResetCredit(true)
}

function handleDocumentVisibilityChange() {
  syncRelativeClock(true)
}

// Collapse on hover-leave with a small grace delay (cancelled on re-enter),
// and never while a popover slider is open.
let collapseTimer: ReturnType<typeof setTimeout> | undefined

// Cursor left the window: tell the backend (edge snap fires only after the
// cursor has stayed out for 500ms — never mid-drag, never while hovering);
// a docked panel additionally slides back into the strip (grace delay,
// cancelled on re-enter).
function handleShellMouseLeave() {
  void setUsageTrayHovered(false).catch(() => {})
  if (docked.value) {
    scheduleDockCollapse()
  }
}

function handleShellMouseEnter() {
  void setUsageTrayHovered(true).catch(() => {})
  cancelDockCollapse()
}

function scheduleDockCollapse() {
  if (!docked.value || !dockExpanded.value) return
  if (opacityOpen.value || intervalOpen.value) return
  if (collapseTimer) clearTimeout(collapseTimer)
  collapseTimer = setTimeout(() => collapseDock(), 350)
}

function cancelDockCollapse() {
  if (collapseTimer) {
    clearTimeout(collapseTimer)
    collapseTimer = undefined
  }
}

/** Strip content: usage bars + counted status lamps — each half follows the
 *  panel's own hide toggles (usageHidden / monitorHidden). The native strip
 *  long axis follows whatever is actually shown (height on left/right,
 *  width on top after a CCW 90° rotate); the watcher lives next to
 *  stripUsage (see below) because watch getters evaluate eagerly and must
 *  not touch TDZ bindings. These constants only seed the first paint — the
 *  strip then re-sizes to the measured content box. */
const STRIP_GAP_PX = 3
const STRIP_PAD_PX = 5
const STRIP_BAR_HEIGHT = 48
const STRIP_THICK_PX = 20
const STRIP_MIN_LONG_PX = 24
/** One lamp is a 15px count badge; the 3px gap that follows it comes from
 *  STRIP_GAP_PX. Seeded for the first paint only — the strip then re-sizes to
 *  the measured content box. */
const STRIP_LAMP_LONG_PX = 15

/** Upper half rows: tip below; lower half: tip above — keeps long prompts inside the panel. */
function monitorTipPlacement(index: number): 'top' | 'bottom' {
  return index < Math.ceil(monitorLimit.value / 2) ? 'bottom' : 'top'
}

/** Full-mode text label (mini uses icons only).
 *  Only refine the label when source is a real, verified client split
 *  (Codex→ChatGPT, Antigravity CLI/app/IDE). Kiro hooks cannot tell CLI
 *  from IDE, so stay on the plain product name "Kiro". */
function monitorAgentLabel(row: AgentSessionState) {
  if (row.agent === 'codex' && row.source === 'chatgpt') {
    return t('session_monitor.source_chatgpt')
  }
  if (row.agent === 'antigravity') {
    if (row.source === 'terminal') return t('session.source_antigravity_cli')
    if (row.source === 'antigravity-ide') return t('session_monitor.source_antigravity_ide')
    if (row.source === 'antigravity') return t('session_monitor.source_antigravity')
  }
  return t(`session_monitor.agent_${row.agent}`)
}

/** Mini strip: ChatGPT client icon vs platform AgentIcon. */
function monitorIsChatgpt(row: AgentSessionState) {
  return row.agent === 'codex' && row.source === 'chatgpt'
}

function formatMonitorRelative(timestamp: number): string {
  if (!timestamp) return ''
  const elapsed = Math.max(0, relativeNow.value - timestamp)
  const minutes = Math.max(1, Math.floor(elapsed / 60_000))
  if (minutes < 60) return t('session_monitor.relative_minutes', { count: minutes })
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return t('session_monitor.relative_hours', { count: hours })
  return t('session_monitor.relative_days', { count: Math.floor(hours / 24) })
}

function formatMonitorExactTime(timestamp: number): string {
  if (!timestamp) return ''
  const date = new Date(timestamp)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat(locale.value, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(date)
}

async function loadMonitorSnapshots() {
  const agents = visibleMonitorAgents.value
  const results = await Promise.allSettled(
    agents.map(agent => MONITOR_SNAPSHOT_API[agent]()),
  )
  agents.forEach((agent, index) => {
    const result = results[index]
    if (result.status === 'fulfilled' && result.value) {
      applyMonitorSnapshot(agent, result.value)
    }
  })
}

type EffectiveStatus = 'running' | 'waiting' | 'unread' | 'ended'

function rowEffectiveStatus(row: AgentSessionState): EffectiveStatus {
  if (row.status === 'ended' && row.unread) return 'unread'
  if (row.status === 'waiting') return 'waiting'
  if (row.status === 'running') return 'running'
  return 'ended'
}

function monitorStatusLabel(row: AgentSessionState): string {
  const status = rowEffectiveStatus(row)
  if (status === 'running') return t('session_monitor.status_running')
  if (status === 'waiting') return t('session_monitor.status_waiting')
  if (status === 'unread') return t('session_monitor.status_unread')
  return t('session_monitor.status_ended')
}

function monitorPrompt(row: AgentSessionState): string {
  return compactSessionPreview(row.userPrompt) || t('session_monitor.no_prompt')
}

// Row identity for hover detection and row tracking. Deliberately
// session-scoped, NOT turn-scoped.
function monitorRowKey(row: AgentSessionState) {
  return `${row.agent}:${row.sessionId}`
}

const hoveredMonitorRowKey = ref<string | null>(null)

interface NativeTrayHoverPayload {
  x: number | null
  y: number | null
}

// macOS supplies this only while the docked tray is expanded but unfocused.
// A brief dwell keeps an expanding panel from acknowledging a row that merely
let nativeHoveredMonitorRowKey: string | null = null
let nativeHoverPoint: NativeTrayHoverPayload | null = null

function clearNativeMonitorHover() {
  const key = nativeHoveredMonitorRowKey
  nativeHoveredMonitorRowKey = null
  nativeHoverPoint = null
  if (key && hoveredMonitorRowKey.value === key) {
    hoveredMonitorRowKey.value = null
  }
}

function nativeHoveredMonitorRow() {
  const point = nativeHoverPoint
  if (!point || point.x == null || point.y == null) return null
  const key = document
    .elementFromPoint(point.x, point.y)
    ?.closest<HTMLElement>('[data-monitor-row-key]')
    ?.dataset.monitorRowKey
  return monitorRows.value.find(row => monitorRowKey(row) === key) ?? null
}

function syncNativeMonitorHover() {
  if (!docked.value || !dockExpanded.value || dockAnimating.value) {
    clearNativeMonitorHover()
    return
  }
  const row = nativeHoveredMonitorRow()
  const key = row ? monitorRowKey(row) : null
  if (key === nativeHoveredMonitorRowKey) return

  clearNativeMonitorHover()
  if (!row || !key) return

  nativeHoveredMonitorRowKey = key
  hoveredMonitorRowKey.value = key
}

function handleNativeTrayHover(payload: NativeTrayHoverPayload) {
  nativeHoverPoint = payload.x == null || payload.y == null ? null : payload
  void nextTick(syncNativeMonitorHover)
}

function markMonitorRowRead(row: AgentSessionState) {
  if (!row.unread) return
  const observedUpdatedAt = row.updatedAt
  void markSessionMonitorSessionRead(row.agent, row.sessionId, observedUpdatedAt)
    .then(() => {
      const current = monitorSnapshots.value[row.agent]
      const latest = current.sessions.find(session => session.sessionId === row.sessionId)
      if (!latest?.unread || latest.updatedAt !== observedUpdatedAt) return
      monitorSnapshots.value[row.agent] = {
        ...current,
        sessions: current.sessions.map(session =>
          session.sessionId === row.sessionId && session.updatedAt === observedUpdatedAt
            ? { ...session, unread: false }
            : session,
        ),
      }
    })
    .catch(() => {
      // A later monitor snapshot remains authoritative; a failed hover write
      // should not disturb the tray or turn into a usage-query error.
    })
}

function markAllMonitorRowsRead() {
  for (const row of monitorRows.value) {
    if (row.unread) {
      markMonitorRowRead(row)
    }
  }
}

async function handleMonitorRowClick(row: AgentSessionState) {
  if (row.unread) {
    markMonitorRowRead(row)
  }
  try {
    await locateMonitorSession(row.agent, row.sessionId, row.turnId)
  } catch (err) {
    console.error('[tray] failed to locate monitor session', err)
  }
}

function clearHoveredMonitorRow(row: AgentSessionState) {
  if (hoveredMonitorRowKey.value === monitorRowKey(row)) {
    hoveredMonitorRowKey.value = null
  }
}



// --- Top-left controls (opacity / hide usage / hide monitor / mini) ---------
// Opacity: compact slider (normal mode only). Usage / monitor: whole-section
// toggles — the two sections cannot both be hidden. Mini mode drops the
// opacity control and simplifies labels / orb layout.
const OPACITY_MIN = 70
const OPACITY_MAX = 100
const OPACITY_STORAGE_KEY = 'ah-tray-opacity'
const storedOpacity = Number(localStorage.getItem(OPACITY_STORAGE_KEY))
const panelOpacity = ref(
  Number.isFinite(storedOpacity)
    ? Math.min(OPACITY_MAX, Math.max(OPACITY_MIN, Math.round(storedOpacity)))
    : 100,
)

const USAGE_HIDDEN_KEY = 'ah-tray-usage-hidden'
const MONITOR_HIDDEN_KEY = 'ah-tray-monitor-hidden'
const PROVIDER_ORDER: UsageProvider[] = ['codex', 'claude-code', 'kiro']

function loadUsageHidden(): boolean {
  const flag = localStorage.getItem(USAGE_HIDDEN_KEY)
  if (flag !== null) return flag === '1'
  try {
    const legacy = JSON.parse(localStorage.getItem('ah-tray-hidden-usage') ?? '[]')
    return Array.isArray(legacy) && legacy.length >= PROVIDER_ORDER.length
  } catch {
    return false
  }
}
function loadMonitorHidden(): boolean {
  const flag = localStorage.getItem(MONITOR_HIDDEN_KEY)
  if (flag !== null) return flag === '1'
  try {
    const legacy = JSON.parse(localStorage.getItem('ah-tray-hidden-monitor') ?? '[]')
    return Array.isArray(legacy) && legacy.length >= MONITOR_AGENTS_LIST.length
  } catch {
    return false
  }
}
const usageHidden = ref(loadUsageHidden())
const monitorHidden = ref(loadMonitorHidden())
// Legacy dual-hide is invalid under the new rule — force both visible.
if (usageHidden.value && monitorHidden.value) {
  usageHidden.value = false
  monitorHidden.value = false
  localStorage.setItem(USAGE_HIDDEN_KEY, '0')
  localStorage.setItem(MONITOR_HIDDEN_KEY, '0')
}

function persistSectionVisibility() {
  localStorage.setItem(USAGE_HIDDEN_KEY, usageHidden.value ? '1' : '0')
  localStorage.setItem(MONITOR_HIDDEN_KEY, monitorHidden.value ? '1' : '0')
}

function toggleUsageHidden() {
  opacityOpen.value = false
  // Hiding usage while monitor is already hidden would leave the panel empty.
  if (!usageHidden.value && monitorHidden.value) {
    showToast(t('tray.cannot_hide_both'), 'warning')
    return
  }
  usageHidden.value = !usageHidden.value
  persistSectionVisibility()
}

function toggleMonitorHidden() {
  opacityOpen.value = false
  if (!monitorHidden.value && usageHidden.value) {
    showToast(t('tray.cannot_hide_both'), 'warning')
    return
  }
  monitorHidden.value = !monitorHidden.value
  persistSectionVisibility()
}

function toggleOpacityOpen() {
  intervalOpen.value = false
  opacityOpen.value = !opacityOpen.value
}

function onOpacityInput(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (!Number.isFinite(value)) return
  panelOpacity.value = Math.min(OPACITY_MAX, Math.max(OPACITY_MIN, Math.round(value)))
}

function persistOpacity() {
  localStorage.setItem(OPACITY_STORAGE_KEY, String(panelOpacity.value))
}

/** Absent key = paused (default off). */
function isListened(provider: UsageProvider) {
  return monitorSettings.value?.listening?.[provider] ?? false
}
// Keep the full tray visible while usage is loading if the selected provider
// is being monitored, so the live monitor strip remains available below it.
const keepPanelVisibleDuringLoading = computed(
  () => !monitorHidden.value && isListened(selectedProvider.value),
)
const initialLoading = computed(
  () => compactLoading.value
    && !keepPanelVisibleDuringLoading.value
    && visibleProviders.value.length <= 1,
)
/** Applies a settings snapshot; follows a selected-agent change from the
 *  other window like a local tab click (queries when unheard). A paused
 *  agent is never followed — its tab is not even visible. */
function applySharedSettings(next: UsageMonitorSettings) {
  monitorSettings.value = next
  const shared = next.selectedAgent as UsageProvider | null
  if (shared && PROVIDER_ORDER.includes(shared) && isListened(shared) && shared !== selectedProvider.value) {
    if (!availability.value || providerAvailable(shared, availability.value)) {
      selectedProvider.value = shared
      if (!queriedProviders.value[shared] && document.visibilityState === 'visible') {
        void refresh(false, false, false)
      }
    }
  }
}

// Refresh-interval popover (1–10 min slider), sibling of the opacity one.
const intervalOpen = ref(false)
const intervalDraft = ref<number | null>(null)
function toggleIntervalOpen() {
  intervalOpen.value = !intervalOpen.value
  if (intervalOpen.value) {
    opacityOpen.value = false
    intervalDraft.value = null
  }
}
function onIntervalInput(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isFinite(value)) intervalDraft.value = Math.min(10, Math.max(1, Math.round(value)))
}
async function persistInterval() {
  if (intervalDraft.value == null) return
  monitorSettings.value = await setUsageRefreshMinutes(intervalDraft.value)
  intervalDraft.value = null
}

// While a popover slider is open the cursor may legitimately leave the panel
// bounds — tell the backend's dock hover watcher to suspend auto-collapse.
watch([opacityOpen, intervalOpen], ([opacity, interval]) => {
  void setUsageTrayOverlay(opacity || interval).catch(() => {})
})

// Providers the panel can actually query right now — only these appear as tabs.
const queryableProviders = computed<UsageProvider[]>(() =>
  PROVIDER_ORDER.filter(provider =>
    availability.value ? providerAvailable(provider, availability.value) : false,
  ),
)
// Tabs = queryable AND listened providers. An agent paused in Accounts
// (listening off) disappears from the switch entirely.
const visibleProviders = computed<UsageProvider[]>(() =>
  queryableProviders.value.filter(provider => isListened(provider)),
)

// A provider that becomes unavailable (or paused) can no longer be selected;
// fall to the first visible tab and query it if unheard. Skipped while the
// popup is hidden — opening it re-queries anyway.
watch(visibleProviders, list => {
  if (list.length && !list.includes(selectedProvider.value)) {
    selectedProvider.value = list[0]
    if (!queriedProviders.value[list[0]] && document.visibilityState === 'visible') {
      void refresh(false, false, false)
    }
  }
})

// Re-query the quota on the shared refresh interval whenever the window is
// visible (floating open, or docked — the strip shows the usage bar). A
// paused agent (listening off) is never auto-queried. Monitor rows stay
// event-driven real-time.
let quotaTimer: number | undefined
watch(refreshMinutes, minutes => {
  window.clearInterval(quotaTimer)
  quotaTimer = window.setInterval(() => {
    if (document.visibilityState !== 'visible') return
    if (!isListened(selectedProvider.value)) return
    void refresh(false, false, true)
  }, minutes * 60_000)
}, { immediate: true })

async function closeTray() {
  handleTrayClosed()
  try {
    await closeUsageTray()
  } catch {
    // Browser preview has no native window to close.
  }
}

// The Accounts view listens for this and re-pulls the shared backend cache,
// so a force refresh here shows the same fresh numbers over there.
async function broadcastUsageRefreshed(provider: UsageProvider) {
  try {
    const { emit } = await import('@tauri-apps/api/event')
    await emit('usage-refreshed', { provider })
  } catch {
    // Browser preview has no Tauri event bus.
  }
}

type WindowTone = OrbTone
type TrayUsageWindow = OrbWindow

function preferredProviderFromAccounts(): UsageProvider {
  const stored = localStorage.getItem('ah-switch-agent')
  if (stored === 'claude-code') return 'claude-code'
  if (stored === 'kiro') return 'kiro'
  return 'codex'
}

function providerAvailable(provider: UsageProvider, status: UsageProviderAvailability) {
  if (provider === 'codex') return status.codex
  if (provider === 'claude-code') return status.claude_code
  return Boolean(status.kiro)
}

const PROVIDER_LABELS: Record<UsageProvider, string> = {
  codex: 'Codex',
  'claude-code': 'Claude Code',
  kiro: 'Kiro',
}
function providerLabel(provider: UsageProvider) {
  return PROVIDER_LABELS[provider]
}

// Fallback order mirrors the provider-switch tab order so a preferred provider
// that is signed out OR paused falls back to the next usable one
// deterministically.
function availableProvider(
  preferred: UsageProvider,
  status: UsageProviderAvailability,
): UsageProvider | null {
  if (providerAvailable(preferred, status) && isListened(preferred)) return preferred
  for (const candidate of PROVIDER_ORDER) {
    if (candidate !== preferred && providerAvailable(candidate, status) && isListened(candidate)) {
      return candidate
    }
  }
  return null
}

function windowLabel(seconds: number) {
  if (Math.abs(seconds - 18_000) <= 600) return '5h'
  if (Math.abs(seconds - 604_800) <= 3_600) return '7d'
  if (Math.abs(seconds - 2_592_000) <= 86_400) return '30d'
  if (seconds >= 86_400) return `${Math.round(seconds / 86_400)}d`
  return `${Math.round(seconds / 3_600)}h`
}

function windowTone(seconds: number): WindowTone {
  if (Math.abs(seconds - 2_592_000) <= 86_400 || seconds > 1_209_600) return 'monthly'
  if (seconds >= 86_400) return 'secondary'
  return 'primary'
}

const usageWindows = computed<TrayUsageWindow[]>(() => {
  const returned = snapshot.value?.usage.usage_windows ?? []
  const fallback = [
    snapshot.value?.usage.primary_window,
    snapshot.value?.usage.secondary_window,
  ].filter((window): window is UsageWindow => Boolean(window?.window_seconds))
  const windows = (returned.length ? returned : fallback)
    .filter(window => window.window_seconds > 0)
    .sort((left, right) => left.window_seconds - right.window_seconds)
    .filter((window, index, all) => index === 0 || window.window_seconds !== all[index - 1].window_seconds)

  return windows.map(window => ({
    key: String(window.window_seconds),
    label: windowLabel(window.window_seconds),
    tone: windowTone(window.window_seconds),
    window,
  }))
})

// Claude's OAuth usage endpoint returns the same 5h + weekly window pair.
const claudeWindows = computed<TrayUsageWindow[]>(() => {
  const windows = (claudeUsage.value?.usage_windows ?? [])
    .filter(window => window.window_seconds > 0)
    .sort((left, right) => left.window_seconds - right.window_seconds)
    .filter((window, index, all) => index === 0 || window.window_seconds !== all[index - 1].window_seconds)

  return windows.map(window => ({
    key: String(window.window_seconds),
    label: windowLabel(window.window_seconds),
    tone: windowTone(window.window_seconds),
    window,
  }))
})

const resetCards = computed<ResetCreditEntry[]>(() => {  const detailed = snapshot.value?.reset_credits
  const available = (detailed?.credits ?? [])
    .filter(credit => credit.status === 'available')
    .sort((left, right) => {
      const leftTime = left.expires_at ? new Date(left.expires_at).getTime() : Number.MAX_SAFE_INTEGER
      const rightTime = right.expires_at ? new Date(right.expires_at).getTime() : Number.MAX_SAFE_INTEGER
      return leftTime - rightTime
    })
  if (available.length) return available

  const count = detailed?.available_count
    ?? snapshot.value?.usage.reset_credits?.available_count
    ?? 0
  return Array.from({ length: count }, (_, index) => ({
    status: 'available',
    expires_at: index === 0 ? detailed?.next_expires_at ?? null : null,
    granted_at: null,
    title: null,
  }))
})
// Six credits (the tray's own ceiling) laid out two per row: three rows fit the
// popover inside the panel even when the panel is at its shortest, i.e. with
// the monitor strip hidden — no window resize, no scrolling. Anything beyond
// six collapses into the "and N more" line.
const visibleResetCards = computed(() => resetCards.value.slice(0, 6))

// Kiro exposes monthly credits via usage_windows.
const kiroWindows = computed<TrayUsageWindow[]>(() => {
  const windows = (kiroUsage.value?.usage_windows ?? [])
    .filter(window => window.window_seconds > 0)
    .sort((left, right) => left.window_seconds - right.window_seconds)
    .filter((window, index, all) => index === 0 || window.window_seconds !== all[index - 1].window_seconds)

  return windows.map(window => ({
    key: String(window.window_seconds),
    label: windowLabel(window.window_seconds),
    tone: windowTone(window.window_seconds),
    window,
  }))
})

/** Docked strip usage bars: up to two smallest quota windows of the visible
 *  provider (5h first, then 7d), drawn as slim vertical bars side by side —
 *  first bar tank-green like the orb's water, second accent-blue like the
 *  ring. Bar height is the quota remaining. */
const stripUsageBars = computed<{ percent: number }[]>(() => {
  const windows = selectedProvider.value === 'codex'
    ? usageWindows.value
    : selectedProvider.value === 'claude-code'
      ? claudeWindows.value
      : kiroWindows.value
  return windows.slice(0, 2).map(entry => ({
    percent: Math.min(100, Math.max(0, entry.window.remaining_percent)),
  }))
})

const totalMonitorUnread = computed(() =>
  monitorRows.value.filter(session => session.unread).length,
)
const monitorUnreadBadge = computed(() => totalMonitorUnread.value > 9 ? '…' : String(totalMonitorUnread.value))

/** Every monitored session, merged but NOT capped: the docked strip must count
 *  states that the panel's newest-N window does not show, otherwise a red pile
 *  would hide a yellow one. Shares the panel's status ordering. */
const allMonitorRows = computed<AgentSessionState[]>(() =>
  visibleMonitorAgents.value
    .flatMap(agent => monitorSnapshots.value[agent].sessions.map(session => ({ ...session, agent })))
    .sort(compareMonitorRows),
)

/** Docked strip status lamps: the per-session dots said "something happens
 *  somewhere"; two counted lamps say how much needs attention. Yellow = stalled
 *  (approval pending, or aborted by an error), red = finished but unread —
 *  running sessions need no lamp, they are visible the moment the panel opens.
 *  A status with no sessions keeps no lamp, so the strip stays short. */
const stripStatusLamps = computed<{ key: 'waiting' | 'unread', label: string, count: number, display: string }[]>(() => {
  if (monitorHidden.value) return []
  const counts = { waiting: 0, unread: 0 }
  for (const row of allMonitorRows.value) {
    const status = rowEffectiveStatus(row)
    if (status === 'waiting') counts.waiting += 1
    else if (status === 'unread') counts.unread += 1
  }
  // Badge fits one digit inside the 20px strip; more reads as "…", same as the
  // expanded panel's unread badge.
  const badge = (count: number) => count > 9 ? '…' : String(count)
  const lamps = [
    { key: 'waiting' as const, label: t('session_monitor.status_waiting'), count: counts.waiting, display: badge(counts.waiting) },
    { key: 'unread' as const, label: t('session_monitor.status_unread'), count: counts.unread, display: badge(counts.unread) },
  ]
  return lamps.filter(lamp => lamp.count > 0)
})

// The strip is sized to what its content actually measures, not to a predicted
// sum: the content box picks up a 1-2px difference from font metrics and dot
// sizes, and in a strip only ~100px long every wrong pixel shows as a dead gap
// at one end or a clipped count at the other. `stripLongPx` stays as the
// fallback for the first paint, before the box has been measured.
const stripContentEl = ref<HTMLElement | null>(null)
const stripLongPx = computed(() => {
  const showBar = !usageHidden.value && stripUsageBars.value.length > 0
  const lamps = stripStatusLamps.value.length
  const items = (showBar ? 1 : 0) + lamps
  return items === 0
    ? STRIP_MIN_LONG_PX
    : STRIP_PAD_PX * 2
      + (showBar ? STRIP_BAR_HEIGHT : 0)
      + lamps * STRIP_LAMP_LONG_PX
      + STRIP_GAP_PX * (items - 1)
})

function measuredStripLong(): number {
  const content = stripContentEl.value
  if (!content) return stripLongPx.value
  const box = content.getBoundingClientRect()
  // The content group is rotated for the top dock, so its screen width is the
  // group's height there — the long axis is the larger of the two.
  const long = Math.max(box.width, box.height)
  return Math.round(long + STRIP_PAD_PX * 2)
}

// The native strip is sized to what its content actually measures. The backend
// drops dock resizes while it is tweening the panel in or out, and it gives no
// acknowledgement, so the length is driven by ONE retry chain instead of a
// fire-and-forget call: `stripTargetLong` is what the content currently needs,
// `stripSyncedLong` is what the backend last took, and the chain keeps trying
// until the two agree. That is what fixes a re-opened strip keeping the size the
// tween left behind and clipping the far badge.
// `dockAnimating` deliberately plays no part here: it is cleared by the
// backend's dock-changed event with a 600ms fallback, and one missed event used
// to leave it stuck true, silently skipping every later resize.
const STRIP_SYNC_RETRY_DELAYS_MS = [150, 400, 900] as const
let stripTargetLong: number | null = null
let stripSyncedLong: number | null = null
let stripSyncAttempt = 0
let stripSyncTimer: ReturnType<typeof setTimeout> | undefined

/** Measure the strip content and hand the length to the backend. `flush: post`
 *  watchers plus nextTick make the box measurable by the time this runs; a pass
 *  that finds no agreement schedules the next retry. */
function stripSyncPass() {
  stripSyncTimer = undefined
  if (!docked.value || dockExpanded.value) return
  const content = stripContentEl.value
  if (!content) return
  const box = content.getBoundingClientRect()
  // The content group is rotated for the top dock, so the strip's long axis is
  // the larger side of its bounding box there.
  const long = Math.max(STRIP_MIN_LONG_PX, Math.round(Math.max(box.width, box.height) + STRIP_PAD_PX * 2))
  stripTargetLong = long
  if (stripSyncedLong === long) {
    stripSyncAttempt = 0
    return
  }
  // The cache records only a length the backend accepted: a rejected call
  // clears it, so the next pass sends again.
  void resizeUsageTrayDock(long)
    .then(() => {
      stripSyncedLong = long
    })
    .catch(() => {
      stripSyncedLong = null
    })
  // The backend silently drops resizes while it tweens the panel in or out, so
  // re-assert the same length a few times instead of assuming one call landed —
  // a dropped one is what left a re-opened strip at the tween's size, clipping
  // the far badge. Each retry clears the cache so the call really goes out.
  if (stripSyncAttempt >= STRIP_SYNC_RETRY_DELAYS_MS.length) return
  const wait = STRIP_SYNC_RETRY_DELAYS_MS[stripSyncAttempt]
  stripSyncAttempt += 1
  stripSyncTimer = setTimeout(() => {
    if (!docked.value || dockExpanded.value) return
    stripSyncedLong = null
    stripSyncPass()
  }, wait)
}

/** Content or dock state changed: drop any retry tail and settle first. */
async function scheduleStripSync() {
  if (stripSyncTimer) {
    clearTimeout(stripSyncTimer)
    stripSyncTimer = undefined
  }
  stripSyncAttempt = 0
  await nextTick()
  stripSyncPass()
}

watch(
  // Primitives only: `stripLongPx` already folds in the bars, the lamps and the
  // two section toggles, whereas the lamp list is a computed that hands back a
  // fresh array on every evaluation — watching it re-fired on each tracker run
  // and cleared the pending retry timer each time, so retries never ran and a
  // dropped resize was never re-sent.
  [stripLongPx, docked, dockExpanded],
  scheduleStripSync,
  { flush: 'post' },
)

// While the panel is out (or dock mode is off) the strip is not on screen, so
// the size the backend holds is unknown — forget it. Without this the next dock
// compares against a stale length, decides there is nothing to send, and leaves
// the strip at whatever size the tween ended on.
watch([docked, dockExpanded], ([edge, expanded]) => {
  if (edge && !expanded) return
  stripSyncedLong = null
  stripTargetLong = null
  if (stripSyncTimer) {
    clearTimeout(stripSyncTimer)
    stripSyncTimer = undefined
  }
})

function clampHeight(height: number) {
  return Math.min(620, Math.max(120, height))
}

// Mini width ≈ one usage orb (112, same as normal) + panel/shell pad. Normal is 400.
const TRAY_NORMAL_WIDTH = 400
const TRAY_MINI_WIDTH = 160

const panelRef = ref<HTMLElement | null>(null)
// The reset-credit popover opens upward from its trigger and always stays
// inside the panel's own height — opening it never resizes the window.
const resetCreditOpen = ref(false)
let resetCreditCloseTimer: ReturnType<typeof setTimeout> | undefined

function openResetCredit() {
  if (resetCreditCloseTimer) {
    clearTimeout(resetCreditCloseTimer)
    resetCreditCloseTimer = undefined
  }
  resetCreditOpen.value = true
}

/** Closing waits out the fade so re-entering the trigger does not flicker. */
function collapseResetCredit(immediate = false) {
  if (resetCreditCloseTimer) {
    clearTimeout(resetCreditCloseTimer)
    resetCreditCloseTimer = undefined
  }
  if (!resetCreditOpen.value) return
  if (immediate) {
    resetCreditOpen.value = false
    return
  }
  resetCreditCloseTimer = setTimeout(() => {
    resetCreditCloseTimer = undefined
    resetCreditOpen.value = false
  }, RESET_CREDIT_FADE_MS)
}

// Measure the rendered panel instead of maintaining per-state height
// constants: the window always fits the content exactly, so footer rows
// (e.g. last-query time) can never be clipped by the sections above.
// Mini mode also shrinks the native window width to roughly one orb.
async function applyContentHeight() {
  // The collapsed dock strip owns its window size; only the floating panel
  // and the slid-out dock panel re-measure.
  if (docked.value && !dockExpanded.value) return
  const sequence = ++resizeSequence
  await nextTick()
  await new Promise<void>(resolve => requestAnimationFrame(() => resolve()))
  if (sequence !== resizeSequence || compactLoading.value) return
  const panel = panelRef.value
  if (!panel) return
  const width = miniMode.value ? TRAY_MINI_WIDTH : TRAY_NORMAL_WIDTH
  // Measure at the TARGET width, not the window's current width: after a
  // mini↔normal switch the native window is still at the old width, and
  // content (legend rows, credit chips) wraps taller at the narrow width —
  // measuring there leaves dead space at the bottom once the window widens.
  panel.style.width = `${width}px`
  // Force synchronous layout at natural height, then restore before paint.
  panel.style.height = 'auto'
  const measured = Math.ceil(panel.getBoundingClientRect().height)
  panel.style.height = ''
  panel.style.width = ''
  try {
    await resizeUsageTray(clampHeight(measured), width)
  } catch {
    // Browser preview and unsupported platforms may not own a native tray window.
  }
}

// Resize from post-render state instead of relying on the query callback's
// timing. This covers cached tab switches as well as the moment fresh data
// replaces the compact loading view.
watch(
  [selectedProvider, snapshot, claudeUsage, kiroUsage, error, loginUnavailable, compactLoading],
  () => {
    if (!compactLoading.value) void applyContentHeight()
  },
  { flush: 'post' },
)

// The monitor strip changes the panel height when rows appear or drain.
watch(
  () => monitorRows.value.length,
  () => {
    if (!compactLoading.value) void applyContentHeight()
  },
  { flush: 'post' },
)

// Hiding/showing a whole section (usage or monitor) or toggling mini mode
// changes panel height.
watch(
  () => [visibleProviders.value.length, usageHidden.value, monitorHidden.value, miniMode.value],
  () => {
    if (!compactLoading.value) void applyContentHeight()
  },
  { flush: 'post' },
)

// Hover-expand used to keep whatever HWND size the backend restored. On
// Windows that can be a Snap/WebView2-inflated monitor box after the strip
// has sat on the edge; remasuring here is the same recovery as hitting
// refresh (which the user already observed fixes it).
watch(
  [docked, dockExpanded, dockAnimating],
  () => {
    syncRelativeClock(true)
    if (!docked.value || !dockExpanded.value || dockAnimating.value) {
      clearNativeMonitorHover()
    } else if (nativeHoverPoint) {
      void nextTick(syncNativeMonitorHover)
    }
    if (dockAnimating.value || compactLoading.value) return
    if (docked.value && dockExpanded.value) void applyContentHeight()
  },
)

function formatDate(value: number | string, withSeconds = false) {
  const date = typeof value === 'number' ? new Date(value * 1000) : new Date(value)
  return new Intl.DateTimeFormat(locale.value, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: withSeconds ? '2-digit' : undefined,
    hour12: false,
  }).format(date)
}

/** Mini mode last-query: "查询: MM/DD HH:mm" (no year, no "上次"). */
function formatLastQuery(value: number | string) {
  if (miniMode.value) {
    const date = typeof value === 'number' ? new Date(value * 1000) : new Date(value)
    const pad = (n: number) => String(n).padStart(2, '0')
    const time = `${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
    return t('tray.last_query_mini', { time })
  }
  return t('tray.last_query', { time: formatDate(value) })
}

// Reset-credit rows show the expiry moment plus how long is left. Current-year
// dates drop the year (MM/DD) to stay compact; later years keep YYYY/MM/DD.
function expiryParts(value?: string | null) {
  if (!value) return { date: t('tray.expiry_unknown'), time: '', valid: false }
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return { date: t('tray.expiry_unknown'), time: '', valid: false }
  }
  const pad = (n: number) => String(n).padStart(2, '0')
  const monthDay = `${pad(date.getMonth() + 1)}/${pad(date.getDate())}`
  const datePart = date.getFullYear() === new Date().getFullYear()
    ? monthDay
    : `${date.getFullYear()}/${monthDay}`
  const timePart = `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  return { date: datePart, time: timePart, valid: true }
}

/** Whole days left; drives both the countdown chip and the urgent (≤7d) accent.
 *  `null` when the backend sent no usable timestamp. */
function resetCardDays(value?: string | null): number | null {
  const parsed = value ? new Date(value).getTime() : Number.NaN
  if (!Number.isFinite(parsed)) return null
  return Math.max(0, Math.ceil((parsed - relativeNow.value) / 86_400_000))
}

/** Compact countdown: "今天" / "明天" / "N 天" (zh), "Today" / "Tomorrow" / "Nd"
 *  (en) — same spirit as the quota-window wording. */
function resetCardCountdown(value?: string | null): string {
  const days = resetCardDays(value)
  if (days === null) return t('tray.expiry_unknown')
  if (locale.value === 'zh-CN') {
    if (days === 0) return '今天'
    if (days === 1) return '明天'
    return `${days} 天`
  }
  if (days === 0) return 'Today'
  if (days === 1) return 'Tomorrow'
  return `${days}d`
}

/** Full timestamp for the row tooltip — the countdown stays short, the exact
 *  moment is one hover away. */
function formatResetCardMoment(value?: string | null): string {
  const parsed = value ? new Date(value) : null
  if (!parsed || Number.isNaN(parsed.getTime())) return t('tray.expiry_unknown')
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${parsed.getFullYear()}/${pad(parsed.getMonth() + 1)}/${pad(parsed.getDate())} `
    + `${pad(parsed.getHours())}:${pad(parsed.getMinutes())}:${pad(parsed.getSeconds())}`
}

function resetCardTitle(card: ResetCreditEntry): string {
  const exact = formatResetCardMoment(card.expires_at)
  const days = resetCardDays(card.expires_at)
  if (days === null) return exact
  const left = locale.value === 'zh-CN' ? `还有 ${days} 天` : `${days} day${days === 1 ? '' : 's'} left`
  return `${exact} · ${left}`
}

/** Credits expiring within a week get the amber treatment — those are the ones
 *  worth spending before they lapse. */
function isResetCardUrgent(card: ResetCreditEntry): boolean {
  const days = resetCardDays(card.expires_at)
  return days !== null && days <= 7
}

// force=true bypasses the shared 10-minute backend cache (Retry button).
// force=false uses the same snapshot as the Accounts view when still fresh.
async function refresh(
  compact = false,
  syncWithAccounts = false,
  force = false,
  requestedProvider: UsageProvider = selectedProvider.value,
) {
  // A slow response from one Agent must not block another tab or invalidate
  // the first Agent's eventual result. Also avoid duplicate requests per Agent.
  if (providerLoading.value[requestedProvider]) return
  const sequence = ++refreshSequences[requestedProvider]
  const isLatest = () => sequence === refreshSequences[requestedProvider]
  const loadingProviders: UsageProvider[] = [requestedProvider]
  let provider = requestedProvider
  if (compact && !keepPanelVisibleDuringLoading.value) {
    compactLoading.value = true
    try { await resizeUsageTray(120) } catch {}
  } else if (keepPanelVisibleDuringLoading.value) {
    compactLoading.value = false
  }
  providerLoading.value[requestedProvider] = true
  if (selectedProvider.value === requestedProvider) loginUnavailable.value = false
  try {
    const status = await getUsageProviderAvailability()
    if (!isLatest()) return
    availability.value = status

    const preferred = syncWithAccounts
      ? preferredProviderFromAccounts()
      : requestedProvider
    const available = availableProvider(preferred, status)
    if (!available) {
      if (selectedProvider.value === requestedProvider) loginUnavailable.value = true
      return
    }

    if (!syncWithAccounts && !providerAvailable(requestedProvider, status)) return
    provider = syncWithAccounts ? available : requestedProvider
    if (provider !== requestedProvider) {
      if (providerLoading.value[provider]) {
        if (selectedProvider.value === requestedProvider) selectedProvider.value = provider
        return
      }
      providerLoading.value[provider] = true
      loadingProviders.push(provider)
    }
    if (syncWithAccounts && selectedProvider.value === requestedProvider) {
      selectedProvider.value = provider
    }
    providerErrors.value[provider] = null
    queriedProviders.value[provider] = true
    // Keep the previous payload until the response arrives so tab switches
    // never flash an empty layout. Backend cache keeps Accounts + tray aligned.
    if (provider === 'codex') {
      const result = await getCodexTrayUsage(force)
      if (!isLatest()) return
      snapshot.value = result
    } else if (provider === 'claude-code') {
      const result = await getClaudeUsage(force)
      if (!isLatest()) return
      claudeUsage.value = result
    } else if (provider === 'kiro') {
      const result = await getKiroUsage(force)
      if (!isLatest()) return
      kiroUsage.value = result
    }
    void broadcastUsageRefreshed(provider)
  } catch (reason: any) {
    if (!isLatest()) return
    providerErrors.value[provider] = String(reason?.message || reason)
    // Keep the previous successful payload so a transient error does not blank
    // the orb. Only clear when there was never any data for this provider.
  } finally {
    if (isLatest()) {
      for (const activeProvider of loadingProviders) {
        providerLoading.value[activeProvider] = false
      }
      compactLoading.value = false
    }
  }
}

/// Reopen the tray from the status-bar icon. Sync the selected provider with
/// the Accounts view, then load the shared backend snapshot (force=false so
/// we reuse data younger than 10 minutes instead of hitting the network again).
async function handleTrayOpened() {
  opacityOpen.value = false
  intervalOpen.value = false
  trayVisible.value = true
  syncRelativeClock(true)
  void loadMonitorSnapshots()
  // Fresh shared settings first: the Accounts view may have changed the
  // selected agent or paused listening while this popup was hidden.
  monitorSettings.value = await getUsageMonitorSettings()
  const status = await getUsageProviderAvailability()
  availability.value = status

  const preferred =
    (monitorSettings.value?.selectedAgent as UsageProvider | null)
    ?? preferredProviderFromAccounts()
  const available = availableProvider(preferred, status)
  if (!available) {
    loginUnavailable.value = true
    compactLoading.value = false
    return
  }
  loginUnavailable.value = false

  // Only flip the visible provider if the Accounts-view selection maps to a
  // usage provider we actually support — otherwise keep what the user last
  // looked at in this tray session.
  if (preferred !== selectedProvider.value) {
    selectedProvider.value = available
  }

  // Show compact loading only when we have nothing to display yet and the
  // monitor strip does not need to stay visible. Existing local data uses a
  // soft refresh from the shared cache without a loading flash.
  const hasLocal = available === 'codex'
    ? snapshot.value !== null
    : available === 'claude-code'
      ? claudeUsage.value !== null
      : kiroUsage.value !== null

  await refresh(!hasLocal, false, false)
}

function selectProvider(provider: UsageProvider) {
  if (provider === selectedProvider.value) return
  if (availability.value && !providerAvailable(provider, availability.value)) return
  selectedProvider.value = provider
  loginUnavailable.value = false
  // Share the selection with the Accounts view (backend memory + event).
  void setUsageSelectedAgent(provider)
  if ((!queriedProviders.value[provider] || providerErrors.value[provider]) && !providerLoading.value[provider]) {
    void refresh(false, false, false, provider)
  }
}

onMounted(async () => {
  document.addEventListener('visibilitychange', handleDocumentVisibilityChange)
  // Browser-only mock route should be immediately previewable. The real hidden
  // tray window waits for a tray click so startup never consumes a query.
  if (import.meta.env.MODE === 'web') {
    syncRelativeClock(true)
    applySharedSettings(await getUsageMonitorSettings())
    await loadMonitorAvailability()
    await Promise.all([refresh(true, true), loadMonitorSnapshots()])
    return
  }

  const listeners = await Promise.all([
    listen('usage-tray-opened', () => handleTrayOpened()),
    listen('usage-tray-closed', () => handleTrayClosed()),
    listen<NativeTrayHoverPayload>('usage-tray-native-hover', event => {
      handleNativeTrayHover(event.payload)
    }),
    // Theme toggle in the main window repaints this popup live.
    listen<string>('theme-changed', event => {
      document.documentElement.setAttribute('data-theme', event.payload)
    }),
    // Language toggle in the main window: this popup has its own vue-i18n
    // instance and must switch too (backend broadcasts locale-changed).
    listen<string>('locale-changed', event => {
      locale.value = event.payload
      localStorage.setItem('ah-locale', event.payload)
    }),
    // Shared usage-monitor settings (interval / selected agent / listening)
    // changed in the main window.
    listen<UsageMonitorSettings>('usage-monitor-settings-changed', event => {
      applySharedSettings(event.payload)
    }),
    // Live monitor rows: same backend events the Monitor tab consumes.
    ...MONITOR_AGENTS_LIST.map(agent =>
      listen<MonitorSnapshot>(MONITOR_CHANGED_EVENTS[agent], event => {
        applyMonitorSnapshot(agent, event.payload)
      }),
    ),
  ])
  unlisteners.push(...listeners)
  monitorSettings.value = await getUsageMonitorSettings()
  await loadMonitorAvailability()
  void loadMonitorSnapshots()
  void initDock()
})

onBeforeUnmount(() => {
  unlisteners.forEach(unlisten => unlisten())
  window.clearInterval(quotaTimer)
  if (resetCreditCloseTimer) clearTimeout(resetCreditCloseTimer)
  stopRelativeClock()
  clearNativeMonitorHover()
  document.removeEventListener('visibilitychange', handleDocumentVisibilityChange)
  disposeDock()
})
</script>

<template>
  <main
    class="tray-shell"
    :class="{ 'tray-shell--dock-anim': dockAnimating }"
    data-tauri-drag-region="deep"
    @mouseenter="handleShellMouseEnter"
    @mouseleave="handleShellMouseLeave"
    @click="opacityOpen = false; intervalOpen = false"
  >
    <!-- Docked edge strip: usage bars plus two counted status lamps (yellow =
         stalled, red = finished unread) instead of one dot per session. Long
         axis follows the content. Top dock rotates this column 90° CCW into a
         horizontal bar. Hovering slides the panel out. -->
    <section
      v-if="docked && !dockExpanded"
      class="tray-dock-strip"
      :class="{
        'tray-dock-strip--top': docked === 'top',
        'tray-dock-strip--left': docked === 'left',
        'tray-dock-strip--right': docked === 'right',
      }"
      data-tauri-drag-region="deep"
      :style="{
        opacity: panelOpacity / 100,
        '--strip-long': `${stripLongPx}px`,
        '--strip-thick': `${STRIP_THICK_PX}px`,
      }"
      @mouseenter="expandDock"
      @click="expandDock"
    >
      <div class="tray-dock-strip__inner">
        <!-- Measured, not estimated: the native strip is sized to this box, so
             a formula that overshoots shows up as dead space and one that
             undershoots clips the counts. -->
        <div ref="stripContentEl" class="tray-dock-strip__content">
          <span
            v-if="!usageHidden && stripUsageBars.length"
            class="tray-dock-bars"
          >
            <span
              v-for="(bar, index) in stripUsageBars"
              :key="index"
              class="tray-dock-bar"
              :class="index === 0 ? 'tray-dock-bar--tank' : 'tray-dock-bar--ring'"
            >
              <span class="tray-dock-bar__fill" :style="{ height: `${bar.percent}%` }" />
            </span>
          </span>
          <span
            v-for="lamp in stripStatusLamps"
            :key="lamp.key"
            class="tray-dock-lamp"
            :class="`tray-dock-lamp--${lamp.key}`"
            :title="`${lamp.label} · ${lamp.count}`"
          >{{ lamp.display }}</span>
        </div>
      </div>
    </section>
    <section
      v-else
      ref="panelRef"
      class="tray-panel"
      :class="{
        'tray-panel--loading': initialLoading,
        'tray-panel--mini': miniMode && !initialLoading,
      }"
      :style="{ opacity: panelOpacity / 100 }"
    >
      <!-- Top chrome: same absolute top/left band in mini and normal so the
           top-left corner does not jump when the window shrinks. Mini only
           drops opacity + last-query; ephemeral icons hide until the title
           band is hovered (pin stays always visible). -->
      <div v-if="!initialLoading" class="tray-titlebar" @click.stop>
        <div class="tray-controls tray-chrome-ephemeral">
          <div v-if="!miniMode" class="tray-control">
            <button
              v-tooltip:top="t('tray.opacity')"
              class="tray-control-btn"
              :class="{ 'is-active': opacityOpen }"
              @click="toggleOpacityOpen"
            >
              <Blend :size="13" />
            </button>
            <div v-if="opacityOpen" class="tray-opacity-popover" @click.stop>
              <span class="tray-opacity-popover__value">{{ panelOpacity }}%</span>
              <input
                class="tray-opacity-slider"
                type="range"
                :min="OPACITY_MIN"
                :max="OPACITY_MAX"
                step="1"
                :value="panelOpacity"
                :aria-label="t('tray.opacity')"
                @input="onOpacityInput"
                @change="persistOpacity"
              >
            </div>
          </div>
          <div v-if="!miniMode" class="tray-control">
            <button
              v-tooltip:top="t('tray.refresh_interval')"
              class="tray-control-btn"
              :class="{ 'is-active': intervalOpen }"
              @click="toggleIntervalOpen"
            >
              <Timer :size="13" />
            </button>
            <div v-if="intervalOpen" class="tray-opacity-popover tray-interval-popover" @click.stop>
              <span class="tray-opacity-popover__value tray-interval-popover__value">
                {{ t('usage_settings.minutes', { n: intervalDraft ?? refreshMinutes }) }}
              </span>
              <input
                class="tray-opacity-slider"
                type="range"
                min="1"
                max="10"
                step="1"
                :value="intervalDraft ?? refreshMinutes"
                :aria-label="t('tray.refresh_interval')"
                @input="onIntervalInput"
                @change="persistInterval"
              >
            </div>
          </div>
          <button
            v-tooltip:top="usageHidden ? t('tray.show_usage') : t('tray.hide_usage')"
            class="tray-control-btn"
            :class="{ 'is-muted': usageHidden }"
            @click="toggleUsageHidden"
          >
            <BarChart3 :size="13" />
          </button>
          <button
            v-tooltip:top="monitorHidden ? t('tray.show_monitor') : t('tray.hide_monitor')"
            class="tray-control-btn"
            :class="{ 'is-muted': monitorHidden }"
            @click="toggleMonitorHidden"
          >
            <Activity :size="13" />
          </button>
          <button
            v-if="SHOW_MINI_TOGGLE"
            v-tooltip:top="miniMode ? t('tray.expand') : t('tray.mini')"
            class="tray-control-btn"
            @click="setMiniMode(!miniMode)"
          >
            <Maximize2 v-if="miniMode" :size="13" />
            <Minimize2 v-else :size="13" />
          </button>
        </div>

        <span v-if="lastQueryAt && !miniMode" class="tray-last-query tray-chrome-ephemeral">
          {{ formatLastQuery(lastQueryAt) }}
        </span>
        <button
          v-tooltip:top="t('tray.refresh')"
          class="tray-refresh tray-chrome-ephemeral"
          :disabled="loading"
          @click="refresh(false, false, true)"
        >
          <RefreshCw :size="13" :class="{ 'is-spinning': loading }" />
        </button>
        <!-- Close replaces the old pin: docking is how the panel persists. -->
        <button
          v-tooltip:top="t('action.close')"
          class="tray-pin"
          @click="closeTray"
        >
          <X :size="13" />
        </button>
      </div>

      <div v-if="initialLoading" class="initial-loading" role="status">
        <span class="loading-spinner" aria-hidden="true" />
      </div>

      <template v-else>
        <!-- Usage section: whole block gone when hidden (not an empty placeholder). -->
        <template v-if="!usageHidden">
          <template v-if="visibleProviders.length">
            <!-- A single usable provider makes the switcher redundant. -->
            <div
              v-if="visibleProviders.length > 1"
              class="provider-switch"
              :class="{ 'provider-switch--icons': miniMode }"
              role="tablist"
              :aria-label="t('tray.provider')"
            >
              <button
                v-for="provider in visibleProviders"
                :key="provider"
                class="provider-option"
                :class="{
                  'is-active': selectedProvider === provider,
                  'provider-option--icon': miniMode,
                }"
                role="tab"
                :aria-selected="selectedProvider === provider"
                :aria-label="providerLabel(provider)"
                @click="selectProvider(provider)"
              >
                <AgentIcon v-if="miniMode" :agent-id="provider" :size="13" />
                <template v-else>{{ providerLabel(provider) }}</template>
              </button>
            </div>
          </template>
          <div v-else class="tray-empty" role="status">
            <span class="tray-empty__icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.8-3.8"/><path d="M8.5 11h5"/></svg>
            </span>
            <span class="tray-empty__text">{{ t('tray.no_query_items') }}</span>
          </div>

          <template v-if="visibleProviders.length">
            <template v-if="selectedProvider === 'codex'">
              <div class="quota-wrap" :class="{ 'is-loading': loading, 'is-mini': miniMode }">
                <div
                  v-if="!miniMode && snapshot && !error && usageWindows.length && resetCards.length"
                  class="reset-credit-menu"
                  :class="{ 'is-open': resetCreditOpen }"
                  @mouseenter="openResetCredit()"
                  @mouseleave="collapseResetCredit()"
                  @focusin="openResetCredit()"
                  @focusout="collapseResetCredit()"
                >
                  <button
                    class="reset-credit-trigger"
                    type="button"
                    :aria-expanded="resetCreditOpen"
                    aria-describedby="reset-credit-popover"
                  >
                    <CreditCard :size="12" :stroke-width="1.8" aria-hidden="true" />
                    <span>{{ t('tray.reset_credit') }}</span>
                    <span class="reset-credit-trigger__count">{{ resetCards.length }}</span>
                  </button>
                  <!-- Hover/focus popover pinned to the top of the quota area:
                       three compact cards per row, two rows. No title row (the
                       trigger above already reads "重置卡 N") and no per-card
                       ordinal — both would cost height the shortest panel
                       (monitor strip hidden) does not have. The countdown line
                       doubles as the urgency cue. -->
                  <div
                    id="reset-credit-popover"
                    class="reset-credit-popover"
                    role="tooltip"
                    :aria-label="t('tray.reset_credit_expiry')"
                  >
                    <ul class="reset-credit-list">
                      <li
                        v-for="(card, index) in visibleResetCards"
                        :key="`${card.expires_at ?? 'unknown'}-${index}`"
                        class="reset-credit-card"
                        :class="{
                          'is-soonest': index === 0 && Boolean(card.expires_at),
                          'is-urgent': isResetCardUrgent(card),
                          'is-unknown': !expiryParts(card.expires_at).valid,
                        }"
                        :title="resetCardTitle(card)"
                      >
                        <span class="reset-credit-card__moment">
                          <span class="reset-credit-card__date">{{ expiryParts(card.expires_at).date }}</span>
                          <span v-if="expiryParts(card.expires_at).time" class="reset-credit-card__time">
                            {{ expiryParts(card.expires_at).time }}
                          </span>
                        </span>
                        <span class="reset-credit-card__left-value">{{ resetCardCountdown(card.expires_at) }}</span>
                      </li>
                    </ul>
                    <div v-if="resetCards.length > visibleResetCards.length" class="reset-credit-more">
                      {{ t('tray.reset_credit_more', { n: resetCards.length - visibleResetCards.length }) }}
                    </div>
                  </div>
                </div>
                <UsageOrb v-if="usageWindows.length" :windows="usageWindows" :mini="miniMode" />
                <UsageOrbPlaceholder
                  v-else-if="error"
                  kind="error"
                  :mini="miniMode"
                  :title="t('tray.failed')"
                  :message="errorMessage"
                />
                <TrayWaveLoader v-else-if="loading">{{ t('tray.query_wait') }}</TrayWaveLoader>
                <UsageOrbPlaceholder
                  v-else
                  kind="empty"
                  :mini="miniMode"
                  :title="t('tray.no_usage_title')"
                  :message="t('tray.no_usage')"
                />
              </div>
            </template>


            <template v-else-if="selectedProvider === 'claude-code'">
              <div class="quota-wrap" :class="{ 'is-loading': loading, 'is-mini': miniMode }">
                <UsageOrb v-if="claudeWindows.length" :windows="claudeWindows" :mini="miniMode" />
                <UsageOrbPlaceholder
                  v-else-if="error"
                  kind="error"
                  :mini="miniMode"
                  :title="t('tray.failed')"
                  :message="errorMessage"
                />
                <TrayWaveLoader v-else-if="loading">{{ t('tray.query_wait') }}</TrayWaveLoader>
                <UsageOrbPlaceholder
                  v-else
                  kind="empty"
                  :mini="miniMode"
                  :title="t('tray.no_usage_title')"
                  :message="t('tray.no_usage')"
                />
              </div>
            </template>


            <template v-else-if="selectedProvider === 'kiro'">
              <div class="quota-wrap" :class="{ 'is-loading': loading, 'is-mini': miniMode }">
                <UsageOrb
                  v-if="kiroWindows.length"
                  :windows="kiroWindows"
                  :mini="miniMode"
                />
                <UsageOrbPlaceholder
                  v-else-if="error"
                  kind="error"
                  :mini="miniMode"
                  :title="t('tray.failed')"
                  :message="errorMessage"
                />
                <TrayWaveLoader v-else-if="loading">{{ t('tray.query_wait') }}</TrayWaveLoader>
                <UsageOrbPlaceholder
                  v-else
                  kind="empty"
                  :mini="miniMode"
                  :title="t('tray.no_usage_title')"
                  :message="t('tray.no_usage')"
                />
              </div>
            </template>
          </template>
        </template>

        <!-- Monitor strip: whole block gone when hidden. Mini drops tooltips. -->
        <div
          v-if="!monitorHidden"
          class="monitor-strip"
          :class="{ 'is-mini': miniMode }"
          data-tauri-drag-region="false"
        >
          <div v-if="!miniMode" class="monitor-strip__title">
            <span>{{ t('ui.monitor_tab') }}</span>
            <span
              v-if="totalMonitorUnread > 0"
              v-tooltip="t('session_monitor.mark_all_read')"
              class="monitor-strip__unread-total monitor-strip__unread-total--clickable"
              role="button"
              tabindex="0"
              data-tauri-drag-region="false"
              :aria-label="t('session_monitor.mark_all_read')"
              @click.stop="markAllMonitorRowsRead"
              @keydown.enter.stop="markAllMonitorRowsRead"
              @keydown.space.stop.prevent="markAllMonitorRowsRead"
            >{{ monitorUnreadBadge }}</span>
          </div>
          <div v-if="!monitorRows.length" class="monitor-empty" role="status">
            <span class="monitor-empty__icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M3 12h4l2.5-6 3 12 2.5-6h6"/></svg>
            </span>
            <span>{{ t('tray.no_monitor_items') }}</span>
          </div>
          <div
            v-for="(row, index) in monitorRows"
            :key="`${row.agent}-${row.sessionId}-${row.turnId}`"
            class="monitor-row"
            :class="{ 'monitor-row--unread': row.unread }"
            :data-monitor-row-key="monitorRowKey(row)"
            data-tauri-drag-region="false"
            role="button"
            tabindex="0"
            @click="handleMonitorRowClick(row)"
            @keydown.enter="handleMonitorRowClick(row)"
            @keydown.space.prevent="handleMonitorRowClick(row)"
            @mouseleave="clearHoveredMonitorRow(row)"
          >
            <span
              v-tooltip:[monitorTipPlacement(index)]="miniMode ? '' : monitorStatusLabel(row)"
              class="monitor-dot"
              :class="`is-${rowEffectiveStatus(row)}`"
            />
            <!-- Mini: icon only (tooltip = full name). Full: icon + label. -->
            <span
              class="monitor-agent"
              :class="{ 'monitor-agent--icon-only': miniMode }"
              v-tooltip:[monitorTipPlacement(index)]="miniMode ? monitorAgentLabel(row) : ''"
            >
              <SessionClientIcon
                v-if="monitorIsChatgpt(row)"
                client-id="chatgpt"
                :size="miniMode ? 13 : 12"
              />
              <AgentIcon
                v-else
                :agent-id="row.agent"
                :size="miniMode ? 13 : 12"
              />
              <span v-if="!miniMode">{{ monitorAgentLabel(row) }}</span>
            </span>
            <span class="monitor-question">
              <!-- Long prompts: clamp tooltip to 3 lines (no ghost 4th line). -->
              <span
                v-tooltip="miniMode ? '' : {
                  text: monitorPrompt(row),
                  clamp: 3,
                  placement: monitorTipPlacement(index),
                }"
              >
                {{ monitorPrompt(row) }}
              </span>
            </span>
            <span
              v-if="!miniMode && row.updatedAt"
              v-tooltip="t('session_monitor.relative_time_exact', { time: formatMonitorExactTime(row.updatedAt) })"
              class="monitor-time"
            >{{ formatMonitorRelative(row.updatedAt) }}</span>
            <span
              v-if="row.unread"
              v-tooltip="t('session_monitor.mark_read')"
              class="monitor-row__unread monitor-row__unread--clickable"
              role="button"
              tabindex="0"
              data-tauri-drag-region="false"
              :aria-label="t('session_monitor.mark_read')"
              @click.stop="markMonitorRowRead(row)"
              @keydown.enter.stop="markMonitorRowRead(row)"
              @keydown.space.stop.prevent="markMonitorRowRead(row)"
            />
          </div>
        </div>
      </template>


    </section>
    <AppToast />
  </main>
</template>

<style scoped>
:global(html[data-view="codex-usage"]),
:global(html[data-view="codex-usage"] body),
:global(html[data-view="codex-usage"] #app) {
  width: 100%;
  height: 100%;
  margin: 0;
  background: transparent !important;
  overflow: hidden;
}

:global(*), :global(*::before), :global(*::after) {
  box-sizing: border-box;
}

.tray-shell {
  /* Notion Zen Ink palette (mirrors src/assets/theme.css). Dark values are applied
     at the bottom of this stylesheet, keyed off the shared data-theme
     attribute (with a prefers-color-scheme fallback when no explicit choice
     exists) so the popup always matches the main window. */
  --tray-canvas: #FFFFFF;
  --tray-surface: #FFFFFF;
  --tray-sunken: #F7F6F3;
  --tray-hover: #EFEFED;
  --tray-ink: #191918;
  --tray-ink-2: #383836;
  --tray-ink-3: #5C5C58;
  --tray-ink-4: #8C8C87;
  --tray-accent: #2C5E8A;
  --tray-accent-strong: #1D4363;
  --tray-accent-soft: rgba(44, 94, 138, .08);
  --tray-accent-mid: rgba(44, 94, 138, .16);
  --tray-highlight: #C28A3E;
  --tray-success: #448361;
  --tray-warning: #C27A2F;
  --tray-danger: #C44536;
  --tray-signal-red: #E03E3E;
  --tray-signal-yellow: #DFAB01;
  --tray-signal-green: #0F7B6C;
  --tray-hairline: rgba(55, 53, 47, .09);
  --tray-border: rgba(55, 53, 47, .14);
  --tray-on-accent: #FFFFFF;
  --tray-inset: var(--tray-sunken);
  --tray-btn-bg: var(--tray-surface);
  --tray-btn-bg-hover: var(--tray-hover);
  --tray-active-bg: var(--tray-surface);
  --tray-panel-bg: color-mix(in srgb, var(--tray-surface) 97%, transparent);
  --tray-panel-shadow: 0 2px 6px rgba(15, 15, 15, .08);
  --tray-active-shadow: 0 1px 4px rgba(15, 15, 15, .06);
  --tray-success-soft: rgba(68, 131, 97, .10);
  --tray-warning-soft: rgba(194, 122, 47, .10);
  --tray-danger-soft: rgba(196, 69, 54, .10);
  /* Orb Dual Fluid (Left HP / Right MP) - Notion ink-wash palette */
  --tray-orb-hp-light: #CF7E77;
  --tray-orb-hp-mid: #C44536;
  --tray-orb-hp-dark: #7A332D;
  --tray-orb-hp-text: #8E2F23;
  --tray-orb-mp-light: #5E8CAE;
  --tray-orb-mp-mid: #2C5E8A;
  --tray-orb-mp-dark: #1D4363;
  --tray-orb-mp-text: #1D4363;
  /* Orb ring track: faint tint of the ring color, stronger in dark mode. */
  --tray-ring-track: color-mix(in srgb, currentColor 14%, transparent);

  width: 100%;
  height: 100%;
  min-width: 0;
  padding: 0;
  overflow: hidden;
  color: var(--tray-ink);
  font-family: "SF Pro Text", "Segoe UI", "PingFang SC", sans-serif;
  user-select: none;
}

.tray-panel {
  position: relative;
  width: 100%;
  height: 100%;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 12px 14px 8px;
  overflow: hidden;
  /* Square corners on every platform, matching the Windows look. */
  border-radius: 0;
  background: var(--tray-panel-bg);
  box-shadow: var(--tray-panel-shadow);
}

/* Dock size tweens: keep the panel background visible while taking the inner
   subtrees out of layout and paint. This avoids repeatedly reflowing provider
   rows as the native window changes size; the settled content returns with
   the dock-changed event. */
.tray-shell--dock-anim .tray-panel > *,
.tray-shell--dock-anim .tray-dock-strip > * {
  display: none !important;
  opacity: 0 !important;
  visibility: hidden !important;
  pointer-events: none !important;
}

/* Docked edge strip: thin bar at the screen edge — usage bars plus the counted
   status lamps (yellow stalled, red unread). Long axis follows the content.
   Fills the whole window (fixed, inset 0) so the shell inset doesn't squeeze
   it. Top dock rotates the same column 90° CCW. */
.tray-dock-strip {
  position: fixed;
  inset: 0;
  overflow: hidden;
  cursor: pointer;
  border-radius: 0;
  background: var(--tray-panel-bg);
  box-shadow: var(--tray-panel-shadow);
}
.tray-dock-strip__inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  width: 100%;
  height: 100%;
}
.tray-dock-strip__content {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
}
.tray-dock-strip--top .tray-dock-strip__inner {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: var(--strip-thick, 20px);
  display: block;
}
.tray-dock-strip--left .tray-dock-strip__inner,
.tray-dock-strip:not(.tray-dock-strip--top):not(.tray-dock-strip--right) .tray-dock-strip__inner {
  position: absolute;
  top: 0;
  left: 0;
  bottom: 0;
  width: var(--strip-thick, 20px);
  height: 100%;
}
.tray-dock-strip--right .tray-dock-strip__inner {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: var(--strip-thick, 20px);
  height: 100%;
}
.tray-dock-strip--top .tray-dock-strip__content {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%) rotate(-90deg);
}
/* Counted status lamp: a round count badge in the Monitor tab's signal colours.
   It runs along the strip's long axis — horizontal on the top dock (the rotated
   group turns it for us, with the digit stood back up), and a row of its own on
   the side docks so the badge sits across the 20px strip instead of hanging off
   it. A badge is 15px wide, so the next value up (20) would clip inside the
   strip; counts above 9 therefore read as "…". */
.tray-dock-lamp {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  flex: 0 0 15px;
  width: 15px;
  height: 15px;
  border-radius: 50%;
  font-size: 10px;
  font-weight: 750;
  font-variant-numeric: tabular-nums;
  line-height: 1;
  white-space: nowrap;
}
.tray-dock-lamp--waiting {
  /* A touch warmer than the raw signal yellow: the strip sits on the desktop
     instead of inside the panel, where the pure hue reads as a highlighter. */
  background: #FFD75E;
  color: #3A2E12;
  box-shadow: 0 0 5px color-mix(in srgb, #FFD75E 65%, transparent);
}
.tray-dock-lamp--unread {
  background: var(--tray-signal-red);
  color: #fff;
  box-shadow: 0 0 5px color-mix(in srgb, var(--tray-signal-red) 65%, transparent);
}
/* Top dock: the whole group is rotated 90° CCW, so turning the pill the other
   way keeps the digit upright and the pill across the strip. */
.tray-dock-strip--top .tray-dock-lamp {
  transform: rotate(90deg);
}
/* Usage half of the strip: up to two slim vertical bars side by side
   (smallest window tank-green, next window accent-blue — mirroring the
   orb's water/ring colors). Fills rise from the bottom. */
.tray-dock-bars {
  display: flex;
  flex: 0 0 auto;
  gap: 3px;
  height: 48px;
}
.tray-dock-bar {
  position: relative;
  flex: 0 0 auto;
  width: 3px;
  height: 100%;
  border-radius: 2px;
  background: color-mix(in srgb, var(--tray-ink) 12%, transparent);
  overflow: hidden;
}
.tray-dock-bar__fill {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  border-radius: 2px;
  transition: height 300ms var(--ease-soft, ease);
}
.tray-dock-bar--tank .tray-dock-bar__fill { background: var(--tray-orb-hp-mid, #B0524A); }
.tray-dock-bar--ring .tray-dock-bar__fill { background: var(--tray-orb-mp-mid, #3A6B8C); }

/* No title bar: the content gets a slim top band instead, which hosts the
   pin button and doubles as a comfortable drag area. */
.tray-panel:not(.tray-panel--loading) {
  padding-top: 34px;
}
/* Full-width top band: drag target + mini hover hit area for title icons. */
.tray-titlebar {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 34px;
  z-index: 6;
}

/* Mini: hide ephemeral title icons until the title band is hovered.
   Pin is intentionally not .tray-chrome-ephemeral — always visible. */
.tray-panel--mini .tray-titlebar .tray-chrome-ephemeral {
  opacity: 0;
  pointer-events: none;
  transition: opacity .15s ease;
}
.tray-panel--mini .tray-titlebar:hover .tray-chrome-ephemeral {
  opacity: 1;
  pointer-events: auto;
}
@media (prefers-reduced-motion: reduce) {
  .tray-panel--mini .tray-titlebar .tray-chrome-ephemeral {
    transition: none;
  }
}

/* Mini keeps the same shell/panel top+left padding as normal so the top-left
   chrome (controls / pin) does not shift when the window shrinks. Only content
   density (icon tabs, no legend) differs. */
.tray-panel--mini .provider-switch {
  flex: 0 0 26px;
  height: 26px;
  margin-bottom: 4px;
  gap: 2px;
  padding: 2px;
}
.tray-panel--mini .provider-option--icon {
  min-width: 0;
  height: 100%;
  padding: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.tray-panel--mini .quota-wrap.is-mini {
  min-height: 0;
  padding-block: 0 2px;
}
.tray-panel--mini .monitor-strip.is-mini {
  gap: 2px;
  padding-top: 4px;
}
.tray-panel--mini .monitor-row {
  font-size: 10px;
  gap: 4px;
}

/* Top-left controls: opacity + section toggles. */
.tray-controls {
  position: absolute;
  top: 7px;
  left: 10px;
  z-index: 6;
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
.tray-control { position: relative; }
.tray-control-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 2px;
  color: var(--tray-ink-3);
  background: transparent;
  cursor: pointer;
  transition: color .15s ease, background-color .15s ease;
}
.tray-control-btn:hover { color: var(--tray-ink); background: var(--tray-hover); }
.tray-control-btn.is-active { color: var(--tray-accent); background: var(--tray-accent-soft); }
/* Muted = section currently hidden; click again to show. */
.tray-control-btn.is-muted { color: var(--tray-ink-4); opacity: .55; }
.tray-control-btn.is-muted:hover { opacity: 1; color: var(--tray-ink); }
/* Compact horizontal opacity slider — one short row, not a tall list. */
.tray-opacity-popover {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 12;
  display: flex;
  align-items: center;
  gap: 8px;
  width: 148px;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--tray-border);
  border-radius: 2px;
  background: var(--tray-surface);
  box-shadow: var(--tray-panel-shadow);
}
.tray-opacity-popover__value {
  flex: 0 0 auto;
  min-width: 34px;
  color: var(--tray-ink-2);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
}
.tray-opacity-slider {
  flex: 1 1 auto;
  min-width: 0;
  height: 14px;
  margin: 0;
  padding: 0;
  -webkit-appearance: none;
  appearance: none;
  background: transparent;
  cursor: pointer;
}
.tray-opacity-slider:focus { outline: none; }
.tray-opacity-slider::-webkit-slider-runnable-track {
  height: 3px;
  border-radius: 999px;
  background: var(--tray-border);
}
.tray-opacity-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  margin-top: -4.5px;
  border: 0;
  border-radius: 50%;
  background: var(--tray-accent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--tray-accent) 18%, transparent);
  cursor: grab;
}
.tray-opacity-slider:active::-webkit-slider-thumb { cursor: grabbing; }

/* Interval popover: same pattern as opacity, wider to fit "10 分钟". */
.tray-interval-popover { width: 172px; }
.tray-interval-popover__value { min-width: 46px; }
.tray-opacity-slider::-moz-range-track {
  height: 3px;
  border: 0;
  border-radius: 999px;
  background: var(--tray-border);
}
.tray-opacity-slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border: 0;
  border-radius: 50%;
  background: var(--tray-accent);
  cursor: grab;
}

/* Close button (top-right ✕): quiet gray, accent on hover. */
.tray-pin {
  position: absolute;
  top: 7px;
  right: 10px;
  z-index: 5;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 2px;
  color: var(--tray-ink-3);
  background: transparent;
  cursor: pointer;
  transition: color .15s ease, background-color .15s ease;
}
.tray-pin:hover { color: var(--tray-ink); background: var(--tray-hover); }

/* Refresh sits immediately left of the pin; the icon spins while a query is
   in flight. Force refresh also feeds the shared backend cache the Accounts
   view reads. */
.tray-refresh {
  position: absolute;
  top: 7px;
  right: 36px;
  z-index: 5;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 2px;
  color: var(--tray-ink-3);
  background: transparent;
  cursor: pointer;
  transition: color .15s ease, background-color .15s ease;
}
.tray-refresh:hover:not(:disabled) { color: var(--tray-ink); background: var(--tray-hover); }
.tray-refresh:disabled { cursor: default; }
.tray-refresh .is-spinning { animation: tray-spin .8s linear infinite; color: var(--tray-accent); }

/* Last-query timestamp lives in the top band, left of the refresh button. */
.tray-last-query {
  position: absolute;
  top: 7px;
  right: 64px;
  z-index: 5;
  display: inline-flex;
  align-items: center;
  height: 22px;
  color: var(--tray-ink-3);
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* Compact loading state: centered spinner, nothing else. */
.tray-panel--loading { padding-top: 18px; }

.initial-loading {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.loading-spinner {
  width: 18px;
  height: 18px;
  border: 2px solid var(--tray-border);
  border-top-color: var(--tray-accent);
  border-radius: 50%;
  animation: tray-spin .8s linear infinite;
}
@keyframes tray-spin { to { transform: rotate(360deg); } }

.provider-switch {
  flex: 0 0 30px;
  height: 30px;
  display: grid;
  /* Equal columns however many providers are currently queryable (2-4). */
  grid-auto-flow: column;
  grid-auto-columns: 1fr;
  gap: 3px;
  padding: 3px;
  border-radius: 3px;
  background: var(--tray-inset);
}

/* Four providers crowd the pill — tighten padding so "Claude Code" fits. */
.provider-switch:has(> :nth-child(4)) .provider-option {
  padding: 0 6px;
  font-size: 11px;
}
.provider-option {
  min-width: 0;
  overflow: hidden;
  border: 0;
  border-radius: 2px;
  padding: 0 14px;
  color: var(--tray-ink-2);
  background: transparent;
  font: inherit;
  font-size: 13px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
  transition: color .16s ease, background-color .16s ease, box-shadow .16s ease;
}
.provider-option:hover:not(:disabled):not(.is-active) { color: var(--tray-ink); }
.provider-option.is-active {
  color: var(--tray-ink);
  background: var(--tray-active-bg);
  box-shadow: var(--tray-active-shadow);
}
.provider-option:focus-visible { outline: 2px solid var(--tray-accent-mid); outline-offset: 1px; }
.provider-option:disabled { opacity: .48; cursor: default; }
.provider-option.is-active:disabled { opacity: 1; }
/* Mini: icon-only tabs — no ellipsis text, equal icon buttons. */
.provider-switch--icons {
  grid-auto-columns: 1fr;
}
.provider-option--icon {
  overflow: visible;
  text-overflow: unset;
}

/* Fixed empty state for the quota area: nothing queryable or every provider
   hidden. Same 112px height as the orb/loader so the panel never jumps. */
.tray-empty {
  flex: 0 0 auto;
  height: 112px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--tray-ink-2);
  font-size: 12px;
}
.tray-empty__icon {
  width: 44px;
  height: 44px;
  display: grid;
  place-items: center;
  border-radius: 999px;
  color: var(--tray-ink-2);
  background: var(--tray-sunken);
  border: 1px solid var(--tray-hairline);
}
.tray-empty__icon svg { width: 22px; height: 22px; }
.tray-empty__text {
  color: var(--tray-ink-2);
  font-weight: 500;
}

/* Monitor-strip empty state: single quiet line under the strip title. */
.monitor-empty {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 22px;
  color: var(--tray-ink-3);
  font-size: 11px;
}
.monitor-empty__icon { display: inline-flex; width: 13px; height: 13px; }
.monitor-empty__icon svg { width: 13px; height: 13px; }

.quota-wrap {
  position: relative;
  flex: 0 0 auto;
  min-height: 112px;
  padding: 6px 0;
  transition: opacity .18s ease;
}
.quota-wrap.is-loading { opacity: .72; }

.reset-credit-menu {
  position: absolute;
  top: 2px;
  right: 0;
  z-index: 8;
}
/* Trigger: same compact pill as the tray's other chips, but the count is the
   only accent so it never competes with the quota orb next to it. */
.reset-credit-trigger {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  min-width: 28px;
  height: 24px;
  padding: 0 7px;
  border: 1px solid var(--tray-hairline);
  border-radius: 999px;
  background: var(--tray-panel-bg);
  color: var(--tray-ink-2);
  cursor: default;
  font-variant-numeric: tabular-nums;
  font-size: 10px;
  font-weight: 650;
  transition: color .15s ease, border-color .15s ease, background-color .15s ease;
}
.reset-credit-trigger__count {
  display: grid;
  place-items: center;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 999px;
  background: var(--tray-accent-soft);
  color: var(--tray-accent);
  line-height: 1;
}
.reset-credit-trigger:hover,
.reset-credit-trigger:focus-visible,
.reset-credit-menu:focus-within .reset-credit-trigger {
  color: var(--tray-accent);
  border-color: color-mix(in srgb, var(--tray-accent) 40%, var(--tray-hairline));
  outline: none;
}
.reset-credit-trigger:hover .reset-credit-trigger__count,
.reset-credit-trigger:focus-visible .reset-credit-trigger__count {
  background: var(--tray-accent-mid);
}

/* Popover: 3×2 compact cards dropping straight out of the trigger (which sits at
   y=36, right under the 34px title row as the quota region's own header) and
   staying inside the panel's height. The panel is never resized to make room for
   it — that is what keeps it working with the monitor strip hidden, i.e. the
   shortest the panel ever gets. */
.reset-credit-popover {
  position: absolute;
  top: 0;
  right: 0;
  box-sizing: border-box;
  width: 292px;
  padding: 7px;
  border: 1px solid var(--tray-border);
  /* Near-square, like the panel itself: a heavy radius on a 292px box this
     short reads as a bubble and fights the tray's flat ink-wash look. */
  border-radius: 4px;
  background: var(--tray-surface);
  color: var(--tray-ink);
  box-shadow:
    0 10px 26px rgba(42, 42, 46, .16),
    0 2px 6px rgba(42, 42, 46, .08);
  opacity: 0;
  visibility: hidden;
  transform: translateY(-4px);
  transition: opacity .15s ease, transform .15s ease, visibility .15s ease;
}
.reset-credit-menu.is-open .reset-credit-popover {
  opacity: 1;
  visibility: visible;
  transform: translateY(0);
}
/* Three cards per row, two rows: six credits in ~84px of height. */
.reset-credit-list {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.reset-credit-card {
  position: relative;
  display: flex;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
  padding: 4px 6px 5px;
  border: 1px solid var(--tray-hairline);
  /* Barely-rounded rectangles: at 30px tall a 7px radius turns each credit into
     a lozenge, which is what made the whole grid look soft and clumsy. */
  border-radius: 3px;
  background: var(--tray-inset);
  transition: border-color .15s ease, background-color .15s ease;
}
.reset-credit-card:hover {
  border-color: color-mix(in srgb, var(--tray-accent) 34%, var(--tray-hairline));
  background: var(--tray-hover);
}
.reset-credit-card.is-soonest {
  border-color: color-mix(in srgb, var(--tray-accent) 26%, var(--tray-hairline));
  background: color-mix(in srgb, var(--tray-accent) 7%, var(--tray-inset));
}
.reset-credit-card.is-soonest::before {
  content: '';
  position: absolute;
  left: 0;
  top: 4px;
  bottom: 4px;
  width: 2px;
  border-radius: 0 1px 1px 0;
  background: var(--tray-accent);
}
.reset-credit-card__moment {
  display: flex;
  min-width: 0;
  align-items: baseline;
  gap: 3px;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
.reset-credit-card__date {
  color: var(--tray-ink);
  font-size: 10px;
  font-weight: 650;
}
.reset-credit-card__time {
  color: var(--tray-ink-3);
  font-size: 8px;
}
/* Remaining time doubles as the urgency cue: amber inside the last week. */
.reset-credit-card__left-value {
  color: var(--tray-ink-3);
  font-size: 9px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.reset-credit-card.is-soonest .reset-credit-card__left-value {
  color: var(--tray-accent);
}
.reset-credit-card.is-urgent .reset-credit-card__left-value {
  color: var(--tray-warning);
}
.reset-credit-card.is-unknown .reset-credit-card__date {
  color: var(--tray-ink-3);
  font-weight: 600;
}
.reset-credit-card.is-unknown .reset-credit-card__left-value {
  color: var(--tray-ink-4);
}
.reset-credit-more {
  padding-top: 6px;
  color: var(--tray-ink-3);
  font-size: 9px;
  text-align: right;
}

/* Monitor strip under the quota area. */
.monitor-strip {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 6px 0 0;
  border-top: 1px solid var(--tray-hairline);
}
.monitor-strip.is-mini {
  gap: 3px;
  padding-top: 6px;
}
.monitor-strip__title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--tray-ink);
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
}
.monitor-strip__unread-total {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 17px;
  width: 17px;
  height: 17px;
  padding: 0;
  border-radius: 50%;
  background: var(--tray-signal-red);
  color: #fff;
  font-size: 10px;
  font-weight: 700;
  line-height: 1;
  font-variant-numeric: tabular-nums;
  text-transform: none;
  white-space: nowrap;
}
.monitor-strip__unread-total--clickable {
  cursor: pointer;
  transition: transform var(--dur-fast) var(--ease-soft), filter var(--dur-fast) var(--ease-soft);
}
.monitor-strip__unread-total--clickable:hover {
  transform: scale(1.15);
  filter: brightness(1.15);
}
.monitor-row__unread {
  flex: 0 0 7px;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--tray-signal-red);
}
.monitor-row__unread--clickable {
  cursor: pointer;
  transition: transform var(--dur-fast) var(--ease-soft), box-shadow var(--dur-fast) var(--ease-soft);
}
.monitor-row__unread--clickable:hover {
  transform: scale(1.4);
  box-shadow: 0 0 6px var(--tray-signal-red);
}
.monitor-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  font-size: 11px;
  line-height: 1.5;
  cursor: pointer;
  padding: 1px 4px;
  margin: 0 -4px;
  border-radius: 2px;
  user-select: none;
  transition: background-color var(--dur-fast, 150ms) ease;
}
.monitor-row:hover {
  background: var(--tray-hover);
}
.monitor-row:focus-visible {
  outline: 1px solid var(--tray-accent);
  outline-offset: -1px;
}
.monitor-dot {
  flex: 0 0 auto;
  width: 7px;
  height: 7px;
  border-radius: 999px;
}
.monitor-dot.is-running { background: var(--tray-signal-green); }
.monitor-dot.is-waiting { background: var(--tray-signal-yellow); }
.monitor-dot.is-unread { background: var(--tray-signal-red); }
.monitor-dot.is-ended { background: var(--tray-ink-4); }


.monitor-agent {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--tray-ink);
  font-weight: 700;
}
.monitor-agent--icon-only {
  min-width: 0;
  max-width: none;
}
.monitor-question {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  color: var(--tray-ink-2);
}
/* The tooltip sits on this inline span so hovering the empty tail of a short
   question does not pop it. */
.monitor-question > span {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: top;
}
.monitor-time {
  flex: 0 0 auto;
  margin-left: auto;
  color: var(--tray-ink-3);
  font: 10px/1.5 var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* Dark tray palette: explicit night choice wins; with no explicit choice the
   OS preference decides (same rule as theme.css). The entire selector must
   live inside :global() — a trailing .tray-shell outside the parens gets
   dropped by the scoped-CSS compiler, which silently breaks dark mode. */
:global(html[data-theme="night"] .tray-shell) {
  --tray-canvas: #191919;
  --tray-surface: #202020;
  --tray-sunken: #141414;
  --tray-hover: #2A2A2A;
  --tray-ink: #F5F5F4;
  --tray-ink-2: #D6D3D1;
  --tray-ink-3: #A8A29E;
  --tray-ink-4: #78716C;
  --tray-accent: #5C8EB8;
  --tray-accent-strong: #7EAED6;
  --tray-accent-soft: rgba(92, 142, 184, .15);
  --tray-accent-mid: rgba(92, 142, 184, .28);
  --tray-highlight: #D9B97C;
  --tray-success: #6CA686;
  --tray-warning: #D99554;
  --tray-danger: #D86358;
  --tray-signal-red: #FF4D4D;
  --tray-signal-yellow: #FFD60A;
  --tray-signal-green: #32D74B;
  --tray-hairline: rgba(255, 255, 255, .07);
  --tray-border: rgba(255, 255, 255, .12);
  /* Text on accent buttons / inverted chips (pairs with light ink). */
  --tray-on-accent: #FFFFFF;
  --tray-inset: var(--tray-hover);
  --tray-btn-bg: var(--tray-hairline);
  --tray-btn-bg-hover: var(--tray-border);
  --tray-active-bg: var(--tray-hover);
  --tray-panel-shadow: 0 2px 6px rgba(0, 0, 0, .45);
  --tray-active-shadow: 0 1px 4px rgba(0, 0, 0, .35);
  --tray-success-soft: rgba(108, 166, 134, .14);
  --tray-warning-soft: rgba(217, 149, 84, .14);
  --tray-danger-soft: rgba(216, 99, 88, .14);
  /* Orb Dual Fluid (Left HP / Right MP) - Dark ink-wash palette */
  --tray-orb-hp-light: #E08A84;
  --tray-orb-hp-mid: #D86358;
  --tray-orb-hp-dark: #7E3731;
  --tray-orb-hp-text: #E08A84;
  --tray-orb-mp-light: #8EB7DB;
  --tray-orb-mp-mid: #5C8EB8;
  --tray-orb-mp-dark: #375F7F;
  --tray-orb-mp-text: #8EB7DB;
  --tray-ring-track: color-mix(in srgb, currentColor 24%, transparent);
}

@media (prefers-color-scheme: dark) {
  :global(html:not([data-theme]) .tray-shell) {
    --tray-canvas: #191919;
    --tray-surface: #202020;
    --tray-sunken: #141414;
    --tray-hover: #2A2A2A;
    --tray-ink: #F5F5F4;
    --tray-ink-2: #D6D3D1;
    --tray-ink-3: #A8A29E;
    --tray-ink-4: #78716C;
    --tray-accent: #5C8EB8;
    --tray-accent-strong: #7EAED6;
    --tray-accent-soft: rgba(92, 142, 184, .15);
    --tray-accent-mid: rgba(92, 142, 184, .28);
    --tray-highlight: #D9B97C;
    --tray-success: #6CA686;
    --tray-warning: #D99554;
    --tray-danger: #D86358;
    --tray-signal-red: #FF4D4D;
    --tray-signal-yellow: #FFD60A;
    --tray-signal-green: #32D74B;
    --tray-hairline: rgba(255, 255, 255, .07);
    --tray-border: rgba(255, 255, 255, .12);
    --tray-on-accent: #FFFFFF;
    --tray-inset: var(--tray-hover);
    --tray-btn-bg: var(--tray-hairline);
    --tray-btn-bg-hover: var(--tray-border);
    --tray-active-bg: var(--tray-hover);
    --tray-panel-shadow: 0 2px 6px rgba(0, 0, 0, .45);
    --tray-active-shadow: 0 1px 4px rgba(0, 0, 0, .35);
    --tray-success-soft: rgba(108, 166, 134, .14);
    --tray-warning-soft: rgba(217, 149, 84, .14);
    --tray-danger-soft: rgba(216, 99, 88, .14);
    /* Orb Dual Fluid (Left HP / Right MP) - Dark ink-wash palette */
    --tray-orb-hp-light: #E08A84;
    --tray-orb-hp-mid: #D86358;
    --tray-orb-hp-dark: #7E3731;
    --tray-orb-hp-text: #E08A84;
    --tray-orb-mp-light: #8EB7DB;
    --tray-orb-mp-mid: #5C8EB8;
    --tray-orb-mp-dark: #375F7F;
    --tray-orb-mp-text: #8EB7DB;
    --tray-ring-track: color-mix(in srgb, currentColor 24%, transparent);
  }
}
</style>
