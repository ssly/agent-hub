<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowDown, ArrowUp, Check, ChevronRight, Copy, Search, X } from 'lucide-vue-next'
import { marked } from 'marked'
import { useToast } from '@/composables/useToast'
import AppModal from '@/components/ui/AppModal.vue'
import AppLoading from '@/components/ui/AppLoading.vue'
import { formatInt, formatSessionTime } from '@/lib/utils'
import * as api from '@/lib/api'

const { showToast } = useToast()
const copiedIdx = ref<number | null>(null)
let copyTimer: ReturnType<typeof setTimeout> | null = null

async function handleCopyMessage(content: string, idx: number) {
  if (!content) return
  try {
    await navigator.clipboard.writeText(content)
    copiedIdx.value = idx
    if (copyTimer) clearTimeout(copyTimer)
    copyTimer = setTimeout(() => {
      copiedIdx.value = null
    }, 2000)
  } catch (e: any) {
    showToast(t('session.copy_failed', { error: e?.message || e }), 'error')
  }
}

// In-session search state (VS Code Find Widget style)
const searchVisible = ref(false)
const searchQuery = ref('')
const matchCase = ref(false)
const matchWholeWord = ref(false)
const useRegex = ref(false)
const regexError = ref(false)

const searchInputRef = ref<HTMLInputElement | null>(null)
const msgListRef = ref<HTMLElement | null>(null)
const currentMatchIndex = ref(0)
const totalMatches = ref(0)
let matchElements: HTMLElement[] = []

function buildSearchRegex(): RegExp | null {
  const q = searchQuery.value
  if (!q) {
    regexError.value = false
    return null
  }

  let pattern = q
  if (!useRegex.value) {
    pattern = pattern.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  }

  if (matchWholeWord.value) {
    pattern = `\\b(?:${pattern})\\b`
  }

  const flags = matchCase.value ? 'g' : 'gi'

  try {
    const re = new RegExp(pattern, flags)
    regexError.value = false
    return re
  } catch {
    regexError.value = true
    return null
  }
}

function clearHighlights() {
  if (!msgListRef.value) return
  const marks = msgListRef.value.querySelectorAll('mark.ah-search-match')
  marks.forEach(mark => {
    const parent = mark.parentNode
    if (parent) {
      while (mark.firstChild) {
        parent.insertBefore(mark.firstChild, mark)
      }
      parent.removeChild(mark)
    }
  })
  msgListRef.value.normalize()
  matchElements = []
  totalMatches.value = 0
  currentMatchIndex.value = 0
}

function updateActiveMatch() {
  if (matchElements.length === 0) return

  matchElements.forEach((el, idx) => {
    if (idx === currentMatchIndex.value) {
      el.classList.add('ah-search-match--current')
      const details = el.closest('details')
      if (details && !details.open) {
        details.open = true
      }
      el.scrollIntoView({ block: 'center', behavior: 'smooth' })
    } else {
      el.classList.remove('ah-search-match--current')
    }
  })
}

function applyHighlights() {
  clearHighlights()
  const q = searchQuery.value
  if (!q || !msgListRef.value) return

  const regex = buildSearchRegex()
  if (!regex) return

  const walker = document.createTreeWalker(
    msgListRef.value,
    NodeFilter.SHOW_TEXT,
    {
      acceptNode(node) {
        if (!node.nodeValue || !node.nodeValue.trim()) {
          return NodeFilter.FILTER_SKIP
        }
        const parent = node.parentElement
        if (!parent) return NodeFilter.FILTER_SKIP
        if (parent.closest('.ah-vscode-find-widget, .ah-msg__copy-btn, .ah-msg__hint, .ah-msg__thinking-icon')) {
          return NodeFilter.FILTER_REJECT
        }
        regex.lastIndex = 0
        if (regex.test(node.nodeValue)) {
          return NodeFilter.FILTER_ACCEPT
        }
        return NodeFilter.FILTER_SKIP
      },
    },
  )

  const textNodes: Text[] = []
  while (walker.nextNode()) {
    textNodes.push(walker.currentNode as Text)
  }

  for (const textNode of textNodes) {
    const parent = textNode.parentNode
    if (!parent) continue

    const text = textNode.nodeValue || ''
    regex.lastIndex = 0

    let match: RegExpExecArray | null
    let startIndex = 0
    const fragment = document.createDocumentFragment()
    let hasMatch = false

    while ((match = regex.exec(text)) !== null) {
      const matchText = match[0]
      if (matchText.length === 0) {
        regex.lastIndex++
        continue
      }
      hasMatch = true
      const matchIndex = match.index

      if (matchIndex > startIndex) {
        fragment.appendChild(document.createTextNode(text.substring(startIndex, matchIndex)))
      }

      const mark = document.createElement('mark')
      mark.className = 'ah-search-match'
      mark.textContent = matchText
      fragment.appendChild(mark)
      matchElements.push(mark)

      startIndex = matchIndex + matchText.length
    }

    if (hasMatch) {
      if (startIndex < text.length) {
        fragment.appendChild(document.createTextNode(text.substring(startIndex)))
      }
      parent.replaceChild(fragment, textNode)
    }
  }

  totalMatches.value = matchElements.length
  if (totalMatches.value > 0) {
    currentMatchIndex.value = 0
    updateActiveMatch()
  }
}

function findNext() {
  if (totalMatches.value === 0) return
  currentMatchIndex.value = (currentMatchIndex.value + 1) % totalMatches.value
  updateActiveMatch()
}

function findPrev() {
  if (totalMatches.value === 0) return
  currentMatchIndex.value = (currentMatchIndex.value - 1 + totalMatches.value) % totalMatches.value
  updateActiveMatch()
}

function openSearch() {
  searchVisible.value = true
  nextTick(() => {
    searchInputRef.value?.focus()
    searchInputRef.value?.select()
    if (searchQuery.value) {
      applyHighlights()
    }
  })
  if (hasMore.value) {
    loadAllRemainingMessages()
  }
}

function closeSearch() {
  searchVisible.value = false
  searchQuery.value = ''
  regexError.value = false
  clearHighlights()
}

function handleKeyDown(e: KeyboardEvent) {
  if (!props.show) return

  // Cmd+F or Ctrl+F
  if ((e.metaKey || e.ctrlKey) && (e.key === 'f' || e.key === 'F')) {
    e.preventDefault()
    e.stopPropagation()
    if (!searchVisible.value) {
      openSearch()
    } else {
      searchInputRef.value?.focus()
      searchInputRef.value?.select()
    }
    return
  }

  if (!searchVisible.value) return

  // Alt+C: Match Case
  if (e.altKey && (e.code === 'KeyC' || e.key === 'c' || e.key === 'C')) {
    e.preventDefault()
    e.stopPropagation()
    matchCase.value = !matchCase.value
    return
  }

  // Alt+W: Match Whole Word
  if (e.altKey && (e.code === 'KeyW' || e.key === 'w' || e.key === 'W')) {
    e.preventDefault()
    e.stopPropagation()
    matchWholeWord.value = !matchWholeWord.value
    return
  }

  // Alt+R: Use Regular Expression
  if (e.altKey && (e.code === 'KeyR' || e.key === 'r' || e.key === 'R')) {
    e.preventDefault()
    e.stopPropagation()
    useRegex.value = !useRegex.value
    return
  }

  // Esc closes search if search is active
  if (e.key === 'Escape') {
    e.preventDefault()
    e.stopPropagation()
    closeSearch()
  }
}

watch([searchQuery, matchCase, matchWholeWord, useRegex], () => {
  if (searchVisible.value && searchQuery.value && hasMore.value) {
    loadAllRemainingMessages()
  }
  nextTick(() => {
    applyHighlights()
  })
})

marked.setOptions({
  gfm: true,
  breaks: true,
})

function renderMarkdown(content: string): string {
  if (!content) return ''
  try {
    return marked.parse(content) as string
  } catch {
    return content
  }
}

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

const PAGE_SIZE = 30
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
const loadingAll = ref(false)
const hasMore = ref(false)
const loadError = ref('')
let offset = 0

const messagesSentinel = ref<HTMLElement | null>(null)
let messagesObserver: IntersectionObserver | null = null

function setupMessagesObserver() {
  if (messagesObserver) messagesObserver.disconnect()
  if (typeof IntersectionObserver === 'undefined') return

  messagesObserver = new IntersectionObserver(
    entries => {
      if (entries[0]?.isIntersecting && hasMore.value && !loadingMore.value && !loading.value && !loadingAll.value) {
        loadMessages(true)
      }
    },
    { root: msgListRef.value, rootMargin: '300px' },
  )

  if (messagesSentinel.value) {
    messagesObserver.observe(messagesSentinel.value)
  }
}

watch(messagesSentinel, () => {
  setupMessagesObserver()
})

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
    if (searchVisible.value && searchQuery.value.trim()) {
      nextTick(() => {
        applyHighlights()
      })
    }
  } catch (e: any) {
    loadError.value = e?.SyncError || e?.message || String(e)
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

async function loadAllRemainingMessages() {
  if (!props.platformId || !props.sessionId || !hasMore.value || loadingAll.value) return
  loadingAll.value = true
  try {
    while (hasMore.value && props.platformId && props.sessionId) {
      const list = await api.getSessionMessages(props.platformId, props.sessionId, offset, 100)
      messages.value = [...messages.value, ...list]
      offset += list.length
      hasMore.value = list.length === 100
      if (list.length < 100) break
    }
    nextTick(() => {
      if (searchVisible.value && searchQuery.value) {
        applyHighlights()
      }
    })
  } catch (e: any) {
    console.error('Failed to load all messages for search:', e)
  } finally {
    loadingAll.value = false
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
    closeSearch()
    loadMessages(false)
  },
)

watch(
  () => props.show,
  open => {
    if (open) {
      window.addEventListener('keydown', handleKeyDown, true)
    } else {
      window.removeEventListener('keydown', handleKeyDown, true)
      closeSearch()
    }
  },
)

onMounted(() => {
  if (props.show) {
    window.addEventListener('keydown', handleKeyDown, true)
  }
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown, true)
  messagesObserver?.disconnect()
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
      <div class="text-xs pb-3 border-b flex items-center justify-between gap-3 flex-wrap" style="color: var(--ink-3); border-color: var(--hairline)">
        <div class="flex items-center gap-3 flex-wrap min-w-0">
          <span v-if="projectPath" class="truncate">{{ projectPath }}</span>
          <span v-if="model" style="color: var(--accent)">{{ model }}</span>
          <span v-if="tokens != null" style="color: var(--warning)">
            {{ t('session.tokens_value', { count: formatInt(tokens) }) }}
          </span>
          <span v-if="startedAt">{{ t('session.started_at', { time: formatSessionTime(startedAt, locale) }) }}</span>
        </div>
      </div>

      <div class="relative flex-1 min-h-0">
        <!-- Floating search trigger button in top-right when search is closed -->
        <Transition name="fade">
          <button
            v-if="!searchVisible"
            v-tooltip:left="t('session.find_in_messages')"
            type="button"
            class="ah-session-floating-search-btn"
            @click="openSearch"
          >
            <Search :size="13" />
          </button>
        </Transition>

        <!-- Floating in-session search panel (VS Code style) -->
        <Transition name="search-slide">
          <div v-if="searchVisible" class="ah-vscode-find-widget">
            <div class="ah-vscode-find-input-box" :class="{ 'is-invalid': regexError }">
              <input
                ref="searchInputRef"
                v-model="searchQuery"
                type="text"
                class="ah-vscode-find-input focus:outline-none focus:ring-0 focus-visible:outline-none focus-visible:ring-0"
                :placeholder="t('session.find_in_messages_placeholder')"
                @keydown.enter.exact.prevent="findNext"
                @keydown.enter.shift.prevent="findPrev"
                @keydown.esc.prevent="closeSearch"
              />
              <div class="ah-vscode-find-toggles">
                <button
                  type="button"
                  class="ah-vscode-find-toggle"
                  :class="{ 'is-active': matchCase }"
                  :title="t('session.find_match_case')"
                  @click="matchCase = !matchCase"
                >
                  <span>Aa</span>
                </button>
                <button
                  type="button"
                  class="ah-vscode-find-toggle"
                  :class="{ 'is-active': matchWholeWord }"
                  :title="t('session.find_match_whole_word')"
                  @click="matchWholeWord = !matchWholeWord"
                >
                  <span class="underline">ab</span>
                </button>
                <button
                  type="button"
                  class="ah-vscode-find-toggle"
                  :class="{ 'is-active': useRegex }"
                  :title="t('session.find_use_regex')"
                  @click="useRegex = !useRegex"
                >
                  <span>.*</span>
                </button>
              </div>
            </div>

            <span class="ah-vscode-find-counter">
              {{
                loadingAll
                  ? '...'
                  : regexError
                    ? t('session.find_no_results')
                    : totalMatches === 0
                      ? (searchQuery ? t('session.find_no_results') : '')
                      : `${currentMatchIndex + 1} of ${totalMatches}`
              }}
            </span>

            <div class="ah-vscode-find-actions">
              <button
                type="button"
                class="ah-vscode-find-btn"
                :disabled="totalMatches === 0"
                :title="t('session.find_prev')"
                @click="findPrev"
              >
                <ArrowUp :size="14" />
              </button>
              <button
                type="button"
                class="ah-vscode-find-btn"
                :disabled="totalMatches === 0"
                :title="t('session.find_next')"
                @click="findNext"
              >
                <ArrowDown :size="14" />
              </button>
              <button
                type="button"
                class="ah-vscode-find-btn"
                :title="t('action.close')"
                @click="closeSearch"
              >
                <X :size="14" />
              </button>
            </div>
          </div>
        </Transition>

        <div ref="msgListRef" class="ah-msg-list h-full">
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
                  <div v-if="msg.content" class="ah-msg__content select-text" v-html="renderMarkdown(msg.content)"></div>
                </div>
                <div class="ah-msg__meta">
                  <span v-if="msg.hint" class="ah-msg__hint">{{ msg.hint }}</span>
                  <button
                    v-if="msg.content"
                    type="button"
                    class="ah-msg__copy-btn"
                    :title="copiedIdx === idx ? t('action.copied') : t('action.copy')"
                    @click="handleCopyMessage(msg.content, idx)"
                  >
                    <Check v-if="copiedIdx === idx" :size="12" class="text-[color:var(--success)]" />
                    <Copy v-else :size="12" />
                    <span>{{ copiedIdx === idx ? t('action.copied') : t('action.copy') }}</span>
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- Infinite scroll sentinel for messages -->
          <div ref="messagesSentinel" class="py-2 flex justify-center">
            <AppLoading v-if="loadingMore" class="py-2">{{ t('session.loading_more') }}</AppLoading>
          </div>
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
