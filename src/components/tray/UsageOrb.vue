<script setup lang="ts">
import { computed, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import type { UsageWindow } from '@/lib/api'

export type OrbTone = 'primary' | 'secondary' | 'monthly'
export interface OrbWindow {
  key: string
  label: string
  tone: OrbTone
  window: UsageWindow
}

const props = withDefaults(defineProps<{
  windows: OrbWindow[]
  /** Ring-only layout: no legend side column, no hover tooltips. */
  mini?: boolean
}>(), {
  mini: false,
})
const { t, locale } = useI18n()

// Fixed viewBox; the graph renders at ~112px wide via CSS.
const CX = 90
const CY = 90
const WAVE_LEN = 52

const clipId = `orb-clip-${useId().replace(/[^a-zA-Z0-9_-]/g, '')}`

// The shortest window becomes the inner "bubble tank"; larger windows wrap it
// as concentric rings, largest outermost. Strokes stay chunky (10px) — thin
// rings read poorly at tray size.
const bubbleWindow = computed(() => props.windows[0])
const OUTER_RING_R = 80
const RING_GAP = 15
const rings = computed(() =>
  props.windows.slice(1).map((item, index, all) => ({
    item,
    // The smaller of the ring windows sits closer to the tank.
    radius: OUTER_RING_R - (all.length - 1 - index) * RING_GAP,
  })),
)
// Single-window orbs still size the tank as if one ring wrapped it (64), so
// providers without a secondary window render at the same visual size.
const bubbleR = computed(() => (props.windows.length >= 3 ? 50 : 64))

function usedPercent(window: UsageWindow) {
  return Math.min(100, Math.max(0, window.used_percent ?? 0))
}

// Water level: the tank fills bottom-up with the *consumed* share, so a nearly
// full tank means the window is nearly exhausted.
const waterDepth = computed(() => (2 * bubbleR.value * usedPercent(bubbleWindow.value.window)) / 100)
const waterStyle = computed(() => ({
  transform: `translate(${CX - bubbleR.value}px, ${CY + bubbleR.value - waterDepth.value}px)`,
}))

// Wave surface: one wavelength wider than the tank on both sides so the
// horizontal drift loops seamlessly.
function buildWave(amplitude: number) {
  const width = 2 * bubbleR.value
  const depth = 2 * bubbleR.value + 10
  let d = `M ${-WAVE_LEN} 0`
  for (let x = -WAVE_LEN; x < width + WAVE_LEN; x += WAVE_LEN) {
    d += ` q ${WAVE_LEN / 4} ${-amplitude} ${WAVE_LEN / 2} 0 t ${WAVE_LEN / 2} 0`
  }
  return `${d} L ${width + WAVE_LEN} ${depth} L ${-WAVE_LEN} ${depth} Z`
}
const waveFront = computed(() => buildWave(3.5))
const waveBack = computed(() => buildWave(2.5))

function mulberry32(seed: number) {
  let a = seed
  return () => {
    a |= 0; a = (a + 0x6d2b79f5) | 0
    let t = Math.imul(a ^ (a >>> 15), 1 | a)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

// Rising bubbles inside the tank. They live in the water group, so the surface
// is local y=0 and each bubble rises from near the water floor. Deterministic
// PRNG keeps the layout stable across re-renders.
interface Bubble { cx: number; r: number; duration: number; delay: number; drift: number }
const bubbles = computed<Bubble[]>(() => {
  const rand = mulberry32(7)
  return Array.from({ length: 6 }, () => ({
    cx: bubbleR.value + (rand() * 2 - 1) * bubbleR.value * 0.58,
    r: 1.3 + rand() * 2.1,
    duration: 3.2 + rand() * 2.8,
    delay: -rand() * 6,
    drift: (rand() * 2 - 1) * 5,
  }))
})
function bubbleStyle(b: Bubble) {
  return {
    '--rise': `${Math.max(10, waterDepth.value - 6)}px`,
    '--drift': `${b.drift}px`,
    animationDuration: `${b.duration}s`,
    animationDelay: `${b.delay}s`,
  }
}

function ringDash(radius: number, percent: number) {
  const c = 2 * Math.PI * radius
  return `${((c * percent) / 100).toFixed(1)} ${c.toFixed(1)}`
}

function formatReset(resetAt: number) {
  return new Intl.DateTimeFormat(locale.value, {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(new Date(resetAt * 1000))
}

/** Center readout is *used* share of the shortest window (not remaining). */
const centerUsed = computed(() => Math.round(usedPercent(bubbleWindow.value.window)))
</script>

<template>
  <div class="usage-orb" :class="{ 'is-mini': mini }">
    <!-- No graph/legend tooltips: used % + reset time sit in the side legend. -->
    <div class="usage-orb__graph">
      <svg viewBox="0 0 180 180" aria-hidden="true">
        <defs>
          <clipPath :id="clipId">
            <circle :cx="CX" :cy="CY" :r="bubbleR" />
          </clipPath>
        </defs>

        <!-- Outer rings (longer windows): accent color, arc = used %. -->
        <g
          v-for="(ring, index) in rings"
          :key="ring.item.key"
          class="usage-orb__ring tone-ring"
        >
          <circle class="ring-track" :cx="CX" :cy="CY" :r="ring.radius" />
          <circle
            class="ring-fill"
            :cx="CX" :cy="CY" :r="ring.radius"
            :stroke-dasharray="ringDash(ring.radius, usedPercent(ring.item.window))"
            :transform="`rotate(-90 ${CX} ${CY})`"
          />
          <g class="ring-orbit" :style="{ animationDuration: `${9 + index * 5}s` }">
            <circle :cx="CX + ring.radius" :cy="CY" r="2.1" />
            <circle :cx="CX + ring.radius * 0.68" :cy="CY - ring.radius * 0.73" r="1.5" />
          </g>
        </g>

        <!-- Inner tank (shortest window): green water, level = used %. -->
        <g class="usage-orb__tank tone-tank">
          <circle class="tank-bg" :cx="CX" :cy="CY" :r="bubbleR" />
          <g :clip-path="`url(#${clipId})`">
            <g class="tank-water" :style="waterStyle">
              <g class="wave wave--back"><path :d="waveBack" /></g>
              <g class="wave wave--front"><path :d="waveFront" /></g>
              <circle
                v-for="(bubble, index) in bubbles"
                :key="index"
                class="tank-bubble"
                :cx="bubble.cx" cy="2" :r="bubble.r"
                :style="bubbleStyle(bubble)"
              />
            </g>
          </g>
          <circle class="tank-edge" :cx="CX" :cy="CY" :r="bubbleR" />
        </g>
      </svg>

      <!-- Center readout: used % of the shortest window only (no label). -->
      <div class="usage-orb__center">
        <strong>{{ centerUsed }}%</strong>
      </div>
    </div>

    <div v-if="!mini" class="usage-orb__side">
      <ul class="usage-orb__legend">
        <li
          v-for="(item, index) in windows"
          :key="item.key"
          :class="index === 0 ? 'tone-tank' : 'tone-ring'"
        >
          <span class="legend-dot" />
          <span class="legend-label">{{ item.label }} {{ t('tray.limit') }}</span>
          <span class="legend-nums">
            <strong class="legend-used">{{ t('tray.used') }} {{ Math.round(usedPercent(item.window)) }}%</strong>
            <span v-if="item.window.reset_at" class="legend-reset">
              {{ t('tray.reset_at', { time: formatReset(item.window.reset_at) }) }}
            </span>
          </span>
        </li>
      </ul>
      <!-- Extra side-column content (e.g. Codex reset-credit chips) so every
           provider panel keeps the same overall height. -->
      <slot />
    </div>
  </div>
</template>

<style scoped>
.usage-orb { display: flex; align-items: center; gap: 14px; }
.usage-orb.is-mini { justify-content: center; }
/* Same graph size in mini and normal so the orb does not jump on mode switch. */
.usage-orb__graph { position: relative; flex: 0 0 auto; width: 112px; }
.usage-orb__graph svg { display: block; width: 100%; height: auto; }
.usage-orb__side {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 8px;
}

/* Inner circle = green; outer ring(s) = accent — both encode *used* share. */
.tone-tank { color: var(--tray-success); }
.tone-ring { color: var(--tray-accent); }

/* Quota rings */
.ring-track {
  fill: none;
  /* Stronger tint in dark mode via --tray-ring-track (see CodexTrayView). */
  stroke: var(--tray-ring-track, color-mix(in srgb, currentColor 14%, transparent));
  stroke-width: 10;
}
.ring-fill {
  fill: none;
  stroke: currentColor;
  stroke-width: 10;
  stroke-linecap: round;
  transition: stroke-dasharray .6s cubic-bezier(.2, .8, .2, 1);
}
.ring-orbit {
  transform-box: view-box;
  transform-origin: center;
  animation: orb-spin 9s linear infinite;
}
.ring-orbit circle { fill: currentColor; opacity: .5; }
@keyframes orb-spin { to { transform: rotate(360deg); } }

/* Bubble tank */
.tank-bg { fill: var(--tray-inset); }
.tank-edge { fill: none; stroke: var(--tray-border); stroke-width: 1; }
.tank-water { transition: transform .6s cubic-bezier(.2, .8, .2, 1); }
.wave { animation: orb-drift 6s linear infinite; }
.wave path { fill: currentColor; }
.wave--front path { opacity: .58; }
.wave--back { animation-duration: 9.5s; animation-direction: reverse; }
.wave--back path { opacity: .3; }
.tank-bubble {
  fill: var(--tray-surface);
  animation-name: orb-rise;
  animation-timing-function: ease-in;
  animation-iteration-count: infinite;
}
@keyframes orb-drift { to { transform: translateX(-52px); } }
@keyframes orb-rise {
  0% { transform: translate(var(--drift), var(--rise)); opacity: 0; }
  14% { opacity: .9; }
  82% { opacity: .9; }
  100% { transform: translate(0, 0); opacity: 0; }
}

/* Center readout: used % of the shortest window. */
.usage-orb__center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.usage-orb__center strong {
  color: var(--tray-ink);
  font-size: 17px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  line-height: 1.1;
}
/* Keep the readout legible when the water level rises behind it. */
.usage-orb__center strong {
  text-shadow: 0 0 3px var(--tray-inset), 0 0 7px var(--tray-inset), 0 0 2px var(--tray-inset);
}

/* Per-window legend rows: dot + label on line 1, used/remaining on line 2,
   sitting to the right of the orb so 1-window and 2-window providers share
   the same overall height. */
.usage-orb__legend {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 8px;
}
.usage-orb__legend li {
  width: fit-content;
  max-width: 100%;
  display: grid;
  grid-template-columns: auto 1fr;
  align-items: center;
  column-gap: 7px;
  row-gap: 1px;
  font-size: 11px;
  cursor: default;
}
.legend-dot {
  grid-row: 1 / 3;
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: currentColor;
}
.legend-label { color: var(--tray-ink-2); font-weight: 600; text-transform: uppercase; }
.legend-nums {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 2px 8px;
  min-width: 0;
}
.legend-used {
  color: var(--tray-ink);
  font-variant-numeric: tabular-nums;
  font-weight: 650;
  white-space: nowrap;
}
.legend-reset {
  color: var(--tray-ink-3);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

@media (prefers-reduced-motion: reduce) {
  .wave, .ring-orbit, .tank-bubble { animation: none; }
}
</style>
