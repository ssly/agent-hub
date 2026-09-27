<script setup lang="ts">
import { CircleAlert, Gauge } from 'lucide-vue-next'

/**
 * Empty / error stand-in for UsageOrb. Both states keep the same 112px-tall
 * usage area; failures use a centered icon and concise error message.
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
    :aria-label="kind === 'error' ? `${title}: ${message}` : undefined"
  >
    <div v-if="kind === 'error'" class="usage-orb-ph__failure">
      <span class="usage-orb-ph__failure-icon" aria-hidden="true">
        <CircleAlert :size="20" />
      </span>
      <p class="usage-orb-ph__title">{{ title }}</p>
      <p v-if="!mini" class="usage-orb-ph__message">{{ message }}</p>
    </div>

    <template v-else>
      <div class="usage-orb-ph__graph" aria-hidden="true">
        <svg viewBox="0 0 180 180">
          <!-- Thin echo of the orb sphere: keeps the footprint without alarm. -->
          <circle class="ph-ring" cx="90" cy="90" r="76" />
          <!-- Soft inner bezel hint -->
          <circle class="ph-dash" cx="90" cy="90" r="74.5" />
          <circle class="ph-badge" cx="90" cy="90" r="38" />
        </svg>
        <div class="usage-orb-ph__center">
          <Gauge :size="mini ? 22 : 24" class="usage-orb-ph__icon" />
        </div>
      </div>

      <div v-if="!mini" class="usage-orb-ph__side">
        <p class="usage-orb-ph__title">{{ title }}</p>
        <p class="usage-orb-ph__message">{{ message }}</p>
      </div>
    </template>
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
.usage-orb-ph.is-error {
  box-sizing: border-box;
  height: 112px;
  justify-content: center;
  padding: 4px 12px;
  text-align: center;
}
.usage-orb-ph__failure {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
}
.usage-orb-ph__failure-icon {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  margin-bottom: 1px;
  border-radius: 50%;
  color: color-mix(in srgb, var(--tray-danger, #d46666) 82%, var(--tray-ink-2));
  background: color-mix(in srgb, var(--tray-danger, #d46666) 11%, transparent);
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
.is-error .usage-orb-ph__title {
  color: var(--tray-ink-2);
  font-size: 12px;
  line-height: 1.25;
}
.is-error .usage-orb-ph__message {
  max-width: 100%;
  color: var(--tray-ink-3);
  font-size: 11px;
  line-height: 1.35;
  overflow-wrap: anywhere;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
}
</style>
