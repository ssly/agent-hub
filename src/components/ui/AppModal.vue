<script setup lang="ts">
import { onMounted, onUnmounted, watch } from 'vue'

const props = defineProps<{
  show: boolean
  title?: string
  widthClass?: string // e.g. 'w-[48rem]'
  bare?: boolean // omit header/footer chrome; render only the body slot (edge-to-edge)
  closeOnOutside?: boolean // whether clicking the backdrop closes the modal (default true)
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

function handleBackdropClick(e: MouseEvent) {
  if (props.closeOnOutside === false) return
  if ((e.target as HTMLElement).classList.contains('ah-modal-overlay')) {
    emit('close')
  }
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.show) {
    emit('close')
  }
}

watch(() => props.show, (newVal) => {
  if (newVal) {
    document.body.style.overflow = 'hidden'
  } else {
    document.body.style.overflow = ''
  }
})

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
  document.body.style.overflow = ''
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="show"
      class="ah-modal-overlay"
      @click="handleBackdropClick"
    >
      <div :class="['ah-modal', bare ? 'ah-modal--bare' : '', widthClass || 'w-[48rem]']">
        <!-- Header (hidden in bare mode; caller renders its own) -->
        <div
          v-if="!bare"
          class="ah-modal__header flex items-center justify-between border-b pb-4"
          style="border-color: var(--hairline)"
        >
          <h3 class="ah-modal__title">{{ title }}</h3>
          <button
            class="text-xl cursor-pointer transition-colors"
            style="color: var(--ink-3)"
            @click="emit('close')"
          >
            &times;
          </button>
        </div>

        <!-- Body -->
        <div :class="bare ? 'ah-modal__body ah-modal__body--bare' : 'ah-modal__body'">
          <slot />
        </div>

        <!-- Footer -->
        <div v-if="$slots.footer" class="ah-modal__footer">
          <slot name="footer" />
        </div>
      </div>
    </div>
  </Teleport>
</template>
