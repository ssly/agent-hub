<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { getCodexTrayUsage, getGrokUsage, getKimiUsage, getUsageProviderAvailability, resizeUsageTray } from '@/lib/api'
import type {
  CodexTraySnapshot,
  GrokUsage,
  KimiUsage,
  ResetCreditEntry,
  UsageProviderAvailability,
  UsageWindow,
} from '@/lib/api'

const { t, locale } = useI18n()
type UsageProvider = 'codex' | 'grok-build' | 'kimi-code'
const selectedProvider = ref<UsageProvider>(preferredProviderFromAccounts())
const availability = ref<UsageProviderAvailability | null>(null)
const snapshot = ref<CodexTraySnapshot | null>(null)
const grokUsage = ref<GrokUsage | null>(null)
const kimiUsage = ref<KimiUsage | null>(null)
// The real tray window starts hidden and compact. Defaulting to loading avoids
// a flash of the empty-state layout before the first tray-click event arrives.
const loading = ref(import.meta.env.MODE !== 'web')
const compactLoading = ref(import.meta.env.MODE !== 'web')
const providerErrors = ref<Record<UsageProvider, string | null>>({
  codex: null,
  'grok-build': null,
  'kimi-code': null,
})
const error = computed(() => providerErrors.value[selectedProvider.value])
const loginUnavailable = ref(false)
const queriedProviders = ref<Record<UsageProvider, boolean>>({
  codex: false,
  'grok-build': false,
  'kimi-code': false,
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
  const stored = localStorage.getItem('ah-switch-agent')
  if (stored === 'grok-build') return 'grok-build'
  if (stored === 'kimi-code') return 'kimi-code'
  return 'codex'
}

function providerAvailable(provider: UsageProvider, status: UsageProviderAvailability) {
  if (provider === 'codex') return status.codex
  if (provider === 'grok-build') return status.grok_build
  return status.kimi_code
}

// Fallback order mirrors the provider-switch tab order so a preferred provider
// that is signed out falls back to the next available one deterministically.
const PROVIDER_ORDER: UsageProvider[] = ['codex', 'grok-build', 'kimi-code']

function availableProvider(
  preferred: UsageProvider,
  status: UsageProviderAvailability,
): UsageProvider | null {
  if (providerAvailable(preferred, status)) return preferred
  for (const candidate of PROVIDER_ORDER) {
    if (candidate !== preferred && providerAvailable(candidate, status)) return candidate
  }
  return null
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

// Kimi exposes the same multi-window shape as Codex (5h primary + weekly), so
// we reuse the same windowing logic, just sourced from kimiUsage.
const kimiWindows = computed<TrayUsageWindow[]>(() => {
  const windows = (kimiUsage.value?.usage_windows ?? [])
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

const kimiAccountName = computed(() =>
  kimiUsage.value?.account_name || t('switch.kimi_default_account')
)
const kimiAuthBadge = computed(() => {
  const method = kimiUsage.value?.auth_method
  if (method === 'METHOD_API_KEY') return t('switch.kimi_auth_api_key')
  if (method === 'METHOD_OAUTH') return t('switch.kimi_auth_oauth')
  return method || t('switch.kimi_auth_api_key')
})

function clampHeight(height: number) {
  return Math.min(620, Math.max(120, height))
}

function contentHeight() {
  if (loginUnavailable.value) return 210
  if (error.value) return 300

  // Constants mirror the tray stylesheet: base chrome = panel padding 24 +
  // header 32 + provider switch 40 + footer 24 = 120. Quota = wrap padding 22
  // + ring (100 single / 88 multi) + gap 6 + label 14. Credit rows = chips of
  // 28px + 8px gaps + 10px section padding, 3 chips per row.
  if (selectedProvider.value === 'codex') {
    if (!snapshot.value) return 240
    const creditCount = resetCards.value.length
    const quotaHeight = usageWindows.value.length > 0
      ? (usageWindows.value.length === 1 ? 142 : 130)
      : 70
    const creditRows = Math.ceil(creditCount / 3)
    const creditHeight = snapshot.value
      ? (creditCount > 0 ? 10 + creditRows * 28 + (creditRows - 1) * 8 : 44)
      : 0
    return clampHeight(120 + quotaHeight + creditHeight)
  }

  if (selectedProvider.value === 'kimi-code') {
    // Same multi-window ring layout as Codex, minus the credit section.
    if (!kimiUsage.value) return 240
    const quotaHeight = kimiWindows.value.length > 0
      ? (kimiWindows.value.length === 1 ? 142 : 130)
      : 70
    return clampHeight(120 + quotaHeight)
  }

  if (!grokUsage.value) return 240
  // Base chrome 120 + metadata 44 + quota ring 142.
  return clampHeight(306 + (grokUsage.value.stale ? 40 : 0))
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
  [selectedProvider, snapshot, grokUsage, kimiUsage, error, loginUnavailable, compactLoading],
  () => {
    if (!compactLoading.value) void applyContentHeight()
  },
  { flush: 'post' },
)

function safePercent(window: UsageWindow) {
  return Math.min(100, Math.max(0, window.used_percent ?? 0))
}

function remainingPercent(window: UsageWindow) {
  const remaining = window.remaining_percent ?? (100 - safePercent(window))
  return Math.min(100, Math.max(0, Math.round(remaining)))
}

// SVG progress-ring geometry (viewBox 0 0 120 120).
const RING_R = 52
const RING_C = 2 * Math.PI * RING_R
function ringDash(percent: number) {
  return `${((RING_C * percent) / 100).toFixed(1)} ${RING_C.toFixed(1)}`
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

// Reset-credit chips show the expiry split into two lines: date on top, exact
// time underneath ("有效期 2026/08/01" / "03:14:48").
function splitExpiry(value?: string | null) {
  if (!value) return { date: t('tray.expiry_unknown'), time: '' }
  const date = new Date(value)
  const datePart = new Intl.DateTimeFormat(locale.value, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(date)
  const timePart = new Intl.DateTimeFormat(locale.value, {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(date)
  return { date: datePart, time: timePart }
}

// force=true bypasses the shared 10-minute backend cache (Retry button).
// force=false uses the same snapshot as the Accounts view when still fresh.
async function refresh(compact = false, syncWithAccounts = false, force = false) {
  const sequence = ++refreshSequence
  let provider = selectedProvider.value
  if (compact) {
    compactLoading.value = true
    try { await resizeUsageTray(120) } catch {}
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
      kimiUsage.value = null
      loginUnavailable.value = true
      return
    }

    provider = available
    selectedProvider.value = provider
    providerErrors.value[provider] = null
    queriedProviders.value[provider] = true
    // Keep the previous payload until the response arrives so tab switches
    // never flash an empty layout. Backend cache keeps Accounts + tray aligned.
    if (provider === 'codex') {
      const result = await getCodexTrayUsage(force)
      if (sequence !== refreshSequence) return
      snapshot.value = result
    } else if (provider === 'kimi-code') {
      const result = await getKimiUsage(force)
      if (sequence !== refreshSequence) return
      kimiUsage.value = result
    } else {
      const result = await getGrokUsage(force)
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

/// Reopen the tray from the status-bar icon. Sync the selected provider with
/// the Accounts view, then load the shared backend snapshot (force=false so
/// we reuse data younger than 10 minutes instead of hitting the network again).
async function handleTrayOpened() {
  const status = await getUsageProviderAvailability()
  availability.value = status

  const preferred = preferredProviderFromAccounts()
  const available = availableProvider(preferred, status)
  if (!available) {
    loginUnavailable.value = true
    compactLoading.value = false
    loading.value = false
    return
  }
  loginUnavailable.value = false

  // Only flip the visible provider if the Accounts-view selection maps to a
  // usage provider we actually support — otherwise keep what the user last
  // looked at in this tray session.
  if (preferred !== selectedProvider.value) {
    selectedProvider.value = available
  }

  // Show compact loading only when we have nothing to display yet. When local
  // data already exists, soft-refresh from the shared backend cache so timers
  // stay current without a full loading flash.
  const hasLocal = available === 'codex'
    ? snapshot.value !== null
    : available === 'kimi-code'
      ? kimiUsage.value !== null
      : grokUsage.value !== null

  await refresh(!hasLocal, false, false)
}

async function selectProvider(provider: UsageProvider) {
  if (provider === selectedProvider.value || loading.value) return
  if (availability.value && !providerAvailable(provider, availability.value)) return
  selectedProvider.value = provider
  if (!queriedProviders.value[provider]) {
    await refresh(false, false, false)
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
    listen('usage-tray-opened', () => handleTrayOpened()),
  ])
  unlisteners.push(...listeners)
})

onBeforeUnmount(() => {
  unlisteners.forEach(unlisten => unlisten())
})
</script>

<template>
  <main class="tray-shell">
    <section class="tray-panel" :class="{ 'tray-panel--loading': initialLoading }">
      <header class="tray-header">
        <div class="tray-title">
          <span v-if="!initialLoading" class="tray-logo" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M4 15a8 8 0 1 1 16 0"/><path d="m12 15 4-5"/><circle cx="12" cy="15" r="1"/></svg>
          </span>
          <h1>{{ t('tray.title') }}</h1>
        </div>
      </header>

      <div v-if="initialLoading" class="initial-loading" role="status">
        <span class="loading-spinner" aria-hidden="true" />
      </div>

      <template v-else>
        <div class="provider-switch provider-switch--triple" role="tablist" :aria-label="t('tray.provider')">
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
          <button
            class="provider-option"
            :class="{ 'is-active': selectedProvider === 'kimi-code' }"
            role="tab"
            :aria-selected="selectedProvider === 'kimi-code'"
            :disabled="loading || availability?.kimi_code === false"
            @click="selectProvider('kimi-code')"
          >
            Kimi Code
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
            <div v-if="usageWindows.length" class="ring-list" :class="{ 'ring-list--single': usageWindows.length === 1 }">
              <div v-for="item in usageWindows" :key="item.key" class="ring-item" :class="`ring-item--${item.tone}`">
                <div class="ring-graph">
                  <svg viewBox="0 0 120 120" aria-hidden="true">
                    <circle class="ring-track" cx="60" cy="60" :r="RING_R" />
                    <circle
                      class="ring-fill"
                      cx="60" cy="60" :r="RING_R"
                      :stroke-dasharray="ringDash(remainingPercent(item.window))"
                      transform="rotate(-90 60 60)"
                    />
                  </svg>
                  <div class="ring-value">
                    <strong>{{ remainingPercent(item.window) }}%</strong>
                    <span>{{ t('tray.remaining') }}</span>
                  </div>
                </div>
                <span class="ring-label">{{ item.label }} {{ t('tray.limit') }}</span>
              </div>
            </div>
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh(false, false, true)">{{ t('tray.retry') }}</button>
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
            <div v-if="resetCards.length" class="credit-chips">
              <div v-for="(card, index) in resetCards" :key="`${card.expires_at ?? 'unknown'}-${index}`" class="credit-chip">
                <span class="credit-chip__date">
                  {{ splitExpiry(card.expires_at).time
                    ? `${t('tray.valid_until')} ${splitExpiry(card.expires_at).date}`
                    : splitExpiry(card.expires_at).date }}
                </span>
                <span v-if="splitExpiry(card.expires_at).time" class="credit-chip__time">
                  {{ splitExpiry(card.expires_at).time }}
                </span>
              </div>
            </div>
            <p v-else class="credit-empty">{{ t('tray.no_reset_credit') }}</p>
          </div>

          <footer class="tray-footer">
            <span v-if="snapshot">{{ t('tray.last_query', { time: formatDate(snapshot.last_query_at) }) }}</span>
          </footer>
        </template>

        <template v-else-if="selectedProvider === 'kimi-code'">
          <div v-if="kimiUsage" class="grok-meta">
            <span class="meta-pill meta-pill--account">{{ kimiAccountName }}</span>
            <span class="meta-pill">{{ kimiAuthBadge }}</span>
            <span class="meta-pill meta-pill--live">{{ t('switch.grok_live_data') }}</span>
          </div>

          <div class="quota-wrap" :class="{ 'is-loading': loading }">
            <div v-if="kimiWindows.length" class="ring-list" :class="{ 'ring-list--single': kimiWindows.length === 1 }">
              <div v-for="item in kimiWindows" :key="item.key" class="ring-item" :class="`ring-item--${item.tone}`">
                <div class="ring-graph">
                  <svg viewBox="0 0 120 120" aria-hidden="true">
                    <circle class="ring-track" cx="60" cy="60" :r="RING_R" />
                    <circle
                      class="ring-fill"
                      cx="60" cy="60" :r="RING_R"
                      :stroke-dasharray="ringDash(remainingPercent(item.window))"
                      transform="rotate(-90 60 60)"
                    />
                  </svg>
                  <div class="ring-value">
                    <strong>{{ remainingPercent(item.window) }}%</strong>
                    <span>{{ t('tray.remaining') }}</span>
                  </div>
                </div>
                <span class="ring-label">{{ item.label }} {{ t('tray.limit') }}</span>
              </div>
            </div>
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh(false, false, true)">{{ t('tray.retry') }}</button>
            </div>
            <div
              v-else
              class="quota-message quota-message--loading"
              :class="{ 'quota-message--compact': !kimiUsage }"
            >
              {{ loading ? t('tray.query_wait') : t('tray.no_usage') }}
            </div>
          </div>

          <footer class="tray-footer">
            <span v-if="kimiUsage">{{ t('tray.last_query', { time: formatDate(kimiUsage.fetched_at) }) }}</span>
          </footer>
        </template>

        <template v-else-if="selectedProvider === 'grok-build'">
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
            <div v-if="grokUsage" class="ring-list ring-list--single">
              <div class="ring-item ring-item--secondary">
                <div class="ring-graph">
                  <svg viewBox="0 0 120 120" aria-hidden="true">
                    <circle class="ring-track" cx="60" cy="60" :r="RING_R" />
                    <circle
                      class="ring-fill"
                      cx="60" cy="60" :r="RING_R"
                      :stroke-dasharray="ringDash(remainingPercent(grokUsage.usage_window))"
                      transform="rotate(-90 60 60)"
                    />
                  </svg>
                  <div class="ring-value">
                    <strong>{{ remainingPercent(grokUsage.usage_window) }}%</strong>
                    <span>{{ t('tray.remaining') }}</span>
                  </div>
                </div>
                <span class="ring-label">{{ grokPeriodLabel }}</span>
              </div>
            </div>
            <div v-else-if="error" class="quota-message">
              <strong>{{ t('tray.failed') }}</strong>
              <span>{{ error }}</span>
              <button @click="refresh(false, false, true)">{{ t('tray.retry') }}</button>
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
:global(html[data-view="codex-usage"]),
:global(html[data-view="codex-usage"] body),
:global(html[data-view="codex-usage"] #app) {
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
  /* Ink-wash palette (mirrors src/assets/theme.css). The tray window does not
     follow the main window's data-theme, so dark values are applied via
     prefers-color-scheme below. */
  --tray-canvas: #F8F6F1;
  --tray-surface: #FFFFFE;
  --tray-sunken: #F0EDE4;
  --tray-hover: #EDE9DE;
  --tray-ink: #2A2A2E;
  --tray-ink-2: #5B5B61;
  --tray-ink-3: #8C8B86;
  --tray-ink-4: #B7B5AC;
  --tray-accent: #3A6B8C;
  --tray-accent-strong: #2E5773;
  --tray-accent-soft: rgba(58, 107, 140, .10);
  --tray-accent-mid: rgba(58, 107, 140, .20);
  --tray-highlight: #C9A961;
  --tray-success: #5A8F6B;
  --tray-warning: #B07A3E;
  --tray-danger: #B0524A;
  --tray-hairline: rgba(42, 42, 46, .07);
  --tray-border: rgba(42, 42, 46, .12);
  --tray-on-accent: #FDFCF9;
  --tray-inset: var(--tray-sunken);
  --tray-btn-bg: var(--tray-surface);
  --tray-btn-bg-hover: var(--tray-surface);
  --tray-active-bg: var(--tray-surface);
  --tray-panel-bg: color-mix(in srgb, var(--tray-surface) 97%, transparent);
  --tray-panel-shadow: 0 2px 6px rgba(42, 42, 46, .12);
  --tray-active-shadow: 0 1px 4px rgba(42, 42, 46, .10);
  --tray-success-soft: rgba(90, 143, 107, .12);
  --tray-warning-soft: rgba(176, 122, 62, .10);
  --tray-danger-soft: rgba(176, 82, 74, .10);

  width: 100%;
  height: 100%;
  min-width: 0;
  padding: 8px;
  overflow: hidden;
  color: var(--tray-ink);
  font-family: "SF Pro Text", "Segoe UI", "PingFang SC", sans-serif;
  user-select: none;
}

.tray-panel {
  width: 100%;
  height: 100%;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 14px 16px 10px;
  overflow: hidden;
  border-radius: 25px;
  background: var(--tray-panel-bg);
  box-shadow: var(--tray-panel-shadow);
}

.tray-header, .tray-title, .tray-footer {
  display: flex;
  align-items: center;
}

.tray-header { width: 100%; min-width: 0; flex: 0 0 auto; min-height: 32px; }
.tray-title { min-width: 0; gap: 9px; }
.tray-title h1 { margin: 0; font-size: 17px; line-height: 1; letter-spacing: -.02em; }

/* Compact loading state: small centered title + spinner, nothing else. */
.tray-panel--loading { padding-top: 18px; }
.tray-panel--loading .tray-header { min-height: 0; justify-content: center; }
.tray-panel--loading .tray-title h1 {
  font-size: 15px;
  font-weight: 600;
  letter-spacing: 0;
  color: var(--tray-ink-2);
}
.tray-logo { display: grid; place-items: center; color: var(--tray-on-accent); }
.tray-logo {
  width: 30px;
  height: 30px;
  border-radius: 9px;
  background: linear-gradient(145deg, var(--tray-accent), var(--tray-accent-strong));
  box-shadow: 0 5px 12px var(--tray-accent-mid);
}
.tray-logo svg { width: 19px; height: 19px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; }

.initial-loading {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.loading-spinner {
  width: 18px;
  height: 18px;
  border: 2px solid var(--tray-border);
  border-top-color: var(--tray-accent);
  border-radius: 50%;
  animation: tray-spin .8s linear infinite;
}
@keyframes tray-spin { to { transform: rotate(360deg); } }

.provider-switch {
  flex: 0 0 30px;
  height: 30px;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 3px;
  margin-top: 10px;
  padding: 3px;
  border-radius: 999px;
  background: var(--tray-inset);
}

/* Three-provider variant for Codex / Grok Build / Kimi Code. */
.provider-switch--triple {
  grid-template-columns: 1fr 1fr 1fr;
}
.provider-option {
  min-width: 0;
  overflow: hidden;
  border: 0;
  border-radius: 999px;
  padding: 0 14px;
  color: var(--tray-ink-2);
  background: transparent;
  font: inherit;
  font-size: 13px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
  transition: color .16s ease, background-color .16s ease, box-shadow .16s ease;
}
.provider-option:hover:not(:disabled):not(.is-active) { color: var(--tray-ink); }
.provider-option.is-active {
  color: var(--tray-ink);
  background: var(--tray-active-bg);
  box-shadow: var(--tray-active-shadow);
}
.provider-option:focus-visible { outline: 2px solid var(--tray-accent-mid); outline-offset: 1px; }
.provider-option:disabled { opacity: .48; cursor: default; }
.provider-option.is-active:disabled { opacity: 1; }

.login-empty {
  flex: 0 0 110px;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 10px 4px;
  color: var(--tray-ink-3);
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
  color: var(--tray-ink-3);
  background: var(--tray-inset);
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
  color: var(--tray-ink-2);
  background: var(--tray-inset);
  font-size: 11px;
  font-weight: 600;
  line-height: 1;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.meta-pill--account { flex: 0 1 auto; }
.meta-pill--live { color: var(--tray-success); background: var(--tray-success-soft); }
.meta-pill--cache { color: var(--tray-ink-3); background: var(--tray-inset); }
.grok-warning {
  flex: 0 0 44px;
  min-height: 44px;
  display: flex;
  align-items: center;
  margin: 0;
  border-radius: 14px;
  padding: 7px 11px;
  color: var(--tray-warning);
  background: var(--tray-warning-soft);
  font-size: 11px;
  line-height: 1.35;
}

.quota-wrap {
  flex: 0 0 auto;
  padding: 12px 0 10px;
  border-bottom: 1px solid var(--tray-hairline);
  transition: opacity .18s ease;
}
.quota-wrap.is-loading { opacity: .72; }

/* Remaining-quota rings. Tone colors come from currentColor on .ring-item. */
.ring-list { display: flex; justify-content: center; gap: 24px; }
.ring-item { display: flex; flex-direction: column; align-items: center; gap: 6px; }
.ring-graph { position: relative; width: 88px; height: 88px; }
.ring-list--single .ring-graph { width: 100px; height: 100px; }
.ring-graph svg { display: block; width: 100%; height: 100%; }
.ring-track { fill: none; stroke: var(--tray-inset); stroke-width: 10; }
.ring-fill {
  fill: none;
  stroke: currentColor;
  stroke-width: 10;
  stroke-linecap: round;
  transition: stroke-dasharray .5s cubic-bezier(.2, .8, .2, 1);
}
.ring-item--primary { color: var(--tray-accent); }
.ring-item--secondary { color: var(--tray-success); }
.ring-item--monthly { color: var(--tray-highlight); }
.ring-value {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0;
}
.ring-value strong {
  color: var(--tray-ink);
  font-size: 16px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.ring-list--single .ring-value strong { font-size: 19px; }
.ring-value span { color: var(--tray-ink-3); font-size: 10px; }
.ring-label {
  color: var(--tray-ink-2);
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  white-space: nowrap;
}

.quota-message {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: flex-start;
  min-height: 110px;
  gap: 6px;
  color: var(--tray-ink-3);
}
.quota-message strong { color: var(--tray-danger); }
.quota-message span { max-height: 42px; overflow: hidden; font-size: 12px; }
.quota-message button { min-height: 34px; padding: 0; border: 0; color: var(--tray-accent); background: none; cursor: pointer; }
.quota-message--loading { align-items: center; font-size: 13px; }
.quota-message--compact { min-height: 48px; }

.credit-section { flex: 1 1 auto; min-height: 0; padding-top: 10px; }
.credit-chips { display: flex; flex-wrap: wrap; gap: 8px; }
.credit-chip {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 5px 12px;
  border: 1px solid var(--tray-hairline);
  border-radius: 9px;
  background: var(--tray-inset);
}
/* Full expiry date floats above the chip on hover; the chip itself stays a
   compact one-line HH:mm:ss tag and never reflows. */
.credit-chip__date {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 50%;
  transform: translateX(-50%);
  padding: 3px 8px;
  border-radius: 6px;
  color: var(--tray-on-accent);
  background: var(--tray-ink);
  font-size: 10px;
  white-space: nowrap;
  opacity: 0;
  pointer-events: none;
  transition: opacity .15s ease;
}
.credit-chip:hover .credit-chip__date { opacity: 1; }
.credit-chip__time {
  color: var(--tray-ink);
  font-size: 13px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.credit-empty { margin: 0; padding: 14px 0; color: var(--tray-ink-3); font-size: 12px; }

.tray-footer {
  flex: 0 0 auto;
  min-height: 24px;
  padding-top: 6px;
  color: var(--tray-ink-3);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

@media (prefers-color-scheme: dark) {
  .tray-shell {
    --tray-canvas: #1C1D1F;
    --tray-surface: #232427;
    --tray-sunken: #18191B;
    --tray-hover: #2D2E32;
    --tray-ink: #E8E6DF;
    --tray-ink-2: #B0AEA6;
    --tray-ink-3: #7C7A73;
    --tray-ink-4: #4F4E48;
    --tray-accent: #7DA8C9;
    --tray-accent-strong: #9FBED7;
    --tray-accent-soft: rgba(125, 168, 201, .14);
    --tray-accent-mid: rgba(125, 168, 201, .24);
    --tray-highlight: #D9B97C;
    --tray-success: #8FB89A;
    --tray-warning: #D69963;
    --tray-danger: #D88078;
    --tray-hairline: rgba(232, 230, 223, .06);
    --tray-border: rgba(232, 230, 223, .10);
    --tray-inset: var(--tray-hover);
    --tray-btn-bg: var(--tray-hairline);
    --tray-btn-bg-hover: var(--tray-border);
    --tray-active-bg: var(--tray-hover);
    --tray-panel-shadow: 0 2px 6px rgba(0, 0, 0, .35);
    --tray-active-shadow: 0 1px 4px rgba(0, 0, 0, .28);
    --tray-success-soft: rgba(143, 184, 154, .14);
    --tray-warning-soft: rgba(214, 153, 99, .13);
    --tray-danger-soft: rgba(216, 128, 120, .13);
  }
}
</style>
