import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'

export type HookAction = 'install' | 'uninstall'
export type SessionSource = 'terminal' | 'chatgpt'
export type RuntimeStatus = 'running' | 'ended'

export interface CodexSessionState {
  sessionId: string
  turnId: string
  source: SessionSource
  status: RuntimeStatus
  cwd?: string | null
  userPrompt?: string | null
  assistantReply?: string | null
  updatedAt: number
}

export interface CodexMonitorSnapshot {
  revision: number
  sessions: CodexSessionState[]
}

export interface CodexHookStatus {
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

export interface CodexHookChangePreview {
  action: HookAction
  configPath: string
  command: string
  beforeHash: string
  diffLines: HookDiffLine[]
  added: number
  removed: number
  changed: boolean
}

function errorMessage(error: any): string {
  return error?.General || error?.message || String(error)
}

export const useSessionMonitorStore = defineStore('session-monitor', () => {
  const snapshot = ref<CodexMonitorSnapshot>({ revision: 0, sessions: [] })
  const hookStatus = ref<CodexHookStatus | null>(null)
  const loading = ref(false)
  const hookLoading = ref(false)
  const error = ref('')
  const previewOpen = ref(false)
  const previewLoading = ref(false)
  const preview = ref<CodexHookChangePreview | null>(null)
  const previewError = ref('')
  let initialized = false
  let unlisten: (() => void) | undefined

  async function refresh() {
    loading.value = true
    error.value = ''
    const [snapshotResult, hookResult] = await Promise.allSettled([
      api.getCodexSessionMonitorSnapshot(),
      api.getCodexHookStatus(),
    ])
    if (snapshotResult.status === 'fulfilled') {
      snapshot.value = snapshotResult.value || { revision: 0, sessions: [] }
    } else {
      error.value = errorMessage(snapshotResult.reason)
    }
    if (hookResult.status === 'fulfilled') {
      hookStatus.value = hookResult.value
    } else {
      error.value = [error.value, errorMessage(hookResult.reason)].filter(Boolean).join('；')
    }
    loading.value = false
  }

  async function initialize() {
    if (!initialized) {
      initialized = true
      const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__
      if (isTauri) {
        const { listen } = await import('@tauri-apps/api/event')
        unlisten = await listen<CodexMonitorSnapshot>('session-monitor:codex-changed', event => {
          snapshot.value = event.payload
        })
      }
    }
    await refresh()
  }

  function dispose() {
    unlisten?.()
    unlisten = undefined
    initialized = false
  }

  async function openHookPreview(action: HookAction) {
    previewOpen.value = true
    previewLoading.value = true
    previewError.value = ''
    preview.value = null
    try {
      preview.value = await api.previewCodexHookChange(action)
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
      hookStatus.value = await api.applyCodexHookChange(
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

  return {
    snapshot,
    hookStatus,
    loading,
    hookLoading,
    error,
    previewOpen,
    previewLoading,
    preview,
    previewError,
    initialize,
    refresh,
    dispose,
    openHookPreview,
    closeHookPreview,
    applyHookPreview,
  }
})
