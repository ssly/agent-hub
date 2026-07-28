<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSessionsStore } from '@/stores/sessions'
import { formatInt, formatSessionTime } from '@/lib/utils'
import { useToast } from '@/composables/useToast'
import { useHoverResetId, useHoverResetBool } from '@/composables/useHoverReset'
import * as api from '@/lib/api'
import { ref, computed } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'
import AppSelect from '@/components/ui/AppSelect.vue'

const { t, locale } = useI18n()
const store = useSessionsStore()
const { showToast } = useToast()
const { armedId: confirmDeleteId, arm: armDelete, reset: resetDelete } = useHoverResetId()

// Batch delete uses the same two-step confirm pattern as single delete: first
// click arms the chip, a second click fires bulkDelete. The chip disarms as
// soon as the pointer leaves the button.
const { armed: confirmBatch, arm: armBatch, reset: resetBatch } = useHoverResetBool()

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

// Map store data into the { value, label, disabled } shape AppSelect expects.
const pathSelectOptions = computed(() =>
  store.pathOptions.map(p => ({
    value: p,
    label: p === 'all' ? t('session.path_filter_all') : p === 'unknown' ? t('session.path_filter_unknown') : p,
  }))
)

const resumeCommandCopied = ref(false)

async function copyResumeCommand() {
  const command = store.resumePreview?.command
  if (!command) return
  try {
    await navigator.clipboard.writeText(command)
    resumeCommandCopied.value = true
    showToast(t('action.copied'), 'success')
    setTimeout(() => { resumeCommandCopied.value = false }, 2000)
  } catch (e: any) {
    showToast(t('session.copy_failed', { error: e?.message || e }), 'error')
  }
}

async function handleDelete(session: any) {
  if (confirmDeleteId.value !== session.id) {
    armDelete(session.id)
    return
  }
  resetDelete()
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
  return escapedText.replace(regex, '<mark class="ah-mark">$1</mark>')
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
        </template>

        <!-- Standard session list view -->
        <template v-else>
          <!-- Toolbar: filter controls in normal mode, morphs into batch
               actions in selection mode (single contextual bar, no stacked strips) -->
          <div
            class="ah-filter-bar flex items-center justify-between gap-3 flex-wrap"
            :class="{ 'ah-filter-bar--selecting': store.selectionMode }"
          >
            <template v-if="!store.selectionMode">
              <span class="ah-filter-bar__stats">
                {{ t('session.loaded_summary', { loaded: formatInt(store.sessions.length), total: formatInt(store.sessionTotal) }) }}
              </span>
              <div class="flex items-center gap-3 flex-wrap">
                <div class="flex items-center gap-1.5">
                  <span class="ah-filter-bar__label">{{ t('session.path_filter_label') }}</span>
                  <div class="w-72">
                    <AppSelect
                      :model-value="store.selectedPathFilter"
                      :options="pathSelectOptions"
                      @update:model-value="store.changePathFilter($event)"
                    />
                  </div>
                </div>
                <button class="btn btn-secondary btn-sm" @click="store.enterSelection()">
                  {{ t('session.batch_select') }}
                </button>
              </div>
            </template>
            <template v-else>
              <div class="flex items-center gap-1">
                <span class="ah-filter-bar__count">
                  {{ store.selectedCount === 0
                    ? t('session.batch_select_none')
                    : `${store.selectedCount} / ${store.sessions.length}` }}
                </span>
                <button class="btn btn-ghost btn-sm" @click="store.selectAllLoaded()">{{ t('session.batch_select_all') }}</button>
                <button class="btn btn-ghost btn-sm" :disabled="store.selectedCount === 0" @click="store.clearSelection()">{{ t('session.batch_clear_selection') }}</button>
              </div>
              <div class="flex items-center gap-2">
                <button
                  class="btn btn-primary btn-sm"
                  :disabled="store.isBulkExporting || store.isBulkDeleting || store.selectedCount === 0"
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
                  :disabled="store.isBulkDeleting || store.isBulkExporting || store.selectedCount === 0"
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
                <span class="ah-filter-bar__divider" />
                <button class="btn btn-secondary btn-sm" @click="store.exitSelection()">
                  {{ t('session.batch_cancel') }}
                </button>
              </div>
            </template>
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
              :class="{
                'session-card--selecting': store.selectionMode,
                'session-card--selected': store.selectionMode && store.selectedIds.has(session.id),
              }"
              @click="store.selectionMode && store.toggleSelected(session.id)"
            >
              <!-- Selected marker: accent triangle ribbon in the top-left corner -->
              <div
                v-if="store.selectionMode && store.selectedIds.has(session.id)"
                class="session-card__corner"
              >
                <svg
                  width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                  stroke-width="4" stroke-linecap="round" stroke-linejoin="round"
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              </div>
              <div class="session-card__head">
                <h3 class="ah-session-card__title truncate">{{ session.title || t('session.untitled') }}</h3>
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
                  @click="store.openResume(session)"
                >
                  {{ t('session.resume') }}
                </button>
                <button
                  class="session-card__delete"
                  :class="{ 'is-confirming': confirmDeleteId === session.id }"
                  :title="confirmDeleteId === session.id ? t('session.confirm_delete') : t('session.delete')"
                  @click="handleDelete(session)"
                  @mouseleave="resetDelete()"
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

            <div class="ah-msg-list">
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
                  class="ah-msg"
                  :class="msg.role === 'user' ? 'ah-msg--user' : 'ah-msg--assistant'"
                >
                  <div class="ah-msg__bubble">
                    <div class="ah-msg__meta">
                      <span class="ah-msg__role">
                        {{ msg.role === 'user' ? t('session.role_user') : t('session.role_assistant') }}
                      </span>
                      <span class="ah-msg__time">
                        {{ msg.timestamp ? formatSessionTime(msg.timestamp, locale) : '' }}
                      </span>
                    </div>
                    <pre class="ah-msg__content select-text">{{ msg.content || '' }}</pre>
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

        <!-- Resume Modal: copy-command flow (terminal launcher removed) -->
        <AppModal
          :show="store.resumeModalOpen"
          :title="t('session.resume')"
          @close="store.resumeModalOpen = false"
          width-class="w-[36rem]"
        >
          <div v-if="store.resumeLoading" class="loading-pulse text-center py-8" style="color: var(--ink-3)">
            {{ t('session.loading_messages') }}
          </div>
          <div v-else-if="store.resumeError" class="py-8 text-center" style="color: var(--danger)">
            {{ t('session.resume_failed', { error: store.resumeError }) }}
          </div>
          <div v-else-if="store.resumePreview" class="flex flex-col gap-2.5">
            <div class="ah-resume-row">
              <span class="ah-resume-row__label">{{ t('session.resume_field_title') }}</span>
              <span class="ah-resume-row__value select-text">
                {{ store.resumeTarget?.title || t('session.untitled') }}
              </span>
            </div>
            <div class="ah-resume-row">
              <span class="ah-resume-row__label">{{ t('session.resume_last_question') }}</span>
              <span class="ah-resume-row__value select-text">
                {{ store.resumePreview.last_user_message || t('session.resume_empty') }}
              </span>
            </div>
            <div class="ah-resume-row">
              <span class="ah-resume-row__label">{{ t('session.resume_last_answer') }}</span>
              <span class="ah-resume-row__value select-text">
                {{ store.resumePreview.last_assistant_message || t('session.resume_empty') }}
              </span>
            </div>
            <div class="ah-resume-row ah-resume-row--command">
              <div class="flex items-center justify-between gap-2">
                <span class="ah-resume-row__label">{{ t('session.resume_command_label') }}</span>
                <span class="ah-resume-row__hint">{{ t('session.resume_command_hint') }}</span>
              </div>
              <div class="ah-resume-command">
                <code class="ah-resume-command__text select-text">{{ store.resumePreview.command }}</code>
                <button class="btn btn-secondary btn-sm shrink-0" @click="copyResumeCommand">
                  {{ resumeCommandCopied ? t('action.copied') : t('action.copy') }}
                </button>
              </div>
            </div>
          </div>
          <template #footer>
            <button class="btn btn-secondary" @click="store.resumeModalOpen = false">{{ t('action.close') }}</button>
          </template>
        </AppModal>
      </template>
    </div>
  </div>
</template>

<style scoped>
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
/* Resume modal rows: fixed-width label + single-line truncated value */
.ah-resume-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}
.ah-resume-row--command {
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  margin-top: 4px;
}
.ah-resume-row__label {
  flex-shrink: 0;
  font-size: 12px;
  font-weight: 600;
  color: var(--ink-2);
  white-space: nowrap;
}
.ah-resume-row__hint {
  font-size: 11px;
  color: var(--ink-4);
  white-space: nowrap;
}
.ah-resume-row__value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
  color: var(--ink);
}
.ah-resume-command {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 10px;
  background: var(--sunken);
  border: 1px solid var(--hairline);
  border-radius: var(--radius-sm);
}
.ah-resume-command__text {
  min-width: 0;
  flex: 1;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  line-height: 1.6;
  color: var(--ink);
  white-space: pre-wrap;
  word-break: break-all;
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
  color: var(--on-accent);
  background: var(--danger);
  border-color: var(--danger);
}
/* Selection mode: the whole card is the toggle, so it gets pointer cursor and
   relative+hidden to host the corner ribbon without leaking past the radius. */
.session-card--selecting {
  position: relative;
  overflow: hidden;
  cursor: pointer;
  user-select: none;
}
.session-card--selected {
  background: var(--accent-soft);
  border-color: var(--accent-mid);
}
/* Top-left accent triangle marking a selected card, with a small check glyph.
   Drawn with clip-path on a real box (not border triangles) so the glyph's
   containing block is the corner square itself. */
.session-card__corner {
  position: absolute;
  top: 0;
  left: 0;
  width: 26px;
  height: 26px;
  background: var(--accent);
  clip-path: polygon(0 0, 100% 0, 0 100%);
  pointer-events: none;
}
.session-card__corner svg {
  position: absolute;
  top: 3px;
  left: 3px;
  color: var(--on-accent);
}
</style>
