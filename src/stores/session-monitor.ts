import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as api from '@/lib/api'

/** Backend write action (install rewrites managed handlers; uninstall removes them). */
export type HookAction = 'install' | 'uninstall'
/** UI intent: reset is install under a different label (one-step reinstall). */
export type HookPreviewKind = HookAction | 'reset'
export type SessionSource =
  | 'terminal'
  | 'chatgpt'
  | 'cursor'
  /** Antigravity 2.0 desktop app. */
  | 'antigravity'
  /** Antigravity IDE surface. */
  | 'antigravity-ide'
export type RuntimeStatus = 'running' | 'waiting' | 'failed' | 'ended'

/** Sort key: working, waiting-for-confirm, failed, then ended. */
export function monitorStatusRank(status: RuntimeStatus): number {
  if (status === 'running') return 0
  if (status === 'waiting') return 1
  if (status === 'failed') return 2
  return 3
}
export type MonitorAgent =
  | 'codex'
  | 'claude'
  | 'cursor'
  | 'antigravity'
  | 'grok'
  | 'kimi'
  | 'qwen'
  | 'zcode'
  | 'workbuddy'
  | 'kiro'
  | 'dsh'
  | 'omp'
/** Same relative order as platform/registry.rs (skip Shared). */
export const MONITOR_AGENTS: MonitorAgent[] = [
  'codex',
  'claude',
  'cursor',
  'antigravity',
  'grok',
  'kimi',
  'qwen',
  'zcode',
  'workbuddy',
  'kiro',
  'dsh',
  'omp',
]
/** Sidebar tab: one of the agents, or the merged "all" view. */
export type MonitorTab = MonitorAgent | 'all'
/** Agents whose hooks can report the yellow waiting-for-confirm light. */
export const WAITING_MONITOR_AGENTS: readonly MonitorAgent[] = [
  'codex',
  'claude',
  'grok',
  'kimi',
  'qwen',
  'zcode',
  'workbuddy',
  'dsh',
  'omp',
]
/** Agents whose hooks can report a red failed light (`StopFailure` / Cursor `error`). */
export const FAILED_MONITOR_AGENTS: readonly MonitorAgent[] = [
  'claude',
  'cursor',
  'grok',
  'kimi',
  'qwen',
  'workbuddy',
  'dsh',
]

/** Capability lights on the agent icon row: 红 / 黄 / 绿 only (no gray). */
export function monitorAgentLights(agent: MonitorTab): RuntimeStatus[] {
  const lights: RuntimeStatus[] = []
  if (agent === 'all' || FAILED_MONITOR_AGENTS.includes(agent as MonitorAgent)) {
    lights.push('failed')
  }
  if (agent === 'all' || WAITING_MONITOR_AGENTS.includes(agent as MonitorAgent)) {
    lights.push('waiting')
  }
  lights.push('running')
  return lights
}
/** Agents whose monitor feed is driven by installed hooks. */
export type HookAgent = MonitorAgent
export const HOOK_AGENTS: HookAgent[] = [...MONITOR_AGENTS]
/** Monitor agent → sessions-browser platform id, so the shared messages /
 *  resume modals can load full history through the sessions adapters. */
export const MONITOR_AGENT_PLATFORM: Partial<Record<MonitorAgent, string>> = {
  codex: 'codex',
  claude: 'claude-code',
  cursor: 'cursor',
  antigravity: 'antigravity',
  grok: 'grok',
  kimi: 'kimi',
  qwen: 'qwen',
  zcode: 'zcode',
  workbuddy: 'workbuddy',
  kiro: 'kiro',
  dsh: 'dsh',
  omp: 'omp',
}

export interface SessionState {
  sessionId: string
  turnId: string
  source: SessionSource
  status: RuntimeStatus
  cwd?: string | null
  userPrompt?: string | null
  assistantReply?: string | null
  updatedAt: number
  unread: boolean
}

/** A session row tagged with the agent it came from, for the merged view. */
export interface AgentSessionState extends SessionState {
  agent: MonitorAgent
}

export interface MonitorSnapshot {
  revision: number
  sessions: SessionState[]
}

export interface HookStatus {
  installed: boolean
  configPath: string
  command: string
  managedHandlerCount: number
  issue?: string | null
}

export interface HookDiffLine {
  tag: 'added' | 'removed' | 'context'
  content: string
}

export interface HookChangePreview {
  action: HookAction
  configPath: string
  command: string
  beforeHash: string
  diffLines: HookDiffLine[]
  added: number
  removed: number
  changed: boolean
}

const CHANGED_EVENTS: Record<MonitorAgent, string> = {
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
}

const snapshotApi: Record<MonitorAgent, () => Promise<MonitorSnapshot>> = {
  codex: api.getCodexSessionMonitorSnapshot,
  claude: api.getClaudeSessionMonitorSnapshot,
  cursor: api.getCursorSessionMonitorSnapshot,
  antigravity: api.getAntigravitySessionMonitorSnapshot,
  grok: api.getGrokSessionMonitorSnapshot,
  kimi: api.getKimiSessionMonitorSnapshot,
  qwen: api.getQwenSessionMonitorSnapshot,
  zcode: api.getZCodeSessionMonitorSnapshot,
  workbuddy: api.getWorkbuddySessionMonitorSnapshot,
  kiro: api.getKiroSessionMonitorSnapshot,
  dsh: api.getDshSessionMonitorSnapshot,
  omp: api.getOmpSessionMonitorSnapshot,
}

const deleteSessionApi: Record<MonitorAgent, (sessionId: string) => Promise<void>> = {
  codex: api.deleteCodexSessionMonitorSession,
  claude: api.deleteClaudeSessionMonitorSession,
  cursor: api.deleteCursorSessionMonitorSession,
  antigravity: api.deleteAntigravitySessionMonitorSession,
  grok: api.deleteGrokSessionMonitorSession,
  kimi: api.deleteKimiSessionMonitorSession,
  qwen: api.deleteQwenSessionMonitorSession,
  zcode: api.deleteZCodeSessionMonitorSession,
  workbuddy: api.deleteWorkbuddySessionMonitorSession,
  kiro: api.deleteKiroSessionMonitorSession,
  dsh: api.deleteDshSessionMonitorSession,
  omp: api.deleteOmpSessionMonitorSession,
}

const hookApi: Record<HookAgent, {
  status: () => Promise<HookStatus>
  preview: (action: HookAction) => Promise<HookChangePreview>
  apply: (action: HookAction, expectedBeforeHash: string) => Promise<HookStatus>
}> = {
  codex: {
    status: api.getCodexHookStatus,
    preview: api.previewCodexHookChange,
    apply: api.applyCodexHookChange,
  },
  claude: {
    status: api.getClaudeHookStatus,
    preview: api.previewClaudeHookChange,
    apply: api.applyClaudeHookChange,
  },
  cursor: {
    status: api.getCursorHookStatus,
    preview: api.previewCursorHookChange,
    apply: api.applyCursorHookChange,
  },
  antigravity: {
    status: api.getAntigravityHookStatus,
    preview: api.previewAntigravityHookChange,
    apply: api.applyAntigravityHookChange,
  },
  grok: {
    status: api.getGrokHookStatus,
    preview: api.previewGrokHookChange,
    apply: api.applyGrokHookChange,
  },
  kimi: {
    status: api.getKimiHookStatus,
    preview: api.previewKimiHookChange,
    apply: api.applyKimiHookChange,
  },
  qwen: {
    status: api.getQwenHookStatus,
    preview: api.previewQwenHookChange,
    apply: api.applyQwenHookChange,
  },
  zcode: {
    status: api.getZCodeHookStatus,
    preview: api.previewZCodeHookChange,
    apply: api.applyZCodeHookChange,
  },
  workbuddy: {
    status: api.getWorkbuddyHookStatus,
    preview: api.previewWorkbuddyHookChange,
    apply: api.applyWorkbuddyHookChange,
  },
  kiro: {
    status: api.getKiroHookStatus,
    preview: api.previewKiroHookChange,
    apply: api.applyKiroHookChange,
  },
  dsh: {
    status: api.getDshHookStatus,
    preview: api.previewDshHookChange,
    apply: api.applyDshHookChange,
  },
  omp: {
    status: api.getOmpHookStatus,
    preview: api.previewOmpHookChange,
    apply: api.applyOmpHookChange,
  },
}

function emptySnapshot(): MonitorSnapshot {
  return { revision: 0, sessions: [] }
}

function errorMessage(error: any): string {
  return error?.General || error?.message || String(error)
}

export const useSessionMonitorStore = defineStore('session-monitor', () => {
  const activeAgent = ref<MonitorTab>('codex')
  const snapshots = ref<Record<MonitorAgent, MonitorSnapshot>>({
    codex: emptySnapshot(),
    claude: emptySnapshot(),
    cursor: emptySnapshot(),
    antigravity: emptySnapshot(),
    grok: emptySnapshot(),
    kimi: emptySnapshot(),
    qwen: emptySnapshot(),
    zcode: emptySnapshot(),
    workbuddy: emptySnapshot(),
    kiro: emptySnapshot(),
    dsh: emptySnapshot(),
    omp: emptySnapshot(),
  })
  const hookStatuses = ref<Record<HookAgent, HookStatus | null>>({
    codex: null,
    claude: null,
    cursor: null,
    antigravity: null,
    grok: null,
    kimi: null,
    qwen: null,
    zcode: null,
    workbuddy: null,
    kiro: null,
    dsh: null,
    omp: null,
  })
  const loading = ref(false)
  const hookLoading = ref(false)
  const dshWeb = ref<api.DshWebStatus | null>(null)
  const dshWebBusy = ref(false)
  const error = ref('')
  const previewOpen = ref(false)
  const previewLoading = ref(false)
  const previewAgent = ref<HookAgent>('codex')
  const preview = ref<HookChangePreview | null>(null)
  const previewError = ref('')
  /** True after the first successful hydrate — re-entering the tab can paint
   *  from cache immediately while a background refresh catches up. */
  const hydrated = ref(false)
  /** Agents whose platform presence directory exists (backend probe).
   *  null = not loaded yet → show every agent (no first-paint flicker or
   *  empty sidebar); a failed probe stays null for the same reason. */
  const availableAgents = ref<MonitorAgent[] | null>(null)
  /** MONITOR_AGENTS filtered to installed platforms once availability lands. */
  const visibleAgents = computed<MonitorAgent[]>(() =>
    availableAgents.value === null
      ? [...MONITOR_AGENTS]
      : MONITOR_AGENTS.filter(agent => availableAgents.value!.includes(agent)),
  )
  let availabilityRequested = false
  let listenersReady = false
  let unlisten: (() => void)[] = []
  let refreshSeq = 0

  const snapshot = computed(() =>
    activeAgent.value === 'all' ? emptySnapshot() : snapshots.value[activeAgent.value],
  )
  const hookStatus = computed(() =>
    (HOOK_AGENTS as string[]).includes(activeAgent.value)
      ? hookStatuses.value[activeAgent.value as HookAgent]
      : null,
  )
  /** Rows for the current tab, both for "all" and single agents: running
   *  sessions first, then newest activity first within each status group. */
  const displaySessions = computed<AgentSessionState[]>(() => {
    const tab = activeAgent.value
    const sessions = tab === 'all'
      ? visibleAgents.value.flatMap(agent =>
        snapshots.value[agent].sessions.map(session => ({ ...session, agent })),
      )
      : snapshots.value[tab].sessions.map(session => ({ ...session, agent: tab }))
    return sessions.sort((a, b) => {
      const rank = monitorStatusRank(a.status) - monitorStatusRank(b.status)
      if (rank !== 0) return rank
      return b.updatedAt - a.updatedAt
    })
  })
  const unreadCountByAgent = computed<Record<MonitorAgent, number>>(() => {
    const counts = {} as Record<MonitorAgent, number>
    for (const agent of MONITOR_AGENTS) {
      counts[agent] = snapshots.value[agent].sessions.filter(session => session.unread).length
    }
    return counts
  })
  const totalUnread = computed(() =>
    visibleAgents.value.reduce((total, agent) => total + unreadCountByAgent.value[agent], 0),
  )

  function unreadForAgent(agent: MonitorAgent) {
    return unreadCountByAgent.value[agent]
  }

  // Snapshot fetches can resolve after a newer cross-window event. Keep the
  // monotonic backend revision so an old response never rolls unread state
  // (or any other visible session field) backwards.
  function applySnapshot(agent: MonitorAgent, next: MonitorSnapshot) {
    if (next.revision < snapshots.value[agent].revision) return
    snapshots.value[agent] = next
  }

  async function loadSnapshot(agent: MonitorAgent): Promise<string | null> {
    try {
      applySnapshot(agent, (await snapshotApi[agent]()) || emptySnapshot())
      return null
    } catch (cause) {
      return errorMessage(cause)
    }
  }

  async function loadHook(agent: HookAgent): Promise<string | null> {
    try {
      hookStatuses.value[agent] = await hookApi[agent].status()
      return null
    } catch (cause) {
      return errorMessage(cause)
    }
  }

  async function loadAgent(agent: MonitorAgent): Promise<string[]> {
    const tasks: Promise<string | null>[] = [loadSnapshot(agent)]
    if ((HOOK_AGENTS as string[]).includes(agent)) {
      tasks.push(loadHook(agent as HookAgent))
    }
    return (await Promise.all(tasks)).filter((msg): msg is string => Boolean(msg))
  }

  /**
   * Called from the sidebar *before* switching tabs. Only flips reactive
   * flags — zero IPC, zero file IO — so the click handler returns instantly
   * and the first paint can show a full-page loader.
   */
  function beginEnter() {
    if (!hydrated.value) {
      loading.value = true
      error.value = ''
    }
  }

  /**
   * Load monitor data. Prioritises the active sidebar agent (usually Codex);
   * remaining agents load in the background. Pass `{ background: true }` to
   * refresh without flipping the full-page loading flag (used on re-entry).
   */
  async function refresh(opts?: { background?: boolean }) {
    const seq = ++refreshSeq
    const background = opts?.background === true
    const primary = activeAgent.value === 'all' ? null : activeAgent.value
    const rest = primary
      ? MONITOR_AGENTS.filter(agent => agent !== primary)
      : [...MONITOR_AGENTS]

    if (!background) {
      loading.value = true
      error.value = ''
    }

    const messages: string[] = []

    // Phase 1 — active agent only, so Codex (default) is interactive ASAP.
    if (primary) {
      messages.push(...(await loadAgent(primary)))
      if (seq !== refreshSeq) return
      if (!background) loading.value = false
      hydrated.value = true
      // Remaining agents only feed the "all" view / tag dots — never block UI.
      void loadRest(seq, rest, messages)
      return
    }

    // "all" tab: need every agent; load them together then clear loading.
    messages.push(...(await Promise.all(rest.map(agent => loadAgent(agent)))).flat())
    if (seq !== refreshSeq) return
    hydrated.value = true
    error.value = messages.filter(Boolean).join('；')
    loading.value = false
  }

  async function loadRest(seq: number, rest: MonitorAgent[], priorMessages: string[]) {
    const restMessages = (await Promise.all(rest.map(agent => loadAgent(agent)))).flat()
    if (seq !== refreshSeq) return
    error.value = [...priorMessages, ...restMessages].filter(Boolean).join('；')
  }

  async function ensureListeners() {
    if (listenersReady) return
    listenersReady = true
    const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__
    if (!isTauri) return
    try {
      const { listen } = await import('@tauri-apps/api/event')
      unlisten = await Promise.all(
        MONITOR_AGENTS.map(agent =>
          listen<MonitorSnapshot>(CHANGED_EVENTS[agent], event => {
            applySnapshot(agent, event.payload)
          }),
        ),
      )
    } catch {
      // Listener setup is best-effort; manual refresh still works.
      listenersReady = false
    }
  }

  /**
   * Probe which agents are installed (platform presence directories, backend
   * side). Best-effort like ensureListeners: outside Tauri or on failure the
   * list stays null and every agent keeps showing.
   */
  async function loadAvailability() {
    if (availabilityRequested) return
    availabilityRequested = true
    const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__
    if (!isTauri) return
    try {
      const ids = await api.listAvailableMonitorAgents()
      availableAgents.value = MONITOR_AGENTS.filter(agent => ids.includes(agent))
      // Active tab got filtered out → fall back to the first visible agent.
      if (
        activeAgent.value !== 'all'
        && !visibleAgents.value.includes(activeAgent.value)
      ) {
        activeAgent.value = visibleAgents.value[0] ?? 'all'
      }
    } catch {
      // Stay null (full list) — availability must never hide agents on error.
      availabilityRequested = false
    }
  }

  /**
   * Start data loading. Must be scheduled *after* the loading UI paints
   * (see SessionMonitorView). Never blocks the caller — all IPC is fire-and-forget.
   */
  function initialize() {
    void ensureListeners()
    void loadAvailability()
    if (hydrated.value) {
      void refresh({ background: true })
      return
    }
    void refresh()
  }

  function dispose() {
    unlisten.forEach(fn => fn())
    unlisten = []
    listenersReady = false
  }

  /** UI-facing kind for modal copy; `reset` previews/applies as install. */
  const previewKind = ref<HookPreviewKind>('install')

  async function openHookPreview(agent: HookAgent, kind: HookPreviewKind) {
    previewAgent.value = agent
    previewKind.value = kind
    previewOpen.value = true
    previewLoading.value = true
    previewError.value = ''
    preview.value = null
    const action: HookAction = kind === 'reset' ? 'install' : kind
    try {
      preview.value = await hookApi[agent].preview(action)
    } catch (cause) {
      previewError.value = errorMessage(cause)
    } finally {
      previewLoading.value = false
    }
  }

  function closeHookPreview() {
    if (hookLoading.value) return
    previewOpen.value = false
    preview.value = null
    previewError.value = ''
    previewKind.value = 'install'
  }

  async function applyHookPreview() {
    if (!preview.value || !preview.value.changed) return
    hookLoading.value = true
    previewError.value = ''
    try {
      hookStatuses.value[previewAgent.value] = await hookApi[previewAgent.value].apply(
        preview.value.action,
        preview.value.beforeHash,
      )
      previewOpen.value = false
      preview.value = null
    } catch (cause) {
      previewError.value = errorMessage(cause)
    } finally {
      hookLoading.value = false
    }
  }

  // Deleting from the monitor only drops the row from the local snapshot —
  // the on-disk session record is untouched (that is the Sessions browser's
  // delete). The card's two-step confirm mirrors the Sessions UX.
  async function refreshDshWebStatus() {
    try {
      dshWeb.value = await api.getDshWebStatus()
    } catch (cause) {
      dshWeb.value = { state: 'stopped', error: errorMessage(cause) }
    }
  }

  async function startDshWeb() {
    dshWebBusy.value = true
    try {
      dshWeb.value = await api.startDshWeb()
    } finally {
      dshWebBusy.value = false
    }
  }

  async function stopDshWeb() {
    dshWebBusy.value = true
    try {
      dshWeb.value = await api.stopDshWeb()
    } finally {
      dshWebBusy.value = false
    }
  }

  async function deleteSession(sessionId: string, agent?: MonitorAgent) {
    const target = agent ?? (activeAgent.value === 'all' ? undefined : activeAgent.value)
    if (!target) return
    try {
      await deleteSessionApi[target](sessionId)
      const current = snapshots.value[target]
      snapshots.value[target] = {
        ...current,
        sessions: current.sessions.filter(session => session.sessionId !== sessionId),
      }
    } catch (cause) {
      error.value = errorMessage(cause)
    }
  }

  async function markSessionRead(session: AgentSessionState) {
    if (!session.unread) return
    const observedUpdatedAt = session.updatedAt
    try {
      await api.markSessionMonitorSessionRead(session.agent, session.sessionId, observedUpdatedAt)
      // Native windows normally receive the persisted snapshot event first.
      // This guarded local mirror also keeps browser debug responsive, while
      // never clearing a newer version that arrived during the IPC roundtrip.
      const current = snapshots.value[session.agent]
      const latest = current.sessions.find(item => item.sessionId === session.sessionId)
      if (!latest?.unread || latest.updatedAt !== observedUpdatedAt) return
      snapshots.value[session.agent] = {
        ...current,
        sessions: current.sessions.map(item =>
          item.sessionId === session.sessionId && item.updatedAt === observedUpdatedAt
            ? { ...item, unread: false }
            : item,
        ),
      }
    } catch (cause) {
      error.value = errorMessage(cause)
    }
  }

  // Shared-modal open state. The modals fetch full history / resume commands
  // through the sessions adapters (MONITOR_AGENT_PLATFORM), so the monitor
  // itself still keeps only its lightweight snapshot data.
  const messagesModalOpen = ref(false)
  const resumeModalOpen = ref(false)
  const modalSession = ref<AgentSessionState | null>(null)
  const resumeSession = ref<AgentSessionState | null>(null)

  function openMessages(session: AgentSessionState) {
    modalSession.value = session
    messagesModalOpen.value = true
  }

  function openResume(session: AgentSessionState) {
    resumeSession.value = session
    resumeModalOpen.value = true
  }

  return {
    activeAgent,
    snapshots,
    snapshot,
    displaySessions,
    unreadCountByAgent,
    totalUnread,
    unreadForAgent,
    hookStatuses,
    hookStatus,
    loading,
    hookLoading,
    dshWeb,
    dshWebBusy,
    error,
    previewOpen,
    previewLoading,
    previewAgent,
    previewKind,
    preview,
    previewError,
    hydrated,
    availableAgents,
    visibleAgents,
    beginEnter,
    initialize,
    refresh,
    dispose,
    openHookPreview,
    closeHookPreview,
    applyHookPreview,
    refreshDshWebStatus,
    startDshWeb,
    stopDshWeb,
    deleteSession,
    markSessionRead,
    messagesModalOpen,
    resumeModalOpen,
    modalSession,
    resumeSession,
    openMessages,
    openResume,
  }
})
