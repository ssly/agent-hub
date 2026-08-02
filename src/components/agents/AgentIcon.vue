<script setup lang="ts">
import { computed, type Component } from 'vue'
import {
  Bot,
  BrainCircuit,
  Moon,
  MousePointer2,
  Network,
  Orbit,
  PanelsTopLeft,
  Route,
  Send,
  Sparkles,
  Triangle,
} from 'lucide-vue-next'

const props = withDefaults(defineProps<{
  agentId: string
  size?: number
}>(), {
  size: 15,
})

// A platform can use a slightly different id in each area of the app. Keep
// the aliases here so every Agent presentation stays visually consistent.
const aliases: Record<string, string> = {
  'claude-code': 'claude',
  'grok-build': 'grok',
  'kimi-code': 'kimi',
}

const icons: Record<string, Component> = {
  // Shared Pool is a shared capability directory, not a branded Agent.
  'shared-pool': Network,
  all: PanelsTopLeft,
  antigravity: Orbit,
  gemini: Sparkles,
  grok: BrainCircuit,
  kimi: Moon,
  cursor: MousePointer2,
  hermes: Send,
  trae: Route,
  opencode: Triangle,
}

const agentKey = computed(() => aliases[props.agentId] ?? props.agentId)
const icon = computed(() => icons[agentKey.value] ?? Bot)
</script>

<template>
  <span
    class="agent-icon"
    :style="{ '--agent-icon-size': `${size}px` }"
    aria-hidden="true"
  >
    <!-- Codex's terminal cloud and >_ prompt, as shown in its CLI identity. -->
    <svg
      v-if="agentKey === 'codex'"
      class="agent-icon__brand"
      viewBox="0 0 24 24"
      focusable="false"
    >
      <path
        fill="currentColor"
        d="M12 2.2c1.92 0 3.55 1.19 4.22 2.88.42-.11.85-.17 1.3-.17 2.64 0 4.78 2.14 4.78 4.78 0 1.17-.42 2.24-1.12 3.07.49.63.78 1.42.78 2.28a3.7 3.7 0 0 1-3.7 3.7c-.47 0-.92-.09-1.34-.25A5.32 5.32 0 0 1 12.3 21c-1.64 0-3.11-.74-4.1-1.91-.43.14-.9.22-1.39.22a4.2 4.2 0 0 1-4.2-4.2c0-1.16.47-2.22 1.23-2.98a4.13 4.13 0 0 1-.62-2.17 4.16 4.16 0 0 1 4.16-4.16c.39 0 .77.05 1.12.15A4.52 4.52 0 0 1 12 2.2Z"
      />
      <path
        d="m9 8.25 2.7 3.75L9 15.75M14.35 15.75h3.35"
        fill="none"
        stroke="var(--surface)"
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="1.8"
      />
    </svg>

    <!-- Kiro's round companion mark: a small ghost inside a circular badge. -->
    <svg
      v-else-if="agentKey === 'kiro'"
      class="agent-icon__brand"
      viewBox="0 0 24 24"
      focusable="false"
    >
      <circle cx="12" cy="12" r="9" fill="currentColor" />
      <path
        d="M8.15 17.55v-5.86a3.85 3.85 0 1 1 7.7 0v5.86l-1.87-1.1L12 17.55l-1.98-1.1-1.87 1.1Z"
        fill="var(--surface)"
      />
      <circle cx="10.55" cy="12.05" r=".7" fill="currentColor" />
      <circle cx="13.45" cy="12.05" r=".7" fill="currentColor" />
    </svg>

    <!-- Claude Code's compact pixel companion with square eyes and legs. -->
    <svg
      v-else-if="agentKey === 'claude'"
      class="agent-icon__brand"
      viewBox="0 0 24 24"
      focusable="false"
    >
      <g fill="currentColor">
        <path d="M6 8h12v9H6zM3 10h3v5H3zM18 10h3v5h-3zM7 16h3v4H7zM14 16h3v4h-3z" />
        <path d="M7 6h2v2H7zM15 6h2v2h-2z" />
      </g>
      <path d="M9.5 10h1.7v3H9.5zM12.8 10h1.7v3h-1.7z" fill="var(--surface)" />
    </svg>

    <component v-else :is="icon" :size="size" />
  </span>
</template>

<style scoped>
.agent-icon {
  width: var(--agent-icon-size);
  height: var(--agent-icon-size);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 0;
}
.agent-icon__brand { width: 100%; height: 100%; }
</style>
