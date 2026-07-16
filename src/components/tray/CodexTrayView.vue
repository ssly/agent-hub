<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { getCodexTrayUsage } from '@/lib/api'
import type { CodexTraySnapshot, ResetCreditEntry, UsageWindow } from '@/lib/api'

const { t, locale } = useI18n()
const snapshot = ref<CodexTraySnapshot | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const now = ref(Date.now())
const unlisteners: UnlistenFn[] = []
let clock: ReturnType<typeof setInterval> | null = null

type WindowTone = 'primary' | 'secondary'
interface TrayUsageWindow {
  key: string
  label: string
  tone: WindowTone
  window: UsageWindow
}

function windowLabel(seconds: number) {
  if (Math.abs(seconds - 18_000) <= 600) return '5h'
  if (Math.abs(seconds - 604_800) <= 3_600) return '7d'
  if (Math.abs(seconds - 2_592_000) <= 86_400) return '30d'
  if (seconds >= 86_400) return `${Math.round(seconds / 86_400)}d`
  return `${Math.round(seconds / 3_600)}h`
}

const usageWindows = computed<TrayUsageWindow[]>(() => {
  const windows = [
    snapshot.value?.usage.primary_window,
    snapshot.value?.usage.secondary_window,
  ]
    .filter((window): window is UsageWindow => Boolean(window?.window_seconds))
    .sort((left, right) => left.window_seconds - right.window_seconds)

  return windows.map((window, index) => ({
    key: `${window.window_seconds}-${index}`,
    label: windowLabel(window.window_seconds),
    tone: windows.length === 1
      ? (window.window_seconds < 86_400 ? 'primary' : 'secondary')
      : (index === 0 ? 'primary' : 'secondary'),
    window,
  }))
})
const innerWindow = computed(() => usageWindows.value.find(item => item.tone === 'primary'))
const outerWindow = computed(() => usageWindows.value.find(item => item.tone === 'secondary'))
const resetCards = computed<ResetCreditEntry[]>(() => {
  const detailed = snapshot.value?.reset_credits
  const available = (detailed?.credits ?? [])
    .filter(credit => credit.status === 'available')
    .sort((left, right) => {
      const leftTime = left.expires_at ? new Date(left.expires_at).getTime() : Number.MAX_SAFE_INTEGER
      const rightTime = right.expires_at ? new Date(right.expires_at).getTime() : Number.MAX_SAFE_INTEGER
      return leftTime - rightTime
    })
  if (available.length) return available

  const count = detailed?.available_count
    ?? snapshot.value?.usage.reset_credits?.available_count
    ?? 0
  return Array.from({ length: count }, (_, index) => ({
    status: 'available',
    expires_at: index === 0 ? detailed?.next_expires_at ?? null : null,
    granted_at: null,
    title: null,
  }))
})
const cooldownMinutes = computed(() => {
  if (!snapshot.value) return 0
  return Math.max(0, Math.ceil((snapshot.value.next_query_at * 1000 - now.value) / 60_000))
})

function ringOffset(percent: number | undefined, radius: number) {
  const circumference = 2 * Math.PI * radius
  const safePercent = Math.min(100, Math.max(0, percent ?? 0))
  return circumference * (1 - safePercent / 100)
}

function formatDate(unixSeconds: number) {
  return new Intl.DateTimeFormat(locale.value, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(unixSeconds * 1000))
}

function formatExpiry(value?: string | null) {
  if (!value) return t('tray.expiry_unknown')
  const seconds = Math.max(0, Math.floor((new Date(value).getTime() - now.value) / 1000))
  const days = Math.floor(seconds / 86_400)
  const hours = Math.floor((seconds % 86_400) / 3_600)
  if (days > 0) return t('tray.expiry_dh', { d: days, h: hours })
  return t('tray.expiry_hm', {
    h: hours,
    m: Math.floor((seconds % 3_600) / 60),
  })
}

function windowPercent(window: UsageWindow | null) {
  return window?.used_percent ?? 0
}

async function refresh() {
  loading.value = true
  error.value = null
  try {
    snapshot.value = await getCodexTrayUsage()
  } catch (reason: any) {
    error.value = String(reason?.message || reason)
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  clock = setInterval(() => { now.value = Date.now() }, 30_000)

  // Browser-only mock route should be immediately previewable. The real hidden
  // tray window waits for a tray click so startup never consumes a query.
  if (import.meta.env.MODE === 'web') {
    await refresh()
    return
  }

  const listeners = await Promise.all([
    listen('codex-tray-loading', () => {
      loading.value = true
      error.value = null
    }),
    listen<CodexTraySnapshot>('codex-tray-updated', event => {
      snapshot.value = event.payload
      loading.value = false
      error.value = null
    }),
    listen<string>('codex-tray-error', event => {
      loading.value = false
      error.value = event.payload
    }),
  ])
  unlisteners.push(...listeners)
})

onBeforeUnmount(() => {
  unlisteners.forEach(unlisten => unlisten())
  if (clock) clearInterval(clock)
})
</script>

<template>
  <main class="tray-shell">
    <section class="tray-panel">
      <header class="tray-header">
        <div class="tray-title">
          <span class="tray-logo" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M4 15a8 8 0 1 1 16 0"/><path d="m12 15 4-5"/><circle cx="12" cy="15" r="1"/></svg>
          </span>
          <h1>{{ t('tray.title') }}</h1>
        </div>
        <button class="refresh-button" :disabled="loading" @click="refresh">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 0 0-14.9-4M4 4v5h5"/><path d="M4 13a8 8 0 0 0 14.9 4M20 20v-5h-5"/></svg>
          {{ loading ? t('tray.querying') : t('tray.refresh') }}
        </button>
      </header>

      <div v-if="!snapshot || usageWindows.length" class="quota-wrap" :class="{ 'is-loading': loading }">
        <svg v-if="usageWindows.length" class="quota-rings" viewBox="0 0 240 240" aria-hidden="true">
          <circle v-if="outerWindow" class="ring-track ring-track--outer" cx="120" cy="120" r="103" />
          <circle
            v-if="outerWindow"
            class="ring-progress ring-progress--outer"
            cx="120" cy="120" r="103"
            :stroke-dasharray="2 * Math.PI * 103"
            :stroke-dashoffset="ringOffset(windowPercent(outerWindow.window), 103)"
          />
          <circle v-if="innerWindow" class="ring-track ring-track--inner" cx="120" cy="120" r="78" />
          <circle
            v-if="innerWindow"
            class="ring-progress ring-progress--inner"
            cx="120" cy="120" r="78"
            :stroke-dasharray="2 * Math.PI * 78"
            :stroke-dashoffset="ringOffset(windowPercent(innerWindow.window), 78)"
          />
        </svg>
        <div v-if="usageWindows.length" class="quota-values">
          <small>{{ t('tray.used') }}</small>
          <div class="quota-metrics">
            <template v-for="(item, index) in usageWindows" :key="item.key">
              <span v-if="index" class="quota-divider" />
              <div class="quota-metric" :class="`quota-metric--${item.tone}`">
                <span>{{ item.label }}</span>
                <strong>{{ windowPercent(item.window) }}<em>%</em></strong>
              </div>
            </template>
          </div>
        </div>
        <div v-else-if="error" class="quota-message">
          <strong>{{ t('tray.failed') }}</strong>
          <span>{{ error }}</span>
          <button @click="refresh">{{ t('tray.retry') }}</button>
        </div>
        <div v-else class="quota-message quota-message--loading">{{ t('tray.querying') }}</div>
      </div>

      <div class="credit-card">
        <div class="credit-heading">
          <span class="credit-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24"><rect x="3" y="5" width="18" height="14" rx="3"/><path d="M3 10h18"/><path d="M7 15h3"/></svg>
          </span>
          <strong>{{ t('tray.reset_credit') }}</strong>
        </div>
        <div v-if="resetCards.length" class="credit-list">
          <div v-for="(card, index) in resetCards" :key="`${card.expires_at ?? 'unknown'}-${index}`" class="credit-row">
            <span class="credit-label">
              <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="6" width="18" height="12" rx="3"/><path d="M3 10h18"/></svg>
              {{ t('tray.reset_credit_item', { n: index + 1 }) }}
            </span>
            <span class="credit-expiry">{{ formatExpiry(card.expires_at) }}</span>
          </div>
        </div>
        <p v-else class="credit-empty">{{ t('tray.no_reset_credit') }}</p>
      </div>

      <footer class="tray-footer">
        <span v-if="snapshot">{{ t('tray.last_query', { time: formatDate(snapshot.last_query_at) }) }}</span>
        <span v-if="cooldownMinutes" class="cooldown">{{ t('tray.cooldown', { n: cooldownMinutes }) }}</span>
      </footer>
    </section>
  </main>
</template>

<style scoped>
:global(html), :global(body), :global(#app) {
  background: transparent !important;
  overflow: hidden;
}

.tray-shell {
  width: 100%;
  height: 100%;
  padding: 6px;
  color: #15213b;
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", "PingFang SC", sans-serif;
  user-select: none;
}

.tray-panel {
  height: 100%;
  padding: 18px 20px 13px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, .78);
  border-radius: 24px;
  background:
    radial-gradient(circle at 88% 12%, rgba(125, 158, 255, .18), transparent 34%),
    rgba(247, 250, 255, .93);
  box-shadow: 0 20px 46px rgba(34, 57, 103, .22), inset 0 1px 0 rgba(255, 255, 255, .88);
  backdrop-filter: blur(28px) saturate(150%);
}

.tray-header, .tray-title, .refresh-button, .credit-heading, .credit-row, .credit-label, .tray-footer {
  display: flex;
  align-items: center;
}

.tray-header { justify-content: space-between; height: 42px; }
.tray-title { gap: 11px; }
.tray-title h1 { margin: 0; font-size: 23px; line-height: 1; letter-spacing: -.03em; }
.tray-logo, .credit-icon { display: grid; place-items: center; color: white; }
.tray-logo {
  width: 38px;
  height: 38px;
  border-radius: 11px;
  background: linear-gradient(145deg, #5898ff, #6554ee);
  box-shadow: 0 8px 18px rgba(80, 105, 238, .3);
}
.tray-logo svg { width: 25px; height: 25px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; }

.refresh-button {
  gap: 6px;
  border: 0;
  border-radius: 13px;
  padding: 7px 11px;
  color: #65718a;
  background: rgba(255, 255, 255, .62);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, .58);
  font: inherit;
  cursor: pointer;
}
.refresh-button:disabled { opacity: .64; cursor: default; }
.refresh-button svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.9; stroke-linecap: round; stroke-linejoin: round; }

.quota-wrap {
  display: grid;
  grid-template-columns: 184px minmax(0, 1fr);
  align-items: center;
  width: 100%;
  height: 196px;
  margin: 2px auto 4px;
  transition: opacity .2s ease;
}
.quota-wrap.is-loading { opacity: .72; }
.quota-rings { width: 184px; height: 184px; overflow: visible; filter: drop-shadow(0 8px 12px rgba(55, 93, 174, .1)); }
.ring-track, .ring-progress { fill: none; }
.ring-track--outer { stroke: rgba(51, 174, 117, .12); stroke-width: 14; }
.ring-track--inner { stroke: rgba(74, 117, 242, .12); stroke-width: 14; }
.ring-progress { transform: rotate(-90deg); transform-origin: 120px 120px; stroke-linecap: round; transition: stroke-dashoffset .45s ease; }
.ring-progress--outer { stroke: #54bd83; stroke-width: 14; }
.ring-progress--inner { stroke: #4f7cf3; stroke-width: 14; }

.quota-values, .quota-message {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 0;
  padding-left: 18px;
}
.quota-values small { margin-bottom: 11px; color: #8490a7; font-size: 11px; letter-spacing: .12em; }
.quota-metrics { display: flex; flex-direction: column; gap: 10px; }
.quota-metric { display: grid; grid-template-columns: 34px minmax(0, 1fr); align-items: baseline; min-width: 0; }
.quota-metric > span { font-size: 14px; font-weight: 700; opacity: .82; }
.quota-metric strong { font-size: 34px; line-height: 1; letter-spacing: -.045em; font-variant-numeric: tabular-nums; }
.quota-metric strong em { margin-left: 1px; font-size: 16px; font-style: normal; letter-spacing: -.02em; }
.quota-metric--primary { color: #4f7cf3; }
.quota-metric--secondary { color: #40a96e; }
.quota-divider { height: 1px; background: linear-gradient(90deg, rgba(67, 85, 128, .15), transparent); }
.quota-message { gap: 5px; color: #707c95; }
.quota-message strong { color: #d35252; }
.quota-message span { max-height: 48px; overflow: hidden; font-size: 11px; }
.quota-message button { border: 0; color: #4f7cf3; background: none; cursor: pointer; }
.quota-message--loading { font-size: 13px; }

.credit-card {
  min-height: 70px;
  max-height: 176px;
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, .72);
  border-radius: 16px;
  background: rgba(255, 255, 255, .58);
  box-shadow: 0 8px 22px rgba(52, 76, 126, .08);
}
.credit-icon {
  flex: 0 0 auto;
  width: 34px;
  height: 34px;
  border-radius: 10px;
  background: linear-gradient(145deg, #62ca91, #45b877);
  box-shadow: 0 7px 15px rgba(63, 177, 112, .2);
}
.credit-icon svg { width: 20px; height: 20px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
.credit-heading { gap: 10px; min-height: 34px; }
.credit-heading strong { font-size: 15px; }
.credit-list { margin-top: 5px; }
.credit-row { justify-content: space-between; min-height: 28px; gap: 14px; border-top: 1px solid rgba(71, 91, 130, .08); }
.credit-label { gap: 6px; color: #3ca96b; font-size: 11.5px; white-space: nowrap; }
.credit-label svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.7; }
.credit-expiry { min-width: 0; overflow: hidden; color: #6e7990; font-size: 11.5px; text-overflow: ellipsis; white-space: nowrap; }
.credit-empty { margin: 7px 0 0 44px; color: #6e7990; font-size: 12px; }

.tray-footer {
  justify-content: space-between;
  min-height: 24px;
  gap: 12px;
  padding: 7px 2px 0;
  color: #8792a8;
  font-size: 10.5px;
  white-space: nowrap;
}
.cooldown { color: #6b78a0; }

@media (prefers-color-scheme: dark) {
  .tray-shell { color: #edf2ff; }
  .tray-panel {
    border-color: rgba(255, 255, 255, .12);
    background: radial-gradient(circle at 88% 12%, rgba(82, 105, 190, .24), transparent 36%), rgba(25, 31, 45, .94);
    box-shadow: 0 22px 50px rgba(0, 0, 0, .44), inset 0 1px 0 rgba(255, 255, 255, .08);
  }
  .refresh-button, .credit-card { background: rgba(255, 255, 255, .07); border-color: rgba(255, 255, 255, .08); }
  .refresh-button, .credit-expiry, .credit-empty, .tray-footer { color: #a7b1c6; }
  .credit-row { border-color: rgba(255, 255, 255, .07); }
}
</style>
