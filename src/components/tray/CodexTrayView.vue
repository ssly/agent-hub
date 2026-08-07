<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Activity, BarChart3, Blend, Maximize2, Minimize2, Pin, PinOff, RefreshCw } from 'lucide-vue-next'
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
  getGrokUsage,
  getKimiSessionMonitorSnapshot,
  getKimiUsage,
  getUsageProviderAvailability,
  getZCodeSessionMonitorSnapshot,
  resizeUsageTray,
  setUsageTrayPinned,
} from '@/lib/api'
import type {
  ClaudeUsage,
  CodexTraySnapshot,
  GrokUsage,
  KimiUsage,
  ResetCreditEntry,
  UsageProviderAvailability,
  UsageWindow,
} from '@/lib/api'
import type { AgentSessionState, MonitorAgent, MonitorSnapshot } from '@/stores/session-monitor'
import AgentIcon from '@/components/agents/AgentIcon.vue'
import SessionClientIcon from '@/components/sessions/SessionClientIcon.vue'
import UsageOrb, { type OrbTone, type OrbWindow } from './UsageOrb.vue'
import UsageOrbPlaceholder from './UsageOrbPlaceholder.vue'
import TrayWaveLoader from './TrayWaveLoader.vue'

const { t, locale } = useI18n()
const { showToast } = useToast()
type UsageProvider = 'codex' | 'claude-code' | 'grok-build' | 'kimi-code'
const selectedProvider = ref<UsageProvider>(preferredProviderFromAccounts())
const availability = ref<UsageProviderAvailability | null>(null)
const snapshot = ref<CodexTraySnapshot | null>(null)
const grokUsage = ref<GrokUsage | null>(null)
const kimiUsage = ref<KimiUsage | null>(null)
const claudeUsage = ref<ClaudeUsage | null>(null)
// The real tray window starts hidden and compact. Defaulting to loading avoids
// a flash of the empty-state layout before the first tray-click event arrives.
const loading = ref(import.meta.env.MODE !== 'web')
const compactLoading = ref(import.meta.env.MODE !== 'web')
const providerErrors = ref<Record<UsageProvider, string | null>>({
  codex: null,
  'claude-code': null,
  'grok-build': null,
  'kimi-code': null,
})
const error = computed(() => providerErrors.value[selectedProvider.value])
// Last successful query time for the visible provider, shown in the top band
// next to the refresh/pin buttons (no footer row needed).
const lastQueryAt = computed(() => {
  if (selectedProvider.value === 'codex') return snapshot.value?.last_query_at ?? null
  if (selectedProvider.value === 'kimi-code') return kimiUsage.value?.fetched_at ?? null
  if (selectedProvider.value === 'claude-code') return claudeUsage.value?.fetched_at ?? null
  return grokUsage.value?.fetched_at ?? null
})
const loginUnavailable = ref(false)
const queriedProviders = ref<Record<UsageProvider, boolean>>({
  codex: false,
  'claude-code': false,
  'grok-build': false,
  'kimi-code': false,
})
const unlisteners: UnlistenFn[] = []
const initialLoading = computed(() => compactLoading.value)
// Pin: the popup hides on blur by default; pinning keeps it visible until the
// user unpins and clicks elsewhere. Flag lives in backend memory only.
const pinned = ref(false)
// Compact tray layout: ring-only usage, short labels, no opacity control.
const MINI_STORAGE_KEY = 'ah-tray-mini'
const miniMode = ref(localStorage.getItem(MINI_STORAGE_KEY) === '1')
const opacityOpen = ref(false)
function setMiniMode(next: boolean) {
  miniMode.value = next
  localStorage.setItem(MINI_STORAGE_KEY, next ? '1' : '0')
  opacityOpen.value = false
}
let refreshSequence = 0
let resizeSequence = 0

// --- Mini monitor strip ---------------------------------------------------
// Same data the Monitor tab shows (backend snapshots + change events), but
// reduced to one line per session: status dot + agent + user question.
// Same order as MONITOR_AGENTS / platform registry (monitor subset).
const MONITOR_AGENTS_LIST: MonitorAgent[] = ['codex', 'claude', 'cursor', 'antigravity', 'grok', 'kimi', 'zcode']
const MONITOR_CHANGED_EVENTS: Record<MonitorAgent, string> = {
  codex: 'session-monitor:codex-changed',
  claude: 'session-monitor:claude-changed',
  cursor: 'session-monitor:cursor-changed',
  antigravity: 'session-monitor:antigravity-changed',
  grok: 'session-monitor:grok-changed',
  kimi: 'session-monitor:kimi-changed',
  zcode: 'session-monitor:zcode-changed',
}
const MONITOR_SNAPSHOT_API: Record<MonitorAgent, () => Promise<MonitorSnapshot>> = {
  codex: getCodexSessionMonitorSnapshot,
  claude: getClaudeSessionMonitorSnapshot,
  cursor: getCursorSessionMonitorSnapshot,
  antigravity: getAntigravitySessionMonitorSnapshot,
  grok: getGrokSessionMonitorSnapshot,
  kimi: getKimiSessionMonitorSnapshot,
  zcode: getZCodeSessionMonitorSnapshot,
}
const monitorSnapshots = ref<Record<MonitorAgent, MonitorSnapshot>>({
  codex: { revision: 0, sessions: [] },
  claude: { revision: 0, sessions: [] },
  cursor: { revision: 0, sessions: [] },
  antigravity: { revision: 0, sessions: [] },
  grok: { revision: 0, sessions: [] },
  kimi: { revision: 0, sessions: [] },
  zcode: { revision: 0, sessions: [] },
})

// Merged like the Monitor tab's "all" view: running first, newest activity
// first within each group, capped so the strip never dominates the panel.
/** Tray strip shows at most 6 sessions; tip placement splits at the midpoint. */
const MONITOR_STRIP_LIMIT = 6
const MONITOR_TIP_SPLIT = 3

const monitorRows = computed<AgentSessionState[]>(() =>
  MONITOR_AGENTS_LIST
    .flatMap(agent => monitorSnapshots.value[agent].sessions.map(session => ({ ...session, agent })))
    .sort((a, b) => {
      if (a.status !== b.status) return a.status === 'running' ? -1 : 1
      return b.updatedAt - a.updatedAt
    })
    .slice(0, MONITOR_STRIP_LIMIT),
)

/** Top 3 rows: tip below; bottom 3: tip above — keeps long prompts inside the panel. */
function monitorTipPlacement(index: number): 'top' | 'bottom' {
  return index < MONITOR_TIP_SPLIT ? 'bottom' : 'top'
}

/** Full-mode text label (mini uses icons only). */
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

async function loadMonitorSnapshots() {
  const results = await Promise.allSettled(
    MONITOR_AGENTS_LIST.map(agent => MONITOR_SNAPSHOT_API[agent]()),
  )
  MONITOR_AGENTS_LIST.forEach((agent, index) => {
    const result = results[index]
    if (result.status === 'fulfilled' && result.value) {
      monitorSnapshots.value[agent] = result.value
    }
  })
}

// --- Status-transition pulse ----------------------------------------------
// When a visible session flips running↔ended, a large dot blooms at the
// panel center, shrinks to row-dot size, then glides along a soft arc to the
// row it belongs to — running flips green, ended flips gray.
interface MonitorPulse { id: number; key: string; status: 'running' | 'ended' }
const pulses = ref<MonitorPulse[]>([])
const knownMonitorStatus = new Map<string, string>()
// The first watcher pass only seeds statuses — everything already on screen
// at panel (re)open must not animate.
let monitorStatusSeeded = false
const reduceMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
let pulseSeq = 0

// Row identity for transition detection and DOM targeting. Deliberately
// session-scoped, NOT turn-scoped: the backend keeps one record per session
// and mutates its turn_id on every new turn (for Kimi the fallback turn_id
// even changes per event), so including turn_id would make every flip look
// like a brand-new row and the pulse would never fire.
function monitorRowKey(row: AgentSessionState) {
  return `${row.agent}:${row.sessionId}`
}

watch(monitorRows, rows => {
  const firstSeeding = !monitorStatusSeeded
  monitorStatusSeeded = true
  const visible = new Set<string>()
  for (const row of rows) {
    const key = monitorRowKey(row)
    visible.add(key)
    const previous = knownMonitorStatus.get(key)
    if (!firstSeeding && !reduceMotion && !monitorHidden.value) {
      // Two cases animate: an existing row flipping status, and a brand-new
      // session appearing already-running (e.g. a fresh ChatGPT desktop
      // thread — it has no prior "ended" row to flip from).
      const flipped = previous !== undefined && previous !== row.status
      const appearedRunning = previous === undefined && row.status === 'running'
      if (flipped || appearedRunning) {
        void spawnPulse(key, row.status as MonitorPulse['status'])
      }
    }
    knownMonitorStatus.set(key, row.status)
  }
  // Forget sessions that scrolled out of the strip so re-appearing rows are
  // treated as fresh (no pulse) instead of stale comparisons.
  for (const key of [...knownMonitorStatus.keys()]) {
    if (!visible.has(key)) knownMonitorStatus.delete(key)
  }
})

async function spawnPulse(key: string, status: MonitorPulse['status']) {
  const id = ++pulseSeq
  pulses.value.push({ id, key, status })
  await nextTick()
  const panel = panelRef.value
  const dotEl = panel?.querySelector<HTMLElement>(`[data-pulse-id="${id}"]`)
  if (!panel || !dotEl) {
    pulses.value = pulses.value.filter(pulse => pulse.id !== id)
    return
  }
  const panelRect = panel.getBoundingClientRect()
  const startX = panelRect.width / 2
  const startY = panelRect.height / 2
  const target = panel.querySelector<HTMLElement>(`[data-session-dot="${CSS.escape(key)}"]`)
  let endX = startX
  let endY = startY
  if (target) {
    const rect = target.getBoundingClientRect()
    endX = rect.left - panelRect.left + rect.width / 2
    endY = rect.top - panelRect.top + rect.height / 2
  }
  // Midpoint lifted upward (with a slight lateral bend) turns the straight
  // slide into a soft arc.
  const midX = (startX + endX) / 2 + (endX - startX) * 0.08
  const midY = Math.min(startY, endY) - Math.max(18, Math.abs(endY - startY) * 0.18)
  const finalScale = 8 / 56 // row dots are ~7-8px; the pulse element is 56px
  const frame = (x: number, y: number, scale: number, opacity: number) => ({
    transform: `translate(${x}px, ${y}px) translate(-50%, -50%) scale(${scale})`,
    opacity,
  })
  const animation = dotEl.animate(
    [
      { ...frame(startX, startY, 0.25, 0), offset: 0 },
      { ...frame(startX, startY, 1, 1), offset: 0.22 },
      { ...frame(startX, startY, finalScale, 1), offset: 0.45 },
      { ...frame(midX, midY, finalScale, 1), offset: 0.7 },
      { ...frame(endX, endY, finalScale, 1), offset: 0.96 },
      { ...frame(endX, endY, finalScale, 0), offset: 1 },
    ],
    { duration: 950, easing: 'cubic-bezier(.4, 0, .2, 1)', fill: 'forwards' },
  )
  try { await animation.finished } catch { /* cancelled on unmount */ }
  pulses.value = pulses.value.filter(pulse => pulse.id !== id)
  // Landing beat: briefly enlarge the row dot so the eye connects the two.
  target?.animate(
    [{ transform: 'scale(1.7)' }, { transform: 'scale(1)' }],
    { duration: 260, easing: 'ease-out' },
  )
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
// Declared above the computeds that reference it: watch() eagerly evaluates
// its source on creation, so a later const would hit the TDZ at setup time.
const PROVIDER_ORDER: UsageProvider[] = ['codex', 'claude-code', 'grok-build', 'kimi-code']

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

// Providers the panel can actually query right now — only these appear as tabs.
const queryableProviders = computed<UsageProvider[]>(() =>
  PROVIDER_ORDER.filter(provider =>
    availability.value ? providerAvailable(provider, availability.value) : false,
  ),
)
// Whole usage section is either shown or gone; no per-provider hide list.
const visibleProviders = computed<UsageProvider[]>(() => queryableProviders.value)

// A provider that becomes unavailable can no longer be selected.
watch(visibleProviders, list => {
  if (list.length && !list.includes(selectedProvider.value)) {
    selectedProvider.value = list[0]
  }
})

// While pinned, re-query the quota every 5 minutes (force=true bypasses the
// shared 10-minute backend cache). Monitor rows stay event-driven real-time.
let quotaTimer: number | undefined
watch(pinned, isPinned => {
  window.clearInterval(quotaTimer)
  quotaTimer = undefined
  if (isPinned) {
    quotaTimer = window.setInterval(() => {
      void refresh(false, false, true)
    }, 5 * 60_000)
  }
})

async function togglePinned() {
  pinned.value = !pinned.value
  try {
    await setUsageTrayPinned(pinned.value)
  } catch {
    // Browser preview has no native window to pin.
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
  if (stored === 'grok-build') return 'grok-build'
  if (stored === 'kimi-code') return 'kimi-code'
  if (stored === 'claude-code') return 'claude-code'
  return 'codex'
}

function providerAvailable(provider: UsageProvider, status: UsageProviderAvailability) {
  if (provider === 'codex') return status.codex
  if (provider === 'claude-code') return status.claude_code
  if (provider === 'grok-build') return status.grok_build
  return status.kimi_code
}

// Fallback order mirrors the provider-switch tab order so a preferred provider
// that is signed out falls back to the next available one deterministically.
const PROVIDER_LABELS: Record<UsageProvider, string> = {
  codex: 'Codex',
  'claude-code': 'Claude Code',
  'grok-build': 'Grok Build',
  'kimi-code': 'Kimi Code',
}
function providerLabel(provider: UsageProvider) {
  return PROVIDER_LABELS[provider]
}

function availableProvider(
  preferred: UsageProvider,
  status: UsageProviderAvailability,
): UsageProvider | null {
  if (providerAvailable(preferred, status)) return preferred
  for (const candidate of PROVIDER_ORDER) {
    if (candidate !== preferred && providerAvailable(candidate, status)) return candidate
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

// Kimi exposes the same multi-window shape as Codex (5h primary + weekly), so
// we reuse the same windowing logic, just sourced from kimiUsage.
const kimiWindows = computed<TrayUsageWindow[]>(() => {
  const windows = (kimiUsage.value?.usage_windows ?? [])
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

// Grok exposes a single window, rendered by the orb as a lone bubble tank.
const grokWindows = computed<TrayUsageWindow[]>(() => {
  const window = grokUsage.value?.usage_window
  if (!window) return []
  return [{
    key: 'grok',
    label: windowLabel(window.window_seconds),
    tone: windowTone(window.window_seconds),
    window,
  }]
})

function clampHeight(height: number) {
  return Math.min(620, Math.max(120, height))
}

// Mini width ≈ one usage orb (132, same as normal) + panel/shell pad. Normal is 400.
const TRAY_NORMAL_WIDTH = 400
const TRAY_MINI_WIDTH = 160

const panelRef = ref<HTMLElement | null>(null)

// Measure the rendered panel instead of maintaining per-state height
// constants: the window always fits the content exactly, so footer rows
// (e.g. last-query time) can never be clipped by the sections above.
// Mini mode also shrinks the native window width to roughly one orb.
async function applyContentHeight() {
  const sequence = ++resizeSequence
  await nextTick()
  await new Promise<void>(resolve => requestAnimationFrame(() => resolve()))
  if (sequence !== resizeSequence || compactLoading.value) return
  const panel = panelRef.value
  if (!panel) return
  // Force synchronous layout at natural height, then restore before paint.
  panel.style.height = 'auto'
  const measured = panel.offsetHeight
  panel.style.height = ''
  const width = miniMode.value ? TRAY_MINI_WIDTH : TRAY_NORMAL_WIDTH
  try {
    // + shell padding 8×2 on height
    await resizeUsageTray(clampHeight(measured + 16), width)
  } catch {
    // Browser preview and unsupported platforms may not own a native tray window.
  }
}

// Resize from post-render state instead of relying on the query callback's
// timing. This covers cached tab switches as well as the moment fresh data
// replaces the compact loading view.
watch(
  [selectedProvider, snapshot, grokUsage, kimiUsage, claudeUsage, error, loginUnavailable, compactLoading],
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

// Reset-credit chips show the expiry date by default; hovering floats the
// full validity above the chip ("重置卡有效期:" / date / hh:mm:ss). Current-
// year dates drop the year (MM/DD) to keep the chips compact; other years
// keep the full YYYY/MM/DD.
function splitExpiry(value?: string | null) {
  if (!value) return { date: t('tray.expiry_unknown'), time: '' }
  const date = new Date(value)
  const pad = (n: number) => String(n).padStart(2, '0')
  const monthDay = `${pad(date.getMonth() + 1)}/${pad(date.getDate())}`
  const datePart = date.getFullYear() === new Date().getFullYear()
    ? monthDay
    : `${date.getFullYear()}/${monthDay}`
  const timePart = `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  return { date: datePart, time: timePart }
}

// force=true bypasses the shared 10-minute backend cache (Retry button).
// force=false uses the same snapshot as the Accounts view when still fresh.
async function refresh(compact = false, syncWithAccounts = false, force = false) {
  const sequence = ++refreshSequence
  let provider = selectedProvider.value
  if (compact) {
    compactLoading.value = true
    try { await resizeUsageTray(120) } catch {}
  }
  loading.value = true
  loginUnavailable.value = false
  try {
    const status = await getUsageProviderAvailability()
    if (sequence !== refreshSequence) return
    availability.value = status

    const preferred = syncWithAccounts
      ? preferredProviderFromAccounts()
      : selectedProvider.value
    const available = availableProvider(preferred, status)
    if (!available) {
      snapshot.value = null
      grokUsage.value = null
      kimiUsage.value = null
      claudeUsage.value = null
      loginUnavailable.value = true
      return
    }

    provider = available
    selectedProvider.value = provider
    providerErrors.value[provider] = null
    queriedProviders.value[provider] = true
    // Keep the previous payload until the response arrives so tab switches
    // never flash an empty layout. Backend cache keeps Accounts + tray aligned.
    if (provider === 'codex') {
      const result = await getCodexTrayUsage(force)
      if (sequence !== refreshSequence) return
      snapshot.value = result
    } else if (provider === 'kimi-code') {
      const result = await getKimiUsage(force)
      if (sequence !== refreshSequence) return
      kimiUsage.value = result
    } else if (provider === 'claude-code') {
      const result = await getClaudeUsage(force)
      if (sequence !== refreshSequence) return
      claudeUsage.value = result
    } else {
      const result = await getGrokUsage(force)
      if (sequence !== refreshSequence) return
      grokUsage.value = result
    }
    void broadcastUsageRefreshed(provider)
  } catch (reason: any) {
    if (sequence !== refreshSequence) return
    providerErrors.value[provider] = String(reason?.message || reason)
  } finally {
    if (sequence === refreshSequence) {
      compactLoading.value = false
      loading.value = false
    }
  }
}

/// Reopen the tray from the status-bar icon. Sync the selected provider with
/// the Accounts view, then load the shared backend snapshot (force=false so
/// we reuse data younger than 10 minutes instead of hitting the network again).
async function handleTrayOpened() {
  opacityOpen.value = false
  void loadMonitorSnapshots()
  const status = await getUsageProviderAvailability()
  availability.value = status

  const preferred = preferredProviderFromAccounts()
  const available = availableProvider(preferred, status)
  if (!available) {
    loginUnavailable.value = true
    compactLoading.value = false
    loading.value = false
    return
  }
  loginUnavailable.value = false

  // Only flip the visible provider if the Accounts-view selection maps to a
  // usage provider we actually support — otherwise keep what the user last
  // looked at in this tray session.
  if (preferred !== selectedProvider.value) {
    selectedProvider.value = available
  }

  // Show compact loading only when we have nothing to display yet. When local
  // data already exists, soft-refresh from the shared backend cache so timers
  // stay current without a full loading flash.
  const hasLocal = available === 'codex'
    ? snapshot.value !== null
    : available === 'kimi-code'
      ? kimiUsage.value !== null
      : grokUsage.value !== null

  await refresh(!hasLocal, false, false)
}

async function selectProvider(provider: UsageProvider) {
  if (provider === selectedProvider.value || loading.value) return
  if (availability.value && !providerAvailable(provider, availability.value)) return
  selectedProvider.value = provider
  if (!queriedProviders.value[provider]) {
    await refresh(false, false, false)
  }
}

onMounted(async () => {
  // Browser-only mock route should be immediately previewable. The real hidden
  // tray window waits for a tray click so startup never consumes a query.
  if (import.meta.env.MODE === 'web') {
    await Promise.all([refresh(true, true), loadMonitorSnapshots()])
    return
  }

  const listeners = await Promise.all([
    listen('usage-tray-opened', () => handleTrayOpened()),
    // Theme toggle in the main window repaints this popup live.
    listen<string>('theme-changed', event => {
      document.documentElement.setAttribute('data-theme', event.payload)
    }),
    // Live monitor rows: same backend events the Monitor tab consumes.
    ...MONITOR_AGENTS_LIST.map(agent =>
      listen<MonitorSnapshot>(MONITOR_CHANGED_EVENTS[agent], event => {
        monitorSnapshots.value[agent] = event.payload
      }),
    ),
  ])
  unlisteners.push(...listeners)
  void loadMonitorSnapshots()
})

onBeforeUnmount(() => {
  unlisteners.forEach(unlisten => unlisten())
  window.clearInterval(quotaTimer)
})
</script>

<template>
  <main
    class="tray-shell"
    data-tauri-drag-region="deep"
    @click="opacityOpen = false"
  >
    <section
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
        <!-- Pin stays visible in mini even when other title icons are hidden. -->
        <button
          v-tooltip:top="pinned ? t('tray.unpin') : t('tray.pin')"
          class="tray-pin"
          :class="{ 'is-pinned': pinned }"
          @click="togglePinned"
        >
          <Pin v-if="pinned" :size="13" />
          <PinOff v-else :size="13" />
        </button>
      </div>

      <div v-if="initialLoading" class="initial-loading" role="status">
        <span class="loading-spinner" aria-hidden="true" />
      </div>

      <template v-else>
        <!-- Usage section: whole block gone when hidden (not an empty placeholder). -->
        <template v-if="!usageHidden">
          <template v-if="visibleProviders.length">
            <div
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
                :disabled="loading"
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
                <UsageOrb v-if="usageWindows.length" :windows="usageWindows" :mini="miniMode">
                  <div v-if="!miniMode && snapshot && !error" class="credit-inline">
                    <span class="credit-inline__title">{{ t('tray.reset_credit') }}</span>
                    <div v-if="resetCards.length" class="credit-chips">
                      <div v-for="(card, index) in resetCards" :key="`${card.expires_at ?? 'unknown'}-${index}`" class="credit-chip">
                        <span class="credit-chip__tooltip">
                          <span>{{ t('tray.reset_credit_expiry') }}</span>
                          <span>{{ splitExpiry(card.expires_at).date }}</span>
                          <span v-if="splitExpiry(card.expires_at).time">{{ splitExpiry(card.expires_at).time }}</span>
                        </span>
                        <span class="credit-chip__date">{{ splitExpiry(card.expires_at).date }}</span>
                      </div>
                    </div>
                    <p v-else class="credit-empty">{{ t('tray.no_reset_credit') }}</p>
                  </div>
                </UsageOrb>
                <UsageOrbPlaceholder
                  v-else-if="error"
                  kind="error"
                  :mini="miniMode"
                  :title="t('tray.failed')"
                  :message="t('tray.failed_hint')"
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

            <template v-else-if="selectedProvider === 'kimi-code'">
              <div class="quota-wrap" :class="{ 'is-loading': loading, 'is-mini': miniMode }">
                <UsageOrb v-if="kimiWindows.length" :windows="kimiWindows" :mini="miniMode" />
                <UsageOrbPlaceholder
                  v-else-if="error"
                  kind="error"
                  :mini="miniMode"
                  :title="t('tray.failed')"
                  :message="t('tray.failed_hint')"
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
                  :message="t('tray.failed_hint')"
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

            <template v-else-if="selectedProvider === 'grok-build'">
              <div class="quota-wrap" :class="{ 'is-loading': loading, 'is-mini': miniMode }">
                <!-- Stale cache: never show the orb with expired numbers; only a calm placeholder. -->
                <UsageOrbPlaceholder
                  v-if="grokUsage?.stale"
                  kind="error"
                  :mini="miniMode"
                  :title="t('tray.stale_title')"
                  :message="t('switch.grok_stale_warning')"
                />
                <UsageOrb
                  v-else-if="grokWindows.length"
                  :windows="grokWindows"
                  :mini="miniMode"
                />
                <UsageOrbPlaceholder
                  v-else-if="error"
                  kind="error"
                  :mini="miniMode"
                  :title="t('tray.failed')"
                  :message="t('tray.failed_hint')"
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
        <div v-if="!monitorHidden" class="monitor-strip" :class="{ 'is-mini': miniMode }">
          <div v-if="!miniMode" class="monitor-strip__title">{{ t('ui.monitor_tab') }}</div>
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
          >
            <span
              v-tooltip:[monitorTipPlacement(index)]="miniMode ? '' : t(`session_monitor.status_${row.status}`)"
              class="monitor-dot"
              :class="`is-${row.status}`"
              :data-session-dot="monitorRowKey(row)"
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
              <span
                v-tooltip:[monitorTipPlacement(index)]="miniMode ? '' : (row.userPrompt || t('session_monitor.no_prompt'))"
              >
                {{ row.userPrompt || t('session_monitor.no_prompt') }}
              </span>
            </span>
          </div>
        </div>
      </template>

      <!-- Transition pulses: a session flipping running↔ended blooms here
           (panel center), shrinks, then arcs to its monitor row. -->
      <span
        v-for="pulse in pulses"
        :key="pulse.id"
        :data-pulse-id="pulse.id"
        class="monitor-pulse"
        :class="`is-${pulse.status}`"
      />
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
  /* Ink-wash palette (mirrors src/assets/theme.css). Dark values are applied
     at the bottom of this stylesheet, keyed off the shared data-theme
     attribute (with a prefers-color-scheme fallback when no explicit choice
     exists) so the popup always matches the main window. */
  --tray-canvas: #F8F6F1;
  --tray-surface: #FFFFFE;
  --tray-sunken: #F0EDE4;
  --tray-hover: #EDE9DE;
  --tray-ink: #2A2A2E;
  --tray-ink-2: #5B5B61;
  --tray-ink-3: #8C8B86;
  --tray-ink-4: #B7B5AC;
  --tray-accent: #3A6B8C;
  --tray-accent-strong: #2E5773;
  --tray-accent-soft: rgba(58, 107, 140, .10);
  --tray-accent-mid: rgba(58, 107, 140, .20);
  --tray-highlight: #C9A961;
  --tray-success: #5A8F6B;
  --tray-warning: #B07A3E;
  --tray-danger: #B0524A;
  --tray-hairline: rgba(42, 42, 46, .07);
  --tray-border: rgba(42, 42, 46, .12);
  --tray-on-accent: #FDFCF9;
  --tray-inset: var(--tray-sunken);
  --tray-btn-bg: var(--tray-surface);
  --tray-btn-bg-hover: var(--tray-surface);
  --tray-active-bg: var(--tray-surface);
  --tray-panel-bg: color-mix(in srgb, var(--tray-surface) 97%, transparent);
  --tray-panel-shadow: 0 2px 6px rgba(42, 42, 46, .12);
  --tray-active-shadow: 0 1px 4px rgba(42, 42, 46, .10);
  --tray-success-soft: rgba(90, 143, 107, .12);
  --tray-warning-soft: rgba(176, 122, 62, .10);
  --tray-danger-soft: rgba(176, 82, 74, .10);
  /* Orb ring track: faint tint of the ring color, stronger in dark mode. */
  --tray-ring-track: color-mix(in srgb, currentColor 14%, transparent);

  width: 100%;
  height: 100%;
  min-width: 0;
  padding: 8px;
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
  padding: 14px 16px 10px;
  overflow: hidden;
  border-radius: 25px;
  background: var(--tray-panel-bg);
  box-shadow: var(--tray-panel-shadow);
}

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
  border-radius: 7px;
  color: var(--tray-ink-4);
  background: transparent;
  cursor: pointer;
  transition: color .15s ease, background-color .15s ease;
}
.tray-control-btn:hover { color: var(--tray-ink-2); background: var(--tray-inset); }
.tray-control-btn.is-active { color: var(--tray-accent); background: var(--tray-accent-soft); }
/* Muted = section currently hidden; click again to show. */
.tray-control-btn.is-muted { color: var(--tray-ink-4); opacity: .42; }
.tray-control-btn.is-muted:hover { opacity: .72; color: var(--tray-ink-2); }
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
  border-radius: 10px;
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

/* Pin: unpinned = accent (invite to pin); pinned = quiet gray (persistent). */
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
  border-radius: 7px;
  color: var(--tray-accent);
  background: transparent;
  cursor: pointer;
  transition: color .15s ease, background-color .15s ease;
}
.tray-pin:hover { color: var(--tray-accent); background: var(--tray-accent-soft); }
.tray-pin.is-pinned { color: var(--tray-ink-4); background: transparent; }
.tray-pin.is-pinned:hover { color: var(--tray-ink-2); background: var(--tray-inset); }

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
  border-radius: 7px;
  color: var(--tray-ink-4);
  background: transparent;
  cursor: pointer;
  transition: color .15s ease, background-color .15s ease;
}
.tray-refresh:hover:not(:disabled) { color: var(--tray-ink-2); background: var(--tray-inset); }
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
  color: var(--tray-ink-4);
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
  border-radius: 999px;
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
  border-radius: 999px;
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
   hidden. Same 132px height as the orb/loader so the panel never jumps. */
.tray-empty {
  flex: 0 0 auto;
  height: 132px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--tray-ink-3);
  font-size: 12px;
}
.tray-empty__icon {
  width: 44px;
  height: 44px;
  display: grid;
  place-items: center;
  border-radius: 999px;
  color: var(--tray-ink-3);
  background: var(--tray-inset);
}
.tray-empty__icon svg { width: 22px; height: 22px; }

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
  flex: 0 0 auto;
  min-height: 132px;
  padding: 12px 0 10px;
  transition: opacity .18s ease;
}
.quota-wrap.is-loading { opacity: .72; }

/* Reset credits sit in the orb's side column under the legend, titled like a
   legend row, so all three provider panels share the same overall height. */
.credit-inline { display: flex; flex-direction: column; gap: 4px; }
.credit-inline__title {
  color: var(--tray-ink-2);
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
}
.credit-chips { display: flex; flex-wrap: wrap; gap: 4px; }
.credit-chip {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 2px 6px;
  border: 1px solid var(--tray-hairline);
  border-radius: 6px;
  background: var(--tray-inset);
}
/* Full validity floats above the chip on hover; the chip itself stays a
   compact one-line YYYY-MM-DD tag and never reflows. */
.credit-chip__tooltip {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1px;
  padding: 5px 9px;
  border-radius: 6px;
  color: var(--tray-on-accent);
  background: var(--tray-ink);
  font-size: 10px;
  line-height: 1.5;
  white-space: nowrap;
  opacity: 0;
  pointer-events: none;
  transition: opacity .15s ease;
}
.credit-chip:hover .credit-chip__tooltip { opacity: 1; }
.credit-chip__date {
  color: var(--tray-ink);
  font-size: 10px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.credit-empty { margin: 0; color: var(--tray-ink-3); font-size: 11px; }

/* Monitor strip under the quota area. */
.monitor-strip {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 0 2px;
  border-top: 1px solid var(--tray-hairline);
}
.monitor-strip.is-mini {
  gap: 3px;
  padding-top: 6px;
}
.monitor-strip__title {
  color: var(--tray-ink-2);
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
}
.monitor-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  font-size: 11px;
  line-height: 1.5;
}
.monitor-dot {
  flex: 0 0 auto;
  width: 7px;
  height: 7px;
  border-radius: 999px;
}
.monitor-dot.is-running { background: var(--tray-success); }
.monitor-dot.is-ended { background: var(--tray-ink-4); }

/* Status-transition pulse: blooms at the panel center at full size, then the
   JS-driven WAAPI animation shrinks it and arcs it to its monitor row.
   Positioning is entirely transform-based (see spawnPulse). */
.monitor-pulse {
  position: absolute;
  left: 0;
  top: 0;
  z-index: 30;
  width: 56px;
  height: 56px;
  border-radius: 999px;
  pointer-events: none;
  opacity: 0;
}
.monitor-pulse.is-running {
  background: var(--tray-success);
  box-shadow: 0 0 26px color-mix(in srgb, var(--tray-success) 55%, transparent);
}
.monitor-pulse.is-ended {
  background: var(--tray-ink-4);
  box-shadow: 0 0 20px color-mix(in srgb, var(--tray-ink-4) 45%, transparent);
}
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

/* Dark tray palette: explicit night choice wins; with no explicit choice the
   OS preference decides (same rule as theme.css). The entire selector must
   live inside :global() — a trailing .tray-shell outside the parens gets
   dropped by the scoped-CSS compiler, which silently breaks dark mode. */
:global(html[data-theme="night"] .tray-shell) {
  --tray-canvas: #171B25;
  --tray-surface: #1E2431;
  --tray-sunken: #131722;
  --tray-hover: #28303F;
  --tray-ink: #E7E9F0;
  --tray-ink-2: #B0B6C4;
  --tray-ink-3: #7D8496;
  --tray-ink-4: #50586A;
  --tray-accent: #7DA8C9;
  --tray-accent-strong: #9FBED7;
  --tray-accent-soft: rgba(125, 168, 201, .14);
  --tray-accent-mid: rgba(125, 168, 201, .24);
  --tray-highlight: #D9B97C;
  --tray-success: #8FB89A;
  --tray-warning: #D69963;
  --tray-danger: #D88078;
  --tray-hairline: rgba(231, 233, 240, .06);
  --tray-border: rgba(231, 233, 240, .10);
  --tray-inset: var(--tray-hover);
  --tray-btn-bg: var(--tray-hairline);
  --tray-btn-bg-hover: var(--tray-border);
  --tray-active-bg: var(--tray-hover);
  --tray-panel-shadow: 0 2px 6px rgba(0, 0, 0, .35);
  --tray-active-shadow: 0 1px 4px rgba(0, 0, 0, .28);
  --tray-success-soft: rgba(143, 184, 154, .14);
  --tray-warning-soft: rgba(214, 153, 99, .13);
  --tray-danger-soft: rgba(216, 128, 120, .13);
  --tray-ring-track: color-mix(in srgb, currentColor 24%, transparent);
}

@media (prefers-color-scheme: dark) {
  :global(html:not([data-theme]) .tray-shell) {
    --tray-canvas: #171B25;
    --tray-surface: #1E2431;
    --tray-sunken: #131722;
    --tray-hover: #28303F;
    --tray-ink: #E7E9F0;
    --tray-ink-2: #B0B6C4;
    --tray-ink-3: #7D8496;
    --tray-ink-4: #50586A;
    --tray-accent: #7DA8C9;
    --tray-accent-strong: #9FBED7;
    --tray-accent-soft: rgba(125, 168, 201, .14);
    --tray-accent-mid: rgba(125, 168, 201, .24);
    --tray-highlight: #D9B97C;
    --tray-success: #8FB89A;
    --tray-warning: #D69963;
    --tray-danger: #D88078;
    --tray-hairline: rgba(231, 233, 240, .06);
    --tray-border: rgba(231, 233, 240, .10);
    --tray-inset: var(--tray-hover);
    --tray-btn-bg: var(--tray-hairline);
    --tray-btn-bg-hover: var(--tray-border);
    --tray-active-bg: var(--tray-hover);
    --tray-panel-shadow: 0 2px 6px rgba(0, 0, 0, .35);
    --tray-active-shadow: 0 1px 4px rgba(0, 0, 0, .28);
    --tray-success-soft: rgba(143, 184, 154, .14);
    --tray-warning-soft: rgba(214, 153, 99, .13);
    --tray-danger-soft: rgba(216, 128, 120, .13);
    --tray-ring-track: color-mix(in srgb, currentColor 24%, transparent);
  }
}
</style>
