<script setup lang="ts">
import { computed, type Component } from 'vue'
import {
  Bot,
  MousePointer2,
  Network,
  PanelsTopLeft,
  Route,
  Send,
  Triangle,
} from 'lucide-vue-next'
import kimiCodeMask from '@/assets/agent-icons/kimi-code.png'
import kiroMask from '@/assets/agent-icons/kiro.png'
import kiroDetailMask from '@/assets/agent-icons/kiro-detail.png'
import antigravityMask from '@/assets/agent-icons/antigravity.png'
import claudeCodeMask from '@/assets/agent-icons/claude-code.png'
import codexMask from '@/assets/agent-icons/codex.png'
import grokBuildMask from '@/assets/agent-icons/grok-build.png'

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

// These masks are derived directly from the supplied brand artwork. The
// colored foreground becomes currentColor while transparent cutouts preserve
// the original terminal/cloud/ghost geometry in every app theme.
const brandMasks: Record<string, string> = {
  kimi: kimiCodeMask,
  kiro: kiroMask,
  antigravity: antigravityMask,
  claude: claudeCodeMask,
  codex: codexMask,
  grok: grokBuildMask,
}

const brandDetailMasks: Record<string, string> = {
  kiro: kiroDetailMask,
}

const icons: Record<string, Component> = {
  // Shared Pool is a shared capability directory, not a branded Agent.
  'shared-pool': Network,
  all: PanelsTopLeft,
  cursor: MousePointer2,
  hermes: Send,
  trae: Route,
  opencode: Triangle,
}

const agentKey = computed(() => aliases[props.agentId] ?? props.agentId)
const brandMask = computed(() => brandMasks[agentKey.value])
const brandDetailMask = computed(() => brandDetailMasks[agentKey.value])
const icon = computed(() => icons[agentKey.value] ?? Bot)

function maskStyle(mask: string) {
  return { '--agent-icon-mask': `url("${mask}")` }
}
</script>

<template>
  <span
    class="agent-icon"
    :style="{ '--agent-icon-size': `${size}px` }"
    aria-hidden="true"
  >
    <template v-if="brandMask">
      <span class="agent-icon__mask" :style="maskStyle(brandMask)" />
      <span
        v-if="brandDetailMask"
        class="agent-icon__mask"
        :style="maskStyle(brandDetailMask)"
      />
    </template>
    <component v-else :is="icon" :size="size" />
  </span>
</template>

<style scoped>
.agent-icon {
  position: relative;
  width: var(--agent-icon-size);
  height: var(--agent-icon-size);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 0;
}
.agent-icon__mask {
  position: absolute;
  inset: 0;
  background: currentColor;
  -webkit-mask-image: var(--agent-icon-mask);
  -webkit-mask-position: center;
  -webkit-mask-repeat: no-repeat;
  -webkit-mask-size: contain;
  mask-image: var(--agent-icon-mask);
  mask-mode: alpha;
  mask-position: center;
  mask-repeat: no-repeat;
  mask-size: contain;
}
</style>
