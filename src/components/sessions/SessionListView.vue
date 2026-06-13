<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSessionsStore } from '@/stores/sessions'
import { formatInt, formatSessionTime } from '@/lib/utils'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import { ref } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'

const { t, locale } = useI18n()
const store = useSessionsStore()
const { showToast } = useToast()
const resumingId = ref<string | null>(null)
const confirmDeleteId = ref<string | null>(null)

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
          <div class="ah-filter-bar">
            <div class="flex items-start justify-between gap-3">
              <div class="text-xs" style="color: var(--ink-2)">
                {{ t('session.loaded_summary', { loaded: formatInt(store.sessions.length), total: formatInt(store.sessionTotal) }) }}
              </div>
              <div class="flex flex-col items-end gap-2">
                <div class="flex items-center gap-2">
                  <span class="text-xs" style="color: var(--ink-3)">{{ t('session.path_filter_label') }}</span>
                  <select
                    :value="store.selectedPathFilter"
                    class="ah-select"
                    @change="store.changePathFilter(($event.target as HTMLSelectElement).value)"
                  >
                    <option v-for="path in store.pathOptions" :key="path" :value="path">
                      {{ path === 'all' ? t('session.path_filter_all') : path === 'unknown' ? t('session.path_filter_unknown') : path }}
                    </option>
                  </select>
                </div>
                <div class="flex items-center gap-2">
                  <span class="text-xs" style="color: var(--ink-3)">{{ t('session.resume_terminal') }}</span>
                  <select
                    :value="store.selectedTerminal"
                    class="ah-select"
                    @change="store.selectedTerminal = ($event.target as HTMLSelectElement).value"
                  >
                    <option
                      v-for="term in (store.terminals.length > 0 ? store.terminals : [{ id: 'terminal-default', display_name: 'Terminal (Default)', available: true }])"
                      :key="term.id"
                      :value="term.id"
                      :disabled="!term.available"
                    >
                      {{ term.display_name }}{{ !term.available ? ` (${t('session.unavailable')})` : '' }}
                    </option>
                  </select>
                </div>
              </div>
            </div>
          </div>

          <!-- Session Cards -->
          <div v-if="store.sessions.length === 0" class="py-8 text-center" style="color: var(--ink-3)">
            {{ store.selectedPathFilter !== 'all' ? t('session.path_filter_empty') : t('session.no_sessions') }}
          </div>

          <div class="space-y-2">
            <div
              v-for="session in store.sessions"
              :key="session.id"
              class="ah-session-card"
            >
              <div class="flex items-center justify-between gap-3">
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
              <div class="mt-2 flex items-center justify-between gap-2">
                <span class="text-xs" style="color: var(--ink-4)">{{ t('session.started_at', { time: formatSessionTime(session.started_at, locale) }) }}</span>
                <div class="flex items-center gap-2">
                  <button class="btn btn-secondary btn-sm" @click="store.openMessages(session)">{{ t('session.view_messages') }}</button>
                  <button
                    class="btn btn-primary btn-sm"
                    :disabled="resumingId === session.id"
                    @click="handleResume(session)"
                  >
                    {{ resumingId === session.id ? t('session.resuming') : t('session.resume') }}
                  </button>
                  <button
                    :class="['btn btn-sm', confirmDeleteId === session.id ? 'btn-danger' : 'btn-secondary']"
                    :style="confirmDeleteId === session.id ? { background: 'var(--danger)', color: '#fff', borderColor: 'var(--danger)' } : {}"
                    @click="handleDelete(session)"
                  >
                    {{ confirmDeleteId === session.id ? t('session.confirm_delete') : t('session.delete') }}
                  </button>
                </div>
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
