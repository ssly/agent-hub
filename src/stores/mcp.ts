import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'

export const useMcpStore = defineStore('mcp', () => {
  const platforms = ref<any[]>([])
  const servers = ref<any[]>([])
  const selectedPlatformId = ref<string | null>(null)
  const expandedServer = ref<string | null>(null)
  const serverDetails = ref<Record<string, { config_text: string; format: string }>>({})

  // Modal States
  const addModalOpen = ref(false)
  const syncModalOpen = ref(false)
  const syncTargets = ref<any[]>([])
  const syncTargetPlatformId = ref<string | null>(null)
  const syncServerName = ref<string | null>(null)
  const deleteConfirmServerName = ref<string | null>(null)

  // Preview Modal States (for add / delete diff confirmation)
  const previewModalOpen = ref(false)
  const previewLoading = ref(false)
  const previewData = ref<any>(null)
  const previewMode = ref<'add' | 'delete'>('add')
  // Stashed add inputs for cancel-back
  const previewAddName = ref('')
  const previewAddConfig = ref('')

  async function refreshPlatforms() {
    platforms.value = await api.listMcpPlatforms()
    const exists = platforms.value.some(p => p.id === selectedPlatformId.value)
    if (!exists && platforms.value.length > 0) {
      await selectPlatform(platforms.value[0].id)
    }
  }

  async function selectPlatform(id: string) {
    selectedPlatformId.value = id
    expandedServer.value = null
    serverDetails.value = {}
    try {
      servers.value = await api.getMcpServers(id)
    } catch {
      servers.value = []
    }
  }

  async function toggleServer(name: string) {
    if (expandedServer.value === name) {
      expandedServer.value = null
      return
    }
    expandedServer.value = name
    if (!serverDetails.value[name] && selectedPlatformId.value) {
      const detail = await api.getMcpServer(selectedPlatformId.value, name)
      serverDetails.value[name] = { config_text: detail.config_text, format: detail.format }
    }
  }

  async function createServer(name: string, configText: string) {
    if (!selectedPlatformId.value) return
    await api.importMcpServer(selectedPlatformId.value, name, configText)
    await selectPlatform(selectedPlatformId.value)
  }

  async function deleteServer(name: string) {
    if (!selectedPlatformId.value) return
    await api.deleteMcpServer(selectedPlatformId.value, name)
    await selectPlatform(selectedPlatformId.value)
  }

  async function loadSyncTargets(serverName: string) {
    if (!selectedPlatformId.value) return
    syncServerName.value = serverName
    syncTargets.value = await api.getMcpSyncTargets(selectedPlatformId.value, serverName)
    if (syncTargets.value.length > 0) {
      syncTargetPlatformId.value = syncTargets.value[0].id
    }
  }

  async function performSync(targetPlatformId: string) {
    if (!selectedPlatformId.value || !syncServerName.value) return
    await api.syncMcpServer(selectedPlatformId.value, targetPlatformId, syncServerName.value)
    await selectPlatform(selectedPlatformId.value)
    syncModalOpen.value = false
  }

  // --- Preview for Add / Delete ---

  async function loadAddPreview(name: string, configText: string) {
    if (!selectedPlatformId.value) return
    previewMode.value = 'add'
    previewAddName.value = name
    previewAddConfig.value = configText
    previewLoading.value = true
    previewModalOpen.value = true
    previewData.value = null
    try {
      previewData.value = await api.previewMcpChange(selectedPlatformId.value, name, configText)
    } catch (e: any) {
      previewData.value = { error: String(e?.message || e) }
    } finally {
      previewLoading.value = false
    }
  }

  async function loadDeletePreview(name: string) {
    if (!selectedPlatformId.value) return
    previewMode.value = 'delete'
    previewLoading.value = true
    previewModalOpen.value = true
    previewData.value = null
    try {
      previewData.value = await api.previewMcpChange(selectedPlatformId.value, name)
    } catch (e: any) {
      previewData.value = { error: String(e?.message || e) }
    } finally {
      previewLoading.value = false
    }
  }

  async function confirmPreview() {
    if (!selectedPlatformId.value || !previewData.value || previewData.value.error) return
    const serverName = previewData.value.server_name
    if (previewMode.value === 'add') {
      await api.importMcpServer(selectedPlatformId.value, serverName, previewAddConfig.value)
    } else {
      await api.deleteMcpServer(selectedPlatformId.value, serverName)
    }
    previewModalOpen.value = false
    previewData.value = null
    await selectPlatform(selectedPlatformId.value)
  }

  function cancelPreview() {
    previewModalOpen.value = false
    previewData.value = null
  }

  return {
    platforms, servers, selectedPlatformId, expandedServer, serverDetails,
    addModalOpen, syncModalOpen, syncTargets, syncTargetPlatformId, syncServerName,
    deleteConfirmServerName,
    previewModalOpen, previewLoading, previewData, previewMode,
    previewAddName, previewAddConfig,
    refreshPlatforms, selectPlatform, toggleServer, createServer, deleteServer,
    loadSyncTargets, performSync,
    loadAddPreview, loadDeletePreview, confirmPreview, cancelPreview,
  }
})
