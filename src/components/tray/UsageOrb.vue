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

const props = defineProps<{ windows: OrbWindow[] }>()
const { t, locale } = useI18n()

// Fixed viewBox; the graph renders at ~152px wide via CSS.
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
const bubbleR = computed(() =>
  props.windows.length === 1 ? 76 : props.windows.length === 2 ? 64 : 50,
)

function usedPercent(window: UsageWindow) {
  return Math.min(100, Math.max(0, window.used_percent ?? 0))
}
function remainingPercent(window: UsageWindow) {
  const remaining = window.remaining_percent ?? 100 - usedPercent(window)
  return Math.min(100, Math.max(0, Math.round(remaining)))
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

function tip(item: OrbWindow) {
  const lines = [
    `${item.label} ${t('tray.limit')} · ${t('tray.used')} ${Math.round(usedPercent(item.window))}% · ${t('tray.remaining')} ${remainingPercent(item.window)}%`,
  ]
  if (item.window.reset_at) lines.push(t('tray.reset_at', { time: formatReset(item.window.reset_at) }))
  return lines.join('\n')
}

const centerRemain = computed(() => remainingPercent(bubbleWindow.value.window))
</script>

<template>
  <div class="usage-orb">
    <div class="usage-orb__graph">
      <svg viewBox="0 0 180 180" aria-hidden="true">
        <defs>
          <clipPath :id="clipId">
            <circle :cx="CX" :cy="CY" :r="bubbleR" />
          </clipPath>
        </defs>

        <!-- Quota rings: larger windows wrap the tank, largest outermost. The
             fill arc tracks consumption; two tiny bubbles orbit each ring. -->
        <g
          v-for="(ring, index) in rings"
          :key="ring.item.key"
          class="usage-orb__ring"
          :class="`tone-${ring.item.tone}`"
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
          <circle
            v-tooltip="tip(ring.item)"
            class="ring-hit"
            :cx="CX" :cy="CY" :r="ring.radius"
          />
        </g>

        <!-- Bubble tank for the shortest window. -->
        <g v-tooltip="tip(bubbleWindow)" class="usage-orb__tank" :class="`tone-${bubbleWindow.tone}`">
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

      <div class="usage-orb__center">
        <strong>{{ centerRemain }}%</strong>
        <span>{{ t('tray.remaining') }}</span>
      </div>
    </div>

    <ul class="usage-orb__legend">
      <li
        v-for="item in windows"
        :key="item.key"
        v-tooltip="tip(item)"
        :class="`tone-${item.tone}`"
      >
        <span class="legend-dot" />
        <span class="legend-label">{{ item.label }} {{ t('tray.limit') }}</span>
        <span class="legend-used">{{ t('tray.used') }} {{ Math.round(usedPercent(item.window)) }}%</span>
        <strong class="legend-remain">{{ t('tray.remaining') }} {{ remainingPercent(item.window) }}%</strong>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.usage-orb { display: flex; flex-direction: column; align-items: stretch; }
.usage-orb__graph { position: relative; width: 152px; margin: 0 auto; }
.usage-orb__graph svg { display: block; width: 100%; height: auto; }

.tone-primary { color: var(--tray-accent); }
.tone-secondary { color: var(--tray-success); }
.tone-monthly { color: var(--tray-highlight); }

/* Quota rings */
.ring-track {
  fill: none;
  stroke: color-mix(in srgb, currentColor 14%, transparent);
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
.ring-hit {
  fill: none;
  stroke: transparent;
  stroke-width: 14;
  pointer-events: stroke;
}
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

/* Center readout: remaining share of the shortest window. */
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
  font-size: 19px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  line-height: 1.1;
}
/* Keep the readout legible when the water level rises behind it. */
.usage-orb__center strong,
.usage-orb__center span {
  text-shadow: 0 0 3px var(--tray-inset), 0 0 7px var(--tray-inset), 0 0 2px var(--tray-inset);
}
.usage-orb__center span { color: var(--tray-ink-3); font-size: 10px; }

/* Per-window legend rows */
.usage-orb__legend {
  list-style: none;
  margin: 10px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.usage-orb__legend li {
  display: grid;
  grid-template-columns: auto 1fr auto auto;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  cursor: default;
}
.legend-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: currentColor;
}
.legend-label { color: var(--tray-ink-2); font-weight: 600; text-transform: uppercase; }
.legend-used { color: var(--tray-ink-3); font-variant-numeric: tabular-nums; }
.legend-remain { color: var(--tray-ink); font-variant-numeric: tabular-nums; }

@media (prefers-reduced-motion: reduce) {
  .wave, .ring-orbit, .tank-bubble { animation: none; }
}
</style>
