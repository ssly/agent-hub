<script setup lang="ts">
import { useToast } from '@/composables/useToast'

const { toasts } = useToast()

const toneClass: Record<string, string> = {
  success: 'ah-toast--success',
  error: 'ah-toast--error',
  warning: 'ah-toast--warning',
  info: 'ah-toast--info',
}
</script>

<template>
  <div class="fixed bottom-5 right-5 z-[100] flex flex-col-reverse gap-2 pointer-events-none">
    <div
      v-for="toast in toasts"
      :key="toast.id"
      :class="['pointer-events-auto ah-toast', toneClass[toast.type] || 'ah-toast--info', toast.exiting ? 'animate-toast-out' : 'animate-toast-in']"
    >
      {{ toast.message }}
    </div>
  </div>
</template>

<style scoped>
.animate-toast-in {
  animation: toastIn 240ms cubic-bezier(.2, .8, .2, 1);
}
.animate-toast-out {
  animation: toastOut 180ms cubic-bezier(.4, 0, .2, 1) forwards;
}
</style>
