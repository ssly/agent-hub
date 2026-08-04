<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Monitor, Play, Terminal, Trash2 } from 'lucide-vue-next'
import { useHoverResetBool } from '@/composables/useHoverReset'
import { formatInt } from '@/lib/utils'

// Shared session card used by both the Sessions browser and the live Monitor.
// Purely presentational: parents map their own data shapes onto these props and
// keep their own stores — no session data crosses between the two views.
//
// Layout contract:
//   head:  [agent badge] [model] [tokens] [source badge] [status]   [note?] [time] [actions]
//   body:  title / subtitle / default slot (e.g. monitor Q&A lines)
// Whole card is clickable: normal mode emits `open` (view messages), selection
// mode emits `toggleSelect` instead.
const props = withDefaults(defineProps<{
  badge?: string
  source?: 'terminal' | 'chatgpt' | 'cursor' | null
  sourceLabel?: string
  status?: 'running' | 'ended' | null
  time?: string
  /** Hint shown left of the time while the delete confirm is armed
   *  (Monitor only: its delete removes the row, not the real session). */
  deleteNote?: string
  title?: string
  subtitle?: string
  model?: string | null
  tokens?: number | null
  selecting?: boolean
  selected?: boolean
  resumable?: boolean
  deletable?: boolean
}>(), {
  source: null,
  status: null,
  model: null,
  tokens: null,
  resumable: true,
  deletable: true,
})

const emit = defineEmits<{
  open: []
  resume: []
  delete: []
  toggleSelect: []
}>()

const { t } = useI18n()
const { armed: confirmDelete, arm: armDelete, reset: resetDelete } = useHoverResetBool()

function handleClick() {
  if (props.selecting) emit('toggleSelect')
  else emit('open')
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
    class="ah-session-card session-card"
    :class="{
      'session-card--clickable': !selecting,
      'session-card--selecting': selecting,
      'session-card--selected': selecting && selected,
      'session-card--running': status === 'running',
    }"
    @click="handleClick"
  >
    <!-- Selected marker: accent triangle ribbon in the top-left corner -->
    <div v-if="selecting && selected" class="session-card__corner">
      <svg
        width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor"
        stroke-width="4" stroke-linecap="round" stroke-linejoin="round"
      >
        <polyline points="20 6 9 17 4 12" />
      </svg>
    </div>

    <div class="session-card__head">
      <div class="session-card__badges">
        <span v-if="badge" class="session-card__badge">{{ badge }}</span>
        <span v-if="model" class="ah-session-card__model">{{ model }}</span>
        <span v-if="tokens != null" class="ah-session-card__tokens">
          {{ t('session.tokens_value', { count: formatInt(tokens) }) }}
        </span>
        <span v-if="source" class="session-card__source">
          <Monitor v-if="source === 'chatgpt'" :size="13" />
          <Terminal v-else :size="13" />
          <span>{{ sourceLabel }}</span>
        </span>
        <span v-if="status" class="session-status" :class="`session-status--${status}`">
          <span class="session-status__dot" />
          {{ status === 'running' ? t('session_monitor.status_running') : t('session_monitor.status_ended') }}
        </span>
      </div>
      <div class="session-card__right">
        <!-- Delete semantics hint (Monitor passes it): shown only while the
             delete confirm is armed, i.e. after the first click. -->
        <span v-if="deleteNote && confirmDelete" class="session-card__delete-note">{{ deleteNote }}</span>
        <span v-if="time" class="session-card__time">{{ time }}</span>
        <!-- Inline actions revealed on card hover: [note] [time] [resume] [delete].
             Icon-only; labels show via v-tooltip. -->
        <div
          v-if="!selecting && (resumable || deletable)"
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

    <h3 v-if="title" class="ah-session-card__title session-card__title truncate">{{ title }}</h3>
    <div v-if="subtitle" class="ah-session-card__path">{{ subtitle }}</div>

    <slot />
  </div>
</template>

<style scoped>
.session-card {
  position: relative;
}
.session-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 6px;
}
.session-card__badges {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex-wrap: wrap;
}
.session-card__badge {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--sunken);
  color: var(--ink-2);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
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
  gap: 8px;
  flex: none;
}
.session-card__delete-note {
  font-size: 11px;
  line-height: 1;
  color: var(--danger);
  opacity: 0.75;
  white-space: nowrap;
}
.session-card__time {
  flex: none;
  color: var(--ink-4);
  font: 11px/1 var(--font-mono);
  white-space: nowrap;
}
/* Inline corner actions: hidden until the card is hovered (or focused
   within, for keyboard users); the delete confirm chip keeps them visible.
   They sit in flow, so the time slides left when they appear. */
.session-card__actions {
  display: flex;
  align-items: center;
  gap: 4px;
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--dur-fast) var(--ease-soft);
}
.session-card:hover .session-card__actions,
.session-card__actions:focus-within,
.session-card__actions--armed {
  opacity: 1;
  pointer-events: auto;
}
.session-card__title {
  margin: 0;
}
.session-status {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  white-space: nowrap;
}
.session-status__dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: currentColor;
}
.session-status--running { color: var(--success); }
.session-status--ended { color: var(--ink-4); }
.session-card--running {
  border-color: color-mix(in srgb, var(--success) 45%, var(--hairline));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--success) 7%, transparent);
}
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
/* Normal mode: the whole card opens the messages view. */
.session-card--clickable {
  cursor: pointer;
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
