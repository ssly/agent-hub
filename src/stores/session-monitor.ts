import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as api from '@/lib/api'

export type HookAction = 'install' | 'uninstall'
export type SessionSource = 'terminal' | 'chatgpt' | 'cursor'
export type RuntimeStatus = 'running' | 'ended'
export type MonitorAgent = 'codex' | 'claude' | 'cursor' | 'grok' | 'kimi' | 'zcode'
export const MONITOR_AGENTS: MonitorAgent[] = ['codex', 'claude', 'cursor', 'grok', 'kimi', 'zcode']
/** Sidebar tab: one of the agents, or the merged "all" view. */
export type MonitorTab = MonitorAgent | 'all'
/** Agents whose monitor feed is driven by installed hooks. */
export type HookAgent = 'codex' | 'claude' | 'cursor' | 'grok' | 'kimi' | 'zcode'
export const HOOK_AGENTS: HookAgent[] = ['codex', 'claude', 'cursor', 'grok', 'kimi', 'zcode']
/** Monitor agent → sessions-browser platform id, so the shared messages /
 *  resume modals can load full history through the sessions adapters. */
export const MONITOR_AGENT_PLATFORM: Partial<Record<MonitorAgent, string>> = {
  codex: 'codex',
  claude: 'claude-code',
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
  grok: 'session-monitor:grok-changed',
  kimi: 'session-monitor:kimi-changed',
  zcode: 'session-monitor:zcode-changed',
}

const snapshotApi: Record<MonitorAgent, () => Promise<MonitorSnapshot>> = {
  codex: api.getCodexSessionMonitorSnapshot,
  claude: api.getClaudeSessionMonitorSnapshot,
  cursor: api.getCursorSessionMonitorSnapshot,
  grok: api.getGrokSessionMonitorSnapshot,
  kimi: api.getKimiSessionMonitorSnapshot,
  zcode: api.getZCodeSessionMonitorSnapshot,
}

const deleteSessionApi: Record<MonitorAgent, (sessionId: string) => Promise<void>> = {
  codex: api.deleteCodexSessionMonitorSession,
  claude: api.deleteClaudeSessionMonitorSession,
  cursor: api.deleteCursorSessionMonitorSession,
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
    grok: emptySnapshot(),
    kimi: emptySnapshot(),
    zcode: emptySnapshot(),
  })
  const hookStatuses = ref<Record<HookAgent, HookStatus | null>>({
    codex: null,
    claude: null,
    cursor: null,
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
  let initialized = false
  let unlisten: (() => void)[] = []

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

  async function refresh() {
    loading.value = true
    error.value = ''
    const messages: string[] = []
    const snapshotResults = await Promise.allSettled(
      MONITOR_AGENTS.map(agent => snapshotApi[agent]()),
    )
    MONITOR_AGENTS.forEach((agent, index) => {
      const result = snapshotResults[index]
      if (result.status === 'fulfilled') {
        snapshots.value[agent] = result.value || emptySnapshot()
      } else {
        messages.push(errorMessage(result.reason))
      }
    })
    const hookResults = await Promise.allSettled(
      HOOK_AGENTS.map(agent => hookApi[agent].status()),
    )
    HOOK_AGENTS.forEach((agent, index) => {
      const result = hookResults[index]
      if (result.status === 'fulfilled') {
        hookStatuses.value[agent] = result.value
      } else {
        messages.push(errorMessage(result.reason))
      }
    })
    error.value = messages.filter(Boolean).join('；')
    loading.value = false
  }

  async function initialize() {
    if (!initialized) {
      initialized = true
      const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__
      if (isTauri) {
        const { listen } = await import('@tauri-apps/api/event')
        unlisten = await Promise.all(
          MONITOR_AGENTS.map(agent =>
            listen<MonitorSnapshot>(CHANGED_EVENTS[agent], event => {
              snapshots.value[agent] = event.payload
            }),
          ),
        )
      }
    }
    await refresh()
  }

  function dispose() {
    unlisten.forEach(fn => fn())
    unlisten = []
    initialized = false
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
