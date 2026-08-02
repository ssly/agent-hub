<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Pin, PinOff, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import {
  getClaudeSessionMonitorSnapshot,
  getClaudeUsage,
  getCodexSessionMonitorSnapshot,
  getCodexTrayUsage,
  getGrokSessionMonitorSnapshot,
  getGrokUsage,
  getKimiSessionMonitorSnapshot,
  getKimiUsage,
  getKiroSessionMonitorSnapshot,
  getUsageProviderAvailability,
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
import UsageOrb, { type OrbTone, type OrbWindow } from './UsageOrb.vue'
import TrayWaveLoader from './TrayWaveLoader.vue'

const { t, locale } = useI18n()
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
let refreshSequence = 0
let resizeSequence = 0

// --- Mini monitor strip ---------------------------------------------------
// Same data the Monitor tab shows (backend snapshots + change events), but
// reduced to one line per session: status dot + agent + user question.
const MONITOR_AGENTS_LIST: MonitorAgent[] = ['codex', 'claude', 'grok', 'kimi', 'kiro']
const MONITOR_CHANGED_EVENTS: Record<MonitorAgent, string> = {
  codex: 'session-monitor:codex-changed',
  claude: 'session-monitor:claude-changed',
  kiro: 'session-monitor:kiro-changed',
  grok: 'session-monitor:grok-changed',
  kimi: 'session-monitor:kimi-changed',
}
const MONITOR_SNAPSHOT_API: Record<MonitorAgent, () => Promise<MonitorSnapshot>> = {
  codex: getCodexSessionMonitorSnapshot,
  claude: getClaudeSessionMonitorSnapshot,
  kiro: getKiroSessionMonitorSnapshot,
  grok: getGrokSessionMonitorSnapshot,
  kimi: getKimiSessionMonitorSnapshot,
}
const monitorSnapshots = ref<Record<MonitorAgent, MonitorSnapshot>>({
  codex: { revision: 0, sessions: [] },
  claude: { revision: 0, sessions: [] },
  kiro: { revision: 0, sessions: [] },
  grok: { revision: 0, sessions: [] },
  kimi: { revision: 0, sessions: [] },
})

// Merged like the Monitor tab's "all" view: running first, newest activity
// first within each group, capped so the strip never dominates the panel.
const monitorRows = computed<AgentSessionState[]>(() =>
  MONITOR_AGENTS_LIST
    .flatMap(agent => monitorSnapshots.value[agent].sessions.map(session => ({ ...session, agent })))
    .sort((a, b) => {
      if (a.status !== b.status) return a.status === 'running' ? -1 : 1
      return b.updatedAt - a.updatedAt
    })
    .slice(0, 5),
)

function monitorAgentLabel(row: AgentSessionState) {
  // Same provenance rule as the Monitor tab: Kiro rows from the cli/ file
  // watcher (source === 'terminal') are provably Kiro CLI; Codex rows whose
  // hook originator marks the ChatGPT desktop/IDE client are labeled so.
  if (row.agent === 'kiro' && row.source === 'terminal') {
    return t('session_monitor.agent_kiro_cli')
  }
  if (row.agent === 'codex' && row.source === 'chatgpt') {
    return t('session_monitor.source_chatgpt')
  }
  return t(`session_monitor.agent_${row.agent}`)
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
    if (!firstSeeding && !reduceMotion && !hiddenMonitors.value.includes(row.agent)) {
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

// --- Context menu (opacity + per-area visibility) ---------------------------
// Right-click anywhere on the panel opens a two-level menu: window opacity,
// per-provider usage hiding, and per-agent monitor hiding. All three persist
// in localStorage.
const OPACITY_OPTIONS = [80, 85, 90, 95, 100]
const OPACITY_STORAGE_KEY = 'ah-tray-opacity'
const storedOpacity = Number(localStorage.getItem(OPACITY_STORAGE_KEY))
const panelOpacity = ref(OPACITY_OPTIONS.includes(storedOpacity) ? storedOpacity : 100)
const opacityMenu = ref<{ x: number; y: number } | null>(null)
const openSubmenu = ref<'opacity' | 'usage' | 'monitor' | null>(null)
// Which side the submenus open on, recomputed per right-click: right when it
// fits fully inside the window, left otherwise (the window clips overflow,
// so the wrong side makes the submenu invisible).
const submenuSide = ref<'left' | 'right'>('right')

const HIDDEN_USAGE_KEY = 'ah-tray-hidden-usage'
const HIDDEN_MONITOR_KEY = 'ah-tray-hidden-monitor'
// Declared above the computeds that reference it: watch() eagerly evaluates
// its source on creation, so a later const would hit the TDZ at setup time.
const PROVIDER_ORDER: UsageProvider[] = ['codex', 'claude-code', 'grok-build', 'kimi-code']
function loadHidden<K extends string>(key: string): K[] {
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? '[]')
    return Array.isArray(value) ? value : []
  } catch {
    return []
  }
}
const hiddenUsage = ref<UsageProvider[]>(loadHidden(HIDDEN_USAGE_KEY))
const hiddenMonitors = ref<MonitorAgent[]>(loadHidden(HIDDEN_MONITOR_KEY))

function persistHidden() {
  localStorage.setItem(HIDDEN_USAGE_KEY, JSON.stringify(hiddenUsage.value))
  localStorage.setItem(HIDDEN_MONITOR_KEY, JSON.stringify(hiddenMonitors.value))
}

// Providers the panel can actually query right now — the only ones that may
// appear as tabs or in the hide menu.
const queryableProviders = computed<UsageProvider[]>(() =>
  PROVIDER_ORDER.filter(provider =>
    availability.value ? providerAvailable(provider, availability.value) : false,
  ),
)
const visibleProviders = computed<UsageProvider[]>(() =>
  queryableProviders.value.filter(provider => !hiddenUsage.value.includes(provider)),
)
const allUsageHidden = computed(() =>
  queryableProviders.value.length > 0
  && queryableProviders.value.every(provider => hiddenUsage.value.includes(provider)),
)
// Monitor rows minus the agents hidden via the context menu.
const visibleMonitorRows = computed(() =>
  monitorRows.value.filter(row => !hiddenMonitors.value.includes(row.agent)),
)
const allMonitorsHidden = computed(() =>
  MONITOR_AGENTS_LIST.every(agent => hiddenMonitors.value.includes(agent)),
)

function toggleHiddenUsage(provider: UsageProvider) {
  hiddenUsage.value = hiddenUsage.value.includes(provider)
    ? hiddenUsage.value.filter(item => item !== provider)
    : [...hiddenUsage.value, provider]
  persistHidden()
}
function toggleAllUsage() {
  hiddenUsage.value = allUsageHidden.value ? [] : [...queryableProviders.value]
  persistHidden()
}
function toggleHiddenMonitor(agent: MonitorAgent) {
  hiddenMonitors.value = hiddenMonitors.value.includes(agent)
    ? hiddenMonitors.value.filter(item => item !== agent)
    : [...hiddenMonitors.value, agent]
  persistHidden()
}
function toggleAllMonitors() {
  hiddenMonitors.value = allMonitorsHidden.value ? [] : [...MONITOR_AGENTS_LIST]
  persistHidden()
}

// A provider that becomes hidden (or signed out) can no longer be selected.
watch(visibleProviders, list => {
  if (list.length && !list.includes(selectedProvider.value)) {
    selectedProvider.value = list[0]
  }
})

function openOpacityMenu(event: MouseEvent) {
  openSubmenu.value = null
  const MENU_W = 118
  const SUBMENU_W = 120
  const fitsRight = event.clientX + MENU_W + 4 + SUBMENU_W + 8 <= window.innerWidth
  submenuSide.value = fitsRight ? 'right' : 'left'
  // Keep the menu (and its submenu) inside the 400px-wide window.
  const minX = fitsRight ? 8 : SUBMENU_W + 8
  opacityMenu.value = {
    x: Math.max(minX, Math.min(event.clientX, window.innerWidth - MENU_W - 12)),
    y: Math.min(event.clientY, window.innerHeight - 190),
  }
}

function selectOpacity(value: number) {
  panelOpacity.value = value
  localStorage.setItem(OPACITY_STORAGE_KEY, String(value))
  opacityMenu.value = null
}

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

// Context-menu labels for the monitor area (the tray strip watches the Kiro
// CLI directory, so it gets the CLI suffix there too).
function monitorMenuLabel(agent: MonitorAgent) {
  return agent === 'kiro' ? t('session_monitor.agent_kiro_cli') : t(`session_monitor.agent_${agent}`)
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

const panelRef = ref<HTMLElement | null>(null)

// Measure the rendered panel instead of maintaining per-state height
// constants: the window always fits the content exactly, so footer rows
// (e.g. last-query time) can never be clipped by the sections above.
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
  try {
    await resizeUsageTray(clampHeight(measured + 16)) // + shell padding 8×2
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
  () => visibleMonitorRows.value.length,
  () => {
    if (!compactLoading.value) void applyContentHeight()
  },
  { flush: 'post' },
)

// Hiding/showing a provider swaps the quota area between the orb and the
// empty state, which changes the natural panel height.
watch(
  () => [visibleProviders.value.length, hiddenMonitors.value.length],
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
    @contextmenu.prevent="openOpacityMenu"
    @click="opacityMenu = null"
  >
    <section
      ref="panelRef"
      class="tray-panel"
      :class="{ 'tray-panel--loading': initialLoading }"
      :style="{ opacity: panelOpacity / 100 }"
    >
      <span v-if="!initialLoading && lastQueryAt" class="tray-last-query">
        {{ t('tray.last_query', { time: formatDate(lastQueryAt) }) }}
      </span>
      <button
        v-if="!initialLoading"
        v-tooltip:top="t('tray.refresh')"
        class="tray-refresh"
        :disabled="loading"
        @click="refresh(false, false, true)"
      >
        <RefreshCw :size="13" :class="{ 'is-spinning': loading }" />
      </button>
      <button
        v-if="!initialLoading"
        v-tooltip:top="pinned ? t('tray.unpin') : t('tray.pin')"
        class="tray-pin"
        :class="{ 'is-pinned': pinned }"
        @click="togglePinned"
      >
        <PinOff v-if="pinned" :size="13" />
        <Pin v-else :size="13" />
      </button>

      <div v-if="initialLoading" class="initial-loading" role="status">
        <span class="loading-spinner" aria-hidden="true" />
      </div>

      <template v-else>
        <template v-if="visibleProviders.length">
          <div class="provider-switch" role="tablist" :aria-label="t('tray.provider')">
            <button
              v-for="provider in visibleProviders"
              :key="provider"
              class="provider-option"
              :class="{ 'is-active': selectedProvider === provider }"
              role="tab"
              :aria-selected="selectedProvider === provider"
              :disabled="loading"
              @click="selectProvider(provider)"
            >
              {{ PROVIDER_LABELS[provider] }}
            </button>
          </div>
        </template>
        <!-- Fixed empty state: nothing queryable (signed out everywhere) or
             every provider hidden via the context menu. -->
        <div v-else class="tray-empty" role="status">
          <span class="tray-empty__icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.8-3.8"/><path d="M8.5 11h5"/></svg>
          </span>
          <span class="tray-empty__text">{{ t('tray.no_query_items') }}</span>
        </div>

        <template v-if="visibleProviders.length">
        <template v-if="selectedProvider === 'codex'">
          <div class="quota-wrap" :class="{ 'is-loading': loading }">
            <UsageOrb v-if="usageWindows.length" :windows="usageWindows">
              <div v-if="snapshot && !error" class="credit-inline">
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
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh(false, false, true)">{{ t('tray.retry') }}</button>
            </div>
            <TrayWaveLoader v-else-if="loading">{{ t('tray.query_wait') }}</TrayWaveLoader>
            <div v-else class="quota-message quota-message--compact">
              {{ t('tray.no_usage') }}
            </div>
          </div>
        </template>

        <template v-else-if="selectedProvider === 'kimi-code'">
          <div class="quota-wrap" :class="{ 'is-loading': loading }">
            <UsageOrb v-if="kimiWindows.length" :windows="kimiWindows" />
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh(false, false, true)">{{ t('tray.retry') }}</button>
            </div>
            <TrayWaveLoader v-else-if="loading">{{ t('tray.query_wait') }}</TrayWaveLoader>
            <div v-else class="quota-message quota-message--compact">
              {{ t('tray.no_usage') }}
            </div>
          </div>
        </template>

        <template v-else-if="selectedProvider === 'claude-code'">
          <div class="quota-wrap" :class="{ 'is-loading': loading }">
            <UsageOrb v-if="claudeWindows.length" :windows="claudeWindows" />
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh(false, false, true)">{{ t('tray.retry') }}</button>
            </div>
            <TrayWaveLoader v-else-if="loading">{{ t('tray.query_wait') }}</TrayWaveLoader>
            <div v-else class="quota-message quota-message--compact">
              {{ t('tray.no_usage') }}
            </div>
          </div>
        </template>

        <template v-else-if="selectedProvider === 'grok-build'">
          <div v-if="grokUsage?.stale" class="grok-warning">
            {{ t('switch.grok_stale_warning') }}
          </div>

          <div class="quota-wrap" :class="{ 'is-loading': loading }">
            <UsageOrb v-if="grokWindows.length" :windows="grokWindows" />
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh(false, false, true)">{{ t('tray.retry') }}</button>
            </div>
            <TrayWaveLoader v-else-if="loading">{{ t('tray.query_wait') }}</TrayWaveLoader>
            <div v-else class="quota-message quota-message--compact">
              {{ t('tray.no_usage') }}
            </div>
          </div>
        </template>
        </template>

        <!-- Mini monitor strip: one line per session (status dot + agent +
             user question), live via session-monitor change events. Always
             rendered — with a fixed empty state when nothing is visible. -->
        <div class="monitor-strip">
          <div class="monitor-strip__title">{{ t('ui.monitor_tab') }}</div>
          <div v-if="!visibleMonitorRows.length" class="monitor-empty" role="status">
            <span class="monitor-empty__icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M3 12h4l2.5-6 3 12 2.5-6h6"/></svg>
            </span>
            <span>{{ t('tray.no_monitor_items') }}</span>
          </div>
          <div
            v-for="row in visibleMonitorRows"
            :key="`${row.agent}-${row.sessionId}-${row.turnId}`"
            class="monitor-row"
          >
            <span
              v-tooltip:top="t(`session_monitor.status_${row.status}`)"
              class="monitor-dot"
              :class="`is-${row.status}`"
              :data-session-dot="monitorRowKey(row)"
            />
            <span class="monitor-agent">{{ monitorAgentLabel(row) }}</span>
            <span class="monitor-question">
              <span v-tooltip:top="row.userPrompt || t('session_monitor.no_prompt')">
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

    <!-- Right-click menu: opacity + per-area visibility, two levels. -->
    <div
      v-if="opacityMenu"
      class="tray-menu"
      :style="{ left: `${opacityMenu.x}px`, top: `${opacityMenu.y}px` }"
      @click.stop
      @contextmenu.stop
      @mouseleave="openSubmenu = null"
    >
      <div class="tray-menu__parent" @mouseenter="openSubmenu = 'opacity'">
        <span>{{ t('tray.opacity') }}</span>
        <span class="tray-menu__caret">{{ submenuSide === 'right' ? '›' : '‹' }}</span>
        <div v-if="openSubmenu === 'opacity'" class="tray-submenu" :class="`tray-submenu--${submenuSide}`">
          <button
            v-for="option in OPACITY_OPTIONS"
            :key="option"
            class="tray-submenu__option"
            :class="{ 'is-active': panelOpacity === option }"
            @click="selectOpacity(option)"
          >
            <span class="tray-submenu__check">{{ panelOpacity === option ? '✓' : '' }}</span>
            {{ option }}%
          </button>
        </div>
      </div>

      <div class="tray-menu__parent" @mouseenter="openSubmenu = 'usage'">
        <span>{{ t('tray.hide_usage') }}</span>
        <span class="tray-menu__caret">{{ submenuSide === 'right' ? '›' : '‹' }}</span>
        <div v-if="openSubmenu === 'usage'" class="tray-submenu" :class="`tray-submenu--${submenuSide}`">
          <button
            class="tray-submenu__option"
            :class="{ 'is-checked': allUsageHidden }"
            @click="toggleAllUsage()"
          >
            <span class="tray-submenu__check">{{ allUsageHidden ? '✓' : '' }}</span>
            {{ t('tray.all') }}
          </button>
          <button
            v-for="provider in queryableProviders"
            :key="provider"
            class="tray-submenu__option"
            :class="{ 'is-checked': hiddenUsage.includes(provider) }"
            @click="toggleHiddenUsage(provider)"
          >
            <span class="tray-submenu__check">{{ hiddenUsage.includes(provider) ? '✓' : '' }}</span>
            {{ PROVIDER_LABELS[provider] }}
          </button>
        </div>
      </div>

      <div class="tray-menu__parent" @mouseenter="openSubmenu = 'monitor'">
        <span>{{ t('tray.hide_monitor') }}</span>
        <span class="tray-menu__caret">{{ submenuSide === 'right' ? '›' : '‹' }}</span>
        <div v-if="openSubmenu === 'monitor'" class="tray-submenu" :class="`tray-submenu--${submenuSide}`">
          <button
            class="tray-submenu__option"
            :class="{ 'is-checked': allMonitorsHidden }"
            @click="toggleAllMonitors()"
          >
            <span class="tray-submenu__check">{{ allMonitorsHidden ? '✓' : '' }}</span>
            {{ t('tray.all') }}
          </button>
          <button
            v-for="agent in MONITOR_AGENTS_LIST"
            :key="agent"
            class="tray-submenu__option"
            :class="{ 'is-checked': hiddenMonitors.includes(agent) }"
            @click="toggleHiddenMonitor(agent)"
          >
            <span class="tray-submenu__check">{{ hiddenMonitors.includes(agent) ? '✓' : '' }}</span>
            {{ monitorMenuLabel(agent) }}
          </button>
        </div>
      </div>
    </div>
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
  color: var(--tray-ink-4);
  background: transparent;
  cursor: pointer;
  transition: color .15s ease, background-color .15s ease;
}
.tray-pin:hover { color: var(--tray-ink-2); background: var(--tray-inset); }
.tray-pin.is-pinned { color: var(--tray-accent); background: var(--tray-accent-soft); }

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

.grok-warning {
  flex: 0 0 44px;
  min-height: 44px;
  display: flex;
  align-items: center;
  margin: 0;
  border-radius: 14px;
  padding: 7px 11px;
  color: var(--tray-warning);
  background: var(--tray-warning-soft);
  font-size: 11px;
  line-height: 1.35;
}

.quota-wrap {
  flex: 0 0 auto;
  padding: 12px 0 10px;
  transition: opacity .18s ease;
}
.quota-wrap.is-loading { opacity: .72; }

.quota-message {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: flex-start;
  min-height: 110px;
  gap: 6px;
  color: var(--tray-ink-3);
}
.quota-message strong { color: var(--tray-danger); }
.quota-message span { max-height: 42px; overflow: hidden; font-size: 12px; }
.quota-message button { min-height: 34px; padding: 0; border: 0; color: var(--tray-accent); background: none; cursor: pointer; }
.quota-message--compact { min-height: 48px; align-items: center; font-size: 13px; }

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

/* Mini monitor strip under the quota area. */
.monitor-strip {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 0 2px;
  border-top: 1px solid var(--tray-hairline);
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
  color: var(--tray-ink);
  font-weight: 700;
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

/* Right-click menu: parent rows open a submenu to their left on hover. */
.tray-menu {
  position: fixed;
  z-index: 20;
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 118px;
  padding: 5px;
  border: 1px solid var(--tray-border);
  border-radius: 10px;
  background: var(--tray-surface);
  box-shadow: var(--tray-panel-shadow);
}
.tray-menu__parent {
  position: relative;
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 26px;
  padding: 0 8px;
  border-radius: 6px;
  color: var(--tray-ink-2);
  font-size: 12px;
  cursor: default;
  user-select: none;
}
.tray-menu__parent:hover { background: var(--tray-inset); color: var(--tray-ink); }
.tray-menu__caret { margin-left: auto; color: var(--tray-ink-3); font-size: 11px; }
.tray-submenu {
  position: absolute;
  /* Default: open to the right of the parent menu. */
  left: calc(100% + 4px);
  top: -6px;
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 108px;
  padding: 5px;
  border: 1px solid var(--tray-border);
  border-radius: 10px;
  background: var(--tray-surface);
  box-shadow: var(--tray-panel-shadow);
}
/* Flipped when the right side would overflow the window. */
.tray-submenu--left {
  left: auto;
  right: calc(100% + 4px);
}
.tray-submenu__option {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 24px;
  padding: 0 8px;
  border: 0;
  border-radius: 6px;
  color: var(--tray-ink-2);
  background: transparent;
  font: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  cursor: pointer;
}
.tray-submenu__option:hover { background: var(--tray-inset); color: var(--tray-ink); }
.tray-submenu__option.is-active,
.tray-submenu__option.is-checked { color: var(--tray-accent); font-weight: 650; }
.tray-submenu__check { width: 12px; flex: 0 0 auto; }

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
