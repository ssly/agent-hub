import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'

export const useSwitchStore = defineStore('switch', () => {
  const selectedAgent = ref<string | null>(null)
  const profiles = ref<any[]>([])
  const currentKey = ref<string | null>(null)
  const addFormOpen = ref(false)
  const clearConfirmOpen = ref(false)
  const editingNoteId = ref<string | null>(null)
  const editingContentId = ref<string | null>(null)
  const contentCache = ref<Record<string, string>>({})
  const deleteConfirmId = ref<string | null>(null)
  const switchConfirmId = ref<string | null>(null)

  async function selectAgent(agentType: string) {
    selectedAgent.value = agentType
    addFormOpen.value = false
    clearConfirmOpen.value = false
    editingNoteId.value = null
    editingContentId.value = null
    contentCache.value = {}
    deleteConfirmId.value = null
    switchConfirmId.value = null
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

  function resetState() {
    addFormOpen.value = false
    clearConfirmOpen.value = false
    editingNoteId.value = null
    editingContentId.value = null
    deleteConfirmId.value = null
    switchConfirmId.value = null
  }

  return {
    selectedAgent, profiles, currentKey, addFormOpen, clearConfirmOpen,
    editingNoteId, editingContentId, contentCache, deleteConfirmId, switchConfirmId,
    selectAgent, loadProfiles, resetState,
  }
})
