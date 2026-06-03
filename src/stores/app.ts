import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'
import { useSkillsStore } from './skills'
import { useMcpStore } from './mcp'

export type TabId = 'skills' | 'mcp' | 'sessions' | 'switch'
export type ViewId = 'skills' | 'detail' | 'diff' | 'search'

export const useAppStore = defineStore('app', () => {
  const currentTab = ref<TabId>('skills')
  const currentView = ref<ViewId>('skills')
  const sidebarCollapsed = ref(false)
  const appVersion = ref('...')
  const trashCount = ref(0)
  const locale = ref('zh-CN')

  // Trash Modal States
  const trashModalOpen = ref(false)
  const trashItems = ref<any[]>([])
  const trashLoading = ref(false)

  async function init() {
    try { locale.value = await api.getLocale() } catch {}
    try { appVersion.value = await api.getAppVersion() } catch { appVersion.value = '0.0.0' }

    const skillsStore = useSkillsStore()
    await skillsStore.refreshPlatforms()

    const mcpStore = useMcpStore()
    await mcpStore.refreshPlatforms()

    await refreshTrashCount()
  }

  async function refreshTrashCount() {
    try {
      const items = await api.listTrash()
      trashCount.value = items.length
    } catch {
      trashCount.value = 0
    }
  }

  async function openTrash() {
    trashLoading.value = true
    trashModalOpen.value = true
    try {
      trashItems.value = await api.listTrash()
      trashCount.value = trashItems.value.length
    } catch {
      trashItems.value = []
      trashCount.value = 0
    } finally {
      trashLoading.value = false
    }
  }

  async function restoreTrash(id: string) {
    await api.restoreTrashItem(id)
    await openTrash()
    const skillsStore = useSkillsStore()
    await skillsStore.reloadPlatforms()
  }

  async function deleteTrashForever(id: string) {
    await api.permanentlyDeleteTrashItem(id)
    await openTrash()
  }

  async function emptyTrash() {
    await api.emptyTrash()
    trashItems.value = []
    trashCount.value = 0
    trashModalOpen.value = false
  }

  function switchTab(tab: TabId) {
    currentTab.value = tab
    if (tab === 'skills') {
      currentView.value = 'skills'
    }
  }

  function setView(view: ViewId) {
    currentView.value = view
  }

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }

  function toggleTheme() {
    const root = document.documentElement
    const current = root.getAttribute('data-theme')
    if (current === 'night') {
      root.removeAttribute('data-theme')
      localStorage.setItem('ah-theme', 'light')
    } else {
      root.setAttribute('data-theme', 'night')
      localStorage.setItem('ah-theme', 'night')
    }
  }

  function isNightTheme(): boolean {
    return document.documentElement.getAttribute('data-theme') === 'night'
  }

  async function switchLocale() {
    locale.value = locale.value === 'en' ? 'zh-CN' : 'en'
    await api.setLocale(locale.value)
    localStorage.setItem('ah-locale', locale.value)
  }

  return {
    currentTab, currentView, sidebarCollapsed, appVersion, trashCount, locale,
    trashModalOpen, trashItems, trashLoading,
    init, refreshTrashCount, switchTab, setView, toggleSidebar, toggleTheme, isNightTheme, switchLocale,
    openTrash, restoreTrash, deleteTrashForever, emptyTrash,
  }
})
