import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'
import type { CodexUsage } from '@/lib/api'

const QUOTA_COOLDOWN_MS = 60 * 60 * 1000 // 1h
const QUOTA_LS_KEY = 'codex_quota_last_query'

export const useSwitchStore = defineStore('switch', () => {
  const selectedAgent = ref<string | null>(localStorage.getItem('ah-switch-agent'))
  const profiles = ref<any[]>([])
  const currentKey = ref<string | null>(null)
  const addFormOpen = ref(false)
  const switchConfirmId = ref<string | null>(null)

  // Codex usage windows for the currently active account.
  // Plus/Pro: primary (5h) + secondary (7d). Free: primary (monthly) only.
  const codexUsage = ref<CodexUsage | null>(null)
  const codexUsageLoading = ref(false)
  const codexUsageError = ref<string | null>(null)
  const codexUsageLastQuery = ref<number>(parseInt(localStorage.getItem(QUOTA_LS_KEY) || '0', 10))

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
    await loadProfiles()
    // Auto-refresh Codex usage when entering the Codex view (1h cooldown)
    if (agentType === 'codex') ensureFreshCodexUsage()
  }

  // Fetch Codex usage only if stale (older than the cooldown) or never fetched.
  async function ensureFreshCodexUsage() {
    if (codexUsageLoading.value) return
    const stale = !codexUsage.value || (Date.now() - codexUsageLastQuery.value > QUOTA_COOLDOWN_MS)
    if (stale) refreshCodexUsage(false)
  }

  async function refreshCodexUsage(showToast = true) {
    if (selectedAgent.value !== 'codex') return
    codexUsageLoading.value = true
    codexUsageError.value = null
    try {
      const data = await api.getCodexUsage()
      codexUsage.value = data
      codexUsageLastQuery.value = Date.now()
      localStorage.setItem(QUOTA_LS_KEY, String(codexUsageLastQuery.value))
      if (showToast) {
        // surfaced by component to keep store free of i18n deps
      }
    } catch (e: any) {
      codexUsageError.value = String(e?.message || e)
      codexUsage.value = null
    } finally {
      codexUsageLoading.value = false
    }
  }

  async function loadProfiles() {
    if (!selectedAgent.value) return
    try {
      const resp = await api.listSwitchProfiles(selectedAgent.value)
      profiles.value = resp.profiles || []
      currentKey.value = resp.current_key || null
    } catch {
      profiles.value = []
      currentKey.value = null
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
    codexUsage, codexUsageLoading, codexUsageError, codexUsageLastQuery,
    selectAgent, loadProfiles, openEditModal, closeEditModal, resetState,
    ensureFreshCodexUsage, refreshCodexUsage,
    openClearActiveModal, closeClearActiveModal, deleteActiveAuth,
  }
})
