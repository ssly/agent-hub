<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSessionsStore } from '@/stores/sessions'
import { formatInt, formatSessionTime } from '@/lib/utils'
import { useToast } from '@/composables/useToast'
import { useHoverResetBool } from '@/composables/useHoverReset'
import * as api from '@/lib/api'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import AppLoading from '@/components/ui/AppLoading.vue'
import { Folder } from 'lucide-vue-next'
import AppSelect from '@/components/ui/AppSelect.vue'
import SessionCard from '@/components/sessions/SessionCard.vue'
import SessionMessagesModal from '@/components/sessions/SessionMessagesModal.vue'
import SessionResumeModal from '@/components/sessions/SessionResumeModal.vue'

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
  window.addEventListener('keydown', onKeydown)
})

watch([loadMoreSentinel, listBody], () => {
  setupObserver()
})

onUnmounted(() => {
  observer?.disconnect()
  window.removeEventListener('keydown', onKeydown)
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

function clearSessionSearch() {
  store.searchQuery = ''
  store.searchResults = []
}
</script>

<template>
  <div class="session-page view-enter">
      <AppLoading v-if="store.isLoading" class="py-16">{{ t('session.loading_messages') }}</AppLoading>

      <div v-else-if="store.loadError" class="p-6" style="color: var(--danger)">{{ store.loadError }}</div>

      <div v-else-if="!store.selectedPlatformId" class="flex flex-col items-center justify-center py-20">
        <p style="color: var(--ink-3)">{{ t('session.select_platform') }}</p>
      </div>

      <template v-else>
        <!-- Search results view -->
        <template v-if="store.searchQuery">
          <div class="ah-filter-bar">
            <div class="ah-filter-bar__inner flex justify-between items-center gap-3">
            <div class="text-sm font-medium" style="color: var(--ink-2)">
              {{ t('session.search_results', { query: store.searchQuery, count: store.searchResults.length }) }}
            </div>
            <button class="btn btn-secondary btn-sm" @click="clearSessionSearch">
              {{ t('session.clear_search') }}
            </button>
            </div>
          </div>

          <div class="session-page__body">
          <div class="ah-view-content">
          <AppLoading v-if="store.isSearching" class="py-12">{{ t('session.loading_messages') }}</AppLoading>

          <div v-else-if="store.searchResults.length === 0" class="py-12 text-center" style="color: var(--ink-3)">
            {{ t('session.no_search_results', { query: store.searchQuery }) }}
          </div>

          <div v-else class="space-y-2">
            <div
              v-for="(result, index) in store.searchResults"
              :key="index"
              class="ah-session-card flex flex-col gap-2"
            >
              <div class="flex items-center justify-between gap-3 border-b pb-2" style="border-color: var(--hairline)">
                <div>
                  <span class="text-xs" style="color: var(--ink-3)">{{ t('session.search_match_in') }}</span>
                  <span class="text-sm font-semibold" style="color: var(--ink)">{{ result.session_title || t('session.untitled') }}</span>
                </div>
                <span class="text-[10px]" style="color: var(--ink-4)">
                  {{ result.message.timestamp ? formatSessionTime(result.message.timestamp, locale) : '' }}
                </span>
              </div>

              <div class="text-xs truncate" style="color: var(--ink-3)">
                {{ result.project_path || t('session.no_project') }}
              </div>

              <div
                class="rounded p-3 flex flex-col gap-1.5 border"
                :style="result.message.role === 'user'
                  ? { background: 'var(--accent-soft)', borderColor: 'var(--accent-mid)' }
                  : { background: 'var(--surface)', borderColor: 'var(--hairline)' }"
              >
                <div class="flex items-center justify-between">
                  <span
                    class="text-xs font-semibold"
                    :style="{ color: result.message.role === 'user' ? 'var(--accent)' : 'var(--success)' }"
                  >
                    {{ result.message.role === 'user' ? t('session.role_user') : t('session.role_assistant') }}
                  </span>
                </div>
                <pre
                  class="text-xs font-mono whitespace-pre-wrap break-words m-0 leading-relaxed select-text"
                  style="color: var(--ink)"
                  v-html="highlightText(result.message.content, store.searchQuery)"
                ></pre>
              </div>

              <div class="mt-1 flex justify-end gap-2">
                <button
                  class="btn btn-secondary btn-sm"
                  @click="store.openMessages({ id: result.session_id, title: result.session_title, project_path: result.project_path, platform_id: result.platform_id })"
                >
                  {{ t('session.view_messages') }}
                </button>
                <button
                  class="btn btn-primary btn-sm"
                  @click="store.openResume({ id: result.session_id, title: result.session_title, project_path: result.project_path, platform_id: result.platform_id })"
                >
                  {{ t('session.resume') }}
                </button>
              </div>
            </div>
          </div>
          </div>
          </div>
        </template>

        <!-- Standard session list view -->
        <template v-else>
          <div
            class="ah-filter-bar"
            :class="{ 'ah-filter-bar--selecting': store.selectedCount > 0 }"
          >
            <div class="ah-filter-bar__inner flex items-center justify-between gap-3">
            <div class="flex items-center gap-2 min-w-0">
              <span v-if="store.selectedCount > 0" class="ah-filter-bar__count">
                {{ t('session.selected_count', { n: formatInt(store.selectedCount) }) }}
              </span>
              <span class="ah-filter-bar__stats">
                {{ t('session.loaded_summary', { loaded: formatInt(store.sessions.length), total: formatInt(store.sessionTotal) }) }}
              </span>
            </div>
            <div class="flex items-center gap-2 flex-none">
              <div
                v-if="store.selectedCount === 0 && !store.directoryFilter && pathSelectOptions.length > 2"
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
            </div>
            </div>
          </div>

          <div ref="listBody" class="session-page__body">
          <div class="ah-view-content">
          <!-- Session Cards -->
          <div v-if="store.sessions.length === 0" class="py-8 text-center" style="color: var(--ink-3)">
            {{ store.directoryFilter ? t('session.path_filter_empty') : t('session.no_sessions') }}
          </div>

          <div class="space-y-1.5">
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
              @open="store.openMessages(session)"
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
}
.session-page__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 14px 24px 24px;
}
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
</style>
