import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as api from '@/lib/api'
import type { ClaudeUsage, CodexUsage, CodexResetCredits, DeepSeekSettings, DeepSeekUsage, GrokUsage, KimiUsage, UsageMonitorSettings } from '@/lib/api'

export const useSwitchStore = defineStore('switch', () => {
  const selectedAgent = ref<string | null>(localStorage.getItem('ah-switch-agent'))
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

  // Grok Build is deliberately read-only: one current CLI account and its
  // billing snapshot, with no profile pool or credential mutations.
  const grokUsage = ref<GrokUsage | null>(null)
  const grokUsageLoading = ref(false)
  const grokUsageError = ref<string | null>(null)
  const grokUsageLastQuery = ref<number>(0)

  // Kimi Code follows the same read-only model as Grok Build: we read the
  // CLI's current OAuth login from the keychain/credential file and never
  // switch accounts from Agent Hub.
  const kimiUsage = ref<KimiUsage | null>(null)
  const kimiUsageLoading = ref(false)
  const kimiUsageError = ref<string | null>(null)
  const kimiUsageLastQuery = ref<number>(0)

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

  async function refreshGrokUsage(force = false) {
    if (selectedAgent.value !== 'grok-build' || grokUsageLoading.value) return
    grokUsageLoading.value = true
    grokUsageError.value = null
    try {
      const next = await api.getGrokUsage(force)
      grokUsage.value = next
      grokUsageLastQuery.value = (next.fetched_at || Math.floor(Date.now() / 1000)) * 1000
    } catch (reason: any) {
      // Keep the last successful numbers on transport/parse failure; only
      // surface the error banner when there is nothing left to show.
      grokUsageError.value = String(reason?.message || reason)
      if (!grokUsage.value) grokUsageLastQuery.value = 0
    } finally {
      grokUsageLoading.value = false
    }
  }

  async function refreshKimiUsage(force = false) {
    if (selectedAgent.value !== 'kimi-code' || kimiUsageLoading.value) return
    kimiUsageLoading.value = true
    kimiUsageError.value = null
    try {
      kimiUsage.value = await api.getKimiUsage(force)
      kimiUsageLastQuery.value = (kimiUsage.value.fetched_at || Math.floor(Date.now() / 1000)) * 1000
    } catch (reason: any) {
      kimiUsageError.value = String(reason?.message || reason)
      kimiUsage.value = null
      kimiUsageLastQuery.value = 0
    } finally {
      kimiUsageLoading.value = false
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
        if (event.payload.provider === 'grok-build') void refreshGrokUsage(false)
        if (event.payload.provider === 'kimi-code') void refreshKimiUsage(false)
        if (event.payload.provider === 'claude-code') void refreshClaudeUsage(false)
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
    // Codex, Grok Build, Kimi Code, and DeepSeek are read-only: one current
    // CLI account (or a single locally-stored API key), no profile pool.
    if (
      selectedAgent.value === 'codex'
      || selectedAgent.value === 'grok-build'
      || selectedAgent.value === 'kimi-code'
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
    if (agent === 'grok-build') await refreshGrokUsage(false)
    if (agent === 'kimi-code') await refreshKimiUsage(false)
    if (agent === 'claude-code') await refreshClaudeUsage(false)
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
    grokUsage, grokUsageLoading, grokUsageError, grokUsageLastQuery,
    kimiUsage, kimiUsageLoading, kimiUsageError, kimiUsageLastQuery,
    claudeUsage, claudeUsageLoading, claudeUsageError, claudeUsageLastQuery, claudeUsageAvailable,
    deepseekSettings, deepseekUsage, deepseekUsageLoading, deepseekUsageError, deepseekUsageLastQuery,
    monitorSettings, refreshMinutes, isAgentListened,
    loadMonitorSettings, updateRefreshMinutes, setAgentListening,
    selectAgent, loadProfiles, loadSelectedAgent, openEditModal, closeEditModal, resetState,
    refreshCodexUsage, refreshGrokUsage, refreshKimiUsage, refreshClaudeUsage,
    loadDeepseekSettings, refreshDeepseekUsage,
    openClearActiveModal, closeClearActiveModal, deleteActiveAuth,
  }
})
