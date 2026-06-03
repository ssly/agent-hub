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

  return {
    platforms, servers, selectedPlatformId, expandedServer, serverDetails,
    addModalOpen, syncModalOpen, syncTargets, syncTargetPlatformId, syncServerName, deleteConfirmServerName,
    refreshPlatforms, selectPlatform, toggleServer, createServer, deleteServer, loadSyncTargets, performSync,
  }
})
