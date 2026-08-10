<script setup lang="ts">
// Tray quota-area loading state: the shared AppLoading wave ball inside a
// 112px-tall container that matches the orb graph, so the panel does not
// jump when real data arrives.
import AppLoading from '@/components/ui/AppLoading.vue'
</script>

<template>
  <div class="tray-wave-loader" role="status">
    <AppLoading :size="56"><slot /></AppLoading>
  </div>
</template>

<style scoped>
.tray-wave-loader {
  height: 112px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.tray-wave-loader :deep(.app-loading__label) {
  font-size: 11px;
}
</style>
