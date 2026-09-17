<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, FolderCheck } from 'lucide-vue-next'
import AppModal from '@/components/ui/AppModal.vue'
import AgentIcon from '@/components/agents/AgentIcon.vue'
import { useSwitchStore } from '@/stores/switch'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import type { SupportedAgentInfo } from '@/lib/api'

const props = defineProps<{
  show: boolean
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'change'): void
}>()

const { t } = useI18n()
const switchStore = useSwitchStore()
const { showToast } = useToast()

const loading = ref(false)
const saving = ref(false)
const supportedAgents = ref<SupportedAgentInfo[]>([])
const enabledIds = ref<Set<string>>(new Set())

// Usage refresh & monitor limit drafts
const refreshMinutesDraft = ref<number | null>(null)
const monitorLimitDraft = ref<number | null>(null)

async function loadData() {
  loading.value = true
  try {
    await switchStore.loadMonitorSettings()
    refreshMinutesDraft.value = null
    monitorLimitDraft.value = null
    const list = await api.getSupportedAgents()
    const filtered = list.filter(a => a.id !== 'shared')
    supportedAgents.value = filtered
    enabledIds.value = new Set(filtered.filter(a => a.enabled).map(a => a.id))
  } catch (error: any) {
    showToast(t('settings.save_failed', { error: error?.message || String(error) }), 'error')
  } finally {
    loading.value = false
  }
}

watch(
  () => props.show,
  (open) => {
    if (open) {
      void loadData()
    }
  },
  { immediate: true },
)

async function persistEnabledAgents(nextIds: string[]) {
  saving.value = true
  try {
    const updated = await api.setEnabledAgents(nextIds)
    const filtered = updated.filter(a => a.id !== 'shared')
    supportedAgents.value = filtered
    enabledIds.value = new Set(filtered.filter(a => a.enabled).map(a => a.id))
    emit('change')
  } catch (error: any) {
    showToast(t('settings.save_failed', { error: error?.message || String(error) }), 'error')
  } finally {
    saving.value = false
  }
}

async function toggleAgent(id: string) {
  const next = new Set(enabledIds.value)
  if (next.has(id)) {
    next.delete(id)
  } else {
    next.add(id)
  }
  enabledIds.value = next
  await persistEnabledAgents(Array.from(next))
}

async function handleSelectDetected() {
  const detected = supportedAgents.value.filter(a => a.exists).map(a => a.id)
  enabledIds.value = new Set(detected)
  await persistEnabledAgents(detected)
}

async function handleSelectAll() {
  const all = supportedAgents.value.map(a => a.id)
  enabledIds.value = new Set(all)
  await persistEnabledAgents(all)
}

async function handleClearAll() {
  enabledIds.value = new Set()
  await persistEnabledAgents([])
}

function onRefreshMinutesInput(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isFinite(value)) {
    refreshMinutesDraft.value = Math.min(10, Math.max(1, Math.round(value)))
  }
}

async function persistRefreshMinutes() {
  if (refreshMinutesDraft.value == null) return
  await switchStore.updateRefreshMinutes(refreshMinutesDraft.value)
  refreshMinutesDraft.value = null
}

function onMonitorLimitInput(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isFinite(value)) {
    monitorLimitDraft.value = Math.min(12, Math.max(6, Math.round(value)))
  }
}

async function persistMonitorLimit() {
  if (monitorLimitDraft.value == null) return
  await switchStore.updateMonitorLimit(monitorLimitDraft.value)
  monitorLimitDraft.value = null
}
</script>

<template>
  <AppModal
    :show="show"
    :title="t('settings.title')"
    width-class="w-[36rem]"
    @close="emit('close')"
  >
    <div class="settings-modal flex flex-col gap-5">
      <!-- Section 1: Supported Agents -->
      <div class="settings-section flex flex-col gap-2.5">
        <div class="flex items-start justify-between gap-2">
          <div>
            <h3 class="settings-section__title">{{ t('settings.supported_agents') }}</h3>
            <p class="settings-section__subtitle">{{ t('settings.supported_agents_hint') }}</p>
          </div>
          <!-- Quick actions -->
          <div class="flex items-center gap-1.5 shrink-0 pt-0.5">
            <button
              type="button"
              class="settings-btn settings-btn--primary"
              :title="t('settings.select_detected')"
              :disabled="saving"
              @click="handleSelectDetected"
            >
              <FolderCheck :size="12" />
              <span>{{ t('settings.select_detected') }}</span>
            </button>
            <button
              type="button"
              class="settings-btn"
              :disabled="saving"
              @click="handleSelectAll"
            >
              {{ t('settings.select_all') }}
            </button>
            <button
              type="button"
              class="settings-btn"
              :disabled="saving"
              @click="handleClearAll"
            >
              {{ t('settings.clear_all') }}
            </button>
          </div>
        </div>

        <!-- Agent list container -->
        <div class="settings-agent-list">
          <div
            v-for="agent in supportedAgents"
            :key="agent.id"
            class="settings-agent-row"
            :class="{ 'is-enabled': enabledIds.has(agent.id) }"
            @click="toggleAgent(agent.id)"
          >
            <!-- Checkbox -->
            <label class="settings-agent-checkbox" @click.stop>
              <input
                type="checkbox"
                :checked="enabledIds.has(agent.id)"
                :disabled="saving"
                @change="toggleAgent(agent.id)"
              />
              <span class="settings-agent-checkbox__box">
                <Check v-if="enabledIds.has(agent.id)" :size="11" class="check-icon" />
              </span>
            </label>

            <!-- Icon & Name -->
            <div class="settings-agent-main">
              <AgentIcon :agent-id="agent.id" :size="16" class="settings-agent-icon" />
              <span class="settings-agent-name">{{ agent.display_name }}</span>
            </div>

            <!-- Path Display (with resolved native path tooltip) -->
            <span
              class="settings-agent-path"
              v-tooltip="agent.user_dir_resolved"
            >
              {{ agent.user_dir_display }}
            </span>

            <!-- Presence status badge -->
            <span
              class="settings-agent-status"
              :class="agent.exists ? 'is-detected' : 'is-missing'"
            >
              <span class="settings-agent-status__dot" />
              {{ agent.exists ? t('settings.status_detected') : t('settings.status_not_detected') }}
            </span>
          </div>
        </div>
      </div>

      <!-- Section 2: Usage auto-refresh & Monitor session limit -->
      <div class="settings-section flex flex-col gap-3 pt-3" style="border-top: 1px solid var(--hairline)">
        <label class="settings-section__title" style="margin-bottom: 0;">
          {{ t('settings.section_usage') }}
        </label>
        <div>
          <p class="settings-section__subtitle">{{ t('settings.refresh_interval') }}</p>
          <div class="flex items-center gap-3 mt-1">
            <input
              type="range"
              class="usage-settings-slider flex-1"
              min="1"
              max="10"
              step="1"
              :value="refreshMinutesDraft ?? switchStore.refreshMinutes"
              :aria-label="t('settings.refresh_interval')"
              @input="onRefreshMinutesInput"
              @change="persistRefreshMinutes"
            />
            <span class="usage-settings-value">
              {{ t('settings.minutes', { n: refreshMinutesDraft ?? switchStore.refreshMinutes }) }}
            </span>
          </div>
        </div>
        <div>
          <p class="settings-section__subtitle">{{ t('settings.monitor_limit') }}</p>
          <div class="flex items-center gap-3 mt-1">
            <input
              type="range"
              class="usage-settings-slider flex-1"
              min="6"
              max="12"
              step="1"
              :value="monitorLimitDraft ?? switchStore.monitorLimit"
              :aria-label="t('settings.monitor_limit')"
              @input="onMonitorLimitInput"
              @change="persistMonitorLimit"
            />
            <span class="usage-settings-value">
              {{ t('settings.items', { n: monitorLimitDraft ?? switchStore.monitorLimit }) }}
            </span>
          </div>
        </div>
      </div>
    </div>
  </AppModal>
</template>

<style scoped>
.settings-section__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--ink);
  line-height: 1.25;
}
.settings-section__subtitle {
  font-size: 11.5px;
  color: var(--ink-3);
  margin-top: 2px;
  line-height: 1.35;
}

.settings-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 24px;
  padding: 0 8px;
  font-size: 11px;
  font-weight: 500;
  color: var(--ink-2);
  background: var(--sunken);
  border: 1px solid var(--hairline);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--dur-fast) var(--ease-soft);
}
.settings-btn:hover:not(:disabled) {
  color: var(--ink);
  background: var(--hover);
  border-color: var(--border);
}
.settings-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.settings-btn--primary {
  color: var(--accent);
  background: var(--accent-soft);
  border-color: transparent;
}
.settings-btn--primary:hover:not(:disabled) {
  color: var(--on-accent);
  background: var(--accent);
  border-color: transparent;
}

.settings-agent-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 280px;
  overflow-y: auto;
  padding: 4px;
  background: var(--surface);
  border: 1px solid var(--hairline);
  border-radius: var(--radius-md);
}

.settings-agent-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-soft);
  user-select: none;
}
.settings-agent-row:hover {
  background: var(--hover);
}
.settings-agent-row.is-enabled {
  background: color-mix(in srgb, var(--hover) 40%, transparent);
}
.settings-agent-row.is-enabled:hover {
  background: var(--hover);
}

.settings-agent-checkbox {
  position: relative;
  display: inline-flex;
  align-items: center;
  cursor: pointer;
}
.settings-agent-checkbox input {
  position: absolute;
  opacity: 0;
  width: 0;
  height: 0;
}
.settings-agent-checkbox__box {
  width: 15px;
  height: 15px;
  border-radius: 2px;
  border: 1.5px solid var(--border);
  background: var(--surface);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all var(--dur-fast) var(--ease-soft);
}
.settings-agent-checkbox input:checked + .settings-agent-checkbox__box {
  background: var(--accent);
  border-color: var(--accent);
}
.check-icon {
  color: var(--on-accent);
  stroke-width: 3;
}

.settings-agent-main {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}
.settings-agent-icon {
  flex: none;
  color: var(--ink-2);
}
.settings-agent-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--ink);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.settings-agent-path {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11px;
  color: var(--ink-3);
  padding: 2px 6px;
  background: var(--sunken);
  border-radius: var(--radius-sm);
  white-space: nowrap;
  flex: none;
}

.settings-agent-status {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  padding: 2px 7px;
  border-radius: 999px;
  flex: none;
  font-weight: 500;
}
.settings-agent-status__dot {
  width: 5px;
  height: 5px;
  border-radius: 999px;
}
.settings-agent-status.is-detected {
  background: var(--success-soft);
  color: var(--success);
}
.settings-agent-status.is-detected .settings-agent-status__dot {
  background: var(--success);
  box-shadow: 0 0 4px color-mix(in srgb, var(--success) 60%, transparent);
}
.settings-agent-status.is-missing {
  background: var(--sunken);
  color: var(--ink-4);
}
.settings-agent-status.is-missing .settings-agent-status__dot {
  background: var(--ink-4);
}

.usage-settings-slider {
  -webkit-appearance: none;
  appearance: none;
  height: 14px;
  margin: 0;
  padding: 0;
  background: transparent;
  cursor: pointer;
}
.usage-settings-slider:focus { outline: none; }
.usage-settings-slider::-webkit-slider-runnable-track {
  height: 3px;
  border-radius: 999px;
  background: var(--border);
}
.usage-settings-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  margin-top: -4.5px;
  border: 0;
  border-radius: 50%;
  background: var(--signal-red);
  cursor: grab;
}
.usage-settings-slider::-moz-range-track {
  height: 3px;
  border: 0;
  border-radius: 999px;
  background: var(--border);
}
.usage-settings-slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border: 0;
  border-radius: 50%;
  background: var(--accent);
  cursor: grab;
}
.usage-settings-value {
  min-width: 46px;
  text-align: right;
  font-size: 12px;
  color: var(--ink-2);
  font-variant-numeric: tabular-nums;
}
</style>
