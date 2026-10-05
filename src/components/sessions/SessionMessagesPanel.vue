<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowDown, ArrowUp, Check, ChevronRight, Copy, Search, X } from 'lucide-vue-next'
import { marked } from 'marked'
import { useToast } from '@/composables/useToast'
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
  // Unwrapping a mark re-splits the text node it lived in, so the pieces are
  // merged back — but ONLY inside the elements that actually held a mark.
  // msgListRef is a Vue-owned subtree: normalizing the whole list would also
  // merge the framework's own anchor text nodes (the ones it inserts around
  // each fragment), and the next patch that unmounts such a fragment would
  // then throw "Cannot read properties of null (reading 'nextSibling')" and
  // abort the update. Mark parents come from v-html / text interpolations,
  // where merging adjacent text nodes is invisible to the renderer.
  const touched = new Set<Node>()
  marks.forEach(mark => {
    const parent = mark.parentNode
    if (parent) {
      while (mark.firstChild) {
        parent.insertBefore(mark.firstChild, mark)
      }
      parent.removeChild(mark)
      touched.add(parent)
    }
  })
  touched.forEach(node => (node as Element).normalize?.())
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
  if (!props.active) return

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

// Rendered markdown is cached per body: the template calls this for every bubble
// on every re-render (a new page, the tally arriving, the find widget opening),
// and real transcripts carry single messages of a megabyte or more — re-parsing
// those on each pass is pure main-thread jank.
const markdownCache = new Map<string, string>()

function renderMarkdown(content: string): string {
  if (!content) return ''
  const cached = markdownCache.get(content)
  if (cached !== undefined) return cached
  let html: string
  try {
    html = marked.parse(content) as string
  } catch {
    html = content
  }
  // A session's worth of messages is plenty; drop everything past that rather
  // than growing without bound while browsing many sessions.
  if (markdownCache.size > 300) markdownCache.clear()
  markdownCache.set(content, html)
  return html
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
  const next = right?.trim() ? right.trim() : ''
  const prev = left?.trim() ? left.trim() : ''
  if (!next) return prev
  return prev ? `${prev}\n\n${next}` : next
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
      thinking: msg.thinking?.trim() || '',
      system: msg.system?.trim() || '',
      content: msg.content?.trim() || '',
    })
  }
  return groups
}

// Self-fetching message body with no modal chrome of its own. Two hosts share
// it — SessionMessagesModal (narrow windows, Monitor, search results) and the
// wide-screen preview pane of the Sessions browser — so message rendering, the
// in-session find widget, paging and copy live here exactly once. Hosts pass
// the platform/session identity plus the meta line; `active` says whether the
// panel is on screen, which drives the ⌘F listener and the first load.
//
// The message tally (total / AI / me) is fetched here too, because this is the
// only place that knows the identity, and handed to the host through `stats`:
// the modal prints it next to its Close button, the pane keeps it in the meta
// row below (`metaStats` off tells the panel not to render it itself).
const props = defineProps<{
  active: boolean
  platformId?: string | null
  sessionId?: string | null
  projectPath?: string | null
  model?: string | null
  tokens?: number | null
  startedAt?: number | string | null
  targetTimestamp?: number | string | null
  targetContent?: string | null
  searchQuery?: string | null
  metaStats?: boolean
}>()

const emit = defineEmits<{ stats: [stats: api.SessionMessageStats | null] }>()

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
// Whole-session tally, counted by the backend (it pages the transcript the same
// way this panel does, so the numbers match the bubbles below).
const messageStats = ref<api.SessionMessageStats | null>(null)
let offset = 0

const messagesSentinel = ref<HTMLElement | null>(null)
let messagesObserver: IntersectionObserver | null = null

function setupMessagesObserver() {
  if (messagesObserver) messagesObserver.disconnect()
  if (typeof IntersectionObserver === 'undefined') return

  messagesObserver = new IntersectionObserver(
    entries => {
      if (entries[0]?.isIntersecting && hasMore.value && !loadingMore.value && !loading.value && !loadingAll.value) {
        loadMessages()
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

let autoScrollPinned = false
let pinTimer: ReturnType<typeof setTimeout> | null = null
let contentObserver: MutationObserver | null = null

function scrollToBottomInstant() {
  if (!msgListRef.value) return
  msgListRef.value.scrollTop = msgListRef.value.scrollHeight
}

function scrollToBottom() {
  autoScrollPinned = true
  if (pinTimer) clearTimeout(pinTimer)
  pinTimer = setTimeout(() => {
    autoScrollPinned = false
  }, 600)

  nextTick(() => {
    scrollToBottomInstant()
    requestAnimationFrame(() => {
      scrollToBottomInstant()
    })
  })
}

function handleListScroll() {
  if (!msgListRef.value) return
  const { scrollTop, scrollHeight, clientHeight } = msgListRef.value
  if (scrollHeight - scrollTop - clientHeight > 80) {
    autoScrollPinned = false
  }
}

function isNearBottom(): boolean {
  if (!msgListRef.value) return true
  const { scrollTop, scrollHeight, clientHeight } = msgListRef.value
  return scrollHeight - scrollTop - clientHeight < 120
}

function setupContentObserver() {
  if (typeof MutationObserver === 'undefined' || !msgListRef.value) return
  if (contentObserver) contentObserver.disconnect()
  contentObserver = new MutationObserver(() => {
    if (autoScrollPinned && msgListRef.value) {
      scrollToBottomInstant()
    }
  })
  contentObserver.observe(msgListRef.value, { childList: true, subtree: true })
}

watch(msgListRef, el => {
  if (el) {
    setupMessagesObserver()
    setupContentObserver()
  }
})

async function loadMessages() {
  if (!props.platformId || !props.sessionId) return
  loading.value = true
  loadError.value = ''
  try {
    const all: SessionMsg[] = []
    let curOffset = 0
    const BATCH_SIZE = 500
    while (props.platformId && props.sessionId) {
      const list = await api.getSessionMessages(props.platformId, props.sessionId, curOffset, BATCH_SIZE)
      all.push(...list)
      if (list.length < BATCH_SIZE) break
      curOffset += list.length
      if (curOffset >= 5000) break
    }
    messages.value = all
    offset = all.length
    hasMore.value = false
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
    nextTick(() => {
      handlePostLoadNavigation()
    })
  }
}

function findTargetIndex(): number {
  const list = displayMessages.value
  if (props.targetTimestamp != null) {
    const tsNum = typeof props.targetTimestamp === 'string' ? Number(props.targetTimestamp) : props.targetTimestamp
    const idx = list.findIndex(
      m => (m.timestamp && Number(m.timestamp) === tsNum) || (m.startedAt && Number(m.startedAt) === tsNum),
    )
    if (idx !== -1) return idx
  }
  if (props.targetContent) {
    const clean = props.targetContent.trim().toLowerCase()
    if (clean) {
      const sub = clean.slice(0, 50)
      const idx = list.findIndex(
        m =>
          m.content?.toLowerCase().includes(sub) ||
          m.thinking?.toLowerCase().includes(sub) ||
          m.system?.toLowerCase().includes(sub),
      )
      if (idx !== -1) return idx
    }
  }
  return -1
}

function locateTargetElement(targetIdx: number) {
  autoScrollPinned = false
  if (pinTimer) {
    clearTimeout(pinTimer)
    pinTimer = null
  }
  if (!msgListRef.value) return
  const msgEls = msgListRef.value.querySelectorAll<HTMLElement>('.ah-msg')
  const el = msgEls[targetIdx]
  if (!el) return

  // If match is inside thinking or system details, expand them
  if (props.searchQuery) {
    const qLower = props.searchQuery.toLowerCase()
    const detailsEls = el.querySelectorAll<HTMLDetailsElement>('details')
    detailsEls.forEach(d => {
      if (d.textContent?.toLowerCase().includes(qLower)) {
        d.open = true
      }
    })
  }

  el.scrollIntoView({ block: 'center', behavior: 'smooth' })
  el.classList.add('ah-msg--target-highlight')
  setTimeout(() => {
    el.classList.remove('ah-msg--target-highlight')
  }, 2600)

  // If search query is provided, open find widget and apply highlights
  if (props.searchQuery && props.searchQuery.trim()) {
    searchQuery.value = props.searchQuery.trim()
    searchVisible.value = true
    nextTick(() => {
      applyHighlights()
    })
  }
}

function handlePostLoadNavigation() {
  const hasTarget = props.targetTimestamp != null || (props.targetContent && props.targetContent.trim().length > 0)
  if (hasTarget) {
    const targetIdx = findTargetIndex()
    if (targetIdx !== -1) {
      locateTargetElement(targetIdx)
      return
    }
  }
  scrollToBottom()
}

watch(
  () => [props.targetTimestamp, props.targetContent] as const,
  ([newTs, newContent]) => {
    if (!props.active || (newTs == null && !newContent)) return
    nextTick(() => {
      const idx = findTargetIndex()
      if (idx !== -1) {
        locateTargetElement(idx)
      }
    })
  },
)

/** Whole-session tally for the footer / meta line, plus the same numbers for
 *  the host (the modal renders them next to its Close button). */
async function loadMessageStats() {
  if (!props.platformId || !props.sessionId) return
  const platformId = props.platformId
  const sessionId = props.sessionId
  try {
    const stats = await api.getSessionMessageStats(platformId, sessionId)
    // A slower answer for a session the user already left must not land.
    if (props.platformId !== platformId || props.sessionId !== sessionId) return
    messageStats.value = stats
    emit('stats', stats)
  } catch (e) {
    console.error('Failed to load session message stats:', e)
  }
}

async function loadAllRemainingMessages() {
  // All messages are already fully loaded on entry
}

/** (Re)start paging for the current identity. Called both when the identity
 *  changes and on mount, because hosts may mount the panel already active
 *  (the modal renders its slot only while open, the pane only while shown). */
function resetAndLoad() {
  if (!props.platformId || !props.sessionId) return
  messages.value = []
  offset = 0
  hasMore.value = false
  loadError.value = ''
  messageStats.value = null
  emit('stats', null)
  if (!props.searchQuery) {
    closeSearch()
  }
  loadMessages()
  scheduleStats()
}

// A preview pane reloads whenever the reader picks another row, and clicking
// through a list is fast. Both backend reads are therefore debounced, so ten
// clicks cost one load and one count instead of ten of each — the tally in
// particular walks the whole transcript on the backend, which is far too much
// work to start on every passing click.
const MESSAGE_SETTLE_MS = 150
const STATS_SETTLE_MS = 700
let messageTimer: ReturnType<typeof setTimeout> | null = null
let statsTimer: ReturnType<typeof setTimeout> | null = null

function clearLoadTimers() {
  if (messageTimer) { clearTimeout(messageTimer); messageTimer = null }
  if (statsTimer) { clearTimeout(statsTimer); statsTimer = null }
}

function scheduleLoad() {
  if (messageTimer) clearTimeout(messageTimer)
  messageTimer = setTimeout(() => {
    messageTimer = null
    resetAndLoad()
  }, MESSAGE_SETTLE_MS)
}

function scheduleStats() {
  if (statsTimer) clearTimeout(statsTimer)
  statsTimer = setTimeout(() => {
    statsTimer = null
    loadMessageStats()
  }, STATS_SETTLE_MS)
}

watch(
  () => [props.active, props.platformId, props.sessionId] as const,
  ([open, platformId, sessionId]) => {
    if (!open || !platformId || !sessionId) return
    scheduleLoad()
  },
)

watch(
  () => props.active,
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
  if (props.active) {
    window.addEventListener('keydown', handleKeyDown, true)
    // Mounting is not a rapid sequence: load straight away.
    resetAndLoad()
  }
})

onUnmounted(() => {
  clearLoadTimers()
  if (pinTimer) clearTimeout(pinTimer)
  window.removeEventListener('keydown', handleKeyDown, true)
  messagesObserver?.disconnect()
  contentObserver?.disconnect()
})
</script>

<template>
  <div class="flex flex-col flex-1 min-h-0 gap-3">
    <div class="text-xs pb-3 border-b flex items-center justify-between gap-3 flex-wrap" style="color: var(--ink-3); border-color: var(--hairline)">
      <div class="flex items-center gap-3 flex-wrap min-w-0">
        <span v-if="projectPath" class="truncate">{{ projectPath }}</span>
        <span v-if="model" style="color: var(--accent)">{{ model }}</span>
        <span v-if="tokens != null" style="color: var(--warning)">
          {{ t('session.tokens_value', { count: formatInt(tokens) }) }}
        </span>
        <span v-if="startedAt">{{ t('session.started_at', { time: formatSessionTime(startedAt, locale) }) }}</span>
        <!-- Hosts with a footer of their own (the modal) print the tally there
             and turn this off, so the numbers never appear twice. -->
        <span v-if="metaStats !== false && messageStats" class="ah-msg-meta__stats">
          {{
            t('session.messages_summary', {
              total: formatInt(messageStats.total),
              ai: formatInt(messageStats.assistant),
              me: formatInt(messageStats.user),
            })
          }}
        </span>
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

      <div ref="msgListRef" class="ah-msg-list h-full" @scroll="handleListScroll">
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
                <div v-if="msg.content?.trim()" class="ah-msg__content select-text" v-html="renderMarkdown(msg.content)"></div>
              </div>
              <div v-if="msg.hint || msg.content?.trim()" class="ah-msg__meta">
                <span v-if="msg.hint" class="ah-msg__hint">{{ msg.hint }}</span>
                <button
                  v-if="msg.content?.trim()"
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
        <div v-if="hasMore" ref="messagesSentinel" class="py-2 flex justify-center">
          <AppLoading v-if="loadingMore" class="py-2">{{ t('session.loading_more') }}</AppLoading>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Whole-session tally in the meta row (used by the reading pane, whose host
   has no footer of its own). Tabular digits keep the line from jiggling. */
.ah-msg-meta__stats {
  color: var(--ink-3);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

@keyframes ah-msg-target-pulse {
  0% {
    box-shadow: 0 0 0 3px var(--accent), 0 0 16px rgba(59, 130, 246, 0.5);
    border-color: var(--accent);
  }
  50% {
    box-shadow: 0 0 0 2.5px var(--accent), 0 0 12px rgba(59, 130, 246, 0.3);
    border-color: var(--accent);
  }
  100% {
    box-shadow: none;
  }
}

:deep(.ah-msg--target-highlight .ah-msg__bubble) {
  animation: ah-msg-target-pulse 2.6s ease-out;
}
</style>
