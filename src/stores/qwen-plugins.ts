import { ref } from 'vue'
import { defineStore } from 'pinia'
import * as api from '@/lib/api'

// 后端 QwenPluginView 为 camelCase 序列化。
export interface QwenPlugin {
  id: string
  name: string
  version: string
  description: string
  mcpServerCount: number
  skillCount: number
  commandCount: number
  agentCount: number
  installPath: string
}

// Qwen Code 扩展只读列表：只存在用户级（~/.qwen/extensions），
// 项目目录范围不加载。
export const useQwenPluginsStore = defineStore('qwen-plugins', () => {
  const plugins = ref<QwenPlugin[]>([])
  const loading = ref(false)
  const error = ref('')

  async function loadPlugins() {
    loading.value = true
    error.value = ''
    try {
      plugins.value = await api.getQwenPlugins()
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
    loadPlugins,
    clear,
  }
})
