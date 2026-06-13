import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'

export const useSwitchStore = defineStore('switch', () => {
  const selectedAgent = ref<string | null>(localStorage.getItem('ah-switch-agent'))
  const profiles = ref<any[]>([])
  const currentKey = ref<string | null>(null)
  const addFormOpen = ref(false)
  const switchConfirmId = ref<string | null>(null)

  // Edit modal state
  const editModalOpen = ref(false)
  const editingProfileId = ref<string | null>(null)
  const editNote = ref('')
  const editContent = ref('')
  const editContentLoading = ref(false)
  const editSaving = ref(false)
  const deleteArmed = ref(false)

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
    await loadProfiles()
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
  }

  return {
    selectedAgent, profiles, currentKey, addFormOpen, switchConfirmId,
    editModalOpen, editingProfileId, editNote, editContent, editContentLoading, editSaving, deleteArmed,
    selectAgent, loadProfiles, openEditModal, closeEditModal, resetState,
  }
})
