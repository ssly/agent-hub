import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
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

  const searchQuery = ref('')
  const searchResults = ref<any[]>([])
  const isSearching = ref(false)
  const searchError = ref('')

  // Batch selection mode (sessions list).
  const selectionMode = ref(false)
  const selectedIds = ref<Set<string>>(new Set())
  const isBulkDeleting = ref(false)
  const isBulkExporting = ref(false)


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
    searchQuery.value = ''
    searchResults.value = []
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
    searchQuery.value = ''
    searchResults.value = []
    isLoading.value = true
    try {
      await loadSessions(false)
    } finally {
      isLoading.value = false
    }
  }

  async function doSearch(query: string) {
    searchQuery.value = query
    if (!query.trim() || !selectedPlatformId.value) {
      searchResults.value = []
      isSearching.value = false
      return
    }
    isSearching.value = true
    searchError.value = ''
    try {
      searchResults.value = await api.searchSessionMessages(selectedPlatformId.value, query)
    } catch (e: any) {
      searchResults.value = []
      searchError.value = e?.message || e?.SyncError || String(e)
    } finally {
      isSearching.value = false
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

  function enterSelection() {
    selectionMode.value = true
  }

  function exitSelection() {
    selectionMode.value = false
    selectedIds.value = new Set()
  }

  function toggleSelected(id: string) {
    const next = new Set(selectedIds.value)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    selectedIds.value = next
  }

  function selectAllLoaded() {
    selectedIds.value = new Set(sessions.value.map((s: any) => s.id))
  }

  function clearSelection() {
    selectedIds.value = new Set()
  }

  const selectedCount = computed(() => selectedIds.value.size)

  async function bulkDelete(): Promise<{ deleted: number; failed: Array<{ session_id: string; error: string }> }> {
    const platformId = selectedPlatformId.value
    const ids = Array.from(selectedIds.value)
    if (!platformId || ids.length === 0) {
      return { deleted: 0, failed: [] }
    }
    isBulkDeleting.value = true
    try {
      const result = await api.deleteSessions(platformId, ids)
      // Refresh once (not per-item). Reuse the path-filter-preserving refresh.
      await refreshPlatforms(true)
      exitSelection()
      return result
    } finally {
      isBulkDeleting.value = false
    }
  }

  async function bulkExport(locale: string): Promise<api.SessionExportResult | null> {
    const platformId = selectedPlatformId.value
    const ids = Array.from(selectedIds.value)
    if (!platformId || ids.length === 0) return null
    isBulkExporting.value = true
    try {
      return await api.exportSessionsHtml(platformId, ids, locale)
    } finally {
      isBulkExporting.value = false
    }
  }

  return {
    platforms, sessions, selectedPlatformId, selectedPathFilter, pathOptions,
    terminals, selectedTerminal, sessionTotal, sessionOffset, hasMore,
    isLoading, loadingMore, loadError,
    messagesModalOpen, messages, activeSession, messagesLoading, messagesLoadingMore,
    messagesOffset, messagesHasMore, messagesError,
    searchQuery, searchResults, isSearching, searchError,
    selectionMode, selectedIds, selectedCount, isBulkDeleting, isBulkExporting,
    refreshPlatforms, refreshTerminals, loadSessions, selectPlatform,
    changePathFilter, loadMore, openMessages, loadMessages, doSearch,
    enterSelection, exitSelection, toggleSelected, selectAllLoaded, clearSelection, bulkDelete, bulkExport,
  }
})
