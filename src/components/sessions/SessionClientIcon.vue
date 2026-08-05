<script setup lang="ts">
import { computed } from 'vue'
import chatgptMask from '@/assets/agent-icons/chatgpt.png'

const props = withDefaults(defineProps<{
  clientId: 'chatgpt'
  size?: number
}>(), {
  size: 13,
})

const iconStyle = computed(() => ({
  width: `${props.size}px`,
  height: `${props.size}px`,
  '--session-client-mask': `url("${chatgptMask}")`,
}))
</script>

<template>
  <span
    class="session-client-icon"
    :style="iconStyle"
    aria-hidden="true"
  />
</template>

<style scoped>
.session-client-icon {
  display: inline-block;
  flex: none;
  background: currentColor;
  -webkit-mask: var(--session-client-mask) center / contain no-repeat;
  mask: var(--session-client-mask) center / contain no-repeat;
}
</style>
