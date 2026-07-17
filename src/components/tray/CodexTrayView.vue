<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { getCodexTrayUsage, getGrokUsage, getUsageProviderAvailability, resizeUsageTray } from '@/lib/api'
import type {
  CodexTraySnapshot,
  GrokUsage,
  ResetCreditEntry,
  UsageProviderAvailability,
  UsageWindow,
} from '@/lib/api'

const { t, locale } = useI18n()
type UsageProvider = 'codex' | 'grok-build'
const selectedProvider = ref<UsageProvider>(preferredProviderFromAccounts())
const availability = ref<UsageProviderAvailability | null>(null)
const snapshot = ref<CodexTraySnapshot | null>(null)
const grokUsage = ref<GrokUsage | null>(null)
// The real tray window starts hidden and compact. Defaulting to loading avoids
// a flash of the empty-state layout before the first tray-click event arrives.
const loading = ref(import.meta.env.MODE !== 'web')
const compactLoading = ref(import.meta.env.MODE !== 'web')
const providerErrors = ref<Record<UsageProvider, string | null>>({
  codex: null,
  'grok-build': null,
})
const error = computed(() => providerErrors.value[selectedProvider.value])
const loginUnavailable = ref(false)
const queriedProviders = ref<Record<UsageProvider, boolean>>({
  codex: false,
  'grok-build': false,
})
const unlisteners: UnlistenFn[] = []
const initialLoading = computed(() => compactLoading.value)
let refreshSequence = 0
let resizeSequence = 0

type WindowTone = 'primary' | 'secondary' | 'monthly'
interface TrayUsageWindow {
  key: string
  label: string
  tone: WindowTone
  window: UsageWindow
}

function preferredProviderFromAccounts(): UsageProvider {
  return localStorage.getItem('ah-switch-agent') === 'grok-build' ? 'grok-build' : 'codex'
}

function providerAvailable(provider: UsageProvider, status: UsageProviderAvailability) {
  return provider === 'codex' ? status.codex : status.grok_build
}

function availableProvider(
  preferred: UsageProvider,
  status: UsageProviderAvailability,
): UsageProvider | null {
  if (providerAvailable(preferred, status)) return preferred
  const fallback: UsageProvider = preferred === 'codex' ? 'grok-build' : 'codex'
  return providerAvailable(fallback, status) ? fallback : null
}

function windowLabel(seconds: number) {
  if (Math.abs(seconds - 18_000) <= 600) return '5h'
  if (Math.abs(seconds - 604_800) <= 3_600) return '7d'
  if (Math.abs(seconds - 2_592_000) <= 86_400) return '30d'
  if (seconds >= 86_400) return `${Math.round(seconds / 86_400)}d`
  return `${Math.round(seconds / 3_600)}h`
}

function windowTone(seconds: number): WindowTone {
  if (Math.abs(seconds - 2_592_000) <= 86_400 || seconds > 1_209_600) return 'monthly'
  if (seconds >= 86_400) return 'secondary'
  return 'primary'
}

const usageWindows = computed<TrayUsageWindow[]>(() => {
  const returned = snapshot.value?.usage.usage_windows ?? []
  const fallback = [
    snapshot.value?.usage.primary_window,
    snapshot.value?.usage.secondary_window,
  ].filter((window): window is UsageWindow => Boolean(window?.window_seconds))
  const windows = (returned.length ? returned : fallback)
    .filter(window => window.window_seconds > 0)
    .sort((left, right) => left.window_seconds - right.window_seconds)
    .filter((window, index, all) => index === 0 || window.window_seconds !== all[index - 1].window_seconds)

  return windows.map(window => ({
    key: String(window.window_seconds),
    label: windowLabel(window.window_seconds),
    tone: windowTone(window.window_seconds),
    window,
  }))
})

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

const grokAccountName = computed(() =>
  grokUsage.value?.account_name || t('switch.grok_default_account')
)
const grokPlanBadge = computed(() =>
  t('switch.usage_plan_badge', { plan: grokUsage.value?.plan_type || 'Grok' })
)
const grokPeriodLabel = computed(() =>
  grokUsage.value?.period_type === 'monthly'
    ? t('switch.grok_monthly_window')
    : t('switch.grok_weekly_window')
)

function clampHeight(height: number) {
  return Math.min(620, Math.max(168, height))
}

function contentHeight() {
  if (loginUnavailable.value) return 254
  if (error.value) return 356

  if (selectedProvider.value === 'codex') {
    if (!snapshot.value) return 270
    const windowCount = usageWindows.value.length
    const creditCount = resetCards.value.length
    const quotaHeight = windowCount > 0
      ? 33 + windowCount * 38 + Math.max(0, windowCount - 1) * 16
      : 180
    const creditHeight = snapshot.value ? (creditCount > 0 ? creditCount * 49 : 60) : 0
    // Base chrome 128px + 48px provider switch.
    return clampHeight(176 + quotaHeight + creditHeight)
  }

  if (!grokUsage.value) return 270
  // Base chrome 128px + provider switch 48px + metadata 44px + quota 71px.
  return clampHeight(291 + (grokUsage.value.stale ? 44 : 0))
}

async function applyContentHeight() {
  const sequence = ++resizeSequence
  await nextTick()
  await new Promise<void>(resolve => requestAnimationFrame(() => resolve()))
  if (sequence !== resizeSequence || compactLoading.value) return
  try {
    await resizeUsageTray(contentHeight())
  } catch {
    // Browser preview and unsupported platforms may not own a native tray window.
  }
}

// Resize from post-render state instead of relying on the query callback's
// timing. This covers cached tab switches as well as the moment fresh data
// replaces the compact loading view.
watch(
  [selectedProvider, snapshot, grokUsage, error, loginUnavailable, compactLoading],
  () => {
    if (!compactLoading.value) void applyContentHeight()
  },
  { flush: 'post' },
)

function safePercent(window: UsageWindow) {
  return Math.min(100, Math.max(0, window.used_percent ?? 0))
}

function progressWidth(window: UsageWindow) {
  const percent = safePercent(window)
  return percent === 0 ? '0%' : `max(${percent}%, 46px)`
}

function formatDate(value: number | string, withSeconds = false) {
  const date = typeof value === 'number' ? new Date(value * 1000) : new Date(value)
  return new Intl.DateTimeFormat(locale.value, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: withSeconds ? '2-digit' : undefined,
    hour12: false,
  }).format(date)
}

function formatExpiry(value?: string | null) {
  if (!value) return t('tray.expiry_unknown')
  return t('tray.expiry_at', { date: formatDate(value, true) })
}

async function refresh(compact = false, syncWithAccounts = false) {
  const sequence = ++refreshSequence
  let provider = selectedProvider.value
  if (compact) {
    compactLoading.value = true
    try { await resizeUsageTray(168) } catch {}
  }
  loading.value = true
  loginUnavailable.value = false
  try {
    const status = await getUsageProviderAvailability()
    if (sequence !== refreshSequence) return
    availability.value = status

    const preferred = syncWithAccounts
      ? preferredProviderFromAccounts()
      : selectedProvider.value
    const available = availableProvider(preferred, status)
    if (!available) {
      snapshot.value = null
      grokUsage.value = null
      loginUnavailable.value = true
      return
    }

    provider = available
    selectedProvider.value = provider
    providerErrors.value[provider] = null
    queriedProviders.value[provider] = true
    if (provider === 'codex') {
      snapshot.value = null
      const result = await getCodexTrayUsage()
      if (sequence !== refreshSequence) return
      snapshot.value = result
    } else {
      grokUsage.value = null
      const result = await getGrokUsage()
      if (sequence !== refreshSequence) return
      grokUsage.value = result
    }
  } catch (reason: any) {
    if (sequence !== refreshSequence) return
    providerErrors.value[provider] = String(reason?.message || reason)
  } finally {
    if (sequence === refreshSequence) {
      compactLoading.value = false
      loading.value = false
    }
  }
}

async function selectProvider(provider: UsageProvider) {
  if (provider === selectedProvider.value || loading.value) return
  if (availability.value && !providerAvailable(provider, availability.value)) return
  selectedProvider.value = provider
  if (!queriedProviders.value[provider]) {
    await refresh()
  }
}

onMounted(async () => {
  // Browser-only mock route should be immediately previewable. The real hidden
  // tray window waits for a tray click so startup never consumes a query.
  if (import.meta.env.MODE === 'web') {
    await refresh(true, true)
    return
  }

  const listeners = await Promise.all([
    listen('usage-tray-opened', () => refresh(true, true)),
  ])
  unlisteners.push(...listeners)
})

onBeforeUnmount(() => {
  unlisteners.forEach(unlisten => unlisten())
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
        <button v-if="!initialLoading" class="refresh-button" :disabled="loading" @click="refresh()">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 0 0-14.9-4M4 4v5h5"/><path d="M4 13a8 8 0 0 0 14.9 4M20 20v-5h-5"/></svg>
          {{ loading ? t('tray.querying') : t('tray.refresh') }}
        </button>
      </header>

      <div v-if="initialLoading" class="initial-loading" role="status">
        <span class="loading-spinner" aria-hidden="true" />
        <span>{{ t('tray.query_wait') }}</span>
      </div>

      <template v-else>
        <div class="provider-switch" role="tablist" :aria-label="t('tray.provider')">
          <button
            class="provider-option"
            :class="{ 'is-active': selectedProvider === 'codex' }"
            role="tab"
            :aria-selected="selectedProvider === 'codex'"
            :disabled="loading || availability?.codex === false"
            @click="selectProvider('codex')"
          >
            Codex
          </button>
          <button
            class="provider-option"
            :class="{ 'is-active': selectedProvider === 'grok-build' }"
            role="tab"
            :aria-selected="selectedProvider === 'grok-build'"
            :disabled="loading || availability?.grok_build === false"
            @click="selectProvider('grok-build')"
          >
            Grok Build
          </button>
        </div>

        <div v-if="loginUnavailable" class="login-empty" role="status">
          <span class="login-empty-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M8.5 16.2c.8-1.8 2-2.7 3.5-2.7s2.7.9 3.5 2.7"/><circle cx="12" cy="9" r="2.3"/></svg>
          </span>
          <span>{{ t('tray.login_required') }}</span>
        </div>

        <template v-else-if="selectedProvider === 'codex'">
          <div class="quota-wrap" :class="{ 'is-loading': loading }">
            <div v-if="usageWindows.length" class="quota-list">
              <div v-for="item in usageWindows" :key="item.key" class="quota-row" :class="`quota-row--${item.tone}`">
                <div class="quota-label">
                  <span class="quota-dot" aria-hidden="true" />
                  <span>{{ item.label }} {{ t('tray.limit') }}</span>
                </div>
                <div class="quota-track">
                  <div class="quota-fill" :style="{ width: progressWidth(item.window) }">
                    <span v-if="safePercent(item.window)">{{ safePercent(item.window) }}%</span>
                  </div>
                  <span v-if="!safePercent(item.window)" class="quota-zero">0%</span>
                </div>
              </div>
            </div>
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh()">{{ t('tray.retry') }}</button>
            </div>
            <div
              v-else
              class="quota-message quota-message--loading"
              :class="{ 'quota-message--compact': !snapshot }"
            >
              {{ loading ? t('tray.query_wait') : t('tray.no_usage') }}
            </div>
          </div>

          <div v-if="snapshot && !error" class="credit-section">
            <div v-if="resetCards.length" class="credit-list">
              <div v-for="(card, index) in resetCards" :key="`${card.expires_at ?? 'unknown'}-${index}`" class="credit-row">
                <span class="credit-label">
                  <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8.5"/><path d="M12 7.5v5l3.2 2"/></svg>
                  {{ t('tray.reset_credit') }}
                </span>
                <span class="credit-expiry">{{ formatExpiry(card.expires_at) }}</span>
              </div>
            </div>
            <p v-else class="credit-empty">{{ t('tray.no_reset_credit') }}</p>
          </div>

          <footer class="tray-footer">
            <span v-if="snapshot">{{ t('tray.last_query', { time: formatDate(snapshot.last_query_at) }) }}</span>
          </footer>
        </template>

        <template v-else>
          <div v-if="grokUsage" class="grok-meta">
            <span class="meta-pill meta-pill--account">{{ grokAccountName }}</span>
            <span class="meta-pill">{{ grokPlanBadge }}</span>
            <span class="meta-pill" :class="grokUsage.source === 'live' ? 'meta-pill--live' : 'meta-pill--cache'">
              {{ grokUsage.source === 'live' ? t('switch.grok_live_data') : t('switch.grok_cached_data') }}
            </span>
          </div>

          <div v-if="grokUsage?.stale" class="grok-warning">
            {{ t('switch.grok_stale_warning') }}
          </div>

          <div class="quota-wrap quota-wrap--grok" :class="{ 'is-loading': loading }">
            <div v-if="grokUsage" class="quota-list">
              <div class="quota-row quota-row--secondary">
                <div class="quota-label">
                  <span class="quota-dot" aria-hidden="true" />
                  <span>{{ grokPeriodLabel }}</span>
                </div>
                <div class="quota-track">
                  <div class="quota-fill" :style="{ width: progressWidth(grokUsage.usage_window) }">
                    <span v-if="safePercent(grokUsage.usage_window)">{{ safePercent(grokUsage.usage_window) }}%</span>
                  </div>
                  <span v-if="!safePercent(grokUsage.usage_window)" class="quota-zero">0%</span>
                </div>
              </div>
            </div>
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh()">{{ t('tray.retry') }}</button>
            </div>
            <div
              v-else
              class="quota-message quota-message--loading"
              :class="{ 'quota-message--compact': !grokUsage }"
            >
              {{ loading ? t('tray.query_wait') : t('tray.no_usage') }}
            </div>
          </div>

          <footer class="tray-footer">
            <span v-if="grokUsage">{{ t('tray.last_query', { time: formatDate(grokUsage.fetched_at) }) }}</span>
          </footer>
        </template>
      </template>
    </section>
  </main>
</template>

<style scoped>
:global(html), :global(body), :global(#app) {
  width: 100%;
  height: 100%;
  margin: 0;
  background: transparent !important;
  overflow: hidden;
}

:global(*), :global(*::before), :global(*::after) {
  box-sizing: border-box;
}

.tray-shell {
  width: 100%;
  height: 100%;
  min-width: 0;
  padding: 8px;
  overflow: hidden;
  color: #172036;
  font-family: "SF Pro Text", "Segoe UI", "PingFang SC", sans-serif;
  user-select: none;
}

.tray-panel {
  width: 100%;
  height: 100%;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 20px 22px 14px;
  overflow: hidden;
  border-radius: 25px;
  background: rgba(251, 252, 255, .97);
  box-shadow: 0 2px 6px rgba(35, 48, 75, .18);
}

.tray-header, .tray-title, .refresh-button, .quota-row, .quota-label, .credit-row, .credit-label, .tray-footer {
  display: flex;
  align-items: center;
}

.tray-header { width: 100%; min-width: 0; flex: 0 0 auto; justify-content: space-between; min-height: 46px; }
.tray-title { min-width: 0; gap: 13px; }
.tray-title h1 { margin: 0; font-size: 24px; line-height: 1; letter-spacing: -.035em; }
.tray-logo { display: grid; place-items: center; color: white; }
.tray-logo {
  width: 44px;
  height: 44px;
  border-radius: 13px;
  background: linear-gradient(145deg, #557cf5, #5f45ee);
  box-shadow: 0 9px 20px rgba(80, 91, 226, .28);
}
.tray-logo svg { width: 28px; height: 28px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; }

.refresh-button {
  flex: 0 0 auto;
  min-height: 42px;
  gap: 7px;
  border: 1px solid #e2e6ef;
  border-radius: 13px;
  padding: 8px 13px;
  color: #5d687f;
  background: rgba(255, 255, 255, .84);
  font: inherit;
  cursor: pointer;
  transition: background-color .16s ease, border-color .16s ease, transform .16s ease;
}
.refresh-button:hover:not(:disabled) { border-color: #d4daea; background: #fff; }
.refresh-button:active:not(:disabled) { transform: translateY(1px); }
.refresh-button:focus-visible { outline: 2px solid rgba(65, 116, 244, .45); outline-offset: 2px; }
.refresh-button:disabled { opacity: .58; cursor: default; }
.refresh-button svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.9; stroke-linecap: round; stroke-linejoin: round; }

.initial-loading {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: #68748c;
  font-size: 14px;
}
.loading-spinner {
  width: 18px;
  height: 18px;
  border: 2px solid #dce3f1;
  border-top-color: #4f72ee;
  border-radius: 50%;
  animation: tray-spin .8s linear infinite;
}
@keyframes tray-spin { to { transform: rotate(360deg); } }

.provider-switch {
  flex: 0 0 34px;
  height: 34px;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 3px;
  margin-top: 14px;
  padding: 3px;
  border-radius: 999px;
  background: #edf0f6;
}
.provider-option {
  min-width: 0;
  overflow: hidden;
  border: 0;
  border-radius: 999px;
  padding: 0 14px;
  color: #69758d;
  background: transparent;
  font: inherit;
  font-size: 13px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
  transition: color .16s ease, background-color .16s ease, box-shadow .16s ease;
}
.provider-option:hover:not(:disabled):not(.is-active) { color: #33405a; }
.provider-option.is-active {
  color: #263450;
  background: rgba(255, 255, 255, .96);
  box-shadow: 0 1px 4px rgba(38, 50, 78, .14);
}
.provider-option:focus-visible { outline: 2px solid rgba(65, 116, 244, .45); outline-offset: 1px; }
.provider-option:disabled { opacity: .48; cursor: default; }
.provider-option.is-active:disabled { opacity: 1; }

.login-empty {
  flex: 0 0 110px;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 10px 4px;
  color: #66728a;
  font-size: 13px;
  line-height: 1.55;
}
.login-empty-icon {
  flex: 0 0 auto;
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  border-radius: 999px;
  color: #7d89a2;
  background: #edf0f6;
}
.login-empty-icon svg {
  width: 21px;
  height: 21px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.grok-meta {
  flex: 0 0 44px;
  height: 44px;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 7px;
  overflow: hidden;
}
.meta-pill {
  flex: 0 0 auto;
  max-width: 128px;
  overflow: hidden;
  border-radius: 999px;
  padding: 5px 9px;
  color: #5f6b82;
  background: #edf0f6;
  font-size: 11px;
  font-weight: 600;
  line-height: 1;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.meta-pill--account { flex: 0 1 auto; }
.meta-pill--live { color: #238b4d; background: #e5f7eb; }
.meta-pill--cache { color: #6f7890; background: #edf0f6; }
.grok-warning {
  flex: 0 0 44px;
  min-height: 44px;
  display: flex;
  align-items: center;
  margin: 0;
  border-radius: 14px;
  padding: 7px 11px;
  color: #bb4b4b;
  background: #fff0f0;
  font-size: 11px;
  line-height: 1.35;
}

.quota-wrap {
  flex: 0 0 auto;
  padding: 18px 0 14px;
  border-bottom: 1px solid #e5e9f1;
  transition: opacity .18s ease;
}
.quota-wrap.is-loading { opacity: .72; }
.quota-list { display: flex; flex-direction: column; gap: 16px; }
.quota-row { min-width: 0; min-height: 38px; gap: 14px; }
.quota-label {
  flex: 0 0 92px;
  gap: 10px;
  color: #20283a;
  font-size: 15px;
  font-weight: 650;
  text-transform: uppercase;
  white-space: nowrap;
}
.quota-dot { flex: 0 0 auto; width: 12px; height: 12px; border-radius: 50%; background: currentColor; }
.quota-track {
  position: relative;
  flex: 1 1 auto;
  min-width: 0;
  height: 32px;
  overflow: hidden;
  border-radius: 999px;
  background: #edf0f6;
}
.quota-fill {
  height: 100%;
  max-width: 100%;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  border-radius: inherit;
  transition: width .42s cubic-bezier(.2, .8, .2, 1);
}
.quota-fill span {
  padding: 0 11px;
  color: white;
  font-size: 16px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
}
.quota-zero {
  position: absolute;
  top: 50%;
  left: 12px;
  transform: translateY(-50%);
  color: #7e889e;
  font-size: 14px;
  font-variant-numeric: tabular-nums;
}
.quota-row--primary .quota-dot { background: #2f72f2; }
.quota-row--primary .quota-fill { background: linear-gradient(90deg, #2f72f2, #3165ed); }
.quota-row--secondary .quota-dot { background: #32bd68; }
.quota-row--secondary .quota-fill { background: linear-gradient(90deg, #39c66e, #30b860); }
.quota-row--monthly .quota-dot { background: #7547ef; }
.quota-row--monthly .quota-fill { background: linear-gradient(90deg, #8155f4, #6f42e8); }

.quota-message {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: flex-start;
  min-height: 147px;
  gap: 6px;
  color: #6d7890;
}
.quota-message strong { color: #d35252; }
.quota-message span { max-height: 42px; overflow: hidden; font-size: 12px; }
.quota-message button { min-height: 34px; padding: 0; border: 0; color: #316fe8; background: none; cursor: pointer; }
.quota-message--loading { align-items: center; font-size: 13px; }
.quota-message--compact { min-height: 61px; }

.credit-section { flex: 1 1 auto; min-height: 0; overflow: hidden; }
.credit-list { height: 100%; overflow-y: auto; scrollbar-width: none; }
.credit-list::-webkit-scrollbar { display: none; }
.credit-row {
  min-width: 0;
  justify-content: space-between;
  min-height: 49px;
  gap: 14px;
  border-bottom: 1px solid #e8ebf2;
}
.credit-label { flex: 0 0 auto; gap: 10px; color: #20283a; font-size: 14px; white-space: nowrap; }
.credit-label svg { width: 21px; height: 21px; fill: none; stroke: #68748c; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
.credit-expiry {
  min-width: 0;
  overflow: hidden;
  color: #65718a;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.credit-empty { margin: 0; padding: 22px 0; color: #7d879a; font-size: 13px; }

.tray-footer {
  flex: 0 0 auto;
  min-height: 32px;
  padding-top: 9px;
  color: #6f7a91;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

@media (prefers-color-scheme: dark) {
  .tray-shell { color: #edf2ff; }
  .tray-panel {
    background: rgba(26, 31, 43, .97);
    box-shadow: 0 2px 6px rgba(0, 0, 0, .36);
  }
  .refresh-button { color: #b8c1d3; background: rgba(255, 255, 255, .06); border-color: rgba(255, 255, 255, .1); }
  .refresh-button:hover:not(:disabled) { background: rgba(255, 255, 255, .1); border-color: rgba(255, 255, 255, .16); }
  .provider-switch { background: rgba(255, 255, 255, .08); }
  .provider-option { color: #aab5ca; }
  .provider-option:hover:not(:disabled):not(.is-active) { color: #edf2ff; }
  .provider-option.is-active { color: #eef3ff; background: rgba(255, 255, 255, .13); box-shadow: 0 1px 4px rgba(0, 0, 0, .28); }
  .login-empty { color: #b7c1d5; }
  .login-empty-icon { color: #aab5ca; background: rgba(255, 255, 255, .08); }
  .meta-pill { color: #b6c0d4; background: rgba(255, 255, 255, .08); }
  .meta-pill--live { color: #78d99b; background: rgba(50, 189, 104, .14); }
  .meta-pill--cache { color: #aab5ca; background: rgba(255, 255, 255, .08); }
  .grok-warning { color: #ffabab; background: rgba(211, 82, 82, .13); }
  .quota-wrap, .credit-row { border-color: rgba(255, 255, 255, .08); }
  .quota-label, .credit-label { color: #edf2ff; }
  .quota-track { background: rgba(255, 255, 255, .09); }
  .initial-loading { color: #b7c1d5; }
  .loading-spinner { border-color: rgba(255, 255, 255, .14); border-top-color: #819bff; }
  .credit-expiry, .credit-empty, .tray-footer { color: #a7b1c6; }
}
</style>
