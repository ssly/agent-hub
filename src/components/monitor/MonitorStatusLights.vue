<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { monitorAgentLights, type MonitorTab } from '@/stores/session-monitor'

const props = defineProps<{
  agent: MonitorTab
}>()

const { t } = useI18n()
const lights = computed(() => monitorAgentLights(props.agent))
const hint = computed(() => lights.value.map(light => t(`session_monitor.light_${light}`)).join(' · '))
</script>

<template>
  <span class="monitor-status-lights" v-tooltip="hint">
    <span
      v-for="light in lights"
      :key="light"
      class="monitor-status-lights__dot"
      :class="`is-${light}`"
    />
  </span>
</template>

<style scoped>
.monitor-status-lights {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  flex: none;
}
.monitor-status-lights__dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
}
.monitor-status-lights__dot.is-failed { background: var(--signal-red); }
.monitor-status-lights__dot.is-waiting { background: var(--signal-yellow); }
.monitor-status-lights__dot.is-running { background: var(--signal-green); }
</style>
