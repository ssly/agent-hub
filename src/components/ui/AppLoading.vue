<script setup lang="ts">
// Shared wave-ball loading indicator — the project-wide default. Same
// water-tank aesthetic as the tray loader: a clipped circle with two
// counter-rotating "water" layers whose wobble reads as a wave surface.
// Colors resolve tray vars first, then the main theme, so one component fits
// both surfaces and follows light/dark automatically.
withDefaults(defineProps<{ size?: number }>(), { size: 56 })
</script>

<template>
  <div class="app-loading" role="status">
    <div
      class="app-loading__ball"
      :style="{ width: `${size}px`, height: `${size}px` }"
      aria-hidden="true"
    >
      <div class="app-loading__wave app-loading__wave--back" />
      <div class="app-loading__wave app-loading__wave--front" />
    </div>
    <span v-if="$slots.default" class="app-loading__label"><slot /></span>
  </div>
</template>

<style scoped>
.app-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
}
.app-loading__ball {
  position: relative;
  border-radius: 999px;
  overflow: hidden;
  background: var(--tray-inset, var(--sunken));
  border: 1px solid var(--tray-border, var(--border));
}
/* Two stacked rounded squares rotate inside the clipped circle; their
   wobbling top edges read as the water surface. */
.app-loading__wave {
  position: absolute;
  left: -50%;
  bottom: 24%;
  width: 200%;
  height: 200%;
  border-radius: 42%;
  background: var(--tray-accent, var(--accent));
  opacity: .55;
  animation: app-loading-spin 6s linear infinite;
}
.app-loading__wave--back {
  bottom: 29%;
  border-radius: 46%;
  opacity: .26;
  animation-duration: 9.5s;
  animation-direction: reverse;
}
.app-loading__label {
  color: var(--tray-ink-3, var(--ink-3));
  font-size: 12px;
}
@keyframes app-loading-spin { to { transform: rotate(360deg); } }

@media (prefers-reduced-motion: reduce) {
  .app-loading__wave { animation: none; }
}
</style>
