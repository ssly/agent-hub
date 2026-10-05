import { defineStore } from 'pinia'
import { ref, computed, reactive } from 'vue'
import * as api from '@/lib/api'

/** Pseudo-platform of the Sessions sidebar: "All" shows aggregate statistics
 *  only — no session list, no per-platform search. */
export const STATS_PLATFORM_ID = 'all'

export type SessionStatsRange = '1d' | '7d' | '31d'

const STATS_RANGE_KEY = 'ah-session-stats-range'
const STATS_RANGE_DAYS: Record<SessionStatsRange, number> = { '1d': 1, '7d': 7, '31d': 31 }

function readStatsRange(): SessionStatsRange {
  try {
    const stored = localStorage.getItem(STATS_RANGE_KEY)
    if (stored === '1d' || stored === '7d' || stored === '31d') return stored
  } catch {
    /* Private mode: fall through to the default window. */
  }
  return '7d'
}

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

  // ---------------------------------------------------------------------------
  // Statistics ("All" view)
  // ---------------------------------------------------------------------------
  // Counted by the backend (one transcript pass per session, memoized there),
  // so range switches reuse everything the previous window already counted.
  // Kept in the store rather than the view: entering the Sessions tab remounts
  // the view, and the tallies should not flash away in between.
  const statsRange = ref<SessionStatsRange>(readStatsRange())
  const stats = ref<api.SessionStatsReport | null>(null)
  const statsLoading = ref(false)
  const statsError = ref('')

  const isStatsView = computed(() => selectedPlatformId.value === STATS_PLATFORM_ID)

  async function loadStats() {
    statsLoading.value = true
    statsError.value = ''
    try {
      stats.value = await api.getSessionStats(STATS_RANGE_DAYS[statsRange.value], directoryFilter.value)
    } catch (e: any) {
      statsError.value = e?.SyncError || e?.message || String(e)
    } finally {
      statsLoading.value = false
    }
  }

  async function setStatsRange(range: SessionStatsRange) {
    if (range === statsRange.value && stats.value) return
    statsRange.value = range
    try {
      localStorage.setItem(STATS_RANGE_KEY, range)
    } catch {
      /* Preference only — a blocked storage must not break the switch. */
    }
    await loadStats()
  }

  /** Enter the "All" statistics view. */
  async function selectStats() {
    selectedPlatformId.value = STATS_PLATFORM_ID
    selectedPathFilter.value = 'all'
    searchQuery.value = ''
    searchResults.value = []
    clearSelection()
    isLoading.value = true
    try {
      await loadStats()
    } finally {
      isLoading.value = false
    }
  }

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
      previewSession.value = null
      previewPlatformId.value = null
      return
    }
    // "All" is not one of the counted platforms, but stays selectable as long
    // as there is anything at all to aggregate.
    if (selectedPlatformId.value !== STATS_PLATFORM_ID) {
      const exists = platforms.value.some(p => p.id === selectedPlatformId.value)
      if (!exists) {
        selectedPlatformId.value = platforms.value[0].id
      }
    }
    if (!keepPathFilter) {
      selectedPathFilter.value = directoryFilter.value || 'all'
    }
    if (isStatsView.value) {
      await loadStats()
      return
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
    rememberPreview(selectedPlatformId.value || session?.platform_id, session?.id)
  }

  // ---------------------------------------------------------------------------
  // Preview session — the wide-screen reading pane of the Sessions browser
  // ---------------------------------------------------------------------------
  // A third kind of "current session", deliberately kept apart from selectedMap
  // (batch checkboxes) and from the messages modal, which stays the narrow-window
  // path. Nothing here fetches: the pane and the modal share SessionMessagesPanel,
  // which loads messages itself. The last previewed session per platform is
  // remembered so coming back to a platform lands where you left off.
  const PREVIEW_MEMORY_KEY = 'ah-session-preview'
  const previewSession = ref<any | null>(null)
  // Platform the current preview was picked for. Session ids are only unique
  // per platform, so "is this row still the preview?" must be asked per
  // platform — otherwise switching platforms can silently keep the old row.
  const previewPlatformId = ref<string | null>(null)

  function readPreviewMemory(): Record<string, string> {
    try {
      const parsed = JSON.parse(localStorage.getItem(PREVIEW_MEMORY_KEY) || '{}')
      return parsed && typeof parsed === 'object' ? parsed : {}
    } catch {
      return {}
    }
  }

  function rememberPreview(platformId: string | null | undefined, sessionId: string | null | undefined) {
    if (!platformId || !sessionId) return
    try {
      const memory = readPreviewMemory()
      memory[platformId] = sessionId
      localStorage.setItem(PREVIEW_MEMORY_KEY, JSON.stringify(memory))
    } catch {
      /* Private mode / quota: the memory is a nicety, never a requirement. */
    }
  }

  /** Decide what the pane shows after the list changed: keep the current preview
   *  while it is still listed, else restore this platform's remembered session,
   *  else fall back to the newest one (the list is updated_at DESC). Never pulls
   *  extra pages to hunt for a remembered id that is not loaded yet. */
  function ensurePreview() {
    const list = sessions.value
    const platformId = selectedPlatformId.value
    if (list.length === 0 || !platformId) {
      previewSession.value = null
      previewPlatformId.value = null
      return
    }
    const currentId = previewSession.value?.id
    const kept =
      previewPlatformId.value === platformId && currentId
        ? list.find((session: any) => session.id === currentId)
        : null
    if (kept) {
      // Re-point at the freshly loaded row so title/time/metadata stay current.
      previewSession.value = kept
      return
    }
    const rememberedId = readPreviewMemory()[platformId]
    previewSession.value = (rememberedId && list.find((session: any) => session.id === rememberedId)) || list[0]
    previewPlatformId.value = platformId
  }

  function openPreview(session: any) {
    previewSession.value = session
    // Keyed by the browsed platform, not by the row's own platform_id: the
    // memory is read back with selectedPlatformId, and the two can differ
    // (a client-source row, a platform whose adapter relabels sessions).
    const platformId = selectedPlatformId.value || session?.platform_id || null
    previewPlatformId.value = platformId
    rememberPreview(platformId, session?.id)
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
    ensurePreview()
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
    if (id === STATS_PLATFORM_ID) {
      await selectStats()
      return
    }
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
    // The statistics view aggregates across platforms and has no search of its own.
    if (!query.trim() || !selectedPlatformId.value || isStatsView.value) {
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
    statsRange, stats, statsLoading, statsError, isStatsView,
    selectStats, setStatsRange, loadStats,
    messagesModalOpen, activeSession,
    previewSession, openPreview, ensurePreview,
    resumeModalOpen, resumeTarget,
    searchQuery, searchResults, isSearching, searchError,
    selectedMap, selectedCount, isBulkDeleting, isBulkExporting, isSelected,
    refreshPlatforms, loadSessions, selectPlatform, openResume,
    changePathFilter, loadMore, openMessages, doSearch,
    toggleSelected, selectAllLoaded, selectRange, clearSelection, bulkDelete, bulkExport,
  }
})
