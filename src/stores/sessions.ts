import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
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

  // Batch selection mode (sessions list).
  const selectionMode = ref(false)
  const selectedIds = ref<Set<string>>(new Set())
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
    sessionTotal, sessionOffset, hasMore,
    isLoading, loadingMore, loadError,
    directoryFilter, setDirectoryFilter,
    messagesModalOpen, activeSession,
    resumeModalOpen, resumeTarget,
    searchQuery, searchResults, isSearching, searchError,
    selectionMode, selectedIds, selectedCount, isBulkDeleting, isBulkExporting,
    refreshPlatforms, loadSessions, selectPlatform, openResume,
    changePathFilter, loadMore, openMessages, doSearch,
    enterSelection, exitSelection, toggleSelected, selectAllLoaded, clearSelection, bulkDelete, bulkExport,
  }
})
