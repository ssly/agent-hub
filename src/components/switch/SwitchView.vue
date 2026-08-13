<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSwitchStore } from '@/stores/switch'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import AppModal from '@/components/ui/AppModal.vue'
import AccountUsagePanel, { type UsageWindowRow } from '@/components/switch/AccountUsagePanel.vue'
import ListeningToggle from '@/components/switch/ListeningToggle.vue'
import { Trash2 } from 'lucide-vue-next'

const { t, locale } = useI18n()
const store = useSwitchStore()
const { showToast } = useToast()

const AGENT_DISPLAY_NAMES: Record<string, string> = {
  codex: 'Codex',
  'claude-code': 'Claude Code',
  'grok-build': 'Grok Build',
  'kimi-code': 'Kimi Code',
  deepseek: 'DeepSeek Harness',
}
const agentName = computed(
  () => AGENT_DISPLAY_NAMES[store.selectedAgent ?? ''] ?? store.selectedAgent ?? '',
)

const isCodex = computed(() => store.selectedAgent === 'codex')
const isGrokBuild = computed(() => store.selectedAgent === 'grok-build')
const isKimiCode = computed(() => store.selectedAgent === 'kimi-code')
const isClaudeCode = computed(() => store.selectedAgent === 'claude-code')
const isDeepSeek = computed(() => store.selectedAgent === 'deepseek')

// Absolute timestamp formatted as "YYYY-MM-DD HH:mm:ss" in the user's local
// timezone. Uniform across locales so zh-CN/en-US render identically.
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
  const abs = resetAt ? t('switch.usage_reset_at', { date: fmtAbsDate(resetAt) }) : ''
  return abs ? `${rel} ${abs}` : rel
}

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

const CREDIT_TITLE_MAP: Record<string, string> = {
  'full reset (weekly + 5 hr)': 'switch.usage_credit_title_full',
  'full reset': 'switch.usage_credit_title_full_plain',
}
function fmtCreditTitle(title?: string | null): string {
  if (!title) return t('switch.usage_credit_default_title')
  const key = CREDIT_TITLE_MAP[title.toLowerCase()]
  return key ? t(key) : title
}

function windowLabel(seconds?: number): string {
  const s = seconds ?? 0
  if (s > 0 && Math.abs(s - 2592000) <= 86400) return t('switch.usage_monthly_window')
  if (s > 0 && Math.abs(s - 604800) <= 3600) return t('switch.usage_secondary_window')
  if (s > 0 && Math.abs(s - 18000) <= 600) return t('switch.usage_primary_window')
  if (s >= 86400) {
    const d = Math.round(s / 86400)
    return t('switch.usage_reset_dh', { d, h: 0 })
  }
  if (s > 0) {
    return t('switch.usage_reset_hm', { h: Math.round(s / 3600), m: 0 })
  }
  return t('switch.usage_primary_window')
}

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

function fmtGrokValue(value: number): string {
  return new Intl.NumberFormat(locale.value === 'zh-CN' ? 'zh-CN' : 'en-US', {
    maximumFractionDigits: 2,
  }).format(value)
}

// --- Codex ---
const codexAccountName = computed(
  () => store.codexUsage?.account_name || t('switch.codex_default_account'),
)
const codexPlanBadge = computed(() => {
  const plan = store.codexUsage?.plan_type || 'unknown'
  const display = plan.charAt(0).toUpperCase() + plan.slice(1)
  return t('switch.usage_plan_badge', { plan: display })
})
const codexWindows = computed<UsageWindowRow[]>(() => {
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
    remainingPercent: window.remaining_percent,
    detail: t('switch.usage_used_reset', {
      used: window.used_percent,
      reset: fmtReset(window.reset_after_seconds, window.reset_at),
    }),
  }))
})

// --- Grok ---
const grokAccountName = computed(
  () => store.grokUsage?.account_name || t('switch.grok_default_account'),
)
const grokPlanBadge = computed(() =>
  t('switch.usage_plan_badge', { plan: store.grokUsage?.plan_type || 'Grok' }),
)
const grokWindows = computed<UsageWindowRow[]>(() => {
  const u = store.grokUsage
  if (!u) return []
  const label = u.period_type === 'monthly'
    ? t('switch.grok_monthly_window')
    : t('switch.grok_weekly_window')
  const detail = u.used_value != null && u.limit_value != null
    ? t('switch.grok_used_limit_reset', {
        used: fmtGrokValue(u.used_value),
        limit: fmtGrokValue(u.limit_value),
        reset: fmtReset(u.usage_window.reset_after_seconds, u.usage_window.reset_at),
      })
    : t('switch.usage_used_reset', {
        used: u.usage_window.used_percent,
        reset: fmtReset(u.usage_window.reset_after_seconds, u.usage_window.reset_at),
      })
  return [{
    key: 'period',
    label,
    remainingPercent: u.usage_window.remaining_percent,
    detail,
  }]
})

// --- Kimi ---
const kimiAccountName = computed(
  () => store.kimiUsage?.account_name || t('switch.kimi_default_account'),
)
const kimiAuthBadge = computed(() => {
  const method = store.kimiUsage?.auth_method
  if (method === 'METHOD_API_KEY') return t('switch.kimi_auth_api_key')
  if (method === 'METHOD_OAUTH') return t('switch.kimi_auth_oauth')
  return method || t('switch.kimi_auth_api_key')
})
const kimiWindows = computed<UsageWindowRow[]>(() => {
  const rows: UsageWindowRow[] = []
  const w5 = store.kimiUsage?.window_5h
  const wWeek = store.kimiUsage?.window_weekly
  if (w5) {
    rows.push({
      key: '5h',
      label: t('switch.kimi_5h_window'),
      remainingPercent: w5.remaining_percent,
      detail: t('switch.kimi_used_reset', {
        used: w5.used_percent,
        reset: fmtReset(w5.reset_after_seconds, w5.reset_at),
      }),
    })
  }
  if (wWeek) {
    rows.push({
      key: 'weekly',
      label: t('switch.kimi_weekly_window'),
      remainingPercent: wWeek.remaining_percent,
      detail: t('switch.kimi_used_reset', {
        used: wWeek.used_percent,
        reset: fmtReset(wWeek.reset_after_seconds, wWeek.reset_at),
      }),
    })
  }
  return rows
})

// --- Claude ---
const claudeAccountName = computed(
  () => store.claudeUsage?.account_name || t('switch.claude_default_account'),
)
const claudeWindows = computed<UsageWindowRow[]>(() => {
  const rows: UsageWindowRow[] = []
  const w5 = store.claudeUsage?.window_5h
  const wWeek = store.claudeUsage?.window_weekly
  if (w5) {
    rows.push({
      key: '5h',
      label: t('switch.kimi_5h_window'),
      remainingPercent: w5.remaining_percent,
      detail: t('switch.kimi_used_reset', {
        used: w5.used_percent,
        reset: fmtReset(w5.reset_after_seconds, w5.reset_at),
      }),
    })
  }
  if (wWeek) {
    rows.push({
      key: 'weekly',
      label: t('switch.kimi_weekly_window'),
      remainingPercent: wWeek.remaining_percent,
      detail: t('switch.kimi_used_reset', {
        used: wWeek.used_percent,
        reset: fmtReset(wWeek.reset_after_seconds, wWeek.reset_at),
      }),
    })
  }
  return rows
})

async function handleRefreshCodex() {
  if (store.codexUsageLoading) return
  await store.refreshCodexUsage(true)
  if (store.codexUsageError) showToast(t('switch.usage_failed'), 'error')
  else showToast(t('switch.usage_refresh_toast'), 'success')
}

async function handleRefreshGrok() {
  if (store.grokUsageLoading) return
  await store.refreshGrokUsage(true)
  if (store.grokUsageError && !store.grokUsage) showToast(t('switch.usage_failed'), 'error')
  else if (!store.grokUsageError) showToast(t('switch.usage_refresh_toast'), 'success')
  else showToast(t('switch.usage_failed'), 'error')
}

async function handleRefreshKimi() {
  if (store.kimiUsageLoading) return
  await store.refreshKimiUsage(true)
  if (store.kimiUsageError) showToast(t('switch.usage_failed'), 'error')
  else showToast(t('switch.usage_refresh_toast'), 'success')
}

async function handleRefreshClaude() {
  if (store.claudeUsageLoading) return
  await store.refreshClaudeUsage(true)
  if (store.claudeUsageError) showToast(t('switch.usage_failed'), 'error')
  else if (store.claudeUsageAvailable === false) showToast(t('switch.claude_usage_login_required'), 'info')
  else showToast(t('switch.usage_refresh_toast'), 'success')
}

// --- DeepSeek Harness (key auto-read from the harness, balance via official endpoint) ---

async function handleRefreshDeepSeek() {
  if (store.deepseekUsageLoading) return
  await store.refreshDeepseekUsage(true)
  if (store.deepseekUsageError) showToast(t('switch.usage_failed'), 'error')
  else showToast(t('switch.usage_refresh_toast'), 'success')
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

function handleCardClick(profile: any) {
  if (profile.is_active) {
    showToast(t('switch.already_active_hint'), 'info')
    return
  }
  if (store.switchConfirmId === profile.id) return
  store.switchConfirmId = profile.id
}

function handleOutsideClick() {
  if (store.switchConfirmId) store.switchConfirmId = null
}

function handleCardLeave(profile: any) {
  if (store.switchConfirmId === profile.id) store.switchConfirmId = null
}

onMounted(() => {
  window.addEventListener('click', handleOutsideClick)
  startAutoTimer()
})
onUnmounted(() => {
  window.removeEventListener('click', handleOutsideClick)
  window.clearInterval(autoTimer)
})

// --- Shared listening state + shared-interval auto refresh ------------------
// The toggle itself lives in ListeningToggle (panel header, next to refresh).
// Paused agents are never auto-queried here or in the tray popup.
const listened = computed(() =>
  store.selectedAgent ? store.isAgentListened(store.selectedAgent) : true,
)

let autoTimer: number | undefined
function startAutoTimer() {
  window.clearInterval(autoTimer)
  autoTimer = window.setInterval(() => {
    // Fire only while this window is actually visible (not hidden/minimized).
    // The tray side has the symmetric rule: it must be pinned.
    if (document.visibilityState !== 'visible') return
    const agent = store.selectedAgent
    if (!agent || !store.isAgentListened(agent)) return
    if (agent === 'codex') void store.refreshCodexUsage(true)
    else if (agent === 'grok-build') void store.refreshGrokUsage(true)
    else if (agent === 'kimi-code') void store.refreshKimiUsage(true)
    else if (agent === 'claude-code') void store.refreshClaudeUsage(true)
    else if (agent === 'deepseek') void store.refreshDeepseekUsage(true)
  }, store.refreshMinutes * 60_000)
}
watch(() => store.refreshMinutes, startAutoTimer)

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
    showToast(String(e?.message || e), 'error')
  }
}

const clearActivePath = computed(() => {
  switch (store.selectedAgent) {
    case 'codex': return '~/.codex/auth.json'
    case 'claude-code': return '~/.claude/settings.json'
    default: return ''
  }
})
const clearActiveAgentName = computed(() => agentName.value || store.selectedAgent || '')

async function handleConfirmClear() {
  const err = await store.deleteActiveAuth()
  if (err) {
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
        <div class="max-w-2xl mx-auto space-y-6">
          <!-- Codex / Grok / Kimi / Claude share one account+usage shell. -->
          <AccountUsagePanel
            v-if="isCodex"
            :account-name="codexAccountName"
            :account-hint="t('switch.codex_default_account_hint')"
            :account-status-label="t('switch.codex_read_only')"
            :usage-title="t('switch.usage_title', { name: codexAccountName })"
            :loading="store.codexUsageLoading && !store.codexUsage"
            :refreshing="store.codexUsageLoading"
            :loading-text="t('switch.usage_loading')"
            :error="store.codexUsageError"
            :empty-hint="t('switch.usage_empty_hint')"
            :badges="store.codexUsage ? [codexPlanBadge] : []"
            :windows="codexWindows"
            :last-query-text="store.codexUsageLastQuery ? fmtQueryTime(store.codexUsageLastQuery) : null"
            :paused="!listened"
            @refresh="handleRefreshCodex"
          >
            <template #headerActions><ListeningToggle /></template>
            <template v-if="store.codexResetCredits?.credits.length || store.codexUsage?.reset_credits?.available_count" #extra>
              <div class="text-xs pt-1 flex items-center gap-2" style="color: var(--ink-3)">
                <span>{{ t('switch.usage_reset_credits') }}</span>
                <span class="inline-flex items-center px-2 py-0.5 rounded-full" style="background: var(--accent-soft); color: var(--accent)">
                  {{ t('switch.usage_reset_credits_count', { n: store.codexResetCredits?.available_count ?? store.codexUsage?.reset_credits?.available_count ?? 0 }) }}
                </span>
              </div>
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
          </AccountUsagePanel>

          <AccountUsagePanel
            v-else-if="isGrokBuild"
            :account-name="grokAccountName"
            :account-hint="t('switch.grok_default_account_hint')"
            :account-status-label="t('switch.grok_read_only')"
            :usage-title="t('switch.grok_usage_title', { name: grokAccountName })"
            :loading="store.grokUsageLoading && !store.grokUsage"
            :refreshing="store.grokUsageLoading"
            :loading-text="t('switch.grok_usage_loading')"
            :error="store.grokUsageError"
            :badges="store.grokUsage ? [grokPlanBadge] : []"
            :windows="grokWindows"
            :last-query-text="store.grokUsageLastQuery ? fmtQueryTime(store.grokUsageLastQuery) : null"
            :paused="!listened"
            @refresh="handleRefreshGrok"
          >
            <template #headerActions><ListeningToggle /></template>
          </AccountUsagePanel>

          <AccountUsagePanel
            v-else-if="isKimiCode"
            :account-name="kimiAccountName"
            :account-hint="t('switch.kimi_default_account_hint')"
            :account-status-label="t('switch.kimi_read_only')"
            :usage-title="t('switch.kimi_usage_title', { name: kimiAccountName })"
            :loading="store.kimiUsageLoading && !store.kimiUsage"
            :refreshing="store.kimiUsageLoading"
            :loading-text="t('switch.kimi_usage_loading')"
            :error="store.kimiUsageError"
            :tip="t('switch.kimi_usage_api_only_hint')"
            :badges="store.kimiUsage ? [kimiAuthBadge] : []"
            :windows="kimiWindows"
            :last-query-text="store.kimiUsageLastQuery ? fmtQueryTime(store.kimiUsageLastQuery) : null"
            :paused="!listened"
            @refresh="handleRefreshKimi"
          >
            <template #headerActions><ListeningToggle /></template>
          </AccountUsagePanel>

          <AccountUsagePanel
            v-else-if="isClaudeCode"
            :account-name="claudeAccountName"
            :account-hint="t('switch.claude_default_account')"
            :account-status-label="t('switch.oauth_badge')"
            :usage-title="t('switch.claude_usage_title', { name: claudeAccountName })"
            :loading="store.claudeUsageLoading && !store.claudeUsage"
            :refreshing="store.claudeUsageLoading"
            :loading-text="t('switch.claude_usage_loading')"
            :error="store.claudeUsageError"
            :soft-notice="store.claudeUsageAvailable === false ? t('switch.claude_usage_login_required') : null"
            :tip="t('switch.claude_usage_oauth_hint')"
            :badges="store.claudeUsage?.plan_type ? [store.claudeUsage.plan_type] : []"
            :windows="claudeWindows"
            :last-query-text="store.claudeUsageLastQuery ? fmtQueryTime(store.claudeUsageLastQuery) : null"
            :paused="!listened"
            @refresh="handleRefreshClaude"
          >
            <template #headerActions><ListeningToggle /></template>
          </AccountUsagePanel>

          <!-- DeepSeek Harness: same AccountUsagePanel as the other read-only
               agents; balances are currency amounts, so they render through
               the extra slot instead of quota windows. -->
          <AccountUsagePanel
            v-else-if="isDeepSeek"
            :account-name="agentName"
            :account-hint="t('switch.deepseek_account_hint')"
            :account-status-label="t('switch.deepseek_read_only')"
            :usage-title="t('switch.deepseek_balance_title')"
            :loading="store.deepseekUsageLoading && !store.deepseekUsage"
            :refreshing="store.deepseekUsageLoading"
            :loading-text="t('switch.deepseek_usage_loading')"
            :error="store.deepseekUsageError"
            :soft-notice="store.deepseekSettings && !store.deepseekSettings.has_key ? t('switch.deepseek_no_key_hint') : null"
            :badges="store.deepseekUsage ? [store.deepseekUsage.is_available ? t('switch.deepseek_available') : t('switch.deepseek_unavailable')] : []"
            :last-query-text="store.deepseekUsageLastQuery ? fmtQueryTime(store.deepseekUsageLastQuery) : null"
            :paused="!listened"
            @refresh="handleRefreshDeepSeek"
          >
            <template #headerActions><ListeningToggle /></template>
            <template #extra>
              <div
                v-for="balance in store.deepseekUsage?.balances || []"
                :key="balance.currency"
                class="p-3 rounded-lg"
                style="background: var(--sunken)"
              >
                <div class="flex justify-between items-center gap-2">
                  <span class="font-medium" style="color: var(--ink)">
                    {{ t('switch.deepseek_total') }} ({{ balance.currency }})
                  </span>
                  <span class="font-semibold flex-shrink-0" style="color: var(--accent)">
                    {{ balance.total_balance }}
                  </span>
                </div>
                <div class="text-xs mt-1" style="color: var(--ink-3)">
                  {{ t('switch.deepseek_granted') }} {{ balance.granted_balance }}
                  · {{ t('switch.deepseek_topped_up') }} {{ balance.topped_up_balance }}
                </div>
              </div>
            </template>
          </AccountUsagePanel>

          <!-- Claude Code is the only profile-pool agent: switchable saved accounts. -->
          <template v-if="isClaudeCode">
            <div class="flex gap-2 flex-wrap items-center">
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

            <div
              v-if="store.addFormOpen"
              class="ah-card space-y-3"
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

            <div v-if="store.profiles.length === 0" class="text-center py-12 text-sm" style="color: var(--ink-4)">
              {{ t('switch.empty') }}
            </div>

            <div v-else class="space-y-2">
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
                      <span
                        v-if="profile.kind === 'oauth'"
                        class="text-xs px-2 py-0.5 rounded-full flex-shrink-0"
                        style="background: var(--accent-soft); color: var(--accent)"
                      >{{ t('switch.oauth_badge') }}</span>
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
          </template>
        </div>
      </template>
    </div>

    <AppModal
      :show="store.editModalOpen"
      :title="t('switch.edit_modal_title')"
      width-class="w-[44rem]"
      @close="closeEditModal"
    >
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
  transition: border-color 0.15s, box-shadow 0.15s;
}
.switch-card:hover {
  border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
}
.switch-card--active {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 18%, transparent);
}
.switch-card--readonly {
  cursor: default;
}
.switch-active-badge {
  font-size: 11px;
  line-height: 1;
  padding: 3px 8px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
  flex-shrink: 0;
}
.switch-confirm {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--hairline);
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
</style>
