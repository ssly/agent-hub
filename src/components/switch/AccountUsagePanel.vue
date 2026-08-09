<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Gauge, Info, RefreshCw } from 'lucide-vue-next'

export interface UsageWindowRow {
  key: string
  label: string
  remainingPercent: number
  detail: string
}

const props = withDefaults(defineProps<{
  /** Current account display name (card header). */
  accountName: string
  /** Subtitle under the account name. */
  accountHint: string
  /** Optional trailing badge on the account card (e.g. plan / auth method). */
  accountBadge?: string | null
  /** Right-side account status label (defaults to read-only). */
  accountStatusLabel?: string | null
  showAccountCard?: boolean
  usageTitle: string
  loading: boolean
  loadingText: string
  error?: string | null
  /** Soft notice (not a hard failure), e.g. Claude not logged in. */
  softNotice?: string | null
  emptyHint?: string | null
  /** Special product tip above usage body (Kimi API-only, Claude OAuth, …). */
  tip?: string | null
  badges?: string[]
  windows?: UsageWindowRow[]
  lastQueryText?: string | null
  refreshing?: boolean
}>(), {
  accountBadge: null,
  accountStatusLabel: null,
  showAccountCard: true,
  error: null,
  softNotice: null,
  emptyHint: null,
  tip: null,
  badges: () => [],
  windows: () => [],
  lastQueryText: null,
  refreshing: false,
})

const emit = defineEmits<{
  refresh: []
}>()

const { t } = useI18n()

const statusLabel = computed(
  () => props.accountStatusLabel || t('switch.codex_read_only'),
)

const showWindows = computed(() => props.windows.length > 0)
const showEmpty = computed(
  () => !props.loading && !props.error && !props.softNotice && !showWindows.value,
)
</script>

<template>
  <div class="account-usage space-y-6">
    <div
      v-if="showAccountCard"
      class="ah-card switch-card--active switch-card--readonly"
    >
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <div class="flex items-center gap-2 mb-1 flex-wrap">
            <span class="text-sm font-medium truncate" style="color: var(--ink)">{{ accountName }}</span>
            <span
              v-if="accountBadge"
              class="text-xs px-2 py-0.5 rounded-full flex-shrink-0"
              style="background: var(--sunken); color: var(--ink-2)"
            >{{ accountBadge }}</span>
            <span class="switch-active-badge">{{ t('switch.active_badge') }}</span>
          </div>
          <div class="text-xs" style="color: var(--ink-3)">{{ accountHint }}</div>
        </div>
        <span
          class="text-xs px-2 py-1 rounded-full flex-shrink-0"
          style="background: var(--sunken); color: var(--ink-2)"
        >
          {{ statusLabel }}
        </span>
      </div>
    </div>

    <div class="ah-card" style="background: var(--surface); border-color: var(--border)">
      <div class="flex items-center justify-between mb-3 gap-3">
        <span class="text-base font-semibold flex items-center gap-2 min-w-0" style="color: var(--ink)">
          <Gauge :size="18" class="flex-shrink-0" :style="{ color: 'var(--accent)' }" />
          <span class="truncate">{{ usageTitle }}</span>
        </span>
        <button
          class="btn btn-secondary btn-sm flex items-center gap-1 flex-shrink-0"
          :disabled="refreshing || loading"
          @click="emit('refresh')"
        >
          <RefreshCw :size="14" :class="{ 'animate-spin': refreshing || loading }" />
          {{ t('switch.usage_refresh') }}
        </button>
      </div>

      <div
        v-if="tip"
        class="text-xs mb-3 flex items-start gap-1.5"
        style="color: var(--ink-4)"
      >
        <Info :size="13" class="flex-shrink-0 mt-0.5" />
        <span>{{ tip }}</span>
      </div>

      <div v-if="loading" class="text-sm py-4" style="color: var(--ink-3)">
        {{ loadingText }}
      </div>

      <div v-else-if="softNotice" class="ah-notice" style="margin: 0">
        {{ softNotice }}
      </div>

      <div v-else-if="error && !showWindows" class="ah-notice ah-notice--warning" style="margin: 0">
        {{ t('switch.usage_failed') }}: {{ error }}
      </div>

      <div v-else-if="showEmpty" class="text-sm py-2" style="color: var(--ink-3)">
        {{ emptyHint || t('switch.usage_empty_hint') }}
      </div>

      <div v-else class="space-y-3 text-sm">
        <div
          v-if="error && showWindows"
          class="ah-notice ah-notice--warning"
          style="margin: 0"
        >
          {{ t('switch.usage_failed') }}: {{ error }}
        </div>

        <div v-if="badges.length" class="text-xs flex items-center gap-2 flex-wrap" style="color: var(--ink-3)">
          <span
            v-for="badge in badges"
            :key="badge"
            class="inline-flex items-center px-2 py-0.5 rounded-full"
            style="background: var(--sunken); color: var(--ink-2)"
          >{{ badge }}</span>
        </div>

        <div
          v-for="win in windows"
          :key="win.key"
          class="p-3 rounded-lg"
          style="background: var(--sunken)"
        >
          <div class="flex justify-between items-center gap-2">
            <span class="font-medium" style="color: var(--ink)">{{ win.label }}</span>
            <span class="font-semibold flex-shrink-0" style="color: var(--accent)">
              {{ t('switch.usage_remaining', { n: win.remainingPercent }) }}
            </span>
          </div>
          <div v-if="win.detail" class="text-xs mt-1" style="color: var(--ink-3)">
            {{ win.detail }}
          </div>
        </div>

        <slot name="extra" />

        <div
          v-if="lastQueryText"
          class="text-xs pt-2 border-t"
          style="color: var(--ink-4); border-color: var(--hairline)"
        >
          {{ t('switch.usage_last_query', { time: lastQueryText }) }}
        </div>
      </div>
    </div>
  </div>
</template>
