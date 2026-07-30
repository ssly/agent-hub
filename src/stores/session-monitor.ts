import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as api from '@/lib/api'

export type HookAction = 'install' | 'uninstall'
export type SessionSource = 'terminal' | 'chatgpt'
export type RuntimeStatus = 'running' | 'ended'
export type MonitorAgent = 'codex' | 'claude' | 'kiro'
export const MONITOR_AGENTS: MonitorAgent[] = ['codex', 'claude', 'kiro']
/** Sidebar tab: one of the agents, or the merged "all" view. */
export type MonitorTab = MonitorAgent | 'all'
/** Monitor agent → sessions-browser platform id, so the shared messages /
 *  resume modals can load full history through the sessions adapters. */
export const MONITOR_AGENT_PLATFORM: Record<MonitorAgent, string> = {
  codex: 'codex',
  claude: 'claude-code',
  kiro: 'kiro',
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

export interface KiroMonitorStatus {
  available: boolean
  sessionsDir: string
  enabled: boolean
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
  kiro: 'session-monitor:kiro-changed',
}

const snapshotApi: Record<MonitorAgent, () => Promise<MonitorSnapshot>> = {
  codex: api.getCodexSessionMonitorSnapshot,
  claude: api.getClaudeSessionMonitorSnapshot,
  kiro: api.getKiroSessionMonitorSnapshot,
}

const deleteSessionApi: Record<MonitorAgent, (sessionId: string) => Promise<void>> = {
  codex: api.deleteCodexSessionMonitorSession,
  claude: api.deleteClaudeSessionMonitorSession,
  kiro: api.deleteKiroSessionMonitorSession,
}

const hookApi: Record<'codex' | 'claude', {
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
    kiro: emptySnapshot(),
  })
  const hookStatuses = ref<Record<'codex' | 'claude', HookStatus | null>>({
    codex: null,
    claude: null,
  })
  const kiroStatus = ref<KiroMonitorStatus | null>(null)
  const loading = ref(false)
  const hookLoading = ref(false)
  const error = ref('')
  const previewOpen = ref(false)
  const previewLoading = ref(false)
  const previewAgent = ref<'codex' | 'claude'>('codex')
  const preview = ref<HookChangePreview | null>(null)
  const previewError = ref('')
  let initialized = false
  let unlisten: (() => void)[] = []

  const snapshot = computed(() =>
    activeAgent.value === 'all' ? emptySnapshot() : snapshots.value[activeAgent.value],
  )
  const hookStatus = computed(() =>
    activeAgent.value === 'codex' || activeAgent.value === 'claude'
      ? hookStatuses.value[activeAgent.value]
      : null,
  )
  /** Rows for the current tab. "all" merges every agent: running sessions
   *  first, then newest activity first within each status group. */
  const displaySessions = computed<AgentSessionState[]>(() => {
    if (activeAgent.value === 'all') {
      return MONITOR_AGENTS.flatMap(agent =>
        snapshots.value[agent].sessions.map(session => ({ ...session, agent })),
      ).sort((a, b) => {
        if (a.status !== b.status) return a.status === 'running' ? -1 : 1
        return b.updatedAt - a.updatedAt
      })
    }
    const agent = activeAgent.value
    return snapshots.value[agent].sessions.map(session => ({ ...session, agent }))
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
    const hookResults = await Promise.allSettled([
      hookApi.codex.status(),
      hookApi.claude.status(),
    ])
    const [codexHook, claudeHook] = hookResults
    if (codexHook.status === 'fulfilled') {
      hookStatuses.value.codex = codexHook.value
    } else {
      messages.push(errorMessage(codexHook.reason))
    }
    if (claudeHook.status === 'fulfilled') {
      hookStatuses.value.claude = claudeHook.value
    } else {
      messages.push(errorMessage(claudeHook.reason))
    }
    try {
      kiroStatus.value = await api.getKiroMonitorStatus()
    } catch (cause) {
      messages.push(errorMessage(cause))
    }
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

  async function openHookPreview(agent: 'codex' | 'claude', action: HookAction) {
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

  async function setKiroEnabled(enabled: boolean) {
    try {
      kiroStatus.value = await api.setKiroMonitorEnabled(enabled)
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
    kiroStatus,
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
    setKiroEnabled,
    messagesModalOpen,
    resumeModalOpen,
    modalSession,
    resumeSession,
    openMessages,
    openResume,
  }
})
