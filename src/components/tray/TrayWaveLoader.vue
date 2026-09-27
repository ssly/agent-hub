<script setup lang="ts">
import { LoaderCircle } from 'lucide-vue-next'
</script>

<template>
  <div class="tray-wave-loader" role="status" aria-live="polite">
    <LoaderCircle :size="22" class="tray-wave-loader__icon" aria-hidden="true" />
    <span class="tray-wave-loader__label"><slot /></span>
  </div>
</template>

<style scoped>
.tray-wave-loader {
  height: 112px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.tray-wave-loader__icon {
  color: var(--tray-accent);
  animation: tray-wave-loader-spin .9s linear infinite;
}
.tray-wave-loader__label {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
@keyframes tray-wave-loader-spin {
  to { transform: rotate(360deg); }
}
@media (prefers-reduced-motion: reduce) {
  .tray-wave-loader__icon { animation-duration: 2.4s; }
}
</style>
