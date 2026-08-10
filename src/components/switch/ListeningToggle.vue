<script setup lang="ts">
// Per-agent usage-listening toggle, shown in the AccountUsagePanel header next
// to the refresh button. State lives in the shared backend settings, so the
// tray popup follows immediately (paused = no auto queries on either side).
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Power } from 'lucide-vue-next'
import { useSwitchStore } from '@/stores/switch'

const { t } = useI18n()
const store = useSwitchStore()

const listened = computed(() =>
  store.selectedAgent ? store.isAgentListened(store.selectedAgent) : true,
)

async function toggle() {
  if (!store.selectedAgent) return
  await store.setAgentListening(store.selectedAgent, !listened.value)
}
</script>

<template>
  <button
    class="btn btn-secondary btn-sm flex items-center gap-1"
    :style="listened ? {} : { color: 'var(--ink-3)' }"
    v-tooltip="listened ? t('switch.listening_on') : t('switch.listening_off')"
    @click="toggle"
  >
    <Power :size="12" :style="{ color: listened ? 'var(--success)' : 'var(--ink-4)' }" />
    {{ listened ? t('switch.listening_disable') : t('switch.listening_enable') }}
  </button>
</template>
