<script setup lang="ts">
import { CircleAlert, Gauge } from 'lucide-vue-next'

/**
 * Empty / error stand-in for UsageOrb.
 * Same 132px graph footprint (+ optional side column) so the tray does not
 * collapse when a provider returns no usage windows or the query fails.
 */
withDefaults(defineProps<{
  /** `empty` = no windows; `error` = query failed */
  kind?: 'empty' | 'error'
  title: string
  message: string
  mini?: boolean
}>(), {
  kind: 'empty',
  mini: false,
})
</script>

<template>
  <div
    class="usage-orb-ph"
    :class="{ 'is-mini': mini, 'is-error': kind === 'error' }"
    role="status"
  >
    <div class="usage-orb-ph__graph" aria-hidden="true">
      <svg viewBox="0 0 180 180">
        <!-- Outer track matches orb ring radius -->
        <circle class="ph-ring" cx="90" cy="90" r="80" />
        <!-- Inner tank outline matches single-window bubble -->
        <circle class="ph-tank" cx="90" cy="90" r="64" />
        <!-- Soft dashed fill hint -->
        <circle class="ph-dash" cx="90" cy="90" r="72" />
      </svg>
      <div class="usage-orb-ph__center">
        <CircleAlert v-if="kind === 'error'" :size="mini ? 22 : 26" class="usage-orb-ph__icon" />
        <Gauge v-else :size="mini ? 22 : 26" class="usage-orb-ph__icon" />
      </div>
    </div>

    <div v-if="!mini" class="usage-orb-ph__side">
      <p class="usage-orb-ph__title">{{ title }}</p>
      <p class="usage-orb-ph__message">{{ message }}</p>
    </div>
    <p v-else class="usage-orb-ph__mini-msg">{{ message }}</p>
  </div>
</template>

<style scoped>
.usage-orb-ph {
  display: flex;
  align-items: center;
  gap: 14px;
  min-height: 132px;
}
.usage-orb-ph.is-mini {
  flex-direction: column;
  justify-content: center;
  gap: 8px;
}

/* Match UsageOrb graph size exactly */
.usage-orb-ph__graph {
  position: relative;
  flex: 0 0 auto;
  width: 132px;
}
.usage-orb-ph__graph svg {
  display: block;
  width: 100%;
  height: auto;
}

.ph-ring,
.ph-tank,
.ph-dash {
  fill: none;
  stroke-linecap: round;
}
.ph-ring {
  stroke: var(--tray-ring-track, color-mix(in srgb, var(--tray-ink-3) 22%, transparent));
  stroke-width: 10;
}
.ph-tank {
  stroke: color-mix(in srgb, var(--tray-ink-3) 16%, transparent);
  stroke-width: 2.5;
  fill: color-mix(in srgb, var(--tray-inset, var(--tray-ink-3)) 35%, transparent);
}
.ph-dash {
  stroke: color-mix(in srgb, var(--tray-ink-3) 28%, transparent);
  stroke-width: 1.5;
  stroke-dasharray: 5 7;
  opacity: .85;
}

.is-error .ph-ring {
  stroke: color-mix(in srgb, var(--tray-danger, #e05) 28%, transparent);
}
.is-error .ph-tank {
  fill: color-mix(in srgb, var(--tray-danger, #e05) 8%, transparent);
  stroke: color-mix(in srgb, var(--tray-danger, #e05) 22%, transparent);
}

.usage-orb-ph__center {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.usage-orb-ph__icon {
  color: var(--tray-ink-3);
  opacity: .72;
}
.is-error .usage-orb-ph__icon {
  color: color-mix(in srgb, var(--tray-danger, #e05) 78%, var(--tray-ink-3));
  opacity: .9;
}

.usage-orb-ph__side {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 6px;
  padding-right: 4px;
}
.usage-orb-ph__title {
  margin: 0;
  color: var(--tray-ink-2);
  font-size: 13px;
  font-weight: 600;
  line-height: 1.3;
}
.usage-orb-ph__message {
  margin: 0;
  max-width: 16em;
  color: var(--tray-ink-3);
  font-size: 12px;
  line-height: 1.45;
}
.usage-orb-ph__mini-msg {
  margin: 0;
  max-width: 12em;
  color: var(--tray-ink-3);
  font-size: 11px;
  line-height: 1.35;
  text-align: center;
}
</style>
