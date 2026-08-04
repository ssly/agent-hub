<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import AppModal from '@/components/ui/AppModal.vue'
import AppLoading from '@/components/ui/AppLoading.vue'
import { formatInt, formatSessionTime } from '@/lib/utils'
import * as api from '@/lib/api'

// Self-fetching messages modal shared by the Sessions browser and the Monitor.
// Parents only hand over the platform/session identity; paging loads through
// the sessions backend adapter (get_session_messages) either way.
const props = defineProps<{
  show: boolean
  platformId?: string | null
  sessionId?: string | null
  title?: string
  projectPath?: string | null
  model?: string | null
  tokens?: number | null
  startedAt?: number | string | null
}>()

const emit = defineEmits<{ close: [] }>()

const { t, locale } = useI18n()

const PAGE_SIZE = 50
const messages = ref<any[]>([])
const loading = ref(false)
const loadingMore = ref(false)
const hasMore = ref(false)
const loadError = ref('')
let offset = 0

async function loadMessages(append: boolean) {
  if (!props.platformId || !props.sessionId) return
  if (append) loadingMore.value = true
  else loading.value = true
  try {
    const list = await api.getSessionMessages(props.platformId, props.sessionId, offset, PAGE_SIZE)
    messages.value = append ? [...messages.value, ...list] : list
    offset += list.length
    hasMore.value = list.length === PAGE_SIZE
    loadError.value = ''
  } catch (e: any) {
    loadError.value = e?.SyncError || e?.message || String(e)
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

watch(() => props.show, open => {
  if (!open) return
  messages.value = []
  offset = 0
  hasMore.value = false
  loadError.value = ''
  loadMessages(false)
})
</script>

<template>
  <AppModal
    :show="show"
    :title="title || t('session.untitled')"
    width-class="w-[94vw]"
    fill-height
    @close="emit('close')"
  >
    <div class="flex flex-col flex-1 min-h-0 gap-3">
      <div class="text-xs pb-3 border-b flex gap-3 flex-wrap" style="color: var(--ink-3); border-color: var(--hairline)">
        <span v-if="projectPath">{{ projectPath }}</span>
        <span v-if="model" style="color: var(--accent)">{{ model }}</span>
        <span v-if="tokens != null" style="color: var(--warning)">
          {{ t('session.tokens_value', { count: formatInt(tokens) }) }}
        </span>
        <span v-if="startedAt">{{ t('session.started_at', { time: formatSessionTime(startedAt, locale) }) }}</span>
      </div>

      <div class="ah-msg-list flex-1 min-h-0">
        <AppLoading v-if="loading" class="py-8">{{ t('session.loading_messages') }}</AppLoading>
        <div v-else-if="loadError" class="text-center py-8" style="color: var(--danger)">
          {{ loadError }}
        </div>
        <div v-else-if="messages.length === 0" class="text-center py-8" style="color: var(--ink-3)">
          {{ t('session.no_messages') }}
        </div>
        <template v-else>
          <div
            v-for="(msg, idx) in messages"
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

        <div v-if="hasMore && messages.length > 0" class="flex justify-center pt-2">
          <button
            class="btn btn-secondary btn-sm"
            :disabled="loadingMore"
            @click="loadMessages(true)"
          >
            {{ loadingMore ? t('session.loading_more') : t('session.load_more') }}
          </button>
        </div>
      </div>
    </div>
    <template #footer>
      <button class="btn btn-secondary" @click="emit('close')">{{ t('action.close') }}</button>
    </template>
  </AppModal>
</template>

<style scoped>
/* The modal runs in fill mode (fixed height, non-scrolling body); the
   message list flexes into the remaining space and owns the only scrollbar. */
</style>
