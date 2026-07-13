import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as api from '@/lib/api'
import { usePluginsStore } from './plugins'
import { useSwitchStore } from './switch'
import { useSessionsStore } from './sessions'

export type TabId = 'plugins' | 'sessions' | 'accounts'
export type ViewId = 'plugins' | 'detail' | 'diff' | 'search'
export type UpdateStatus = 'idle' | 'checking' | 'available' | 'uptodate' | 'installing' | 'error'
export type UpdateDownloadSource = 'direct' | 'mirror'

const VALID_TABS: TabId[] = ['plugins', 'sessions', 'accounts']

export const useAppStore = defineStore('app', () => {
  const storedTab = localStorage.getItem('ah-tab')
  const migratedTab = storedTab === 'skills' || storedTab === 'mcp'
    ? 'plugins'
    : storedTab === 'switch' ? 'accounts' : storedTab
  const currentTab = ref<TabId>(VALID_TABS.includes(migratedTab as TabId) ? migratedTab as TabId : 'plugins')
  const currentView = ref<ViewId>('plugins')
  const sidebarCollapsed = ref(false)
  const appVersion = ref('...')
  const trashCount = ref(0)
  const locale = ref('zh-CN')

  // Trash Modal States
  const trashModalOpen = ref(false)
  const trashItems = ref<any[]>([])
  const trashLoading = ref(false)

  // About Modal
  const aboutModalOpen = ref(false)
  // Only store metadata (version/body/date), NOT the raw Update object.
  // The Update class uses private fields (#backend) that break when wrapped by Vue/Pinia Proxy.
  const availableUpdate = ref<{ version: string; body?: string; date?: string } | null>(null)

  // Updater state — kept in the store so the sidebar can surface download
  // progress even after the About modal is closed mid-download.
  const updateStatus = ref<UpdateStatus>('idle')
  const updateProgress = ref(0)
  const updateDownloaded = ref(0)
  const updateTotal = ref(0)
  const updateError = ref('')
  const updateInfo = ref<{ version: string; body?: string; date?: string } | null>(null)
  const updateDownloadSource = ref<UpdateDownloadSource>('direct')
  const isDownloading = computed(() => updateStatus.value === 'installing')

  function resetUpdateState() {
    updateStatus.value = 'idle'
    updateInfo.value = null
    updateProgress.value = 0
    updateDownloaded.value = 0
    updateTotal.value = 0
    updateError.value = ''
    updateDownloadSource.value = 'direct'
  }

  async function init() {
    try { locale.value = await api.getLocale() } catch {}
    try { appVersion.value = await api.getAppVersion() } catch { appVersion.value = '0.0.0' }

    const pluginsStore = usePluginsStore()
    await pluginsStore.refreshPlatforms()

    await refreshTrashCount()

    // Restore the content of the last-opened tab (plugins are already refreshed above).
    if (currentTab.value === 'accounts') {
      const switchStore = useSwitchStore()
      if (switchStore.selectedAgent) await switchStore.loadProfiles()
    } else if (currentTab.value === 'sessions') {
      const sessionsStore = useSessionsStore()
      sessionsStore.isLoading = true
      try {
        await Promise.all([sessionsStore.refreshPlatforms(), sessionsStore.refreshTerminals()])
      } finally {
        sessionsStore.isLoading = false
      }
    }
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
    const pluginsStore = usePluginsStore()
    await pluginsStore.refreshPlatforms()
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

  function openAbout() {
    aboutModalOpen.value = true
  }

  function switchTab(tab: TabId) {
    currentTab.value = tab
    localStorage.setItem('ah-tab', tab)
    if (tab === 'plugins') {
      currentView.value = 'plugins'
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
    aboutModalOpen, availableUpdate,
    updateStatus, updateProgress, updateDownloaded, updateTotal, updateError, updateInfo, updateDownloadSource,
    isDownloading, resetUpdateState,
    init, refreshTrashCount, switchTab, setView, toggleSidebar, toggleTheme, isNightTheme, switchLocale,
    openTrash, restoreTrash, deleteTrashForever, emptyTrash, openAbout,
  }
})
