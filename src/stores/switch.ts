import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'
import type { CodexUsage, CodexResetCredits } from '@/lib/api'

const QUOTA_COOLDOWN_MS = 60 * 1000 // 1 min — prevent rapid re-query on click spam

export const useSwitchStore = defineStore('switch', () => {
  const selectedAgent = ref<string | null>(localStorage.getItem('ah-switch-agent'))
  const profiles = ref<any[]>([])
  const currentKey = ref<string | null>(null)
  const addFormOpen = ref(false)
  const switchConfirmId = ref<string | null>(null)

  // Codex usage windows for the currently active account.
  // Window presence/order varies; views label each item from window_seconds.
  // Usage is NEVER auto-fetched — the user must click "Refresh" explicitly.
  // Within the cooldown window (1 min), a manual refresh short-circuits and
  // returns the cached payload instead of hitting the API again.
  const codexUsage = ref<CodexUsage | null>(null)
  const codexUsageLoading = ref(false)
  const codexUsageError = ref<string | null>(null)
  const codexUsageLastQuery = ref<number>(0)

  // Codex rate-limit reset credits + their validity period. Fetched alongside
  // usage on a manual refresh; failure here must NOT blank out the usage data.
  const codexResetCredits = ref<CodexResetCredits | null>(null)

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
    // Codex usage is intentionally NOT auto-fetched here. The user must
    // click "Refresh" on the usage panel to trigger a query.
  }

  // Whether the cached usage is still within the cooldown window.
  function codexUsageInCooldown(): boolean {
    if (!codexUsageLastQuery.value) return false
    return Date.now() - codexUsageLastQuery.value < QUOTA_COOLDOWN_MS
  }

  // Manually refresh Codex usage. Behavior:
  //   - Never auto-triggered; the component calls this on button click.
  //   - If a fresh payload exists within the cooldown window (1 min), we skip
  //     the API call and just reuse the cached value (component shows a toast).
  //   - `force` bypasses the cooldown.
  // Fetches usage + reset-credits in parallel. Reset-credits failure is
  // non-fatal (we keep usage data and just null out the credits).
  async function refreshCodexUsage(force = false) {
    if (selectedAgent.value !== 'codex' || codexUsageLoading.value) return
    if (!force && codexUsage.value && codexUsageInCooldown()) return
    codexUsageLoading.value = true
    codexUsageError.value = null
    try {
      const [usageRes, creditsRes] = await Promise.allSettled([
        api.getCodexUsage(),
        api.getCodexResetCredits(),
      ])
      if (usageRes.status === 'fulfilled') {
        codexUsage.value = usageRes.value
        codexUsageLastQuery.value = Date.now()
      } else {
        // Usage is the primary payload — if it fails, surface the error.
        codexUsageError.value = String((usageRes.reason as any)?.message || usageRes.reason)
        codexUsage.value = null
      }
      codexResetCredits.value =
        creditsRes.status === 'fulfilled' ? creditsRes.value : null
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
    codexUsage, codexUsageLoading, codexUsageError, codexUsageLastQuery, codexResetCredits,
    selectAgent, loadProfiles, openEditModal, closeEditModal, resetState,
    codexUsageInCooldown, refreshCodexUsage,
    openClearActiveModal, closeClearActiveModal, deleteActiveAuth,
  }
})
