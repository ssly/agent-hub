import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as api from '@/lib/api'

export type HookAction = 'install' | 'uninstall'
export type SessionSource =
  | 'terminal'
  | 'chatgpt'
  | 'cursor'
  /** Antigravity 2.0 desktop app. */
  | 'antigravity'
  /** Antigravity IDE surface. */
  | 'antigravity-ide'
export type RuntimeStatus = 'running' | 'ended'
export type MonitorAgent = 'codex' | 'claude' | 'cursor' | 'antigravity' | 'grok' | 'kimi' | 'zcode'
/** Same relative order as platform/registry.rs (skip Shared Pool / no-hook agents). */
export const MONITOR_AGENTS: MonitorAgent[] = [
  'codex',
  'claude',
  'cursor',
  'antigravity',
  'grok',
  'kimi',
  'zcode',
]
/** Sidebar tab: one of the agents, or the merged "all" view. */
export type MonitorTab = MonitorAgent | 'all'
/** Agents whose monitor feed is driven by installed hooks. */
export type HookAgent = MonitorAgent
export const HOOK_AGENTS: HookAgent[] = [...MONITOR_AGENTS]
/** Monitor agent → sessions-browser platform id, so the shared messages /
 *  resume modals can load full history through the sessions adapters. */
export const MONITOR_AGENT_PLATFORM: Partial<Record<MonitorAgent, string>> = {
  codex: 'codex',
  claude: 'claude-code',
  // cursor: no sessions browser adapter yet
  antigravity: 'antigravity',
  grok: 'grok',
  kimi: 'kimi',
  zcode: 'zcode',
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
  zcode: 'session-monitor:zcode-changed',
}

const snapshotApi: Record<MonitorAgent, () => Promise<MonitorSnapshot>> = {
  codex: api.getCodexSessionMonitorSnapshot,
  claude: api.getClaudeSessionMonitorSnapshot,
  cursor: api.getCursorSessionMonitorSnapshot,
  antigravity: api.getAntigravitySessionMonitorSnapshot,
  grok: api.getGrokSessionMonitorSnapshot,
  kimi: api.getKimiSessionMonitorSnapshot,
  zcode: api.getZCodeSessionMonitorSnapshot,
}

const deleteSessionApi: Record<MonitorAgent, (sessionId: string) => Promise<void>> = {
  codex: api.deleteCodexSessionMonitorSession,
  claude: api.deleteClaudeSessionMonitorSession,
  cursor: api.deleteCursorSessionMonitorSession,
  antigravity: api.deleteAntigravitySessionMonitorSession,
  grok: api.deleteGrokSessionMonitorSession,
  kimi: api.deleteKimiSessionMonitorSession,
  zcode: api.deleteZCodeSessionMonitorSession,
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
  zcode: {
    status: api.getZCodeHookStatus,
    preview: api.previewZCodeHookChange,
    apply: api.applyZCodeHookChange,
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
    zcode: emptySnapshot(),
  })
  const hookStatuses = ref<Record<HookAgent, HookStatus | null>>({
    codex: null,
    claude: null,
    cursor: null,
    antigravity: null,
    grok: null,
    kimi: null,
    zcode: null,
  })
  const loading = ref(false)
  const hookLoading = ref(false)
  const error = ref('')
  const previewOpen = ref(false)
  const previewLoading = ref(false)
  const previewAgent = ref<HookAgent>('codex')
  const preview = ref<HookChangePreview | null>(null)
  const previewError = ref('')
  /** True after the first successful hydrate — re-entering the tab can paint
   *  from cache immediately while a background refresh catches up. */
  const hydrated = ref(false)
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
      ? MONITOR_AGENTS.flatMap(agent =>
        snapshots.value[agent].sessions.map(session => ({ ...session, agent })),
      )
      : snapshots.value[tab].sessions.map(session => ({ ...session, agent: tab }))
    return sessions.sort((a, b) => {
      if (a.status !== b.status) return a.status === 'running' ? -1 : 1
      return b.updatedAt - a.updatedAt
    })
  })

  async function loadSnapshot(agent: MonitorAgent): Promise<string | null> {
    try {
      snapshots.value[agent] = (await snapshotApi[agent]()) || emptySnapshot()
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
            snapshots.value[agent] = event.payload
          }),
        ),
      )
    } catch {
      // Listener setup is best-effort; manual refresh still works.
      listenersReady = false
    }
  }

  /**
   * Start data loading. Must be scheduled *after* the loading UI paints
   * (see SessionMonitorView). Never blocks the caller — all IPC is fire-and-forget.
   */
  function initialize() {
    void ensureListeners()
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

  async function openHookPreview(agent: HookAgent, action: HookAction) {
    previewAgent.value = agent
    previewOpen.value = true
    previewLoading.value = true
    previewError.value = ''
    preview.value = null
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
    hookStatuses,
    hookStatus,
    loading,
    hookLoading,
    error,
    previewOpen,
    previewLoading,
    previewAgent,
    preview,
    previewError,
    hydrated,
    beginEnter,
    initialize,
    refresh,
    dispose,
    openHookPreview,
    closeHookPreview,
    applyHookPreview,
    deleteSession,
    messagesModalOpen,
    resumeModalOpen,
    modalSession,
    resumeSession,
    openMessages,
    openResume,
  }
})
