<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, Monitor, Play, Terminal, Trash2 } from 'lucide-vue-next'
import { useHoverResetBool } from '@/composables/useHoverReset'
import { formatInt, formatSessionTime } from '@/lib/utils'
import AgentIcon from '@/components/agents/AgentIcon.vue'
import SessionClientIcon from '@/components/sessions/SessionClientIcon.vue'

// Shared session card used by both the Sessions browser and the live Monitor.
// Purely presentational: parents map their own data shapes onto these props and
// keep their own stores — no session data crosses between the two views.
//
// Layout contract:
//   head:  [agent/client icon + short label] [model?] [tokens?] [source?] [status] [time] [actions]
//   unread: top-right corner marker with a subtle red card border
//   body:  title / subtitle / default slot (e.g. monitor Q&A lines)
// Source is only shown when it adds info the badge does not already carry
// (e.g. "终端" under Grok). ChatGPT-as-badge never doubles with a source chip.
// Whole card is clickable and emits `open` (view messages). When `selectable`,
// a checkbox sits at the bottom-right; clicking it (or ⌘/Ctrl / Shift on the
// card) emits `select` with the original mouse event instead of opening.
const props = withDefaults(defineProps<{
  badge?: string
  /** Platform/agent id for AgentIcon (codex, claude, grok, …). */
  badgeAgentId?: string | null
  /** Client-source icon key when the badge itself is a client (e.g. chatgpt). */
  badgeIcon?: string | null
  source?: 'terminal' | 'chatgpt' | 'cursor' | 'antigravity' | 'antigravity-ide' | null
  sourceLabel?: string
  status?: 'running' | 'waiting' | 'failed' | 'ended' | null
  time?: string
  /** Unix seconds or ms; formatted here so the parent list does not
   *  toLocaleString every row on each selection toggle. Ignored when `time` is set. */
  updatedAt?: number | string | null
  /** Exact timestamp used as the hover tooltip when `time` is a relative label. */
  timeTooltip?: string
  /** Hint shown left of the time while the delete confirm is armed
   *  (Monitor only: its delete removes the row, not the real session). */
  deleteNote?: string
  title?: string
  subtitle?: string
  model?: string | null
  tokens?: number | null
  unread?: boolean
  selectable?: boolean
  selected?: boolean
  resumable?: boolean
  deletable?: boolean
}>(), {
  badgeAgentId: null,
  badgeIcon: null,
  source: null,
  status: null,
  updatedAt: null,
  model: null,
  tokens: null,
  unread: false,
  selectable: false,
  resumable: true,
  deletable: true,
})

const emit = defineEmits<{
  open: []
  resume: []
  delete: []
  select: [event: MouseEvent]
  read: []
}>()

const { t, locale } = useI18n()
const { armed: confirmDelete, arm: armDelete, reset: resetDelete } = useHoverResetBool()

/** Hide source when the primary badge already names the same client/source. */
const showSource = computed(() => {
  if (!props.source || !props.sourceLabel) return false
  // Badge already is "ChatGPT 客户端" — do not paint a second identical chip.
  if (props.badgeIcon === 'chatgpt' && props.source === 'chatgpt') return false
  if (props.badge && props.sourceLabel === props.badge) return false
  return true
})

const statusLabel = computed(() => {
  if (props.status === 'running') return t('session_monitor.status_running')
  if (props.status === 'waiting') return t('session_monitor.status_waiting')
  if (props.status === 'failed') return t('session_monitor.status_failed')
  return t('session_monitor.status_ended')
})

const displayTime = computed(() => {
  if (props.time) return props.time
  if (props.updatedAt == null || props.updatedAt === '') return ''
  return formatSessionTime(props.updatedAt, locale.value)
})

function isToggleModifier(event: MouseEvent) {
  const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform)
  return mac ? event.metaKey : event.ctrlKey
}

function isSelectModifier(event: MouseEvent) {
  return event.shiftKey || isToggleModifier(event)
}

function handleClick(event: MouseEvent) {
  if (props.selectable && isSelectModifier(event)) {
    event.preventDefault()
    emit('select', event)
    return
  }
  emit('open')
}

function handleSelectClick(event: MouseEvent) {
  event.stopPropagation()
  emit('select', event)
}

function handleCardMouseDown(event: MouseEvent) {
  if (props.selectable && isSelectModifier(event)) event.preventDefault()
}

function handleDelete() {
  if (!confirmDelete.value) {
    armDelete()
    return
  }
  resetDelete()
  emit('delete')
}
</script>

<template>
  <div
    class="ah-session-card session-card session-card--clickable"
    :class="{
      'session-card--selectable': selectable,
      'session-card--selected': selectable && selected,
      'session-card--unread': unread,
      'session-card--running': status === 'running',
      'session-card--waiting': status === 'waiting',
      'session-card--failed': status === 'failed',
    }"
    @click="handleClick"
    @mousedown="handleCardMouseDown"
    @mouseenter="emit('read')"
  >
    <div class="session-card__head">
      <div class="session-card__badges">
        <span v-if="badge" class="session-card__badge">
          <SessionClientIcon v-if="badgeIcon === 'chatgpt'" client-id="chatgpt" :size="12" />
          <AgentIcon v-else-if="badgeAgentId" :agent-id="badgeAgentId" :size="12" />
          {{ badge }}
        </span>
        <span v-if="model" class="ah-session-card__model">{{ model }}</span>
        <span v-if="tokens != null" class="ah-session-card__tokens">
          {{ t('session.tokens_value', { count: formatInt(tokens) }) }}
        </span>
        <span v-if="showSource" class="session-card__source">
          <SessionClientIcon v-if="source === 'chatgpt'" client-id="chatgpt" :size="13" />
          <Monitor v-else-if="source === 'cursor'" :size="13" />
          <AgentIcon v-else-if="source === 'antigravity' || source === 'antigravity-ide'" agent-id="antigravity" :size="13" />
          <Terminal v-else :size="13" />
          <span>{{ sourceLabel }}</span>
        </span>
        <span
          v-if="status"
          v-tooltip="statusLabel"
          class="session-status"
          :class="`session-status--${status}`"
          :aria-label="statusLabel"
        >
          <span class="session-status__dot" />
        </span>
      </div>
      <div class="session-card__right">
        <!-- Delete semantics hint (Monitor passes it): shown only while the
             delete confirm is armed, i.e. after the first click. -->
        <span v-if="deleteNote && confirmDelete" class="session-card__delete-note">{{ deleteNote }}</span>
        <span v-if="displayTime" v-tooltip="timeTooltip || ''" class="session-card__time">{{ displayTime }}</span>
        <!-- Inline actions revealed on card hover: [note] [time] [resume] [delete].
             Icon-only; labels show via v-tooltip. -->
        <div
          v-if="resumable || deletable"
          class="session-card__actions"
          :class="{ 'session-card__actions--armed': confirmDelete }"
          @click.stop
        >
          <button
            v-if="resumable"
            v-tooltip="t('session.resume')"
            class="session-card__icon-btn"
            @click="emit('resume')"
          >
            <Play :size="13" />
          </button>
          <button
            v-if="deletable"
            class="session-card__icon-btn session-card__delete"
            :class="{ 'is-confirming': confirmDelete }"
            v-tooltip="confirmDelete ? '' : t('session.delete')"
            @click="handleDelete"
            @mouseleave="resetDelete()"
          >
            <Trash2 v-if="!confirmDelete" :size="13" />
            <span v-else>{{ t('session.confirm_delete') }}</span>
          </button>
        </div>
      </div>
    </div>

    <span
      v-if="unread"
      v-tooltip="t('session_monitor.unread')"
      class="session-card__unread"
      role="img"
      :aria-label="t('session_monitor.unread')"
    />

    <h3 v-if="title" v-tooltip="title" class="ah-session-card__title session-card__title truncate">{{ title }}</h3>
    <div v-if="subtitle" v-tooltip="subtitle" class="ah-session-card__path">{{ subtitle }}</div>

    <slot />

    <button
      v-if="selectable"
      type="button"
      class="session-card__check"
      :aria-label="t('session.select_session')"
      :aria-pressed="selected"
      @click="handleSelectClick"
      @mousedown.stop
    >
      <span class="ah-select-check" :class="{ 'is-checked': selected }">
        <Check v-if="selected" :size="11" :stroke-width="2.75" />
      </span>
    </button>
  </div>
</template>

<style scoped>
.session-card {
  position: relative;
  padding: 9px 12px;
}
.session-card--selectable {
  user-select: none;
}
.session-card--selectable .ah-session-card__path {
  padding-right: 36px;
}
.session-card__check {
  position: absolute;
  right: 4px;
  bottom: 4px;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  appearance: none;
  cursor: pointer;
  pointer-events: auto;
}
.session-card__check:hover .ah-select-check {
  border-color: var(--accent);
}
.session-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 3px;
}
.session-card__badges {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex-wrap: wrap;
}
.session-card__badge {
  display: inline-flex;
  align-items: center;
  padding: 0;
  color: var(--ink-2);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
  gap: 5px;
}
.session-card__source {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--ink-2);
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}
.session-card__right {
  display: flex;
  align-items: center;
  /* Gap only appears once actions expand on hover — keeps time flush-right. */
  gap: 0;
  flex: none;
}
.session-card--unread .session-card__right {
  padding-right: 16px;
}
.session-card:hover .session-card__right:has(.session-card__actions),
.session-card__right:has(.session-card__actions:focus-within),
.session-card__right:has(.session-card__actions--armed) {
  gap: 8px;
}
.session-card__delete-note {
  font-size: 11px;
  line-height: 1;
  color: var(--danger);
  opacity: 0.75;
  white-space: nowrap;
}
.session-card__unread {
  position: absolute;
  top: 10px;
  right: 10px;
  z-index: 1;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--signal-red);
}
.session-card__time {
  flex: none;
  color: var(--ink-4);
  font: 11px/1 var(--font-mono);
  white-space: nowrap;
}
/* Inline corner actions: collapsed until hover / focus / delete-armed, so the
   time stays flush-right when idle (no empty reserved slot). On reveal the
   time slides left as the action cluster expands. */
.session-card__actions {
  display: flex;
  align-items: center;
  gap: 0;
  max-width: 0;
  opacity: 0;
  overflow: hidden;
  pointer-events: none;
  transition:
    max-width var(--dur-fast) var(--ease-soft),
    opacity var(--dur-fast) var(--ease-soft),
    gap var(--dur-fast) var(--ease-soft);
}
.session-card:hover .session-card__actions,
.session-card__actions:focus-within,
.session-card__actions--armed {
  max-width: 12rem;
  gap: 4px;
  opacity: 1;
  overflow: visible;
  pointer-events: auto;
}
.session-card__title {
  margin: 0;
  line-height: 1.45;
}
.session-card .ah-session-card__path {
  margin-top: 3px;
  line-height: 1.45;
}
.session-status {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 18px;
  white-space: nowrap;
}
.session-status__dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: currentColor;
}
.session-status--running { color: var(--signal-green); }
.session-status--waiting { color: var(--signal-yellow); }
.session-status--failed { color: var(--signal-red); }
.session-status--ended { color: var(--ink-4); }
.session-card__icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 24px;
  width: 24px;
  padding: 0;
  color: var(--ink-4);
  background: var(--surface);
  border: 1px solid var(--hairline);
  border-radius: var(--radius-sm);
  cursor: pointer;
  white-space: nowrap;
  transition: color var(--dur-fast) var(--ease-soft), background var(--dur-fast) var(--ease-soft),
    border-color var(--dur-fast) var(--ease-soft), width var(--dur-fast) var(--ease-soft),
    padding var(--dur-fast) var(--ease-soft);
}
.session-card__icon-btn:hover {
  color: var(--accent);
  background: var(--accent-soft);
  border-color: var(--accent-mid);
}
/* Delete: quiet icon by default, turns into a red confirm chip on first click. */
.session-card__delete:hover {
  color: var(--danger);
  background: var(--danger-soft);
  border-color: color-mix(in srgb, var(--danger) 35%, var(--hairline));
}
.session-card__delete.is-confirming {
  width: auto;
  padding: 0 10px;
  font-size: 12px;
  color: var(--on-accent);
  background: var(--danger);
  border-color: var(--danger);
}
.session-card--clickable {
  cursor: pointer;
}
.session-card--unread {
  border-color: color-mix(in srgb, var(--signal-red) 35%, var(--hairline));
}
.session-card--selected {
  background: var(--accent-soft);
  border-color: var(--accent-mid);
}
</style>
