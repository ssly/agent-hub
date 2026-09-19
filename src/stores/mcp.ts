import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'

export const useMcpStore = defineStore('mcp', () => {
  const platforms = ref<any[]>([])
  const servers = ref<any[]>([])
  const selectedPlatformId = ref<string | null>(null)
  const workspaceDirectory = ref('')
  const expandedServer = ref<string | null>(null)
  const serverDetails = ref<Record<string, { config_text: string; format: string }>>({})

  // Modal States
  const addModalOpen = ref(false)
  const deleteConfirmServerName = ref<string | null>(null)

  async function refreshPlatforms(workspaceDir = workspaceDirectory.value) {
    workspaceDirectory.value = workspaceDir
    platforms.value = await api.listMcpPlatforms(workspaceDir)
    const exists = platforms.value.some(p => p.id === selectedPlatformId.value)
    if (!exists && platforms.value.length > 0) {
      await selectPlatform(platforms.value[0].id, workspaceDir)
    }
  }

  async function selectPlatform(id: string, workspaceDir = workspaceDirectory.value) {
    selectedPlatformId.value = id
    workspaceDirectory.value = workspaceDir
    expandedServer.value = null
    serverDetails.value = {}
    try {
      servers.value = await api.getMcpServers(id, workspaceDirectory.value)
    } catch {
      servers.value = []
    }
    const platform = platforms.value.find(item => item.id === id)
    if (platform) platform.server_count = servers.value.length
  }

  function clearPlatform(id: string, workspaceDir = workspaceDirectory.value) {
    selectedPlatformId.value = id
    workspaceDirectory.value = workspaceDir
    servers.value = []
    expandedServer.value = null
    serverDetails.value = {}
  }

  async function toggleServer(name: string) {
    if (expandedServer.value === name) {
      expandedServer.value = null
      return
    }
    expandedServer.value = name
    if (!serverDetails.value[name] && selectedPlatformId.value) {
      const detail = await api.getMcpServer(selectedPlatformId.value, name, workspaceDirectory.value)
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

  return {
    platforms, servers, selectedPlatformId, workspaceDirectory, expandedServer, serverDetails,
    addModalOpen,
    deleteConfirmServerName,
    refreshPlatforms, selectPlatform, clearPlatform, toggleServer, createServer, deleteServer,
  }
})
