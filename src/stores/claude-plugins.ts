import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import * as api from '@/lib/api'

export interface ClaudeCodePlugin {
  id: string
  name: string
  marketplace: string
  version: string
  scope: string
  enabled: boolean
  manageable: boolean
  description: string
  install_path: string
  installed_at?: string | null
  last_updated?: string | null
}

export const useClaudePluginsStore = defineStore('claude-plugins', () => {
  const plugins = ref<ClaudeCodePlugin[]>([])
  const loading = ref(false)
  const error = ref('')
  const workspaceDirectory = ref('')
  const togglingIds = ref<Set<string>>(new Set())

  const enabledCount = computed(() => plugins.value.filter(plugin => plugin.enabled).length)

  function sorted(items: ClaudeCodePlugin[]) {
    return [...items].sort((left, right) =>
      Number(right.enabled) - Number(left.enabled) || left.name.localeCompare(right.name)
    )
  }

  async function loadPlugins(workspaceDir = workspaceDirectory.value) {
    workspaceDirectory.value = workspaceDir
    loading.value = true
    error.value = ''
    try {
      plugins.value = sorted(await api.listClaudePlugins(workspaceDirectory.value))
    } catch (e: any) {
      error.value = String(e?.message || e)
      plugins.value = []
    } finally {
      loading.value = false
    }
  }

  async function setPluginEnabled(plugin: ClaudeCodePlugin, enabled: boolean) {
    const next = new Set(togglingIds.value)
    next.add(plugin.id)
    togglingIds.value = next
    try {
      await api.setClaudePluginEnabled(plugin.id, plugin.scope, enabled)
      plugins.value = sorted(plugins.value.map(item =>
        item.id === plugin.id ? { ...item, enabled } : item
      ))
      try {
        plugins.value = sorted(await api.listClaudePlugins(workspaceDirectory.value))
      } catch {
        // The official toggle succeeded. Keep the confirmed local state if a
        // follow-up refresh fails instead of reporting the whole action failed.
      }
    } finally {
      const remaining = new Set(togglingIds.value)
      remaining.delete(plugin.id)
      togglingIds.value = remaining
    }
  }

  function clear() {
    plugins.value = []
    workspaceDirectory.value = ''
    error.value = ''
    loading.value = false
    togglingIds.value = new Set()
  }

  return {
    plugins,
    loading,
    error,
    workspaceDirectory,
    togglingIds,
    enabledCount,
    loadPlugins,
    setPluginEnabled,
    clear,
  }
})
