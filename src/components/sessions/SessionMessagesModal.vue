<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import AppModal from '@/components/ui/AppModal.vue'
import SessionMessagesPanel from '@/components/sessions/SessionMessagesPanel.vue'
import { formatInt } from '@/lib/utils'
import { compactSessionPreview } from '@/lib/session-display'
import type { SessionMessageStats } from '@/lib/api'

// Modal chrome around the shared message panel: the Sessions browser uses it
// on narrow windows (its wide layout previews in a side pane instead), the
// Monitor and the search results use it at any width.
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

const { t } = useI18n()
const modalTitle = computed(() => compactSessionPreview(props.title) || t('session.untitled'))

// Tally of the open session, printed on the left of the Close row. The panel
// owns the fetch (it is the only place that knows the identity) and hands the
// numbers over; clearing on identity change keeps a stale count from lingering
// while the next session loads.
const stats = ref<SessionMessageStats | null>(null)

watch(
  () => [props.show, props.platformId, props.sessionId] as const,
  () => {
    stats.value = null
  },
)
</script>

<template>
  <AppModal
    :show="show"
    :title="modalTitle"
    width-class="w-[94vw]"
    fill-height
    @close="emit('close')"
  >
    <SessionMessagesPanel
      :active="show"
      :platform-id="platformId"
      :session-id="sessionId"
      :project-path="projectPath"
      :model="model"
      :tokens="tokens"
      :started-at="startedAt"
      :meta-stats="false"
      @stats="stats = $event"
    />
    <template #footer>
      <span v-if="stats" class="messages-modal__stats">
        {{
          t('session.messages_summary', {
            total: formatInt(stats.total),
            ai: formatInt(stats.assistant),
            me: formatInt(stats.user),
          })
        }}
      </span>
      <button class="btn btn-secondary" @click="emit('close')">{{ t('action.close') }}</button>
    </template>
  </AppModal>
</template>

<style scoped>
/* The modal footer is a right-aligned flex row; the tally takes the left side
   so the Close button keeps its place. */
.messages-modal__stats {
  margin-right: auto;
  min-width: 0;
  font-size: 11.5px;
  color: var(--ink-3);
  font-variant-numeric: tabular-nums;
}
</style>
