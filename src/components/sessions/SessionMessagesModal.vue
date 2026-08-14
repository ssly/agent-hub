<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'
import AppModal from '@/components/ui/AppModal.vue'
import AppLoading from '@/components/ui/AppLoading.vue'
import { formatInt, formatSessionTime } from '@/lib/utils'
import * as api from '@/lib/api'

type SessionMsg = {
  role: string
  content: string
  timestamp?: number | string | null
  thinking?: string | null
  system?: string | null
}

type DisplayMsg = {
  role: string
  startedAt?: number | string | null
  timestamp?: number | string | null
  thinking: string
  system: string
  content: string
}

const SYSTEM_REMINDER_RE = /<system-reminder\b[^>]*>[\s\S]*?<\/system-reminder>/gi
const USER_QUERY_RE = /<user_query\b[^>]*>([\s\S]*?)<\/user_query>/i

function splitInjectedContext(text: string): { body: string; system: string } {
  const blocks = text.match(SYSTEM_REMINDER_RE) ?? []
  const rest = text.replace(SYSTEM_REMINDER_RE, '')
  const query = rest.match(USER_QUERY_RE)
  return {
    body: (query ? query[1] : rest).trim(),
    system: blocks.map(block => block.trim()).filter(Boolean).join('\n\n'),
  }
}

function normalizeSessionMsg(msg: SessionMsg): SessionMsg {
  const split = splitInjectedContext(msg.content || '')
  return {
    ...msg,
    content: split.body,
    system: [msg.system, split.system].filter(text => text && text.trim()).join('\n\n') || msg.system,
  }
}

function joinParts(left: string, right: string | null | undefined): string {
  const next = right?.trim() ? right : ''
  if (!next) return left
  return left ? `${left}\n\n${next}` : next
}

// Consecutive assistant replies belong to one turn (tools in between). Fold
// them into a single bubble, separated by a blank line. Consecutive user
// messages stay split — a new prompt after an interrupt is a new bubble.
function groupSessionMessages(list: SessionMsg[]): DisplayMsg[] {
  const groups: DisplayMsg[] = []
  for (const raw of list) {
    const msg = normalizeSessionMsg(raw)
    const last = groups[groups.length - 1]
    if (msg.role === 'assistant' && last?.role === 'assistant') {
      last.timestamp = msg.timestamp
      last.thinking = joinParts(last.thinking, msg.thinking)
      last.system = joinParts(last.system, msg.system)
      last.content = joinParts(last.content, msg.content)
      continue
    }
    groups.push({
      role: msg.role,
      startedAt: msg.timestamp,
      timestamp: msg.timestamp,
      thinking: msg.thinking?.trim() ? msg.thinking : '',
      system: msg.system?.trim() ? msg.system : '',
      content: msg.content || '',
    })
  }
  return groups
}

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
const messages = ref<SessionMsg[]>([])
const displayMessages = computed(() =>
  groupSessionMessages(messages.value).map(msg => ({
    ...msg,
    hint: messageHint(msg, locale.value),
  })),
)

function messageHint(msg: DisplayMsg, loc: string): string {
  const end = msg.timestamp ? formatSessionTime(msg.timestamp, loc) : ''
  const start = msg.startedAt ? formatSessionTime(msg.startedAt, loc) : ''
  if (start && end && start !== end) return `${start} – ${end}`
  return end || start
}
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

watch(
  () => [props.show, props.platformId, props.sessionId] as const,
  ([open, platformId, sessionId]) => {
    if (!open || !platformId || !sessionId) return
    messages.value = []
    offset = 0
    hasMore.value = false
    loadError.value = ''
    loadMessages(false)
  },
)
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
            v-for="(msg, idx) in displayMessages"
            :key="idx"
            class="ah-msg"
            :class="msg.role === 'user' ? 'ah-msg--user' : 'ah-msg--assistant'"
          >
            <div class="ah-msg__stack">
              <div class="ah-msg__bubble">
                <details v-if="msg.system" class="ah-msg__thinking">
                  <summary>
                    <ChevronRight :size="18" stroke-width="2.25" class="ah-msg__thinking-icon" />
                    <span>{{ t('session.system_reminder') }}</span>
                  </summary>
                  <pre class="ah-msg__thinking-body select-text">{{ msg.system }}</pre>
                </details>
                <details v-if="msg.thinking" class="ah-msg__thinking">
                  <summary>
                    <ChevronRight :size="18" stroke-width="2.25" class="ah-msg__thinking-icon" />
                    <span>{{ t('session.thinking') }}</span>
                  </summary>
                  <pre class="ah-msg__thinking-body select-text">{{ msg.thinking }}</pre>
                </details>
                <pre v-if="msg.content" class="ah-msg__content select-text">{{ msg.content }}</pre>
              </div>
              <div v-if="msg.hint" class="ah-msg__hint">{{ msg.hint }}</div>
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
