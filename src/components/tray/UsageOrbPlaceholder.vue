<script setup lang="ts">
import { CircleAlert, Gauge } from 'lucide-vue-next'

/**
 * Empty / error stand-in for UsageOrb.
 * Same 112px graph footprint (+ optional side column) so the tray does not
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
        <!-- Thin echo of the orb ring: keeps the footprint without alarm. -->
        <circle class="ph-ring" cx="90" cy="90" r="80" />
        <!-- Soft dashed fill hint -->
        <circle class="ph-dash" cx="90" cy="90" r="72" />
        <!-- Soft badge disc anchoring the center icon (empty state only — the
             error state floats a bare, slightly larger "!"). -->
        <circle v-if="kind !== 'error'" class="ph-badge" cx="90" cy="90" r="40" />
      </svg>
      <div class="usage-orb-ph__center">
        <CircleAlert v-if="kind === 'error'" :size="mini ? 26 : 28" class="usage-orb-ph__icon" />
        <Gauge v-else :size="mini ? 22 : 24" class="usage-orb-ph__icon" />
      </div>
    </div>

    <!-- Mini mode shows no message text at all: the orb alone carries the
         state, and the strip below already lists what happened. -->
    <div v-if="!mini" class="usage-orb-ph__side">
      <p class="usage-orb-ph__title">{{ title }}</p>
      <p class="usage-orb-ph__message">{{ message }}</p>
    </div>
  </div>
</template>

<style scoped>
.usage-orb-ph {
  display: flex;
  align-items: center;
  gap: 14px;
  min-height: 112px;
}
.usage-orb-ph.is-mini {
  flex-direction: column;
  justify-content: center;
  gap: 8px;
}

/* Match UsageOrb graph size exactly. Child combinator: the center overlay's
   lucide icon is also an svg and must keep its own size. */
.usage-orb-ph__graph {
  position: relative;
  flex: 0 0 auto;
  width: 112px;
}
.usage-orb-ph__graph > svg {
  display: block;
  width: 100%;
  height: auto;
}

.ph-ring,
.ph-dash {
  fill: none;
  stroke-linecap: round;
}
.ph-ring {
  stroke: color-mix(in srgb, var(--tray-ink-3) 18%, transparent);
  stroke-width: 2.5;
}
.ph-dash {
  stroke: color-mix(in srgb, var(--tray-ink-3) 24%, transparent);
  stroke-width: 1.5;
  stroke-dasharray: 4 8;
  opacity: .8;
}

.ph-badge {
  fill: var(--tray-inset, var(--tray-sunken));
}

.is-error .ph-ring {
  stroke: color-mix(in srgb, var(--tray-danger, #e05) 30%, transparent);
}
.is-error .ph-dash {
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
  color: color-mix(in srgb, var(--tray-danger, #e05) 68%, var(--tray-ink-3));
  opacity: .85;
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
</style>
