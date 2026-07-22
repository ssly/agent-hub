<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSwitchStore } from '@/stores/switch'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import AppModal from '@/components/ui/AppModal.vue'
import { RefreshCw, Gauge, Trash2 } from 'lucide-vue-next'

const { t, locale } = useI18n()
const store = useSwitchStore()
const { showToast } = useToast()

const AGENT_DISPLAY_NAMES: Record<string, string> = {
  codex: 'Codex',
  'claude-code': 'Claude Code',
  'grok-build': 'Grok Build',
  'kimi-code': 'Kimi Code',
}
const agentName = computed(
  () => AGENT_DISPLAY_NAMES[store.selectedAgent ?? ''] ?? store.selectedAgent ?? ''
)

// --- Codex usage panel ---
const isCodex = computed(() => store.selectedAgent === 'codex')
const isGrokBuild = computed(() => store.selectedAgent === 'grok-build')
const isKimiCode = computed(() => store.selectedAgent === 'kimi-code')
// Name of the currently active account (the one usage is actually queried for).
const activeAccountName = computed(() => {
  const active = store.profiles.find((p) => p.is_active)
  if (!active) return ''
  const idx = store.profiles.indexOf(active)
  return active.note || t('switch.account_fallback', { n: idx + 1 })
})

// Absolute timestamp formatted as "YYYY-MM-DD HH:mm:ss" in the user's local
// timezone. Uniform across locales so zh-CN/en-US render identically, and the
// seconds are included so the user gets the concrete moment, not just the hour.
function fmtAbsDate(value: number | string | null | undefined): string {
  if (value == null) return ''
  const d = typeof value === 'number' ? new Date(value * 1000) : new Date(value)
  if (Number.isNaN(d.getTime())) return ''
  const pad = (n: number) => String(n).padStart(2, '0')
  return (
    `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ` +
    `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
  )
}

function fmtReset(secs?: number, resetAt?: number): string {
  if (!secs || secs <= 0) return t('switch.usage_reset_now')
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  const rel = h >= 24
    ? t('switch.usage_reset_dh', { d: Math.floor(h / 24), h: h % 24 })
    : t('switch.usage_reset_hm', { h, m })
  // Append the absolute reset time when the backend provides reset_at (unix sec).
  const abs = resetAt ? t('switch.usage_reset_at', { date: fmtAbsDate(resetAt) }) : ''
  return abs ? `${rel} ${abs}` : rel
}

// Format the reset-credit expiry as a coarse countdown ("28 天后到期")
// with the absolute expiry date appended as a small hint, since the user
// explicitly asked for the concrete expiry time to be visible on the page.
function fmtCreditExpiry(iso?: string | null): string {
  if (!iso) return ''
  const target = new Date(iso).getTime()
  if (Number.isNaN(target)) return ''
  const secs = Math.floor((target - Date.now()) / 1000)
  if (secs <= 0) return t('switch.usage_credit_expired')
  const d = Math.floor(secs / 86400)
  const h = Math.floor((secs % 86400) / 3600)
  const rel = d > 0
    ? t('switch.usage_credit_expires_dh', { d, h })
    : t('switch.usage_credit_expires_hm', { h, m: Math.floor((secs % 3600) / 60) })
  const abs = t('switch.usage_credit_expires_at', { date: fmtAbsDate(iso) })
  return `${rel} ${abs}`
}

// The reset-credit title comes back from the Codex backend as a fixed English
// string (e.g. "Full reset (Weekly + 5 hr)"). Map it to a localized label so
// the panel reads fully translated under zh-CN.
const CREDIT_TITLE_MAP: Record<string, string> = {
  'full reset (weekly + 5 hr)': 'switch.usage_credit_title_full',
}
function fmtCreditTitle(title?: string | null): string {
  if (!title) return t('switch.usage_credit_default_title')
  const key = CREDIT_TITLE_MAP[title.toLowerCase()]
  return key ? t(key) : title
}

// Pick a localized window label from its duration in seconds.
// The backend now forwards `window_seconds` from OpenAI, so we label the
// window by what the API actually says rather than hard-coding 5h/7d.
//   ~2592000s (30d) → monthly   (Free plan)
//   604800s   (7d)   → 7-day     (Plus/Pro secondary)
//   18000s    (5h)   → 5-hour    (Plus/Pro primary)
// Anything else falls back to a generic "Xh/Xd" label.
function windowLabel(seconds?: number): string {
  const s = seconds ?? 0
  if (s > 0 && Math.abs(s - 2592000) <= 86400) return t('switch.usage_monthly_window')
  if (s > 0 && Math.abs(s - 604800) <= 3600) return t('switch.usage_secondary_window')
  if (s > 0 && Math.abs(s - 18000) <= 600) return t('switch.usage_primary_window')
  // Generic fallback derived from the duration itself.
  if (s >= 86400) {
    const d = Math.round(s / 86400)
    return t('switch.usage_reset_dh', { d, h: 0 })
  }
  if (s > 0) {
    return t('switch.usage_reset_hm', { h: Math.round(s / 3600), m: 0 })
  }
  return t('switch.usage_primary_window')
}

// Build the list of windows that actually exist for this account from the same
// normalized payload as the tray. The named fields remain only as a fallback
// for older payloads; 5h/7d/30d can all be shown when returned.
interface UsageCard { key: string; label: string; w: import('@/lib/api').UsageWindow }
const usageWindows = computed<UsageCard[]>(() => {
  const u = store.codexUsage
  if (!u) return []
  const fallback = [u.primary_window, u.secondary_window]
    .filter((window): window is import('@/lib/api').UsageWindow => Boolean(window?.window_seconds))
  const windows = (u.usage_windows?.length ? u.usage_windows : fallback)
    .filter(window => window.window_seconds > 0)
    .sort((left, right) => left.window_seconds - right.window_seconds)
    .filter((window, index, all) => index === 0 || window.window_seconds !== all[index - 1].window_seconds)

  return windows.map(window => ({
    key: String(window.window_seconds),
    label: windowLabel(window.window_seconds),
    w: window,
  }))
})

// Human-readable plan name for the badge (e.g. "free" → "Free").
const planBadge = computed(() => {
  const plan = (store.codexUsage?.plan_type || 'unknown')
  const display = plan.charAt(0).toUpperCase() + plan.slice(1)
  return t('switch.usage_plan_badge', { plan: display })
})

const grokAccountName = computed(
  () => store.grokUsage?.account_name || t('switch.grok_default_account')
)
const grokPlanBadge = computed(() =>
  t('switch.usage_plan_badge', { plan: store.grokUsage?.plan_type || 'Grok' })
)
const grokPeriodLabel = computed(() =>
  store.grokUsage?.period_type === 'monthly'
    ? t('switch.grok_monthly_window')
    : t('switch.grok_weekly_window')
)

function fmtGrokValue(value: number): string {
  return new Intl.NumberFormat(locale.value === 'zh-CN' ? 'zh-CN' : 'en-US', {
    maximumFractionDigits: 2,
  }).format(value)
}

// --- Kimi Code usage panel (5h rate-limit window + weekly quota) ---
const kimiAccountName = computed(
  () => store.kimiUsage?.account_name || t('switch.kimi_default_account')
)
// `METHOD_API_KEY` → "API 认证"; anything else falls back to a neutral label.
const kimiAuthBadge = computed(() => {
  const method = store.kimiUsage?.auth_method
  if (method === 'METHOD_API_KEY') return t('switch.kimi_auth_api_key')
  if (method === 'METHOD_OAUTH') return t('switch.kimi_auth_oauth')
  return method || t('switch.kimi_auth_api_key')
})
// The 5-hour rolling rate-limit window. Its `reset_at` is the exact moment
// the window rolls over, surfaced separately from the weekly reset.
const kimiWindow5h = computed(() => store.kimiUsage?.window_5h ?? null)
const kimiWindowWeekly = computed(() => store.kimiUsage?.window_weekly ?? null)

function fmtQueryTime(value: number): string {
  if (!value) return ''
  return new Date(value).toLocaleString(locale.value === 'zh-CN' ? 'zh-CN' : 'en-US', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function fmtLastQuery(): string {
  return fmtQueryTime(store.codexUsageLastQuery)
}

async function handleRefreshUsage() {
  if (store.codexUsageLoading) return
  await store.refreshCodexUsage()
  if (store.codexUsageError) {
    showToast(t('switch.usage_failed'), 'error')
  } else {
    showToast(t('switch.usage_refresh_toast'), 'success')
  }
}

async function handleRefreshGrokUsage() {
  if (store.grokUsageLoading) return
  await store.refreshGrokUsage()
  if (store.grokUsageError) {
    showToast(t('switch.usage_failed'), 'error')
  } else {
    showToast(t('switch.usage_refresh_toast'), 'success')
  }
}

async function handleRefreshKimiUsage() {
  if (store.kimiUsageLoading) return
  await store.refreshKimiUsage()
  if (store.kimiUsageError) {
    showToast(t('switch.usage_failed'), 'error')
  } else {
    showToast(t('switch.usage_refresh_toast'), 'success')
  }
}

const addNote = ref('')
const addContent = ref('')

async function handleSaveCurrent() {
  if (!store.selectedAgent) return
  try {
    await api.saveCurrentAuthProfile(store.selectedAgent, '')
    showToast(t('switch.saved_toast'), 'success')
    await store.loadProfiles()
  } catch (e: any) {
    if (e === 'duplicate_key' || e?.message === 'duplicate_key') showToast(t('switch.duplicate_key_error'), 'error')
    else if (e === 'no_active_auth' || e?.message === 'no_active_auth') showToast(t('switch.no_active_auth_error', { agent: agentName.value }), 'error')
    else showToast(String(e?.message || e), 'error')
  }
}

// Clicking a card toggles the inline switch-confirm state.
// The active card never enters the flow — it just hints it's in use.
function handleCardClick(profile: any) {
  if (profile.is_active) {
    showToast(t('switch.already_active_hint'), 'info')
    return
  }
  // Clicking the already-confirming card keeps it in place (no toggle-off);
  // clicking another card switches the confirm target to it.
  if (store.switchConfirmId === profile.id) return
  store.switchConfirmId = profile.id
}

// Dismiss the inline switch-confirm when clicking anywhere outside the cards.
// Cards stop propagation (@click.stop) so clicking inside a card won't dismiss.
function handleOutsideClick() {
  if (store.switchConfirmId) store.switchConfirmId = null
}

// Moving the pointer off a card disarms only that card's confirm state.
function handleCardLeave(profile: any) {
  if (store.switchConfirmId === profile.id) store.switchConfirmId = null
}

onMounted(() => {
  window.addEventListener('click', handleOutsideClick)
})
onUnmounted(() => window.removeEventListener('click', handleOutsideClick))

async function doSwitch(id: string) {
  if (!store.selectedAgent) return
  try {
    await api.switchAuthProfile(store.selectedAgent, id)
    store.switchConfirmId = null
    showToast(t('switch.switched_toast', { agent: agentName.value }), 'success')
    await store.loadSelectedAgent()
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

async function handleConfirmAdd() {
  if (!store.selectedAgent) return
  const content = addContent.value.trim()
  const note = addNote.value.trim()
  if (!content) {
    showToast(t('switch.invalid_json_error') || 'Content cannot be empty', 'error')
    return
  }
  try {
    await api.addAuthProfile(store.selectedAgent, content, note)
    addNote.value = ''
    addContent.value = ''
    store.addFormOpen = false
    showToast(t('switch.added_toast'), 'success')
    await store.loadProfiles()
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

async function openEditModal(profile: any) {
  try {
    await store.openEditModal(profile)
  } catch {
    showToast(t('switch.content_load_failed'), 'error')
  }
}

function closeEditModal() {
  store.closeEditModal()
}

async function handleSaveEdit() {
  if (!store.selectedAgent || !store.editingProfileId || store.editSaving) return
  store.editSaving = true
  const id = store.editingProfileId
  const note = store.editNote.trim()
  const content = store.editContent.trim()
  // Save note first; if either step fails, keep the modal open so the user can retry.
  try {
    await api.updateAuthProfileNote(store.selectedAgent, id, note)
  } catch (e: any) {
    store.editSaving = false
    showToast(String(e?.message || e), 'error')
    return
  }
  try {
    await api.updateAuthProfileContent(store.selectedAgent, id, content)
  } catch (e: any) {
    store.editSaving = false
    showToast(String(e?.message || e), 'error')
    return
  }
  store.editSaving = false
  showToast(t('switch.content_saved_toast'), 'success')
  store.closeEditModal()
  await store.loadProfiles()
}

function armDelete() {
  store.deleteArmed = true
}

async function confirmDelete() {
  if (!store.selectedAgent || !store.editingProfileId) return
  try {
    await api.deleteAuthProfile(store.selectedAgent, store.editingProfileId)
    showToast(t('switch.deleted_toast'), 'success')
    store.closeEditModal()
    await store.loadProfiles()
  } catch (e: any) {
    // keep armed so the user can retry or click away to cancel
    showToast(String(e?.message || e), 'error')
  }
}

// --- Clear active account (delete the live auth file, keep the pool) ---
// Path of the auth file that will be deleted, shown in the confirm modal.
const clearActivePath = computed(() => {
  switch (store.selectedAgent) {
    case 'codex': return '~/.codex/auth.json'
    case 'claude-code': return '~/.claude/settings.json'
    default: return ''
  }
})
// Display name of the agent for the "{agent} will be signed out" line.
const clearActiveAgentName = computed(() => agentName.value || store.selectedAgent || '')

async function handleConfirmClear() {
  // Store returns null on success or an error string on failure.
  const err = await store.deleteActiveAuth()
  if (err) {
    // "no_active_auth" is the backend sentinel for a missing auth file.
    const msg = err === 'no_active_auth'
      ? t('switch.clear_active_no_auth_error', { path: clearActivePath.value })
      : err
    showToast(msg, 'error')
  } else {
    showToast(t('switch.clear_active_done_toast'), 'success')
  }
}
</script>

<template>
  <div class="p-6 view-enter">
    <div class="ah-view-content">
      <div v-if="!store.selectedAgent" class="flex flex-col items-center justify-center py-20">
        <p style="color: var(--ink-3)">{{ t('switch.select_agent') }}</p>
      </div>

      <template v-else>
        <div class="max-w-2xl mx-auto">
          <!-- Grok Build deliberately exposes only the CLI's current account. -->
          <div v-if="isGrokBuild" class="space-y-6">
            <div class="ah-card switch-card--active switch-card--readonly">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <div class="flex items-center gap-2 mb-1">
                    <span class="text-sm font-medium truncate" style="color: var(--ink)">{{ grokAccountName }}</span>
                    <span class="switch-active-badge">{{ t('switch.active_badge') }}</span>
                  </div>
                  <div class="text-xs" style="color: var(--ink-3)">{{ t('switch.grok_default_account_hint') }}</div>
                </div>
                <span class="text-xs px-2 py-1 rounded-full flex-shrink-0" style="background: var(--sunken); color: var(--ink-2)">
                  {{ t('switch.grok_read_only') }}
                </span>
              </div>
            </div>

            <div class="ah-card" style="background: var(--surface); border-color: var(--border)">
              <div class="flex items-center justify-between mb-3">
                <span class="text-base font-semibold flex items-center gap-2" style="color: var(--ink)">
                  <Gauge :size="18" :style="{ color: 'var(--accent)' }" />
                  {{ t('switch.grok_usage_title', { name: grokAccountName }) }}
                </span>
                <button
                  class="btn btn-secondary btn-sm flex items-center gap-1"
                  :disabled="store.grokUsageLoading"
                  @click="handleRefreshGrokUsage"
                >
                  <RefreshCw :size="14" :class="{ 'animate-spin': store.grokUsageLoading }" />
                  {{ t('switch.usage_refresh') }}
                </button>
              </div>

              <div v-if="store.grokUsageLoading" class="text-sm py-4" style="color: var(--ink-3)">
                {{ t('switch.grok_usage_loading') }}
              </div>

              <div v-else-if="store.grokUsageError" class="text-sm py-2 flex items-center justify-between gap-3" style="color: var(--danger)">
                <span>{{ t('switch.usage_failed') }}: {{ store.grokUsageError }}</span>
                <button class="btn btn-danger btn-sm" @click="handleRefreshGrokUsage">{{ t('switch.usage_retry') }}</button>
              </div>

              <div v-else-if="store.grokUsage" class="space-y-3 text-sm">
                <div class="text-xs flex items-center gap-2 flex-wrap" style="color: var(--ink-3)">
                  <span class="inline-flex items-center px-2 py-0.5 rounded-full" style="background: var(--sunken); color: var(--ink-2)">{{ grokPlanBadge }}</span>
                  <span
                    class="inline-flex items-center px-2 py-0.5 rounded-full"
                    :style="store.grokUsage.source === 'live'
                      ? { background: 'var(--accent-soft)', color: 'var(--accent)' }
                      : { background: 'var(--sunken)', color: 'var(--ink-3)' }"
                  >
                    {{ store.grokUsage.source === 'live' ? t('switch.grok_live_data') : t('switch.grok_cached_data') }}
                  </span>
                </div>

                <div
                  v-if="store.grokUsage.stale"
                  class="p-3 rounded-lg text-xs"
                  style="background: color-mix(in srgb, var(--danger) 10%, transparent); color: var(--danger)"
                >
                  {{ t('switch.grok_stale_warning') }}
                </div>

                <div class="p-3 rounded-lg" style="background: var(--sunken)">
                  <div class="flex justify-between items-center">
                    <span class="font-medium" style="color: var(--ink)">{{ grokPeriodLabel }}</span>
                    <span class="font-semibold" style="color: var(--accent)">
                      {{ t('switch.usage_remaining', { n: store.grokUsage.usage_window.remaining_percent }) }}
                    </span>
                  </div>
                  <div class="text-xs mt-1" style="color: var(--ink-3)">
                    <template v-if="store.grokUsage.used_value != null && store.grokUsage.limit_value != null">
                      {{ t('switch.grok_used_limit_reset', {
                        used: fmtGrokValue(store.grokUsage.used_value),
                        limit: fmtGrokValue(store.grokUsage.limit_value),
                        reset: fmtReset(store.grokUsage.usage_window.reset_after_seconds, store.grokUsage.usage_window.reset_at),
                      }) }}
                    </template>
                    <template v-else>
                      {{ t('switch.usage_used_reset', {
                        used: store.grokUsage.usage_window.used_percent,
                        reset: fmtReset(store.grokUsage.usage_window.reset_after_seconds, store.grokUsage.usage_window.reset_at),
                      }) }}
                    </template>
                  </div>
                </div>

                <div class="text-xs pt-2 border-t" style="color: var(--ink-4); border-color: var(--hairline)">
                  {{ t('switch.usage_last_query', { time: fmtQueryTime(store.grokUsageLastQuery) }) }}
                </div>
              </div>
            </div>
          </div>

          <!-- Kimi Code mirrors the read-only model of Grok Build, but exposes
               the multi-window layout (5h primary + weekly) like Codex. -->
          <div v-if="isKimiCode" class="space-y-6">
            <div class="ah-card switch-card--active switch-card--readonly">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <div class="flex items-center gap-2 mb-1">
                    <span class="text-sm font-medium truncate" style="color: var(--ink)">{{ kimiAccountName }}</span>
                    <span class="switch-active-badge">{{ t('switch.active_badge') }}</span>
                  </div>
                  <div class="text-xs" style="color: var(--ink-3)">{{ t('switch.kimi_default_account_hint') }}</div>
                </div>
                <span class="text-xs px-2 py-1 rounded-full flex-shrink-0" style="background: var(--sunken); color: var(--ink-2)">
                  {{ t('switch.kimi_read_only') }}
                </span>
              </div>
            </div>

            <div class="ah-card" style="background: var(--surface); border-color: var(--border)">
              <div class="flex items-center justify-between mb-3">
                <span class="text-base font-semibold flex items-center gap-2" style="color: var(--ink)">
                  <Gauge :size="18" :style="{ color: 'var(--accent)' }" />
                  {{ t('switch.kimi_usage_title', { name: kimiAccountName }) }}
                </span>
                <button
                  class="btn btn-secondary btn-sm flex items-center gap-1"
                  :disabled="store.kimiUsageLoading"
                  @click="handleRefreshKimiUsage"
                >
                  <RefreshCw :size="14" :class="{ 'animate-spin': store.kimiUsageLoading }" />
                  {{ t('switch.usage_refresh') }}
                </button>
              </div>

              <div v-if="store.kimiUsageLoading" class="text-sm py-4" style="color: var(--ink-3)">
                {{ t('switch.kimi_usage_loading') }}
              </div>

              <div v-else-if="store.kimiUsageError" class="text-sm py-2 flex items-center justify-between gap-3" style="color: var(--danger)">
                <span>{{ t('switch.usage_failed') }}: {{ store.kimiUsageError }}</span>
                <button class="btn btn-danger btn-sm" @click="handleRefreshKimiUsage">{{ t('switch.usage_retry') }}</button>
              </div>

              <div v-else-if="store.kimiUsage" class="space-y-3 text-sm">
                <div class="text-xs" style="color: var(--ink-3)">
                  <span class="inline-flex items-center px-2 py-0.5 rounded-full" style="background: var(--sunken); color: var(--ink-2)">{{ kimiAuthBadge }}</span>
                </div>

                <!-- 5-hour rolling rate-limit window -->
                <div v-if="kimiWindow5h" class="p-3 rounded-lg" style="background: var(--sunken)">
                  <div class="flex justify-between items-center">
                    <span class="font-medium" style="color: var(--ink)">{{ t('switch.kimi_5h_window') }}</span>
                    <span class="font-semibold" style="color: var(--accent)">{{ t('switch.usage_remaining', { n: kimiWindow5h.remaining_percent }) }}</span>
                  </div>
                  <div class="text-xs mt-1" style="color: var(--ink-3)">
                    {{ t('switch.kimi_used_reset', {
                      used: kimiWindow5h.used_percent,
                      reset: fmtReset(kimiWindow5h.reset_after_seconds, kimiWindow5h.reset_at),
                    }) }}
                  </div>
                </div>

                <!-- Weekly quota window -->
                <div v-if="kimiWindowWeekly" class="p-3 rounded-lg" style="background: var(--sunken)">
                  <div class="flex justify-between items-center">
                    <span class="font-medium" style="color: var(--ink)">{{ t('switch.kimi_weekly_window') }}</span>
                    <span class="font-semibold" style="color: var(--accent)">{{ t('switch.usage_remaining', { n: kimiWindowWeekly.remaining_percent }) }}</span>
                  </div>
                  <div class="text-xs mt-1" style="color: var(--ink-3)">
                    <template v-if="store.kimiUsage.weekly_limit != null">
                      {{ t('switch.kimi_used_limit_reset', {
                        used: store.kimiUsage.weekly_used ?? 0,
                        limit: store.kimiUsage.weekly_limit ?? 0,
                        reset: fmtReset(kimiWindowWeekly.reset_after_seconds, kimiWindowWeekly.reset_at),
                      }) }}
                    </template>
                    <template v-else>
                      {{ t('switch.kimi_used_reset', {
                        used: kimiWindowWeekly.used_percent,
                        reset: fmtReset(kimiWindowWeekly.reset_after_seconds, kimiWindowWeekly.reset_at),
                      }) }}
                    </template>
                  </div>
                </div>

                <div class="text-xs pt-2 border-t" style="color: var(--ink-4); border-color: var(--hairline)">
                  {{ t('switch.usage_last_query', { time: fmtQueryTime(store.kimiUsageLastQuery) }) }}
                </div>
              </div>
            </div>
          </div>

          <!-- Toolbar -->
          <div v-if="!isGrokBuild && !isKimiCode" class="flex gap-2 mb-4 flex-wrap items-center">
            <button class="btn btn-primary" @click="handleSaveCurrent">{{ t('switch.save_current') }}</button>
            <button class="btn btn-secondary" @click="store.addFormOpen = !store.addFormOpen">{{ t('switch.add_account') }}</button>
            <button
              class="btn btn-danger flex items-center gap-1"
              :disabled="store.clearActiveLoading"
              @click="store.openClearActiveModal()"
            >
              <Trash2 :size="14" />
              {{ t('switch.clear_active') }}
            </button>
            <div class="flex-1" />
            <span v-if="store.currentKey" class="text-xs truncate max-w-[200px]" style="color: var(--ink-3); font-family: var(--font-mono)">
              {{ t('switch.current_key') }}: {{ store.currentKey }}
            </span>
          </div>

          <!-- Add Form Card -->
          <div
            v-if="store.addFormOpen"
            class="ah-card mb-4 space-y-3"
            style="background: var(--surface); border-color: var(--border)"
          >
            <h3 class="text-sm font-semibold" style="color: var(--ink)">{{ t('switch.add_account') }}</h3>
            <div class="flex flex-col gap-1">
              <label class="text-xs" style="color: var(--ink-3)">{{ t('switch.note_placeholder') }}</label>
              <input
                v-model="addNote"
                type="text"
                class="ah-search-input"
                :placeholder="t('switch.note_placeholder')"
              />
            </div>
            <div class="flex flex-col gap-1">
              <label class="text-xs" style="color: var(--ink-3)">{{ t('mcp.config') }} (auth.json key / contents)</label>
              <textarea
                v-model="addContent"
                class="ah-config-editor"
                placeholder="Paste key content or JSON..."
                style="min-height: 120px"
              />
            </div>
            <div class="flex justify-end gap-2">
              <button class="btn btn-secondary btn-sm" @click="store.addFormOpen = false">{{ t('action.cancel') }}</button>
              <button class="btn btn-primary btn-sm" @click="handleConfirmAdd">{{ t('action.confirm') }}</button>
            </div>
          </div>

          <!-- Profiles -->
          <div v-if="!isGrokBuild && !isKimiCode && store.profiles.length === 0" class="text-center py-12 text-sm" style="color: var(--ink-4)">
            {{ t('switch.empty') }}
          </div>

          <div v-if="!isGrokBuild && !isKimiCode" class="space-y-2">
            <div
              v-for="(profile, idx) in store.profiles"
              :key="profile.id"
              :class="['ah-card', 'switch-card', profile.is_active ? 'switch-card--active' : '']"
              @click.stop="handleCardClick(profile)"
              @mouseleave="handleCardLeave(profile)"
            >
              <div class="flex items-start justify-between gap-2">
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2 mb-0.5">
                    <span class="text-sm font-medium" style="color: var(--ink)">
                      {{ profile.note || t('switch.account_fallback', { n: idx + 1 }) }}
                    </span>
                    <span v-if="profile.is_active" class="switch-active-badge">{{ t('switch.active_badge') }}</span>
                  </div>
                  <div class="text-xs" style="color: var(--ink-3)">
                    {{ profile.saved_at ? fmtAbsDate(profile.saved_at) : '' }}
                    {{ profile.key ? ` · ${profile.key}` : '' }}
                  </div>
                </div>

                <div class="flex-shrink-0">
                  <button class="btn btn-secondary btn-sm" @click.stop="openEditModal(profile)">
                    {{ t('switch.edit_content_btn') }}
                  </button>
                </div>
              </div>

              <!-- Inline switch confirmation -->
              <div
                v-if="store.switchConfirmId === profile.id && !profile.is_active"
                class="switch-confirm"
                @click.stop
              >
                <span class="text-xs" style="color: var(--accent)">{{ t('switch.confirm_switch', { agent: agentName }) }}</span>
                <button class="btn btn-primary btn-sm" @click.stop="doSwitch(profile.id)">{{ t('action.confirm') }}</button>
                <button class="btn btn-ghost btn-sm" @click.stop="store.switchConfirmId = null">{{ t('action.cancel') }}</button>
              </div>
            </div>
          </div>

          <!-- Codex usage panel -->
          <div v-if="isCodex" class="ah-card mt-6" style="background: var(--surface); border-color: var(--border)">
            <div class="flex items-center justify-between mb-3">
              <span class="text-base font-semibold flex items-center gap-2" style="color: var(--ink)">
                <Gauge :size="18" :style="{ color: 'var(--accent)' }" />
                {{ t('switch.usage_title', { name: activeAccountName || t('switch.active_badge') }) }}
              </span>
              <button
                class="btn btn-secondary btn-sm flex items-center gap-1"
                :disabled="store.codexUsageLoading"
                @click="handleRefreshUsage"
              >
                <RefreshCw :size="14" :class="{ 'animate-spin': store.codexUsageLoading }" />
                {{ t('switch.usage_refresh') }}
              </button>
            </div>

            <!-- Loading -->
            <div v-if="store.codexUsageLoading" class="text-sm py-4" style="color: var(--ink-3)">
              {{ t('switch.usage_loading') }}
            </div>

            <!-- Error -->
            <div v-else-if="store.codexUsageError" class="text-sm py-2 flex items-center justify-between gap-3" style="color: var(--danger)">
              <span>{{ t('switch.usage_failed') }}: {{ store.codexUsageError }}</span>
              <button class="btn btn-danger btn-sm" @click="handleRefreshUsage">{{ t('switch.usage_retry') }}</button>
            </div>

            <!-- Empty hint (before first fetch) -->
            <div v-else-if="!store.codexUsage" class="text-sm py-2" style="color: var(--ink-3)">
              {{ t('switch.usage_empty_hint') }}
            </div>

            <!-- No windows returned (e.g. some accounts return rate_limit but no usable windows) -->
            <div v-else-if="usageWindows.length === 0" class="text-sm py-2 flex items-center justify-between gap-3" style="color: var(--ink-3)">
              <span>{{ t('switch.usage_no_data') }}</span>
              <button class="btn btn-secondary btn-sm" @click="handleRefreshUsage">{{ t('switch.usage_retry') }}</button>
            </div>

            <!-- Data: render every quota window returned by the shared snapshot -->
            <div v-else class="space-y-3 text-sm">
              <div class="text-xs" style="color: var(--ink-3)">
                <span class="inline-flex items-center px-2 py-0.5 rounded-full" style="background: var(--sunken); color: var(--ink-2)">{{ planBadge }}</span>
              </div>
              <div
                v-for="win in usageWindows"
                :key="win.key"
                class="p-3 rounded-lg"
                style="background: var(--sunken)"
              >
                <div class="flex justify-between items-center">
                  <span class="font-medium" style="color: var(--ink)">{{ win.label }}</span>
                  <span class="font-semibold" style="color: var(--accent)">{{ t('switch.usage_remaining', { n: win.w.remaining_percent }) }}</span>
                </div>
                <div class="text-xs mt-1" style="color: var(--ink-3)">
                  {{ t('switch.usage_used_reset', { used: win.w.used_percent, reset: fmtReset(win.w.reset_after_seconds, win.w.reset_at) }) }}
                </div>
              </div>
              <!-- Rate-limit reset credits — one card per banked credit.
                   Each credit has its own expiry (valid ~30d from grant), so we
                   list them individually rather than collapsing to a total.
                   Falls back to a single summary card when the detailed list
                   endpoint returned no per-credit entries but usage still
                   reported a count. -->
              <template v-if="store.codexResetCredits?.credits.length || store.codexUsage?.reset_credits?.available_count">
                <div class="text-xs pt-1 flex items-center gap-2" style="color: var(--ink-3)">
                  <span>{{ t('switch.usage_reset_credits') }}</span>
                  <span class="inline-flex items-center px-2 py-0.5 rounded-full" style="background: var(--accent-soft); color: var(--accent)">
                    {{ t('switch.usage_reset_credits_count', { n: store.codexResetCredits?.available_count ?? store.codexUsage?.reset_credits?.available_count ?? 0 }) }}
                  </span>
                </div>

                <!-- One card per credit (from the detailed reset-credits endpoint) -->
                <div
                  v-for="(credit, idx) in store.codexResetCredits?.credits ?? []"
                  :key="idx"
                  class="p-3 rounded-lg"
                  style="background: var(--sunken)"
                >
                  <div class="flex justify-between items-center gap-2">
                    <span class="font-medium truncate" style="color: var(--ink)">
                      {{ fmtCreditTitle(credit.title) }}
                    </span>
                    <span
                      class="text-xs px-2 py-0.5 rounded-full flex-shrink-0"
                      :style="credit.status === 'available'
                        ? { background: 'var(--accent-soft)', color: 'var(--accent)' }
                        : { background: 'var(--sunken)', color: 'var(--ink-3)' }"
                    >
                      {{ credit.status === 'available' ? t('switch.usage_credit_available') : t('switch.usage_credit_used') }}
                    </span>
                  </div>
                  <div class="text-xs mt-1" style="color: var(--ink-3)">
                    <template v-if="credit.status === 'available'">
                      {{ fmtCreditExpiry(credit.expires_at) || t('switch.usage_credit_no_expiry') }}
                    </template>
                    <template v-else>
                      {{ t('switch.usage_credit_redeemed') }}
                    </template>
                  </div>
                </div>

                <!-- Fallback: detailed endpoint returned nothing but usage reported a count -->
                <div
                  v-if="!store.codexResetCredits?.credits.length && store.codexUsage?.reset_credits"
                  class="p-3 rounded-lg"
                  style="background: var(--sunken)"
                >
                  <div class="flex justify-between items-center">
                    <span class="font-medium" style="color: var(--ink)">{{ t('switch.usage_reset_credits') }}</span>
                    <span class="font-semibold" style="color: var(--accent)">
                      {{ t('switch.usage_reset_credits_count', { n: store.codexUsage.reset_credits.available_count }) }}
                    </span>
                  </div>
                </div>
              </template>
              <div class="text-xs pt-2 border-t" style="color: var(--ink-4); border-color: var(--hairline)">
                {{ t('switch.usage_last_query', { time: fmtLastQuery() }) }}
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- Edit Modal -->
    <AppModal
      :show="store.editModalOpen"
      :title="t('switch.edit_modal_title')"
      width-class="w-[44rem]"
      @close="closeEditModal"
    >
      <!-- body -->
      <div class="flex flex-col gap-4">
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('switch.note_label') }}</label>
          <input
            v-model="store.editNote"
            type="text"
            class="ah-search-input"
            :placeholder="t('switch.note_placeholder')"
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('switch.content_label') }}</label>
          <textarea
            v-if="!store.editContentLoading"
            v-model="store.editContent"
            v-auto-resize
            class="ah-config-editor ah-config-editor--auto"
          />
          <div
            v-else
            class="ah-config-editor flex items-center justify-center text-xs"
            style="color: var(--ink-3)"
          >
            {{ t('switch.content_loading') }}
          </div>
        </div>
      </div>

      <template #footer>
        <div class="flex items-center gap-2 w-full">
          <button
            :class="store.deleteArmed ? 'btn btn-sm' : 'btn btn-danger btn-sm'"
            :style="store.deleteArmed ? { background: 'var(--danger)', color: 'var(--on-accent)', borderColor: 'var(--danger)' } : {}"
            @click.stop="store.deleteArmed ? confirmDelete() : armDelete()"
            @mouseleave="store.deleteArmed = false"
          >
            {{ store.deleteArmed ? t('action.confirm') : t('switch.delete_btn') }}
          </button>
          <div class="flex-1" />
          <button class="btn btn-secondary" @click="closeEditModal">{{ t('action.cancel') }}</button>
          <button
            class="btn btn-primary"
            :disabled="store.editSaving || store.editContentLoading"
            @click="handleSaveEdit"
          >
            {{ t('switch.save_note') }}
          </button>
        </div>
      </template>
    </AppModal>

    <!-- Clear Active Account Modal -->
    <AppModal
      :show="store.clearActiveModalOpen"
      :title="t('switch.clear_active_title')"
      width-class="w-[40rem]"
      @close="store.closeClearActiveModal()"
    >
      <div class="space-y-3 text-sm">
        <p style="color: var(--ink)">{{ t('switch.clear_active_warning_path', { path: clearActivePath }) }}</p>
        <p style="color: var(--ink)">{{ t('switch.clear_active_warning_logout', { agent: clearActiveAgentName }) }}</p>
        <p class="p-3 rounded-lg" style="background: var(--sunken); color: var(--ink-2)">
          {{ t('switch.clear_active_warning_pool') }}
        </p>
      </div>

      <template #footer>
        <div class="flex items-center gap-2 w-full">
          <button
            class="btn btn-danger"
            :disabled="store.clearActiveLoading"
            @click="handleConfirmClear"
          >
            {{ t('switch.confirm_clear') }}
          </button>
          <div class="flex-1" />
          <button
            class="btn btn-secondary"
            :disabled="store.clearActiveLoading"
            @click="store.closeClearActiveModal()"
          >
            {{ t('action.cancel') }}
          </button>
        </div>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.switch-card {
  cursor: pointer;
}
.switch-card--readonly {
  cursor: default;
}
.switch-card:hover {
  border-color: var(--border);
  box-shadow: var(--shadow-soft);
}
.switch-card--active {
  background: var(--accent-soft);
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent-mid) inset;
}
.switch-active-badge {
  background: var(--accent);
  color: var(--on-accent);
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
}
.switch-confirm {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px dashed var(--hairline);
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
</style>
