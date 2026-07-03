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
}
const agentName = computed(
  () => AGENT_DISPLAY_NAMES[store.selectedAgent ?? ''] ?? store.selectedAgent ?? ''
)

// --- Codex usage panel ---
const isCodex = computed(() => store.selectedAgent === 'codex')
// Name of the currently active account (the one usage is actually queried for).
const activeAccountName = computed(() => {
  const active = store.profiles.find((p) => p.is_active)
  if (!active) return ''
  const idx = store.profiles.indexOf(active)
  return active.note || t('switch.account_fallback', { n: idx + 1 })
})

function fmtReset(secs?: number): string {
  if (!secs || secs <= 0) return t('switch.usage_reset_now')
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h >= 24) {
    const d = Math.floor(h / 24)
    return t('switch.usage_reset_dh', { d, h: h % 24 })
  }
  return t('switch.usage_reset_hm', { h, m })
}

// Format the reset-credit expiry as a coarse countdown ("28 天后到期").
// We don't show a precise timer — just days/hours, consistent with fmtReset.
function fmtCreditExpiry(iso?: string | null): string {
  if (!iso) return ''
  const target = new Date(iso).getTime()
  if (Number.isNaN(target)) return ''
  const secs = Math.floor((target - Date.now()) / 1000)
  if (secs <= 0) return t('switch.usage_credit_expired')
  const d = Math.floor(secs / 86400)
  const h = Math.floor((secs % 86400) / 3600)
  if (d > 0) return t('switch.usage_credit_expires_dh', { d, h })
  const m = Math.floor((secs % 3600) / 60)
  return t('switch.usage_credit_expires_hm', { h, m })
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

// Build the list of windows that actually exist for this account.
// Free accounts only have a primary (monthly) window; Plus/Pro also have a
// secondary (7d) window. We drop nulls so the UI never renders an empty card.
interface UsageCard { key: string; label: string; w: import('@/lib/api').UsageWindow }
const usageWindows = computed<UsageCard[]>(() => {
  const u = store.codexUsage
  if (!u) return []
  const cards: UsageCard[] = []
  if (u.primary_window) {
    cards.push({ key: 'primary', label: windowLabel(u.primary_window.window_seconds), w: u.primary_window })
  }
  if (u.secondary_window) {
    cards.push({ key: 'secondary', label: windowLabel(u.secondary_window.window_seconds), w: u.secondary_window })
  }
  return cards
})

// Human-readable plan name for the badge (e.g. "free" → "Free").
const planBadge = computed(() => {
  const plan = (store.codexUsage?.plan_type || 'unknown')
  const display = plan.charAt(0).toUpperCase() + plan.slice(1)
  return t('switch.usage_plan_badge', { plan: display })
})

function fmtLastQuery(): string {
  if (!store.codexUsageLastQuery) return ''
  return new Date(store.codexUsageLastQuery).toLocaleString(locale.value === 'zh-CN' ? 'zh-CN' : 'en-US', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

async function handleRefreshUsage() {
  if (store.codexUsageLoading) return
  // Cooldown gate: if a fresh payload exists, don't hit the API again —
  // just tell the user to wait. No precise countdown shown.
  if (store.codexUsage && store.codexUsageInCooldown()) {
    showToast(t('switch.usage_cooldown_toast'), 'info')
    return
  }
  await store.refreshCodexUsage(false)
  if (store.codexUsageError) {
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
  // Codex usage is intentionally NOT auto-fetched on mount. The user must
  // click "Refresh" on the usage panel to trigger a query; cached results
  // from a previous session are shown directly if present.
})
onUnmounted(() => window.removeEventListener('click', handleOutsideClick))

async function doSwitch(id: string) {
  if (!store.selectedAgent) return
  try {
    await api.switchAuthProfile(store.selectedAgent, id)
    store.switchConfirmId = null
    showToast(t('switch.switched_toast', { agent: agentName.value }), 'success')
    await store.loadProfiles()
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
          <!-- Toolbar -->
          <div class="flex gap-2 mb-4 flex-wrap items-center">
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
          <div v-if="store.profiles.length === 0" class="text-center py-12 text-sm" style="color: var(--ink-4)">
            {{ t('switch.empty') }}
          </div>

          <div class="space-y-2">
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
                    {{ profile.saved_at ? profile.saved_at.substring(0, 19).replace('T', ' ') + ' UTC' : '' }}
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

            <!-- Data: render whatever windows the API returned (1 for Free, 2 for Plus/Pro) -->
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
                  {{ t('switch.usage_used_reset', { used: win.w.used_percent, reset: fmtReset(win.w.reset_after_seconds) }) }}
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
                      {{ credit.title || t('switch.usage_credit_default_title') }}
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
      <div class="space-y-4">
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
            class="ah-config-editor"
            style="min-height: 200px"
          />
          <div
            v-else
            class="ah-config-editor flex items-center justify-center text-xs"
            style="min-height: 200px; color: var(--ink-3)"
          >
            {{ t('switch.content_loading') }}
          </div>
        </div>
      </div>

      <template #footer>
        <div class="flex items-center gap-2 w-full">
          <button
            :class="store.deleteArmed ? 'btn btn-sm' : 'btn btn-danger btn-sm'"
            :style="store.deleteArmed ? { background: 'var(--danger)', color: '#fff', borderColor: 'var(--danger)' } : {}"
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
  position: relative;
}
.switch-card:hover {
  border-color: var(--border);
  box-shadow: var(--shadow-soft);
}
.switch-card--active {
  background: var(--accent-soft);
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent-mid) inset;
  padding-left: 18px;
}
.switch-card--active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 4px;
  background: var(--accent);
  border-radius: 0 2px 2px 0;
}
.switch-active-badge {
  background: var(--accent);
  color: #fff;
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
