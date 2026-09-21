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
  /** Orb-only layout: no legend side column, no hover tooltips. */
  mini?: boolean
}>(), {
  mini: false,
})
const { t, locale } = useI18n()

// Fixed viewBox: 180x180; the graph renders at 112px wide via CSS.
const CX = 90
const CY = 90
const ORB_R = 76 // 152px diameter inside 180px box

const instanceId = useId().replace(/[^a-zA-Z0-9_-]/g, '')
const clipSphereId = `orb-sphere-${instanceId}`
const clipLeftId = `orb-left-${instanceId}`
const clipRightId = `orb-right-${instanceId}`
const gradRedId = `orb-grad-red-${instanceId}`
const gradBlueId = `orb-grad-blue-${instanceId}`
const sphereShadeId = `orb-shade-${instanceId}`

// Dual-window mode: windows[0] (short-term / HP) on left, windows[1] (long-term / MP) on right.
// Single-window mode: windows[0] (lifeblood HP) fills the entire sphere in red.
const isDual = computed(() => props.windows.length >= 2)
const windowHp = computed(() => props.windows[0])
const windowMp = computed(() => (isDual.value ? props.windows[1] : null))

function remainingPercent(window?: UsageWindow | null) {
  if (!window) return 100
  const remaining = window.remaining_percent ?? 100 - (window.used_percent ?? 0)
  return Math.min(100, Math.max(0, remaining))
}

const hpPercent = computed(() => Math.round(remainingPercent(windowHp.value?.window)))
const mpPercent = computed(() => (windowMp.value ? Math.round(remainingPercent(windowMp.value.window)) : 100))

// Water depth mapping:
// Circle Top = CY - ORB_R = 90 - 76 = 14.
// Circle Bottom = CY + ORB_R = 90 + 76 = 166.
// Wave amplitude = ~4.5px.
// 100%: y = 8 (wave trough at 8 + 4.5 = 12.5 < 14 => 100% full, zero top gap).
// 0%: y = 172 (wave crest at 172 - 4.5 = 167.5 > 166 => 100% drained, zero puddle).
// Linear range: 172 - (p / 100) * 164
function calcFluidY(percent: number) {
  return 172 - (percent / 100) * 164
}

const hpFluidStyle = computed(() => ({
  transform: `translateY(${calcFluidY(hpPercent.value)}px)`,
  opacity: hpPercent.value === 0 ? 0 : 1,
}))

const mpFluidStyle = computed(() => ({
  transform: `translateY(${calcFluidY(mpPercent.value)}px)`,
  opacity: mpPercent.value === 0 ? 0 : 1,
}))

// Wave paths: continuous smooth sinusoidal curves spanning -60px to 300px
const WAVE_LEN = 60
function buildWave(amplitude: number) {
  let d = `M ${-WAVE_LEN} 0`
  for (let x = -WAVE_LEN; x < 180 + WAVE_LEN * 2; x += WAVE_LEN) {
    d += ` q ${WAVE_LEN / 4} ${-amplitude} ${WAVE_LEN / 2} 0 t ${WAVE_LEN / 2} 0`
  }
  return `${d} L ${180 + WAVE_LEN * 2} 240 L ${-WAVE_LEN} 240 Z`
}
const waveFront = buildWave(4.5)
const waveBack = buildWave(3.2)

function formatReset(resetAt: number) {
  return new Intl.DateTimeFormat(locale.value, {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(new Date(resetAt * 1000))
}
</script>

<template>
  <div class="usage-orb" :class="{ 'is-mini': mini }">
    <div class="usage-orb__graph">
      <svg viewBox="0 0 180 180" aria-hidden="true">
        <defs>
          <!-- Orb Sphere Mask -->
          <clipPath :id="clipSphereId">
            <circle :cx="CX" :cy="CY" :r="ORB_R" />
          </clipPath>

          <!-- Left Half Clip (HP) -->
          <clipPath :id="clipLeftId">
            <rect x="0" y="0" :width="CX + 0.3" height="180" />
          </clipPath>

          <!-- Right Half Clip (MP) -->
          <clipPath :id="clipRightId">
            <rect :x="CX" y="0" :width="CX" height="180" />
          </clipPath>

          <!-- HP Liquid Gradient (Muted Terracotta/Brick Red) -->
          <linearGradient :id="gradRedId" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stop-color="var(--tray-orb-hp-light, #CF7E77)" stop-opacity="0.95" />
            <stop offset="45%" stop-color="var(--tray-orb-hp-mid, #B0524A)" stop-opacity="0.90" />
            <stop offset="100%" stop-color="var(--tray-orb-hp-dark, #7A332D)" stop-opacity="0.95" />
          </linearGradient>

          <!-- MP Liquid Gradient (Muted Slate/Steel Blue) -->
          <linearGradient :id="gradBlueId" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stop-color="var(--tray-orb-mp-light, #5E8CAE)" stop-opacity="0.95" />
            <stop offset="45%" stop-color="var(--tray-orb-mp-mid, #3A6B8C)" stop-opacity="0.90" />
            <stop offset="100%" stop-color="var(--tray-orb-mp-dark, #224762)" stop-opacity="0.95" />
          </linearGradient>

          <!-- Inner Sphere Vignette / 3D Depth Shade -->
          <radialGradient :id="sphereShadeId" cx="45%" cy="38%" r="55%">
            <stop offset="60%" stop-color="transparent" />
            <stop offset="100%" stop-color="#000000" stop-opacity="0.38" />
          </radialGradient>
        </defs>

        <!-- Base Chamber background -->
        <circle class="orb-chamber-bg" :cx="CX" :cy="CY" :r="ORB_R" />
        <circle :cx="CX" :cy="CY" :r="ORB_R" :fill="`url(#${sphereShadeId})`" />

        <!-- Liquid Contents masked to the sphere -->
        <g :clip-path="`url(#${clipSphereId})`">
          <!-- LEFT / FULL: Red Liquid (Window 0 - HP) -->
          <g :clip-path="isDual ? `url(#${clipLeftId})` : undefined">
            <g class="orb-fluid-tank" :style="hpFluidStyle">
              <path class="orb-wave orb-wave--back" :fill="`url(#${gradRedId})`" :d="waveBack" />
              <path class="orb-wave orb-wave--front" :fill="`url(#${gradRedId})`" :d="waveFront" />
              <!-- Subtle rising micro-bubbles -->
              <circle class="orb-bubble bubble--1" cx="50" cy="30" r="1.8" />
              <circle class="orb-bubble bubble--2" cx="72" cy="50" r="1.2" />
              <circle class="orb-bubble bubble--3" cx="32" cy="70" r="1.5" />
              <circle v-if="!isDual" class="orb-bubble bubble--1" cx="120" cy="40" r="1.6" />
              <circle v-if="!isDual" class="orb-bubble bubble--2" cx="140" cy="65" r="1.3" />
            </g>
          </g>

          <!-- RIGHT: Blue Liquid (Window 1 - MP, dual mode only) -->
          <g v-if="isDual" :clip-path="`url(#${clipRightId})`">
            <g class="orb-fluid-tank" :style="mpFluidStyle">
              <path class="orb-wave orb-wave--back" :fill="`url(#${gradBlueId})`" :d="waveBack" />
              <path class="orb-wave orb-wave--front" :fill="`url(#${gradBlueId})`" :d="waveFront" />
              <!-- Subtle rising micro-bubbles -->
              <circle class="orb-bubble bubble--3" cx="130" cy="32" r="1.8" />
              <circle class="orb-bubble bubble--1" cx="110" cy="55" r="1.2" />
              <circle class="orb-bubble bubble--2" cx="150" cy="72" r="1.4" />
            </g>
          </g>

          <!-- Glass Refraction & Specular Highlights -->
          <ellipse cx="74" cy="45" rx="42" ry="18" class="orb-highlight" transform="rotate(-20 74 45)" />
          <path d="M 32 90 A 58 58 0 0 1 90 32" class="orb-highlight-arc" />
        </g>

        <!-- Outer Bezel Ring -->
        <circle class="orb-bezel-outer" :cx="CX" :cy="CY" :r="ORB_R" />
        <circle class="orb-bezel-inner" :cx="CX" :cy="CY" :r="ORB_R - 1.5" />
      </svg>
    </div>

    <!-- Side Legend (Normal mode) -->
    <div v-if="!mini" class="usage-orb__side">
      <ul class="usage-orb__legend">
        <li
          v-for="(item, index) in windows"
          :key="item.key"
          :class="index === 0 ? 'tone-hp' : 'tone-mp'"
        >
          <span class="legend-dot" />
          <span class="legend-label">{{ item.label }} {{ t('tray.limit') }}</span>
          <span class="legend-nums">
            <strong class="legend-remaining">{{ Math.round(remainingPercent(item.window)) }}% {{ t('tray.remaining') }}</strong>
            <span v-if="item.window.reset_at" class="legend-reset">
              {{ t('tray.reset_at', { time: formatReset(item.window.reset_at) }) }}
            </span>
          </span>
        </li>
      </ul>
      <slot />
    </div>
  </div>
</template>

<style scoped>
.usage-orb {
  display: flex;
  align-items: center;
  gap: 14px;
}
.usage-orb.is-mini {
  justify-content: center;
}
.usage-orb__graph {
  position: relative;
  flex: 0 0 auto;
  width: 112px;
}
.usage-orb__graph svg {
  display: block;
  width: 100%;
  height: auto;
}
.usage-orb__side {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 8px;
}

/* Chamber Background */
.orb-chamber-bg {
  fill: var(--tray-inset, var(--tray-sunken, #1A1F2C));
}

/* Fluid Tank */
.orb-fluid-tank {
  transition: transform .45s cubic-bezier(.2, .8, .2, 1), opacity .3s ease;
}

/* Wave Animations: softened translucent fluid */
.orb-wave {
  animation: orb-drift 5.5s linear infinite;
}
.orb-wave--front {
  opacity: .74;
}
.orb-wave--back {
  opacity: .36;
  animation-duration: 8.5s;
  animation-direction: reverse;
}
@keyframes orb-drift {
  from { transform: translateX(0); }
  to { transform: translateX(-60px); }
}

/* Micro-Bubbles: delicate and semi-transparent */
.orb-bubble {
  fill: #FFFFFF;
  opacity: .50;
  animation-name: orb-rise;
  animation-timing-function: ease-in;
  animation-iteration-count: infinite;
}
.bubble--1 {
  animation-duration: 3.2s;
  animation-delay: -0.4s;
}
.bubble--2 {
  animation-duration: 4.2s;
  animation-delay: -1.8s;
}
.bubble--3 {
  animation-duration: 2.7s;
  animation-delay: -1.1s;
}
@keyframes orb-rise {
  0% { transform: translateY(0) scale(0.8); opacity: 0; }
  35% { opacity: .55; }
  85% { opacity: .55; }
  100% { transform: translateY(-40px) scale(1.1); opacity: 0; }
}

/* Glass Refraction / Specular Highlight */
.orb-highlight {
  fill: #FFFFFF;
  opacity: .10;
}
.orb-highlight-arc {
  fill: none;
  stroke: #FFFFFF;
  opacity: .12;
  stroke-width: 1.5;
  stroke-linecap: round;
}

/* Bezel */
.orb-bezel-outer {
  fill: none;
  stroke: var(--tray-border);
  stroke-width: 1.5;
}
.orb-bezel-inner {
  fill: none;
  stroke: color-mix(in srgb, var(--tray-ink) 8%, transparent);
  stroke-width: 1;
}

/* Tones for legend */
.tone-hp {
  color: var(--tray-orb-hp-mid, #B0524A);
}
.tone-mp {
  color: var(--tray-orb-mp-mid, #3A6B8C);
}

/* Per-window legend rows */
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
.legend-label {
  color: var(--tray-ink-2);
  font-weight: 600;
  text-transform: uppercase;
}
.legend-nums {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 2px 8px;
  min-width: 0;
}
.legend-remaining {
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
  .orb-wave, .orb-bubble { animation: none; }
}
</style>
