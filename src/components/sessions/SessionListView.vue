<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSessionsStore } from '@/stores/sessions'
import { formatInt, formatSessionTime } from '@/lib/utils'
import { useToast } from '@/composables/useToast'
import { useHoverResetBool } from '@/composables/useHoverReset'
import * as api from '@/lib/api'
import { computed } from 'vue'
import AppSelect from '@/components/ui/AppSelect.vue'
import AppLoading from '@/components/ui/AppLoading.vue'
import SessionCard from '@/components/sessions/SessionCard.vue'
import SessionMessagesModal from '@/components/sessions/SessionMessagesModal.vue'
import SessionResumeModal from '@/components/sessions/SessionResumeModal.vue'

const { t, locale } = useI18n()
const store = useSessionsStore()
const { showToast } = useToast()

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

/** Display name for the card's agent badge, resolved from the platforms list. */
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
      <AppLoading v-if="store.isLoading" class="py-16">{{ t('session.loading_messages') }}</AppLoading>

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

          <AppLoading v-if="store.isSearching" class="py-12">{{ t('session.loading_messages') }}</AppLoading>

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
            <SessionCard
              v-for="session in store.sessions"
              :key="session.id"
              :badge="sessionBadge(session)"
              :badge-agent-id="sessionBadgeAgentId(session)"
              :badge-icon="sessionBadgeIcon(session)"
              :time="formatSessionTime(session.updated_at, locale)"
              :title="session.title || t('session.untitled')"
              :subtitle="session.project_path || t('session.no_project')"
              :model="session.model"
              :tokens="session.tokens_used"
              :selecting="store.selectionMode"
              :selected="store.selectedIds.has(session.id)"
              @open="store.openMessages(session)"
              @resume="store.openResume(session)"
              @delete="handleDelete(session)"
              @toggle-select="store.toggleSelected(session.id)"
            />
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
  </div>
</template>

<style scoped>
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
