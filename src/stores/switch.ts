import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as api from '@/lib/api'
import type { ClaudeUsage, CodexUsage, CodexResetCredits, DeepSeekSettings, DeepSeekUsage, KiroUsage, UsageMonitorSettings } from '@/lib/api'

export const VALID_SWITCH_AGENTS = ['codex', 'claude-code', 'kiro', 'deepseek']

export const useSwitchStore = defineStore('switch', () => {
  const initialAgent = localStorage.getItem('ah-switch-agent')
  const selectedAgent = ref<string | null>(
    initialAgent && VALID_SWITCH_AGENTS.includes(initialAgent) ? initialAgent : 'codex',
  )
  const profiles = ref<any[]>([])
  const currentKey = ref<string | null>(null)
  const addFormOpen = ref(false)
  const switchConfirmId = ref<string | null>(null)

  // Codex is read-only: display the current CLI login and its usage without
  // reading from or mutating Agent Hub's legacy profile pool.
  // Window presence/order varies; views label each item from window_seconds.
  // The Accounts view and tray popup both consume the same snapshot command.
  const codexUsage = ref<CodexUsage | null>(null)
  const codexUsageLoading = ref(false)
  const codexUsageError = ref<string | null>(null)
  const codexUsageLastQuery = ref<number>(0)

  // Codex rate-limit reset credits + their validity period. Fetched in the
  // same snapshot as usage; failure here must NOT blank out the usage data.
  const codexResetCredits = ref<CodexResetCredits | null>(null)

  // Claude Code official-login (OAuth subscription) usage. Independent of the
  // switchable custom-token pool: this always reflects the official /login
  // account whose credentials live in the keychain/credentials file.
  const claudeUsage = ref<ClaudeUsage | null>(null)
  const claudeUsageLoading = ref(false)
  const claudeUsageError = ref<string | null>(null)
  const claudeUsageLastQuery = ref<number>(0)
  /** Local OAuth credential presence; null until the first check runs. */
  const claudeUsageAvailable = ref<boolean | null>(null)

  // DeepSeek: the key is auto-read from DeepSeek Harness's own credential
  // layering (env → ~/.dsh/.credentials.yaml → ~/.dsh/.env); we only query
  // the official /user/balance endpoint with it. Read-only.
  const deepseekSettings = ref<DeepSeekSettings | null>(null)
  const deepseekUsage = ref<DeepSeekUsage | null>(null)
  const deepseekUsageLoading = ref(false)
  const deepseekUsageError = ref<string | null>(null)
  const deepseekUsageLastQuery = ref<number>(0)

  // Kiro / Kiro CLI usage and plan credit limits.
  const kiroUsage = ref<KiroUsage | null>(null)
  const kiroUsageLoading = ref(false)
  const kiroUsageError = ref<string | null>(null)
  const kiroUsageLastQuery = ref<number>(0)

  // Edit modal state
  const editModalOpen = ref(false)
  const editingProfileId = ref<string | null>(null)
  const editNote = ref('')
  const editContent = ref('')
  const editContentLoading = ref(false)
  const editSaving = ref(false)
  const deleteArmed = ref(false)

  // "Clear active account" confirmation modal (deletes the live auth file,
  // e.g. ~/.codex/auth.json, but never the account pool).
  const clearActiveModalOpen = ref(false)
  const clearActiveLoading = ref(false)

  async function selectAgent(agentType: string) {
    selectedAgent.value = agentType
    localStorage.setItem('ah-switch-agent', agentType)
    // Share the selection with the tray popup (backend memory + event).
    void api.setUsageSelectedAgent(agentType).then(s => { monitorSettings.value = s }).catch(() => {})
    addFormOpen.value = false
    switchConfirmId.value = null
    editModalOpen.value = false
    editingProfileId.value = null
    editNote.value = ''
    editContent.value = ''
    editContentLoading.value = false
    editSaving.value = false
    deleteArmed.value = false
    clearActiveModalOpen.value = false
    await loadSelectedAgent()
  }

  // --- Shared usage-monitor settings (backend file + event, synced with
  // the tray popup). Absent listening key = paused; user must turn it on. --
  const monitorSettings = ref<UsageMonitorSettings | null>(null)
  const refreshMinutes = computed(() => monitorSettings.value?.refreshMinutes ?? 5)
  const monitorLimit = computed(() => monitorSettings.value?.monitorLimit ?? 6)
  /** Absent key = paused (default off). */
  function isAgentListened(agent: string) {
    return monitorSettings.value?.listening?.[agent] ?? false
  }
  async function loadMonitorSettings() {
    try {
      monitorSettings.value = await api.getUsageMonitorSettings()
    } catch { /* keep defaults */ }
  }
  async function updateRefreshMinutes(minutes: number) {
    monitorSettings.value = await api.setUsageRefreshMinutes(minutes)
  }
  async function updateMonitorLimit(limit: number) {
    monitorSettings.value = await api.setUsageMonitorLimit(limit)
  }
  async function setAgentListening(agent: string, enabled: boolean) {
    monitorSettings.value = await api.setUsageAgentListening(agent, enabled)
  }

  // Shared with the tray popup via backend 10-minute cache.
  // Pass force=true only for the explicit Refresh button.
  async function refreshCodexUsage(force = false) {
    if (selectedAgent.value !== 'codex' || codexUsageLoading.value) return
    codexUsageLoading.value = true
    codexUsageError.value = null
    try {
      const snapshot = await api.getCodexTrayUsage(force)
      codexUsage.value = snapshot.usage
      codexResetCredits.value = snapshot.reset_credits
      codexUsageLastQuery.value = snapshot.last_query_at * 1000
    } catch (reason: any) {
      codexUsageError.value = String(reason?.message || reason)
      codexUsage.value = null
      codexResetCredits.value = null
    } finally {
      codexUsageLoading.value = false
    }
  }


  async function refreshClaudeUsage(force = false) {
    if (selectedAgent.value !== 'claude-code' || claudeUsageLoading.value) return
    claudeUsageLoading.value = true
    claudeUsageError.value = null
    try {
      // Local credential check first (no network): without OAuth credentials
      // the query would only fail, so skip it and let the panel show a quiet
      // sign-in hint instead of an error banner.
      const availability = await api.getUsageProviderAvailability()
      claudeUsageAvailable.value = availability.claude_code
      if (!availability.claude_code) {
        claudeUsage.value = null
        claudeUsageLastQuery.value = 0
        return
      }
      claudeUsage.value = await api.getClaudeUsage(force)
      claudeUsageLastQuery.value = (claudeUsage.value.fetched_at || Math.floor(Date.now() / 1000)) * 1000
    } catch (reason: any) {
      claudeUsageError.value = String(reason?.message || reason)
      claudeUsage.value = null
      claudeUsageLastQuery.value = 0
    } finally {
      claudeUsageLoading.value = false
    }
  }

  async function loadDeepseekSettings() {
    try {
      deepseekSettings.value = await api.getDeepseekSettings()
    } catch { /* keep previous */ }
  }

  async function refreshDeepseekUsage(force = false) {
    if (selectedAgent.value !== 'deepseek' || deepseekUsageLoading.value) return
    if (!deepseekSettings.value) await loadDeepseekSettings()
    if (!deepseekSettings.value?.has_key) return
    deepseekUsageLoading.value = true
    deepseekUsageError.value = null
    try {
      deepseekUsage.value = await api.getDeepseekUsage(force)
      deepseekUsageLastQuery.value = (deepseekUsage.value.fetched_at || Math.floor(Date.now() / 1000)) * 1000
    } catch (reason: any) {
      deepseekUsageError.value = String(reason?.message || reason)
      deepseekUsage.value = null
      deepseekUsageLastQuery.value = 0
    } finally {
      deepseekUsageLoading.value = false
    }
  }

  async function refreshKiroUsage(force = false) {
    if (selectedAgent.value !== 'kiro' || kiroUsageLoading.value) return
    kiroUsageLoading.value = true
    kiroUsageError.value = null
    try {
      kiroUsage.value = await api.getKiroUsage(force)
      kiroUsageLastQuery.value = (kiroUsage.value.fetched_at || Math.floor(Date.now() / 1000)) * 1000
    } catch (reason: any) {
      kiroUsageError.value = String(reason?.message || reason)
      kiroUsage.value = null
      kiroUsageLastQuery.value = 0
    } finally {
      kiroUsageLoading.value = false
    }
  }

  // The tray popup emits `usage-refreshed` after a successful query; re-pull
  // the shared backend cache (force=false, already fresh) so an open Accounts
  // view shows the same numbers. Refreshers no-op for non-selected agents.
  let usageListenerReady = false
  async function ensureUsageListener() {
    if (usageListenerReady || import.meta.env.MODE === 'web') return
    usageListenerReady = true
    try {
      const { listen } = await import('@tauri-apps/api/event')
      await listen<{ provider: string }>('usage-refreshed', event => {
        if (event.payload.provider === 'codex') void refreshCodexUsage(false)
        if (event.payload.provider === 'claude-code') void refreshClaudeUsage(false)
        if (event.payload.provider === 'kiro') void refreshKiroUsage(false)
      })
      // Tray-side settings changes (interval slider, provider tabs) land here.
      await listen<UsageMonitorSettings>('usage-monitor-settings-changed', event => {
        monitorSettings.value = event.payload
        const shared = event.payload.selectedAgent
        if (shared && shared !== selectedAgent.value) void selectAgent(shared)
      })
      monitorSettings.value ??= await api.getUsageMonitorSettings()
    } catch {
      usageListenerReady = false
    }
  }

  async function loadProfiles() {
    if (!selectedAgent.value) return
    // Codex, Kiro, and DeepSeek are read-only: one current
    // CLI account (or a single locally-stored API key), no profile pool.
    if (
      selectedAgent.value === 'codex'
      || selectedAgent.value === 'kiro'
      || selectedAgent.value === 'deepseek'
    ) {
      profiles.value = []
      currentKey.value = null
      return
    }
    try {
      const resp = await api.listSwitchProfiles(selectedAgent.value)
      profiles.value = resp.profiles || []
      currentKey.value = resp.current_key || null
    } catch {
      profiles.value = []
      currentKey.value = null
    }
  }

  // Entering the selected account section loads the shared usage snapshot
  // (backend serves cache when younger than 10 minutes) or Claude profiles.
  // A paused agent (listening off) is never auto-queried; the manual refresh
  // buttons stay available.
  async function loadSelectedAgent() {
    void ensureUsageListener()
    if (!monitorSettings.value) await loadMonitorSettings()
    await loadProfiles()
    const agent = selectedAgent.value
    if (!agent || !isAgentListened(agent)) return
    if (agent === 'codex') await refreshCodexUsage(false)
    if (agent === 'claude-code') await refreshClaudeUsage(false)
    if (agent === 'kiro') await refreshKiroUsage(false)
    if (agent === 'deepseek') {
      await loadDeepseekSettings()
      await refreshDeepseekUsage(false)
    }
  }

  async function openEditModal(profile: any) {
    if (!selectedAgent.value) return
    editingProfileId.value = profile.id
    editNote.value = profile.note || ''
    editContent.value = ''
    editContentLoading.value = true
    deleteArmed.value = false
    editModalOpen.value = true
    try {
      editContent.value = await api.getAuthProfileContent(selectedAgent.value, profile.id)
    } catch (e) {
      editContent.value = ''
      throw e // let the component surface a toast
    } finally {
      editContentLoading.value = false
    }
  }

  function closeEditModal() {
    editModalOpen.value = false
    editingProfileId.value = null
    deleteArmed.value = false
  }

  function openClearActiveModal() {
    clearActiveModalOpen.value = true
  }
  function closeClearActiveModal() {
    if (clearActiveLoading.value) return
    clearActiveModalOpen.value = false
  }
  // Delete the live auth file (e.g. ~/.codex/auth.json) without backing it up.
  // The account pool is never touched. Returns null on success, or an error
  // string on failure so the component can surface a toast (the store stays
  // free of i18n deps).
  async function deleteActiveAuth(): Promise<string | null> {
    if (!selectedAgent.value || clearActiveLoading.value) return null
    clearActiveLoading.value = true
    try {
      await api.deleteActiveAuth(selectedAgent.value)
      await loadProfiles()
      if (selectedAgent.value === 'codex') {
        codexUsage.value = null
        codexResetCredits.value = null
        codexUsageLastQuery.value = 0
      }
      clearActiveModalOpen.value = false
      return null
    } catch (e: any) {
      return String(e?.message || e)
    } finally {
      clearActiveLoading.value = false
    }
  }

  function resetState() {
    addFormOpen.value = false
    switchConfirmId.value = null
    editModalOpen.value = false
    editingProfileId.value = null
    editNote.value = ''
    editContent.value = ''
    editContentLoading.value = false
    editSaving.value = false
    deleteArmed.value = false
    clearActiveModalOpen.value = false
    clearActiveLoading.value = false
  }

  return {
    selectedAgent, profiles, currentKey, addFormOpen, switchConfirmId,
    editModalOpen, editingProfileId, editNote, editContent, editContentLoading, editSaving, deleteArmed,
    clearActiveModalOpen, clearActiveLoading,
    codexUsage, codexUsageLoading, codexUsageError, codexUsageLastQuery, codexResetCredits,
    claudeUsage, claudeUsageLoading, claudeUsageError, claudeUsageLastQuery, claudeUsageAvailable,
    deepseekSettings, deepseekUsage, deepseekUsageLoading, deepseekUsageError, deepseekUsageLastQuery,
    kiroUsage, kiroUsageLoading, kiroUsageError, kiroUsageLastQuery,
    monitorSettings, refreshMinutes, monitorLimit, isAgentListened,
    loadMonitorSettings, updateRefreshMinutes, updateMonitorLimit, setAgentListening,
    selectAgent, loadProfiles, loadSelectedAgent, openEditModal, closeEditModal, resetState,
    refreshCodexUsage, refreshClaudeUsage, refreshKiroUsage,
    loadDeepseekSettings, refreshDeepseekUsage,
    openClearActiveModal, closeClearActiveModal, deleteActiveAuth,
  }
})
