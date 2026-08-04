import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import * as api from '@/lib/api'

export interface ZCodePlugin {
  id: string
  name: string
  marketplace: string
  version: string
  description: string
  author: string
  installed: boolean
  skill_count: number
  command_count: number
  hook_count: number
  install_path: string
}

// ZCode 插件市场只读列表：只存在用户级（~/.zcode/cli/plugins），
// 项目目录范围不加载。
export const useZCodePluginsStore = defineStore('zcode-plugins', () => {
  const plugins = ref<ZCodePlugin[]>([])
  const loading = ref(false)
  const error = ref('')

  const installedCount = computed(() => plugins.value.filter(plugin => plugin.installed).length)

  async function loadPlugins() {
    loading.value = true
    error.value = ''
    try {
      plugins.value = await api.getZCodePlugins()
    } catch (e: any) {
      error.value = String(e?.message || e)
      plugins.value = []
    } finally {
      loading.value = false
    }
  }

  function clear() {
    plugins.value = []
    error.value = ''
    loading.value = false
  }

  return {
    plugins,
    loading,
    error,
    installedCount,
    loadPlugins,
    clear,
  }
})
