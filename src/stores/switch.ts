import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'
import type { CodexUsage, CodexResetCredits, GrokUsage, KimiUsage } from '@/lib/api'

export const useSwitchStore = defineStore('switch', () => {
  const selectedAgent = ref<string | null>(localStorage.getItem('ah-switch-agent'))
  const profiles = ref<any[]>([])
  const currentKey = ref<string | null>(null)
  const addFormOpen = ref(false)
  const switchConfirmId = ref<string | null>(null)

  // Codex usage windows for the currently active account.
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

  // Every invocation performs a fresh query. This is used when entering Codex,
  // after switching accounts, and when the user presses Refresh.
  async function refreshCodexUsage() {
    if (selectedAgent.value !== 'codex' || codexUsageLoading.value) return
    codexUsageLoading.value = true
    codexUsageError.value = null
    try {
      const snapshot = await api.getCodexTrayUsage()
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

  async function refreshGrokUsage() {
    if (selectedAgent.value !== 'grok-build' || grokUsageLoading.value) return
    grokUsageLoading.value = true
    grokUsageError.value = null
    try {
      grokUsage.value = await api.getGrokUsage()
      grokUsageLastQuery.value = (grokUsage.value.fetched_at || Math.floor(Date.now() / 1000)) * 1000
    } catch (reason: any) {
      grokUsageError.value = String(reason?.message || reason)
      grokUsage.value = null
      grokUsageLastQuery.value = 0
    } finally {
      grokUsageLoading.value = false
    }
  }

  async function refreshKimiUsage() {
    if (selectedAgent.value !== 'kimi-code' || kimiUsageLoading.value) return
    kimiUsageLoading.value = true
    kimiUsageError.value = null
    try {
      kimiUsage.value = await api.getKimiUsage()
      kimiUsageLastQuery.value = (kimiUsage.value.fetched_at || Math.floor(Date.now() / 1000)) * 1000
    } catch (reason: any) {
      kimiUsageError.value = String(reason?.message || reason)
      kimiUsage.value = null
      kimiUsageLastQuery.value = 0
    } finally {
      kimiUsageLoading.value = false
    }
  }

  async function loadProfiles() {
    if (!selectedAgent.value) return
    // Grok Build and Kimi Code are read-only: one current CLI account, no pool.
    if (selectedAgent.value === 'grok-build' || selectedAgent.value === 'kimi-code') {
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

  // Entering the selected account section always reloads profile state. Codex
  // additionally performs a fresh quota query with no time-based cooldown.
  async function loadSelectedAgent() {
    await loadProfiles()
    if (selectedAgent.value === 'codex') await refreshCodexUsage()
    if (selectedAgent.value === 'grok-build') await refreshGrokUsage()
    if (selectedAgent.value === 'kimi-code') await refreshKimiUsage()
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
    selectAgent, loadProfiles, loadSelectedAgent, openEditModal, closeEditModal, resetState,
    refreshCodexUsage, refreshGrokUsage, refreshKimiUsage,
    openClearActiveModal, closeClearActiveModal, deleteActiveAuth,
  }
})
