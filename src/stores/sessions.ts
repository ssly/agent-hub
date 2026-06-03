import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/lib/api'

export const useSessionsStore = defineStore('sessions', () => {
  const platforms = ref<any[]>([])
  const sessions = ref<any[]>([])
  const selectedPlatformId = ref<string | null>(null)
  const selectedPathFilter = ref('all')
  const pathOptions = ref<string[]>(['all', 'unknown'])
  const terminals = ref<any[]>([])
  const selectedTerminal = ref('terminal-default')
  const sessionTotal = ref(0)
  const sessionOffset = ref(0)
  const hasMore = ref(false)
  const isLoading = ref(false)
  const loadingMore = ref(false)
  const loadError = ref('')
  const pageSize = 50

  async function refreshPlatforms(keepPathFilter = false) {
    loadError.value = ''
    try {
      platforms.value = await api.listSessionPlatforms()
    } catch (e: any) {
      platforms.value = []
      loadError.value = e?.SyncError || e?.message || String(e)
    }
    if (platforms.value.length === 0) {
      selectedPlatformId.value = null
      sessions.value = []
      return
    }
    const exists = platforms.value.some(p => p.id === selectedPlatformId.value)
    if (!exists) {
      selectedPlatformId.value = platforms.value[0].id
    }
    if (!keepPathFilter) selectedPathFilter.value = 'all'
    await loadSessions(false)
  }

  async function refreshTerminals() {
    try {
      const list = await api.listSessionTerminals()
      terminals.value = Array.isArray(list) ? list : []
    } catch {
      terminals.value = []
    }
    if (terminals.value.length > 0) {
      const active = terminals.value.find((t: any) => t.id === selectedTerminal.value && t.available)
      if (!active) {
        const first = terminals.value.find((t: any) => t.available)
        selectedTerminal.value = first ? first.id : terminals.value[0].id
      }
    }
  }

  async function loadSessions(append: boolean) {
    if (!selectedPlatformId.value) return
    const offset = append ? sessionOffset.value : 0
    try {
      const page = await api.listSessions(selectedPlatformId.value, selectedPathFilter.value || 'all', offset, pageSize)
      const pagePaths = Array.isArray(page?.paths) && page.paths.length > 0 ? page.paths : ['all', 'unknown']
      pathOptions.value = pagePaths
      const pageSessions = Array.isArray(page?.sessions) ? page.sessions : []
      sessions.value = append ? [...sessions.value, ...pageSessions] : pageSessions
      sessionTotal.value = Number(page?.total) || sessions.value.length
      sessionOffset.value = (Number(page?.offset ?? offset)) + pageSessions.length
      hasMore.value = Boolean(page?.has_more) && sessionOffset.value < sessionTotal.value
      loadError.value = ''
    } catch (e: any) {
      if (!append) {
        sessions.value = []
        sessionTotal.value = 0
        hasMore.value = false
        loadError.value = e?.SyncError || e?.message || String(e)
      }
    }
  }

  async function selectPlatform(id: string) {
    selectedPlatformId.value = id
    selectedPathFilter.value = 'all'
    isLoading.value = true
    try {
      await loadSessions(false)
    } finally {
      isLoading.value = false
    }
  }

  // Messages Modal States
  const messagesModalOpen = ref(false)
  const messages = ref<any[]>([])
  const activeSession = ref<any | null>(null)
  const messagesLoading = ref(false)
  const messagesLoadingMore = ref(false)
  const messagesOffset = ref(0)
  const messagesHasMore = ref(true)
  const messagesError = ref('')
  const messagesPageSize = 50

  async function changePathFilter(filter: string) {
    selectedPathFilter.value = filter || 'all'
    isLoading.value = true
    try {
      await loadSessions(false)
    } finally {
      isLoading.value = false
    }
  }

  async function loadMore() {
    if (loadingMore.value || !hasMore.value) return
    loadingMore.value = true
    try {
      await loadSessions(true)
    } finally {
      loadingMore.value = false
    }
  }

  async function openMessages(session: any) {
    activeSession.value = session
    messagesModalOpen.value = true
    messages.value = []
    messagesOffset.value = 0
    messagesHasMore.value = true
    messagesError.value = ''
    await loadMessages(false)
  }

  async function loadMessages(append: boolean) {
    if (!activeSession.value) return
    if (append) messagesLoadingMore.value = true
    else messagesLoading.value = true

    try {
      const platformId = activeSession.value.platform_id || selectedPlatformId.value!
      const list = await api.getSessionMessages(platformId, activeSession.value.id, messagesOffset.value, messagesPageSize)
      messages.value = append ? [...messages.value, ...list] : list
      messagesOffset.value += list.length
      messagesHasMore.value = list.length === messagesPageSize
      messagesError.value = ''
    } catch (e: any) {
      messagesError.value = e?.SyncError || e?.message || String(e)
    } finally {
      messagesLoading.value = false
      messagesLoadingMore.value = false
    }
  }

  return {
    platforms, sessions, selectedPlatformId, selectedPathFilter, pathOptions,
    terminals, selectedTerminal, sessionTotal, sessionOffset, hasMore,
    isLoading, loadingMore, loadError,
    messagesModalOpen, messages, activeSession, messagesLoading, messagesLoadingMore,
    messagesOffset, messagesHasMore, messagesError,
    refreshPlatforms, refreshTerminals, loadSessions, selectPlatform,
    changePathFilter, loadMore, openMessages, loadMessages,
  }
})
