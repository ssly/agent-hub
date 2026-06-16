<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSessionsStore } from '@/stores/sessions'
import { formatInt, formatSessionTime } from '@/lib/utils'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import { ref, computed } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'
import AppSelect from '@/components/ui/AppSelect.vue'

const { t, locale } = useI18n()
const store = useSessionsStore()
const { showToast } = useToast()
const resumingId = ref<string | null>(null)
const confirmDeleteId = ref<string | null>(null)

// Batch delete: a two-click confirm keeps it consistent with the single-delete
// pattern but uses a dialog-free chip. `confirmBatch` flips to true on first
// click of the delete button; a second click within the window fires bulkDelete.
const confirmBatch = ref(false)
let confirmBatchTimer: ReturnType<typeof setTimeout> | null = null

function armConfirmBatch() {
  confirmBatch.value = true
  if (confirmBatchTimer) clearTimeout(confirmBatchTimer)
  confirmBatchTimer = setTimeout(() => { confirmBatch.value = false }, 3500)
}
function disarmConfirmBatch() {
  confirmBatch.value = false
  if (confirmBatchTimer) { clearTimeout(confirmBatchTimer); confirmBatchTimer = null }
}

async function handleBulkDelete() {
  if (store.selectedCount === 0) {
    showToast(t('session.batch_select_none'), 'error')
    return
  }
  if (!confirmBatch.value) {
    armConfirmBatch()
    return
  }
  disarmConfirmBatch()
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

// Map store data into the { value, label, disabled } shape AppSelect expects.
const pathSelectOptions = computed(() =>
  store.pathOptions.map(p => ({
    value: p,
    label: p === 'all' ? t('session.path_filter_all') : p === 'unknown' ? t('session.path_filter_unknown') : p,
  }))
)
const terminalSelectOptions = computed(() => {
  const list = store.terminals.length > 0
    ? store.terminals
    : [{ id: 'terminal-default', display_name: 'Terminal (Default)', available: true }]
  return list.map((term: any) => ({
    value: term.id,
    label: `${term.display_name}${term.available ? '' : ` (${t('session.unavailable')})`}`,
    disabled: !term.available,
  }))
})

async function handleResume(session: any) {
  resumingId.value = session.id
  try {
    const cmd = await api.resumeSession(
      session.platform_id || store.selectedPlatformId!,
      session.id,
      session.project_path || '',
      store.selectedTerminal
    )
    showToast(t('session.resume_started', { command: cmd }), 'success', 5000)
  } catch (e: any) {
    showToast(t('session.resume_failed', { error: e?.SyncError || e?.message || e }), 'error')
  } finally {
    resumingId.value = null
  }
}

async function handleDelete(session: any) {
  if (confirmDeleteId.value !== session.id) {
    confirmDeleteId.value = session.id
    setTimeout(() => { if (confirmDeleteId.value === session.id) confirmDeleteId.value = null }, 3000)
    return
  }
  confirmDeleteId.value = null
  try {
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
  return escapedText.replace(regex, '<mark class="bg-[#C9A961]/25 text-[var(--ink)] rounded px-0.5 font-medium">$1</mark>')
}

function clearSessionSearch() {
  store.searchQuery = ''
  store.searchResults = []
}
</script>

<template>
  <div class="p-6 view-enter">
    <div class="ah-view-content">
      <div v-if="store.isLoading" class="loading-pulse" style="color: var(--ink-3)">{{ t('session.loading_messages') }}</div>

      <div v-else-if="store.loadError" style="color: var(--danger)">{{ store.loadError }}</div>

      <div v-else-if="!store.selectedPlatformId" class="flex flex-col items-center justify-center py-20">
        <p style="color: var(--ink-3)">{{ t('session.select_platform') }}</p>
      </div>

      <template v-else>
        <!-- Search results view -->
        <template v-if="store.searchQuery">
          <div class="ah-filter-bar flex justify-between items-center gap-3">
            <div class="text-sm font-medium" style="color: var(--ink-2)">
              {{ t('session.search_results', { query: store.searchQuery, count: store.searchResults.length }) }}
            </div>
            <button class="btn btn-secondary btn-sm" @click="clearSessionSearch">
              {{ t('session.clear_search') }}
            </button>
          </div>

          <div v-if="store.isSearching" class="loading-pulse py-12 text-center" style="color: var(--ink-3)">
            {{ t('session.loading_messages') }}
          </div>

          <div v-else-if="store.searchResults.length === 0" class="py-12 text-center" style="color: var(--ink-3)">
            {{ t('session.no_search_results', { query: store.searchQuery }) }}
          </div>

          <div v-else class="space-y-3">
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
                  class="text-xs font-mono whitespace-pre-wrap break-words m-0 leading-relaxed"
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
                  :disabled="resumingId === result.session_id"
                  @click="handleResume({ id: result.session_id, project_path: result.project_path, platform_id: result.platform_id })"
                >
                  {{ resumingId === result.session_id ? t('session.resuming') : t('session.resume') }}
                </button>
              </div>
            </div>
          </div>
        </template>

        <!-- Standard session list view -->
        <template v-else>
          <!-- Filter bar -->
          <div class="ah-filter-bar flex items-center justify-between gap-3 flex-wrap">
            <div class="text-xs whitespace-nowrap" style="color: var(--ink-3)">
              {{ t('session.loaded_summary', { loaded: formatInt(store.sessions.length), total: formatInt(store.sessionTotal) }) }}
            </div>
            <div class="flex items-center gap-4 flex-wrap">
              <div class="flex items-center gap-2">
                <span class="text-xs whitespace-nowrap" style="color: var(--ink-3)">{{ t('session.path_filter_label') }}</span>
                <div class="w-52">
                  <AppSelect
                    :model-value="store.selectedPathFilter"
                    :options="pathSelectOptions"
                    @update:model-value="store.changePathFilter($event)"
                  />
                </div>
              </div>
              <div class="flex items-center gap-2">
                <span class="text-xs whitespace-nowrap" style="color: var(--ink-3)">{{ t('session.resume_terminal') }}</span>
                <div class="w-44">
                  <AppSelect
                    :model-value="store.selectedTerminal"
                    :options="terminalSelectOptions"
                    @update:model-value="store.selectedTerminal = $event"
                  />
                </div>
              </div>
              <button
                class="btn btn-sm"
                :class="store.selectionMode ? 'btn-primary' : 'btn-secondary'"
                @click="store.selectionMode ? store.exitSelection() : store.enterSelection()"
              >
                {{ store.selectionMode ? t('session.batch_cancel') : t('session.batch_select') }}
              </button>
            </div>
          </div>

          <!-- Batch selection toolbar -->
          <div v-if="store.selectionMode" class="ah-batch-bar flex items-center justify-between gap-3 flex-wrap">
            <div class="flex items-center gap-3">
              <span class="text-xs font-medium" style="color: var(--ink-2)">
                {{ t('session.batch_select_none') && store.selectedCount === 0
                  ? t('session.batch_select_none')
                  : `${store.selectedCount} / ${store.sessions.length}` }}
              </span>
              <button class="btn btn-secondary btn-sm" @click="store.selectAllLoaded()">{{ t('session.batch_select_all') }}</button>
              <button class="btn btn-secondary btn-sm" :disabled="store.selectedCount === 0" @click="store.clearSelection()">{{ t('session.batch_clear_selection') }}</button>
            </div>
            <button
              class="btn btn-sm"
              :class="confirmBatch ? 'session-card__delete is-confirming' : 'btn-danger'"
              :style="confirmBatch ? { width: 'auto' } : null"
              :disabled="store.isBulkDeleting || store.selectedCount === 0"
              :title="confirmBatch ? t('session.batch_delete_confirm', { n: store.selectedCount }) : ''"
              @click="handleBulkDelete"
            >
              {{ store.isBulkDeleting
                ? t('session.deleting')
                : confirmBatch
                  ? t('session.confirm_delete')
                  : t('session.batch_delete_n', { n: store.selectedCount }) }}
            </button>
          </div>

          <!-- Session Cards -->
          <div v-if="store.sessions.length === 0" class="py-8 text-center" style="color: var(--ink-3)">
            {{ store.selectedPathFilter !== 'all' ? t('session.path_filter_empty') : t('session.no_sessions') }}
          </div>

          <div class="space-y-2">
            <div
              v-for="session in store.sessions"
              :key="session.id"
              class="ah-session-card session-card"
              :class="{ 'session-card--selected': store.selectionMode && store.selectedIds.has(session.id) }"
            >
              <div class="session-card__head">
                <div class="flex items-center gap-2 min-w-0">
                  <label v-if="store.selectionMode" class="session-card__check" @click.stop>
                    <input
                      type="checkbox"
                      :checked="store.selectedIds.has(session.id)"
                      @change="store.toggleSelected(session.id)"
                    />
                  </label>
                  <h3 class="ah-session-card__title truncate">{{ session.title || t('session.untitled') }}</h3>
                </div>
                <span class="text-xs whitespace-nowrap" style="color: var(--ink-3)">
                  {{ formatSessionTime(session.updated_at, locale) }}
                </span>
              </div>
              <div class="ah-session-card__path">{{ session.project_path || t('session.no_project') }}</div>
              <div v-if="session.model || session.tokens_used != null" class="ah-session-card__meta">
                <span v-if="session.model" class="ah-session-card__model">{{ session.model }}</span>
                <span v-if="session.tokens_used != null" class="ah-session-card__tokens">{{ t('session.tokens_value', { count: formatInt(session.tokens_used) }) }}</span>
              </div>
              <div v-if="!store.selectionMode" class="session-card__actions">
                <button class="btn btn-secondary btn-sm" @click="store.openMessages(session)">{{ t('session.view_messages') }}</button>
                <button
                  class="btn btn-primary btn-sm"
                  :disabled="resumingId === session.id"
                  @click="handleResume(session)"
                >
                  {{ resumingId === session.id ? t('session.resuming') : t('session.resume') }}
                </button>
                <button
                  class="session-card__delete"
                  :class="{ 'is-confirming': confirmDeleteId === session.id }"
                  :title="confirmDeleteId === session.id ? t('session.confirm_delete') : t('session.delete')"
                  @click="handleDelete(session)"
                >
                  <svg
                    v-if="confirmDeleteId !== session.id"
                    width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                    stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
                  >
                    <polyline points="3 6 5 6 21 6" />
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                  </svg>
                  <span v-else>{{ t('session.confirm_delete') }}</span>
                </button>
              </div>
            </div>
          </div>

          <!-- Load more -->
          <div v-if="store.hasMore" class="mt-3 flex justify-center">
            <button
              class="btn btn-secondary btn-sm"
              :disabled="store.loadingMore"
              @click="store.loadMore()"
            >
              {{ store.loadingMore ? t('session.loading_more') : t('session.load_more') }}
            </button>
          </div>
        </template>

        <!-- Messages Modal -->
        <AppModal
          :show="store.messagesModalOpen"
          :title="store.activeSession?.title || t('session.untitled')"
          @close="store.messagesModalOpen = false"
          width-class="w-[48rem]"
        >
          <div v-if="store.activeSession" class="space-y-4">
            <div class="text-xs pb-3 border-b flex gap-3 flex-wrap" style="color: var(--ink-3); border-color: var(--hairline)">
              <span v-if="store.activeSession.project_path">{{ store.activeSession.project_path }}</span>
              <span v-if="store.activeSession.model" style="color: var(--accent)">{{ store.activeSession.model }}</span>
              <span v-if="store.activeSession.tokens_used != null" style="color: var(--warning)">
                {{ t('session.tokens_value', { count: formatInt(store.activeSession.tokens_used) }) }}
              </span>
              <span>{{ t('session.started_at', { time: formatSessionTime(store.activeSession.started_at, locale) }) }}</span>
            </div>

            <div
              class="rounded-lg border p-4 space-y-4 max-h-[50vh] overflow-y-auto"
              style="background: var(--sunken); border-color: var(--border)"
            >
              <div v-if="store.messagesLoading" class="loading-pulse text-center py-8" style="color: var(--ink-3)">
                {{ t('session.loading_messages') }}
              </div>
              <div v-else-if="store.messages.length === 0" class="text-center py-8" style="color: var(--ink-3)">
                {{ t('session.no_messages') }}
              </div>
              <template v-else>
                <div
                  v-for="(msg, idx) in store.messages"
                  :key="idx"
                  class="flex w-full"
                  :class="msg.role === 'user' ? 'justify-end' : 'justify-start'"
                >
                  <div
                    class="rounded-lg border p-3 flex flex-col gap-1 max-w-[85%] relative group transition-all"
                    :style="msg.role === 'user' 
                      ? { background: 'var(--accent-soft)', borderColor: 'var(--accent-mid)', borderRadius: '12px 12px 0px 12px' } 
                      : { background: 'var(--surface)', borderColor: 'var(--hairline)', borderRadius: '12px 12px 12px 0px' }"
                  >
                    <div class="flex items-center justify-between gap-6 mb-1">
                      <span
                        class="text-[10px] font-semibold tracking-wider uppercase"
                        :style="{ color: msg.role === 'user' ? 'var(--accent)' : 'var(--success)' }"
                      >
                        {{ msg.role === 'user' ? t('session.role_user') : t('session.role_assistant') }}
                      </span>
                      <span class="text-[9px]" style="color: var(--ink-4)">
                        {{ msg.timestamp ? formatSessionTime(msg.timestamp, locale) : '' }}
                      </span>
                    </div>
                    <pre class="text-sm font-sans whitespace-pre-wrap break-words m-0 leading-relaxed" style="color: var(--ink)">{{ msg.content || '' }}</pre>
                  </div>
                </div>
              </template>

              <!-- Load more messages button -->
              <div v-if="store.messagesHasMore && store.messages.length > 0" class="flex justify-center pt-2">
                <button
                  class="btn btn-secondary btn-sm"
                  :disabled="store.messagesLoadingMore"
                  @click="store.loadMessages(true)"
                >
                  {{ store.messagesLoadingMore ? t('session.loading_more') : t('session.load_more') }}
                </button>
              </div>
            </div>
          </div>
          <template #footer>
            <button class="btn btn-secondary" @click="store.messagesModalOpen = false">{{ t('action.close') }}</button>
          </template>
        </AppModal>
      </template>
    </div>
  </div>
</template>

<style scoped>
.session-card {
  position: relative;
  overflow: hidden;
}
.session-card::before {
  content: '';
  position: absolute;
  left: 0;
  top: 12px;
  bottom: 12px;
  width: 3px;
  border-radius: 0 2px 2px 0;
  background: transparent;
  transition: background var(--dur-fast) var(--ease-soft);
}
.session-card:hover::before {
  background: var(--accent);
}
.session-card__head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 4px;
}
.session-card__actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 10px;
}
/* Delete: quiet icon by default, turns into a red confirm chip on first click.
   Kept self-contained (not using .btn) so the confirming state is fully controlled. */
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
  transition: color var(--dur-fast) var(--ease-soft), background var(--dur-fast) var(--ease-soft),
    border-color var(--dur-fast) var(--ease-soft), width var(--dur-fast) var(--ease-soft),
    padding var(--dur-fast) var(--ease-soft);
}
.session-card__delete:hover {
  color: var(--danger);
  background: var(--danger-soft);
}
.session-card__delete.is-confirming {
  width: auto;
  padding: 0 10px;
  color: #fff;
  background: var(--danger);
  border-color: var(--danger);
}
/* Batch selection toolbar — mirrors the filter bar spacing. */
.ah-batch-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  padding: 8px 12px;
  margin-bottom: 8px;
  border-radius: var(--radius);
  background: var(--accent-soft);
  border: 1px solid var(--accent-mid);
}
.session-card--selected {
  background: var(--accent-soft);
  border-color: var(--accent-mid);
}
.session-card--selected::before {
  background: var(--accent);
}
.session-card__check {
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  cursor: pointer;
}
.session-card__check input {
  width: 15px;
  height: 15px;
  cursor: pointer;
  accent-color: var(--accent);
}
</style>
