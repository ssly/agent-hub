import { defineStore } from 'pinia'
import { ref, computed, reactive } from 'vue'
import * as api from '@/lib/api'

export const useSessionsStore = defineStore('sessions', () => {
  const platforms = ref<any[]>([])
  const sessions = ref<any[]>([])
  const selectedPlatformId = ref<string | null>(null)
  const selectedPathFilter = ref('all')
  const pathOptions = ref<string[]>(['all', 'unknown'])
  const sessionTotal = ref(0)
  const sessionOffset = ref(0)
  const hasMore = ref(false)
  const isLoading = ref(false)
  const loadingMore = ref(false)
  const loadError = ref('')
  const pageSize = 150

  const searchQuery = ref('')
  const searchResults = ref<any[]>([])
  const isSearching = ref(false)
  const searchError = ref('')

  // Per-id map so toggling one card only invalidates that card's selected
  // binding, not the whole Set. No explicit selection mode — checkboxes are
  // always on the list, and bulk actions appear when anything is checked.
  const selectedMap = reactive<Record<string, true>>({})
  const isBulkDeleting = ref(false)
  const isBulkExporting = ref(false)

  // Directory filter for path-first session exploration (mirrors plugins workspace)
  const directoryFilter = ref<string | null>(localStorage.getItem('ah-sessions-directory-filter') || null)

  async function refreshPlatforms(keepPathFilter = false) {
    loadError.value = ''
    try {
      platforms.value = await api.listSessionPlatforms(directoryFilter.value)
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
    if (!keepPathFilter) {
      selectedPathFilter.value = directoryFilter.value || 'all'
    }
    await loadSessions(false)
  }

  // Modal open state only — fetching lives in the shared SessionMessagesModal /
  // SessionResumeModal components, which load through the sessions backend by
  // platform/session identity. The Monitor view reuses the same components.
  const messagesModalOpen = ref(false)
  const activeSession = ref<any | null>(null)

  function openMessages(session: any) {
    activeSession.value = session
    messagesModalOpen.value = true
  }

  async function loadSessions(append: boolean) {
    if (!selectedPlatformId.value) return
    const offset = append ? sessionOffset.value : 0
    const filter = directoryFilter.value || selectedPathFilter.value || 'all'
    try {
      const page = await api.listSessions(selectedPlatformId.value, filter, offset, pageSize)
      const pagePaths = Array.isArray(page?.paths) && page.paths.length > 0 ? page.paths : ['all', 'unknown']
      if (directoryFilter.value && !pagePaths.includes(directoryFilter.value)) {
        pagePaths.push(directoryFilter.value)
      }
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

  async function setDirectoryFilter(directory: string | null) {
    directoryFilter.value = directory || null
    if (directory) {
      localStorage.setItem('ah-sessions-directory-filter', directory)
      selectedPathFilter.value = directory
    } else {
      localStorage.removeItem('ah-sessions-directory-filter')
      selectedPathFilter.value = 'all'
    }
    searchQuery.value = ''
    searchResults.value = []
    clearSelection()
    isLoading.value = true
    try {
      await refreshPlatforms(true)
    } finally {
      isLoading.value = false
    }
  }

  async function selectPlatform(id: string) {
    selectedPlatformId.value = id
    selectedPathFilter.value = directoryFilter.value || 'all'
    searchQuery.value = ''
    searchResults.value = []
    clearSelection()
    isLoading.value = true
    try {
      await loadSessions(false)
    } finally {
      isLoading.value = false
    }
  }

  // Resume modal open state (SessionResumeModal fetches the preview itself).
  const resumeModalOpen = ref(false)
  const resumeTarget = ref<any | null>(null)

  function openResume(session: any) {
    resumeTarget.value = session
    resumeModalOpen.value = true
  }

  async function changePathFilter(filter: string) {
    selectedPathFilter.value = filter
    searchQuery.value = ''
    searchResults.value = []
    clearSelection()
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

  function clearSelection() {
    for (const id of Object.keys(selectedMap)) delete selectedMap[id]
  }

  function toggleSelected(id: string) {
    if (selectedMap[id]) delete selectedMap[id]
    else selectedMap[id] = true
  }

  function selectAllLoaded() {
    for (const session of sessions.value) selectedMap[session.id] = true
  }

  function selectRange(fromId: string, toId: string) {
    const ids = sessions.value.map(session => session.id as string)
    const from = ids.indexOf(fromId)
    const to = ids.indexOf(toId)
    if (from < 0 || to < 0) {
      toggleSelected(toId)
      return
    }
    const start = Math.min(from, to)
    const end = Math.max(from, to)
    for (let i = start; i <= end; i++) selectedMap[ids[i]] = true
  }

  function isSelected(id: string) {
    return selectedMap[id] === true
  }

  const selectedCount = computed(() => Object.keys(selectedMap).length)

  async function bulkDelete(): Promise<{ deleted: number; failed: Array<{ session_id: string; error: string }> }> {
    const platformId = selectedPlatformId.value
    const ids = Object.keys(selectedMap)
    if (!platformId || ids.length === 0) {
      return { deleted: 0, failed: [] }
    }
    isBulkDeleting.value = true
    try {
      const result = await api.deleteSessions(platformId, ids)
      // Refresh once (not per-item). Reuse the path-filter-preserving refresh.
      await refreshPlatforms(true)
      clearSelection()
      return result
    } finally {
      isBulkDeleting.value = false
    }
  }

  async function bulkExport(locale: string): Promise<api.SessionExportResult | null> {
    const platformId = selectedPlatformId.value
    const ids = Object.keys(selectedMap)
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
    sessionTotal, sessionOffset, hasMore,
    isLoading, loadingMore, loadError,
    directoryFilter, setDirectoryFilter,
    messagesModalOpen, activeSession,
    resumeModalOpen, resumeTarget,
    searchQuery, searchResults, isSearching, searchError,
    selectedMap, selectedCount, isBulkDeleting, isBulkExporting, isSelected,
    refreshPlatforms, loadSessions, selectPlatform, openResume,
    changePathFilter, loadMore, openMessages, doSearch,
    toggleSelected, selectAllLoaded, selectRange, clearSelection, bulkDelete, bulkExport,
  }
})
