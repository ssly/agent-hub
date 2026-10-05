<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSessionsStore } from '@/stores/sessions'
import { formatInt, formatSessionTime, isPlatformResumable } from '@/lib/utils'
import { useToast } from '@/composables/useToast'
import { useHoverResetBool } from '@/composables/useHoverReset'
import { compactSessionPreview, extractSearchSnippets, type SearchSnippetMatch } from '@/lib/session-display'
import * as api from '@/lib/api'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import AppLoading from '@/components/ui/AppLoading.vue'
import { Folder, MessagesSquare, PanelRightClose, PanelRightOpen, Play, Search, X } from 'lucide-vue-next'
import AppSelect from '@/components/ui/AppSelect.vue'
import AgentIcon from '@/components/agents/AgentIcon.vue'
import SessionCard from '@/components/sessions/SessionCard.vue'
import SessionClientIcon from '@/components/sessions/SessionClientIcon.vue'
import SessionMessagesModal from '@/components/sessions/SessionMessagesModal.vue'
import SessionMessagesPanel from '@/components/sessions/SessionMessagesPanel.vue'
import SessionResumeModal from '@/components/sessions/SessionResumeModal.vue'
import SessionStatsView from '@/components/sessions/SessionStatsView.vue'

const { t, locale } = useI18n()
const store = useSessionsStore()
const { showToast } = useToast()

const pathFilterModel = computed({
  get: () => store.selectedPathFilter,
  set: (val: string) => store.changePathFilter(val),
})

const pathSelectOptions = computed(() =>
  store.pathOptions.map(p => ({
    value: p,
    label:
      p === 'all'
        ? t('session.path_filter_all')
        : p === 'unknown'
          ? t('session.path_filter_unknown')
          : p,
  })),
)

const loadMoreSentinel = ref<HTMLElement | null>(null)
const listBody = ref<HTMLElement | null>(null)
let observer: IntersectionObserver | null = null

// ---------------------------------------------------------------------------
// Wide-screen master–detail. A split only earns its keep when BOTH panes still
// fit what they are for, so the threshold is the sum of their comfortable
// widths rather than "whatever is left over":
//   list  30rem — a full badge/time row plus a title that is not chopped short
//   pane  44rem — a transcript measure you can actually read
// Below that, one full-width column beats two cramped ones (see the fluid /
// roomy content modifiers below). Keep these numbers in step with the
// --session-* variables in the style block.
// Measured on the page element, not the viewport: the sidebar can collapse (or
// be a different width) without the window changing size.
// ---------------------------------------------------------------------------
const SPLIT_MIN_WIDTH = 480 + 704 // 30rem list + 44rem pane
const PANE_COLLAPSED_KEY = 'ah-session-pane-collapsed'

const pageRoot = ref<HTMLElement | null>(null)
const isSplit = ref(false)
const paneCollapsed = ref(localStorage.getItem(PANE_COLLAPSED_KEY) === '1')
let pageObserver: ResizeObserver | null = null

function measurePage() {
  isSplit.value = (pageRoot.value?.clientWidth || 0) >= SPLIT_MIN_WIDTH
}

function togglePane() {
  paneCollapsed.value = !paneCollapsed.value
  try {
    localStorage.setItem(PANE_COLLAPSED_KEY, paneCollapsed.value ? '1' : '0')
  } catch {
    /* Preference only — a blocked storage must not break the toggle. */
  }
}

/** Card click: preview in the pane when it is open, modal otherwise. */
function handleOpen(session: any) {
  if (isSplit.value) store.openPreview(session)
  else store.openMessages(session)
}

function previewTitle(session: any): string {
  return compactSessionPreview(session?.title) || t('session.untitled')
}

/** Same badge resolution as the list cards, so the pane names the client the
 *  same way the selected row does. */
const previewBadge = computed(() => sessionBadge(store.previewSession || {}))
const previewBadgeAgentId = computed(() => sessionBadgeAgentId(store.previewSession || {}) || '')
const previewBadgeIcon = computed(() => sessionBadgeIcon(store.previewSession || {}) || '')

/** Empty-pane copy. When the list itself is empty the pane only echoes the
 *  list's own message (one quiet line, no illustration, no second paragraph). */
const paneEmptyTitle = computed(() => {
  if (store.sessions.length > 0) return t('session.preview_empty')
  return store.directoryFilter ? t('session.path_filter_empty') : t('session.no_sessions')
})
const paneEmptyHint = computed(() =>
  store.sessions.length > 0 ? t('session.preview_empty_hint') : '',
)

function setupObserver() {
  if (observer) observer.disconnect()
  if (typeof IntersectionObserver === 'undefined') return

  observer = new IntersectionObserver(
    entries => {
      if (entries[0]?.isIntersecting && store.hasMore && !store.loadingMore && !store.isLoading) {
        store.loadMore()
      }
    },
    { root: listBody.value, rootMargin: '300px' },
  )

  if (loadMoreSentinel.value) {
    observer.observe(loadMoreSentinel.value)
  }
}

const selectAnchorId = ref<string | null>(null)

const allLoadedSelected = computed(() =>
  store.sessions.length > 0 && store.sessions.every(session => store.selectedMap[session.id]),
)

function isTypingTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  return Boolean(target.closest('input, textarea, select, [contenteditable="true"]'))
}

function onKeydown(event: KeyboardEvent) {
  if (store.searchQuery || store.messagesModalOpen || store.resumeModalOpen) return
  if (isTypingTarget(event.target)) return
  if (event.key === 'Escape' && store.selectedCount > 0) {
    event.preventDefault()
    store.clearSelection()
    selectAnchorId.value = null
    return
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'a') {
    event.preventDefault()
    store.selectAllLoaded()
  }
}

onMounted(() => {
  setupObserver()
  measurePage()
  if (typeof ResizeObserver !== 'undefined') {
    pageObserver = new ResizeObserver(measurePage)
    if (pageRoot.value) pageObserver.observe(pageRoot.value)
  }
  window.addEventListener('keydown', onKeydown)
})

watch([loadMoreSentinel, listBody], () => {
  setupObserver()
})

onUnmounted(() => {
  observer?.disconnect()
  pageObserver?.disconnect()
  window.removeEventListener('keydown', onKeydown)
  if (searchDebounceTimer) clearTimeout(searchDebounceTimer)
})

function handleSelect(sessionId: string, event: MouseEvent) {
  if (event.shiftKey && selectAnchorId.value) {
    store.selectRange(selectAnchorId.value, sessionId)
    return
  }
  store.toggleSelected(sessionId)
  selectAnchorId.value = sessionId
}

function toggleSelectAll() {
  if (store.sessions.length === 0) return
  store.selectAllLoaded()
}

function clearSelection() {
  store.clearSelection()
  selectAnchorId.value = null
}

// Batch delete uses the same two-step confirm pattern as single delete: first
// click arms the chip, a second click fires bulkDelete. The chip disarms as
// soon as the pointer leaves the button.
const { armed: confirmBatch, arm: armBatch, reset: resetBatch } = useHoverResetBool()

watch(() => store.selectedCount, count => {
  if (count === 0) resetBatch()
})

async function handleBulkExport() {
  if (store.selectedCount === 0) {
    showToast(t('session.batch_select_none'), 'error')
    return
  }
  try {
    const result = await store.bulkExport(locale.value)
    if (result) {
      showToast(
        t('session.batch_exported', { sessions: result.session_count, messages: result.message_count }),
        'success',
        6000,
      )
    }
  } catch (e: any) {
    showToast(t('session.batch_export_failed', { error: e?.SyncError || e?.message || e }), 'error')
  }
}

async function handleBulkDelete() {
  if (store.selectedCount === 0) {
    showToast(t('session.batch_select_none'), 'error')
    return
  }
  if (!confirmBatch.value) {
    armBatch()
    return
  }
  resetBatch()
  const count = store.selectedCount
  try {
    const result = await store.bulkDelete()
    if (result.failed.length === 0) {
      showToast(t('session.batch_deleted', { deleted: result.deleted }), 'success')
    } else {
      showToast(
        t('session.batch_deleted_with_failed', { deleted: result.deleted, failed: result.failed.length }),
        result.deleted > 0 ? 'warning' : 'error',
        6000,
      )
    }
  } catch (e: any) {
    showToast(t('session.batch_delete_failed', { error: e?.SyncError || e?.message || e }), 'error')
  }
}

// Display name for the card's agent badge, resolved from the platforms list.
function platformName(platformId: string | undefined): string {
  const id = platformId || store.selectedPlatformId || ''
  return store.platforms.find(p => p.id === id)?.display_name || id
}

/** Badge with client-source refinement: Codex threads recorded as created by
 *  the ChatGPT desktop/IDE client (threads.source = "vscode") are marked as
 *  such; Kiro sessions all come from the kiro-cli transcript directory, so
 *  they are marked "Kiro CLI"; Antigravity splits CLI / desktop / IDE via
 *  app_data_dir; anything else keeps the plain platform name. */
function sessionBadge(session: { platform_id?: string; source?: string | null }): string {
  const id = session.platform_id || store.selectedPlatformId || ''
  if (id === 'codex' && session.source === 'chatgpt') {
    return t('session_monitor.source_chatgpt')
  }
  if (id === 'kiro' && session.source === 'terminal') {
    return t('session.source_kiro_cli')
  }
  if (id === 'cursor' && session.source === 'terminal') {
    return t('session.source_cursor_cli')
  }
  if (id === 'antigravity') {
    if (session.source === 'terminal') return t('session.source_antigravity_cli')
    if (session.source === 'antigravity-ide') return t('session.source_antigravity_ide')
    if (session.source === 'antigravity') return t('session.source_antigravity_app')
  }
  return platformName(session.platform_id)
}

/** ChatGPT is a concrete Codex client source, not a standalone platform.
 *  Its icon is therefore scoped to session cards only. */
function sessionBadgeIcon(session: { platform_id?: string; source?: string | null }): string | undefined {
  const id = session.platform_id || store.selectedPlatformId || ''
  return id === 'codex' && session.source === 'chatgpt' ? 'chatgpt' : undefined
}

/** Platform id for AgentIcon. ChatGPT badge uses SessionClientIcon instead. */
function sessionBadgeAgentId(session: { platform_id?: string; source?: string | null }): string | undefined {
  if (sessionBadgeIcon(session)) return undefined
  return session.platform_id || store.selectedPlatformId || undefined
}

// Single delete: the card already ran its two-step confirm, so this fires the
// real delete straight away (removes the on-disk session record).
async function handleDelete(session: any) {
  try {
    if (store.selectedMap[session.id]) store.toggleSelected(session.id)
    await api.deleteSession(session.platform_id || store.selectedPlatformId!, session.id)
    await store.refreshPlatforms(true)
    showToast(t('session.deleted'), 'success')
  } catch (e: any) {
    showToast(t('session.delete_failed', { error: e?.SyncError || e?.message || e }), 'error')
  }
}

function escapeHtml(text: string): string {
  return (text || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}

function highlightText(text: string, query: string) {
  const escapedText = escapeHtml(text)
  if (!query) return escapedText
  const escapedQuery = escapeHtml(query).replace(/[-\/\\^$*+?.()|[\]{}]/g, '\\$&')
  const regex = new RegExp(`(${escapedQuery})`, 'gi')
  return escapedText.replace(regex, '<mark class="ah-mark">$1</mark>')
}

const currentPlatformName = computed(() => {
  const p = store.platforms.find(p => p.id === store.selectedPlatformId)
  return p?.display_name || ''
})

const searchInputRef = ref<HTMLInputElement | null>(null)
const searchInputValue = ref(store.searchQuery)
let searchDebounceTimer: ReturnType<typeof setTimeout> | undefined

watch(() => store.searchQuery, (newVal) => {
  searchInputValue.value = newVal
})

function handleSearchInput(e: Event) {
  const val = (e.target as HTMLInputElement).value
  searchInputValue.value = val
  clearTimeout(searchDebounceTimer)
  searchDebounceTimer = setTimeout(() => {
    store.doSearch(val)
  }, 250)
}

function clearSessionSearch() {
  clearTimeout(searchDebounceTimer)
  searchInputValue.value = ''
  store.searchQuery = ''
  store.searchResults = []
  searchInputRef.value?.focus()
}

function handleSearchKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    clearSessionSearch()
    searchInputRef.value?.blur()
  }
}

function getResultSnippets(result: any): SearchSnippetMatch[] {
  return extractSearchSnippets(result.message, store.searchQuery)
}

function handleOpenSearchResult(result: any, snippet?: SearchSnippetMatch) {
  const payload = {
    id: result.session_id,
    title: result.session_title,
    project_path: result.project_path,
    platform_id: result.platform_id,
    target_timestamp: result.message?.timestamp,
    target_content: snippet?.matchText || result.message?.content?.slice(0, 80),
    search_query: store.searchQuery,
  }
  store.openMessages(payload)
}
</script>

<template>
  <div ref="pageRoot" class="session-page view-enter" :class="{ 'session-page--split': isSplit }">
      <AppLoading v-if="store.isLoading" class="py-16">{{ t('session.loading_messages') }}</AppLoading>

      <div v-else-if="store.loadError" class="p-6" style="color: var(--danger)">{{ store.loadError }}</div>

      <div v-else-if="!store.selectedPlatformId" class="flex flex-col items-center justify-center py-20">
        <p style="color: var(--ink-3)">{{ t('session.select_platform') }}</p>
      </div>

      <!-- "All": aggregate statistics only — no list, no reading pane. -->
      <SessionStatsView v-else-if="store.isStatsView" />

      <template v-else>
        <!-- Filter / Action bar: integrated search and actions -->
        <div
          class="ah-filter-bar ah-filter-bar--list"
          :class="{ 'ah-filter-bar--selecting': !store.searchQuery && store.selectedCount > 0 }"
        >
          <div class="ah-filter-bar__inner flex items-center justify-between gap-3">
            <div class="flex items-center gap-2 min-w-0">
              <template v-if="store.searchQuery">
                <span class="text-sm font-medium truncate" style="color: var(--ink-2)">
                  {{ t('session.search_results', { query: store.searchQuery, count: store.searchResults.length }) }}
                </span>
              </template>
              <template v-else>
                <span v-if="store.selectedCount > 0" class="ah-filter-bar__count">
                  {{ t('session.selected_count', { n: formatInt(store.selectedCount) }) }}
                </span>
                <span class="ah-filter-bar__stats">
                  {{ t('session.loaded_summary', { loaded: formatInt(store.sessions.length), total: formatInt(store.sessionTotal) }) }}
                </span>
              </template>
            </div>

            <div class="flex items-center gap-2 flex-none">
              <!-- Path filter (standard list only) -->
              <div
                v-if="!store.searchQuery && store.selectedCount === 0 && !store.directoryFilter && pathSelectOptions.length > 2"
                class="ah-path-select"
              >
                <AppSelect
                  v-model="pathFilterModel"
                  :options="pathSelectOptions"
                  searchable
                  :search-placeholder="t('session.path_filter_search')"
                  :search-empty="t('session.path_filter_search_empty')"
                  :search-clear-label="t('session.path_filter_search_clear')"
                >
                  <template #prefix>
                    <Folder :size="13" :stroke-width="2" />
                  </template>
                </AppSelect>
              </div>

              <!-- Search in action bar -->
              <div class="ah-session-search">
                <Search :size="13" :stroke-width="2" class="ah-session-search-icon" />
                <input
                  ref="searchInputRef"
                  type="text"
                  class="ah-session-search-input"
                  :placeholder="t('session.search_placeholder', { agent: currentPlatformName })"
                  :value="searchInputValue"
                  autocomplete="off"
                  spellcheck="false"
                  @input="handleSearchInput"
                  @keydown="handleSearchKeydown"
                />
                <button
                  v-if="searchInputValue"
                  type="button"
                  class="ah-session-search-clear"
                  :aria-label="t('session.clear_search')"
                  @mousedown.prevent
                  @click="clearSessionSearch"
                >
                  <X :size="12" :stroke-width="2.25" />
                </button>
              </div>

              <!-- Clear button when searching -->
              <button
                v-if="store.searchQuery"
                class="btn btn-secondary btn-sm"
                @click="clearSessionSearch"
              >
                {{ t('session.clear_search') }}
              </button>

              <!-- Standard action buttons -->
              <template v-else>
                <button
                  v-if="store.selectedCount > 0"
                  class="btn btn-secondary btn-sm"
                  @click="clearSelection"
                >
                  {{ t('session.batch_deselect') }}
                </button>
                <button
                  class="btn btn-secondary btn-sm"
                  :disabled="store.sessions.length === 0 || allLoadedSelected"
                  @click="toggleSelectAll"
                >
                  {{ t('session.batch_select_all') }}
                </button>
                <template v-if="store.selectedCount > 0">
                  <button
                    class="btn btn-primary btn-sm"
                    :disabled="store.isBulkExporting || store.isBulkDeleting"
                    @click="handleBulkExport"
                  >
                    {{ store.isBulkExporting
                      ? t('session.batch_exporting')
                      : t('session.batch_export_n', { n: store.selectedCount }) }}
                  </button>
                  <button
                    class="btn btn-sm"
                    :class="confirmBatch ? 'session-card__delete is-confirming' : 'btn-danger'"
                    :style="confirmBatch ? { width: 'auto' } : null"
                    :disabled="store.isBulkDeleting || store.isBulkExporting"
                    :title="confirmBatch ? t('session.batch_delete_confirm', { n: store.selectedCount }) : ''"
                    @click="handleBulkDelete"
                    @mouseleave="resetBatch()"
                  >
                    {{ store.isBulkDeleting
                      ? t('session.deleting')
                      : confirmBatch
                        ? t('session.confirm_delete')
                        : t('session.batch_delete_n', { n: store.selectedCount }) }}
                  </button>
                </template>
                <button
                  v-if="isSplit"
                  v-tooltip="paneCollapsed ? t('session.preview_show') : t('session.preview_hide')"
                  type="button"
                  class="btn btn-secondary btn-sm btn-icon"
                  :aria-label="paneCollapsed ? t('session.preview_show') : t('session.preview_hide')"
                  :aria-pressed="!paneCollapsed"
                  @click="togglePane"
                >
                  <PanelRightOpen v-if="paneCollapsed" :size="14" />
                  <PanelRightClose v-else :size="14" />
                </button>
              </template>
            </div>
          </div>
        </div>

        <!-- Search results view -->
        <template v-if="store.searchQuery">
          <div class="session-page__body session-page__body--single">
          <div class="ah-view-content">
          <AppLoading v-if="store.isSearching" class="py-12">{{ t('session.loading_messages') }}</AppLoading>

          <div v-else-if="store.searchResults.length === 0" class="py-12 text-center" style="color: var(--ink-3)">
            {{ t('session.no_search_results', { query: store.searchQuery }) }}
          </div>

          <div v-else class="space-y-3">
            <div
              v-for="(result, index) in store.searchResults"
              :key="index"
              class="ah-session-card ah-search-result-card flex flex-col gap-2 cursor-pointer transition-all hover:border-[color:var(--accent)]"
              @click="handleOpenSearchResult(result, getResultSnippets(result)[0])"
            >
              <div class="flex items-center justify-between gap-3 border-b pb-2" style="border-color: var(--hairline)">
                <div class="flex items-center gap-2 min-w-0">
                  <span class="text-xs" style="color: var(--ink-3)">{{ t('session.search_match_in') }}</span>
                  <span class="text-sm font-semibold truncate" style="color: var(--ink)">
                    {{ compactSessionPreview(result.session_title) || t('session.untitled') }}
                  </span>
                  <span
                    class="text-[11px] px-1.5 py-0.5 rounded font-medium flex-none"
                    :style="result.message.role === 'user'
                      ? { background: 'var(--accent-soft)', color: 'var(--accent)' }
                      : { background: 'var(--success-soft)', color: 'var(--success)' }"
                  >
                    {{ result.message.role === 'user' ? t('session.role_user') : t('session.role_assistant') }}
                  </span>
                </div>
                <span class="text-[10px] flex-none" style="color: var(--ink-4)">
                  {{ result.message.timestamp ? formatSessionTime(result.message.timestamp, locale) : '' }}
                </span>
              </div>

              <div class="text-xs truncate" style="color: var(--ink-3)">
                {{ result.project_path || t('session.no_project') }}
              </div>

              <!-- Compact contextual snippets around query match -->
              <div class="space-y-2 mt-1">
                <div
                  v-for="(snippet, sIdx) in getResultSnippets(result)"
                  :key="sIdx"
                  class="ah-search-snippet rounded p-2.5 flex flex-col gap-1 border transition-colors hover:border-[color:var(--accent)]"
                  :style="result.message.role === 'user'
                    ? { background: 'var(--accent-soft)', borderColor: 'var(--accent-mid)' }
                    : { background: 'var(--surface)', borderColor: 'var(--hairline)' }"
                  @click.stop="handleOpenSearchResult(result, snippet)"
                >
                  <div class="flex items-center justify-between text-[11px]" style="color: var(--ink-3)">
                    <span v-if="snippet.locationLabelKey" class="font-medium text-[10px] px-1.5 py-0.5 rounded bg-[color:var(--sunken)]" style="color: var(--accent)">
                      {{ t(snippet.locationLabelKey) }}
                    </span>
                    <span v-if="getResultSnippets(result).length > 1" class="text-[10px] ml-auto font-mono text-[color:var(--ink-4)]">
                      #{{ sIdx + 1 }}
                    </span>
                  </div>
                  <pre
                    class="text-xs font-mono whitespace-pre-wrap break-words m-0 leading-relaxed select-text"
                    style="color: var(--ink)"
                    v-html="highlightText(snippet.snippet, store.searchQuery)"
                  ></pre>
                </div>
              </div>

              <div class="mt-1 flex justify-between items-center pt-1 border-t" style="border-color: var(--hairline)">
                <span class="text-[11px]" style="color: var(--ink-4)">
                  {{ t('session.click_to_locate') }}
                </span>
                <div class="flex gap-2">
                  <button
                    class="btn btn-secondary btn-sm"
                    @click.stop="handleOpenSearchResult(result, getResultSnippets(result)[0])"
                  >
                    {{ t('session.view_messages') }}
                  </button>
                  <button
                    v-if="isPlatformResumable(result.platform_id)"
                    class="btn btn-primary btn-sm"
                    @click.stop="store.openResume({ id: result.session_id, title: result.session_title, project_path: result.project_path, platform_id: result.platform_id })"
                  >
                    {{ t('session.resume') }}
                  </button>
                </div>
              </div>
            </div>
          </div>
          </div>
          </div>
        </template>

        <!-- Standard session list view -->
        <template v-else>
          <div
            class="session-page__body"
            :class="{ 'session-page__body--split': isSplit && !paneCollapsed }"
          >
          <div ref="listBody" class="session-page__list">
          <div class="ah-view-content" :class="{
            'ah-view-content--fluid': !isSplit,
            'ah-view-content--roomy': isSplit && paneCollapsed,
          }">
          <!-- Session Cards -->
          <div v-if="store.sessions.length === 0" class="py-8 text-center" style="color: var(--ink-3)">
            {{ store.directoryFilter ? t('session.path_filter_empty') : t('session.no_sessions') }}
          </div>

          <div class="session-list-grid">
            <SessionCard
              v-for="session in store.sessions"
              :key="session.id"
              :badge="sessionBadge(session)"
              :badge-agent-id="sessionBadgeAgentId(session)"
              :badge-icon="sessionBadgeIcon(session)"
              :updated-at="session.updated_at"
              :title="session.title || t('session.untitled')"
              :subtitle="session.project_path || t('session.no_project')"
              :selectable="true"
              :selected="!!store.selectedMap[session.id]"
              :active="isSplit && !paneCollapsed && !store.isStatsView && store.previewSession?.id === session.id"
              :resumable="isPlatformResumable(session.platform_id || store.selectedPlatformId)"
              @open="handleOpen(session)"
              @resume="store.openResume(session)"
              @delete="handleDelete(session)"
              @select="handleSelect(session.id, $event)"
            />
          </div>

          <!-- Infinite scroll sentinel & status -->
          <div ref="loadMoreSentinel" class="py-4 flex justify-center">
            <AppLoading v-if="store.loadingMore" class="py-2">{{ t('session.loading_more') }}</AppLoading>
            <span
              v-else-if="!store.hasMore && store.sessions.length > 20"
              class="text-xs"
              style="color: var(--ink-4)"
            >
              {{ t('session.no_more_sessions') }}
            </span>
          </div>
          </div>
          </div>

          <!-- Reading pane. The store keeps it filled (newest session, or the
               last one previewed on this platform), so the empty state below
               only appears when the platform/filter really has no session. -->
          <aside
            v-if="isSplit && !paneCollapsed && !store.isStatsView"
            class="session-preview"
          >
            <template v-if="store.previewSession">
              <!-- Reading column: past a certain width a full-bleed transcript
                   is as unreadable as a full-bleed list row, so the pane keeps
                   a measure and lets its background carry the rest. -->
              <div class="session-preview__inner">
              <header class="session-preview__head">
                <span class="session-preview__badge">
                  <SessionClientIcon
                    v-if="previewBadgeIcon === 'chatgpt'"
                    client-id="chatgpt"
                    :size="12"
                  />
                  <AgentIcon v-else-if="previewBadgeAgentId" :agent-id="previewBadgeAgentId" :size="12" />
                  {{ previewBadge }}
                </span>
                <h3
                  v-tooltip.clamp="previewTitle(store.previewSession)"
                  class="session-preview__title truncate"
                >
                  {{ previewTitle(store.previewSession) }}
                </h3>
                <div class="session-preview__actions">
                  <button
                    v-if="isPlatformResumable(store.previewSession?.platform_id || store.selectedPlatformId)"
                    class="btn btn-secondary btn-sm"
                    @click="store.openResume(store.previewSession)"
                  >
                    <Play :size="12" />
                    {{ t('session.resume') }}
                  </button>
                  <button
                    v-tooltip="t('session.preview_hide')"
                    type="button"
                    class="btn btn-secondary btn-sm btn-icon"
                    :aria-label="t('session.preview_hide')"
                    @click="togglePane"
                  >
                    <PanelRightClose :size="14" />
                  </button>
                </div>
              </header>

              <SessionMessagesPanel
                :active="true"
                :platform-id="store.previewSession.platform_id || store.selectedPlatformId"
                :session-id="store.previewSession.id"
                :project-path="store.previewSession.project_path"
                :model="store.previewSession.model"
                :tokens="store.previewSession.tokens_used"
                :started-at="store.previewSession.started_at"
                :target-timestamp="store.previewSession.target_timestamp"
                :target-content="store.previewSession.target_content"
                :search-query="store.previewSession.search_query"
              />
              </div>
            </template>

            <!-- No illustration on purpose: this pane is part of a list view,
                 and the list next to it already states the same thing. -->
            <div v-else class="session-preview__empty">
              <MessagesSquare :size="26" />
              <strong>{{ paneEmptyTitle }}</strong>
              <span v-if="paneEmptyHint">{{ paneEmptyHint }}</span>
            </div>
          </aside>
          </div>
        </template>

        <SessionMessagesModal
          :show="store.messagesModalOpen"
          :platform-id="store.activeSession?.platform_id || store.selectedPlatformId"
          :session-id="store.activeSession?.id"
          :title="store.activeSession?.title"
          :project-path="store.activeSession?.project_path"
          :model="store.activeSession?.model"
          :tokens="store.activeSession?.tokens_used"
          :started-at="store.activeSession?.started_at"
          :target-timestamp="store.activeSession?.target_timestamp"
          :target-content="store.activeSession?.target_content"
          :search-query="store.activeSession?.search_query"
          @close="store.messagesModalOpen = false"
        />

        <SessionResumeModal
          :show="store.resumeModalOpen"
          :platform-id="store.resumeTarget?.platform_id || store.selectedPlatformId"
          :session-id="store.resumeTarget?.id"
          :project-path="store.resumeTarget?.project_path"
          :title="store.resumeTarget?.title"
          @close="store.resumeModalOpen = false"
        />
      </template>
  </div>
</template>

<style scoped>
.session-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
  /* List gutter + column widths, shared by the filter bar and the split body so
     the toolbar's left edge stays on the cards' left edge in both layouts.
     The split minimum (see SPLIT_MIN_WIDTH) is --session-list-min +
     --session-pane-min: 480 + 704 = 1184px of content. */
  --session-list-min: 30rem;
  --session-list-col: 34rem;
  --session-pane-min: 44rem;
  --session-list-pad: 24px;
}
.session-page--split {
  --session-list-pad: 20px;
}
/* The body never scrolls itself: the list (and, in split mode, the reading
   pane) own their scrollbars, so the two panes scroll independently. */
.session-page__body {
  flex: 1;
  min-height: 0;
  display: flex;
}
.session-page__list {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 14px var(--session-list-pad) 24px;
}
/* Split layout: the list starts at its comfortable minimum and grows to
   --session-list-col; the pane keeps at least --session-pane-min and takes the
   rest. Both sides are therefore always usable — the split is only allowed to
   happen once those two minimums fit side by side. */
.session-page__body--split {
  display: grid;
  grid-template-columns: minmax(var(--session-list-min), var(--session-list-col)) minmax(var(--session-pane-min), 1fr);
}
/* Search results keep the pre-split shape: one scrolling column. */
.session-page__body--single {
  display: block;
  overflow-y: auto;
  padding: 14px 24px 24px;
}
/* Session cards. One column while the pane is open (that column is only
   30–34rem wide anyway), a responsive grid whenever the list owns the window:
   one row stretched over 1000px+ wastes exactly the space it just reclaimed.
   auto-fill drops back to a single column as soon as a card would go under
   24rem, which also covers the narrow and search layouts. */
.session-list-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(24rem, 100%), 1fr));
  gap: 6px 12px;
}
/* The list branch's toolbar tracks the list column in every layout: it starts
   on the cards' left edge and uses the full row (the right-hand group — path
   filter, batch actions — needs the room). The search branch has its own bar
   and keeps the centred readable column. */
.ah-filter-bar--list {
  padding-left: var(--session-list-pad);
  padding-right: var(--session-list-pad);
}
.ah-filter-bar--list .ah-filter-bar__inner {
  max-width: none;
  margin-left: 0;
  margin-right: 0;
}
/* No room for two panes: one column, and it fills the window (the list is the
   only thing on screen, so capping it would just move the emptiness inward). */
.ah-view-content--fluid {
  max-width: none;
}
/* Two panes but the reader collapsed the second one: keep a generous measure
   instead of stretching rows across an ultrawide display. */
.ah-view-content--roomy {
  max-width: 72rem;
}

/* ---- Reading pane -------------------------------------------------------- */
.session-preview {
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 12px 18px 16px;
  border-left: 1px solid var(--hairline);
  background: var(--surface);
}
/* Readable measure, hugging the list column: past a certain width a full-bleed
   transcript is as unreadable as a full-bleed list row, so the pane keeps a
   measure and lets the leftover space fall at the window edge instead of
   splitting it into two voids around the text. */
.session-preview__inner {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  width: 100%;
  max-width: 60rem;
  margin-right: auto;
}
.session-preview__head {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 2px 10px;
}
.session-preview__badge {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ink-3);
  white-space: nowrap;
}
.session-preview__title {
  flex: 1;
  min-width: 0;
  margin: 0;
  font-size: 14px;
  font-weight: 500;
  color: var(--ink);
}
.session-preview__actions {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
}
/* Same shape as the Monitor's empty state: icon + one line (+ one hint). */
.session-preview__empty {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--ink-4);
  text-align: center;
}
.session-preview__empty strong { color: var(--ink-2); font-size: 14px; font-weight: 500; }
.session-preview__empty span { font-size: 12px; }

/* Batch-mode confirm chip reuses the card delete styling; it lives outside
   SessionCard so the classes are duplicated here. */
.session-card__delete {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 26px;
  width: 28px;
  padding: 0;
  color: var(--ink-4);
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  cursor: pointer;
  white-space: nowrap;
}
.session-card__delete.is-confirming {
  width: auto;
  padding: 0 10px;
  color: var(--on-accent);
  background: var(--danger);
  border-color: var(--danger);
}

.ah-search-result-card {
  cursor: pointer;
  transition: border-color var(--dur-fast) var(--ease-soft), box-shadow var(--dur-fast) var(--ease-soft);
}
.ah-search-result-card:hover {
  border-color: var(--accent);
}
.ah-search-snippet {
  cursor: pointer;
  transition: border-color var(--dur-fast) var(--ease-soft), background-color var(--dur-fast) var(--ease-soft);
}
.ah-search-snippet:hover {
  border-color: var(--accent);
  filter: brightness(0.97);
}
:root.dark .ah-search-snippet:hover {
  filter: brightness(1.15);
}

/* Session search in action bar */
.ah-session-search {
  position: relative;
  display: flex;
  align-items: center;
  width: 190px;
  flex-shrink: 0;
  transition: width var(--dur-fast) var(--ease-soft);
}

.ah-session-search:focus-within {
  width: 230px;
}

.ah-session-search-icon {
  position: absolute;
  left: 8px;
  color: var(--ink-4);
  pointer-events: none;
  transition: color var(--dur-fast) var(--ease-soft);
}

.ah-session-search:focus-within .ah-session-search-icon {
  color: var(--accent);
}

.ah-session-search-input {
  width: 100%;
  height: 28px;
  padding: 0 24px 0 26px;
  font-size: 12px;
  color: var(--ink);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  outline: none;
  transition: border-color var(--dur-fast) var(--ease-soft),
              box-shadow var(--dur-fast) var(--ease-soft);
}

.ah-session-search-input::placeholder {
  color: var(--ink-4);
  font-size: 12px;
}

.ah-session-search-input:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-soft);
}

.ah-session-search-clear {
  position: absolute;
  right: 5px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--ink-4);
  cursor: pointer;
  transition: color var(--dur-fast) var(--ease-soft), background var(--dur-fast) var(--ease-soft);
}

.ah-session-search-clear:hover {
  color: var(--ink);
  background: var(--hover);
}
</style>
