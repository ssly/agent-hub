<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Activity, Blocks, FolderOpen, Gauge, Globe2, MessagesSquare, Settings, UserRound, X } from 'lucide-vue-next'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { usePluginsStore } from '@/stores/plugins'
import { useSessionsStore } from '@/stores/sessions'
import { MONITOR_AGENTS, useSessionMonitorStore, type MonitorTab } from '@/stores/session-monitor'
import { useSwitchStore } from '@/stores/switch'
import { useToast } from '@/composables/useToast'
import { openUsageTray, pickPluginDirectory } from '@/lib/api'
import AgentIcon from '@/components/agents/AgentIcon.vue'
import AppModal from '@/components/ui/AppModal.vue'

const { t } = useI18n()
const appStore = useAppStore()
const skillsStore = useSkillsStore()
const pluginsStore = usePluginsStore()
const sessionsStore = useSessionsStore()
const sessionMonitorStore = useSessionMonitorStore()
const switchStore = useSwitchStore()
const { showToast } = useToast()
const isPickingDirectory = ref(false)

// Usage settings modal (auto-refresh interval slider, 1–10 min). The value is
// shared with the tray popup through backend in-memory settings; the
// `usage-monitor-settings-changed` listener in the switch store keeps this
// modal in sync when the tray changes it.
const usageSettingsOpen = ref(false)
const refreshMinutesDraft = ref<number | null>(null)

async function openUsageSettings() {
  await switchStore.loadMonitorSettings()
  refreshMinutesDraft.value = null
  usageSettingsOpen.value = true
}

function onRefreshMinutesInput(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isFinite(value)) refreshMinutesDraft.value = Math.min(10, Math.max(1, Math.round(value)))
}

async function persistRefreshMinutes() {
  if (refreshMinutesDraft.value == null) return
  await switchStore.updateRefreshMinutes(refreshMinutesDraft.value)
  refreshMinutesDraft.value = null
}

const workspaceName = computed(() => {
  if (!pluginsStore.workspaceDirectory) return t('plugin.scope_global')
  const parts = pluginsStore.workspaceDirectory.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] || pluginsStore.workspaceDirectory
})

async function handlePickDirectory() {
  if (isPickingDirectory.value) return
  isPickingDirectory.value = true
  try {
    const directory = await pickPluginDirectory()
    if (!directory) return
    await pluginsStore.setWorkspaceDirectory(directory)
    appStore.setView('plugins')
  } catch (error: any) {
    showToast(t('plugin.scope_pick_failed', { error: error?.message || String(error) }), 'error')
  } finally {
    isPickingDirectory.value = false
  }
}

async function handleUseGlobalDirectory() {
  await pluginsStore.setWorkspaceDirectory(null)
  appStore.setView('plugins')
}

const tabs = [
  { id: 'plugins' as const, labelKey: 'ui.plugins_tab', icon: Blocks },
  { id: 'sessions' as const, labelKey: 'ui.sessions_tab', icon: MessagesSquare },
  { id: 'monitor' as const, labelKey: 'ui.monitor_tab', icon: Activity },
  { id: 'accounts' as const, labelKey: 'ui.accounts_tab', icon: UserRound },
]

async function handleTabClick(tabId: typeof tabs[number]['id']) {
  // Monitor: flip loading on *before* the view mounts so the first paint
  // already shows the wave loader (Pinia store HMR / cold IPC shouldn't look
  // like a frozen tab switch).
  if (tabId === 'monitor') {
    sessionMonitorStore.beginEnter()
  }
  appStore.switchTab(tabId)
  if (tabId === 'sessions') {
    sessionsStore.isLoading = true
    sessionsStore.refreshPlatforms().finally(() => {
      sessionsStore.isLoading = false
    })
  } else if (tabId === 'accounts') {
    if (!switchStore.selectedAgent) {
      await switchStore.selectAgent(localStorage.getItem('ah-switch-agent') || 'codex')
    } else {
      // Entering Accounts reloads profiles and, for Codex, performs a fresh
      // quota query through the same snapshot command as the tray popup.
      await switchStore.loadSelectedAgent()
    }
  }
}

async function handleRefresh() {
  if (appStore.currentTab === 'plugins') {
    await pluginsStore.refreshPlatforms()
  } else if (appStore.currentTab === 'sessions') {
    sessionsStore.isLoading = true
    await sessionsStore.refreshPlatforms(true)
    sessionsStore.isLoading = false
  } else if (appStore.currentTab === 'accounts') {
    if (switchStore.selectedAgent) await switchStore.loadSelectedAgent()
  } else if (appStore.currentTab === 'monitor') {
    await sessionMonitorStore.refresh()
  }
}

function getSidebarItems() {
  if (appStore.currentTab === 'sessions') return sessionsStore.platforms
  if (appStore.currentTab === 'monitor') return [
    { id: 'all', display_name: t('session_monitor.agent_all') },
    ...MONITOR_AGENTS.map(agent => ({
      id: agent,
      display_name: t(`session_monitor.agent_${agent}`),
    })),
  ]
  if (appStore.currentTab === 'accounts') return [
    { id: 'codex', display_name: 'Codex' },
    { id: 'claude-code', display_name: 'Claude Code' },
    { id: 'grok-build', display_name: 'Grok Build' },
    { id: 'kimi-code', display_name: 'Kimi Code' },
  ]
  return pluginsStore.platforms
}

function platformLabel(item: { id: string; display_name?: string }) {
  if (item.id === 'shared') return t('plugin.platform_shared')
  return item.display_name || item.id
}

function getSelectedId() {
  if (appStore.currentTab === 'sessions') return sessionsStore.selectedPlatformId
  if (appStore.currentTab === 'monitor') return sessionMonitorStore.activeAgent
  if (appStore.currentTab === 'accounts') return switchStore.selectedAgent
  return pluginsStore.selectedPlatformId
}

async function handleItemClick(id: string) {
  if (appStore.currentTab === 'sessions') sessionsStore.selectPlatform(id)
  else if (appStore.currentTab === 'monitor') sessionMonitorStore.activeAgent = id as MonitorTab
  else if (appStore.currentTab === 'accounts') await switchStore.selectAgent(id)
  else {
    pluginsStore.selectPlatform(id)
    appStore.setView('plugins')
  }
}

let searchDebounce: ReturnType<typeof setTimeout>
function handleSearch(e: Event) {
  const query = (e.target as HTMLInputElement).value
  clearTimeout(searchDebounce)
  searchDebounce = setTimeout(() => {
    if (query.trim()) {
      skillsStore.doSearch(query)
      appStore.setView('search')
    } else {
      skillsStore.searchResults = []
      appStore.setView('plugins')
    }
  }, 300)
}

let sessionSearchDebounce: ReturnType<typeof setTimeout>
function handleSessionSearch(e: Event) {
  const query = (e.target as HTMLInputElement).value
  clearTimeout(sessionSearchDebounce)
  sessionSearchDebounce = setTimeout(() => {
    sessionsStore.doSearch(query)
  }, 300)
}
</script>

<template>
  <aside :class="['ah-sidebar', appStore.sidebarCollapsed ? 'ah-sidebar--collapsed' : 'ah-sidebar--expanded']">
    <!-- Header (window drag region) -->
    <div class="ah-sidebar__header" data-tauri-drag-region="deep">
      <span v-show="!appStore.sidebarCollapsed" class="ah-sidebar__brand">
        {{ t('ui.title') }}
      </span>
      <div class="flex items-center gap-0.5 shrink-0 ml-auto">
        <button
          v-show="!appStore.sidebarCollapsed"
          v-tooltip="t('ui.refresh')"
          class="ah-sidebar__header-btn"
          @click="handleRefresh"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
        </button>
      </div>
    </div>

    <!-- Content (hidden when collapsed) -->
    <template v-if="!appStore.sidebarCollapsed">
      <!-- Vertical nav (language-agnostic widths, matches platform list) -->
      <nav class="ah-nav">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          :class="['ah-nav-item', appStore.currentTab === tab.id ? 'is-active' : '']"
          @click="handleTabClick(tab.id)"
        >
          <component :is="tab.icon" :size="15" class="ah-nav-item__icon" />
          <span>{{ t(tab.labelKey) }}</span>
        </button>
      </nav>

      <div v-if="appStore.currentTab === 'plugins'" class="ah-scope-picker">
        <div class="ah-scope-picker__label">{{ t('plugin.scope_label') }}</div>
        <div class="ah-scope-picker__control">
          <button
            type="button"
            class="ah-scope-picker__main"
            :disabled="isPickingDirectory"
            :title="pluginsStore.workspaceDirectory || t('plugin.scope_global_hint')"
            @click="handlePickDirectory"
          >
            <Globe2 v-if="pluginsStore.isGlobalScope" :size="14" />
            <FolderOpen v-else :size="14" />
            <span class="ah-scope-picker__text">
              <strong>{{ workspaceName }}</strong>
              <small v-if="pluginsStore.workspaceDirectory">{{ pluginsStore.workspaceDirectory }}</small>
              <small v-else>{{ t('plugin.scope_choose') }}</small>
            </span>
          </button>
          <button
            v-if="!pluginsStore.isGlobalScope"
            type="button"
            class="ah-scope-picker__reset"
            :title="t('plugin.scope_use_global')"
            :aria-label="t('plugin.scope_use_global')"
            @click="handleUseGlobalDirectory"
          >
            <X :size="13" />
          </button>
        </div>
      </div>

      <!-- Platform List -->
      <div class="ah-platform-list flex-1 overflow-y-auto space-y-0.5">
        <button
          v-for="item in getSidebarItems()"
          :key="item.id"
          :class="['ah-platform-item', getSelectedId() === item.id ? 'is-active' : '']"
          @click="handleItemClick(item.id)"
        >
          <div class="flex items-center justify-between">
            <span class="ah-platform-item__label">
              <AgentIcon :agent-id="item.id" class="ah-platform-item__icon" />
              <span class="ah-platform-item__name">{{ platformLabel(item) }}</span>
            </span>
            <span v-if="appStore.currentTab === 'sessions' && item.session_count != null" class="ah-platform-item__count">
              {{ item.session_count }}
            </span>
          </div>
        </button>
        <p v-if="getSidebarItems().length === 0" class="text-sm p-3" style="color: var(--ink-3)">
          {{ t('ui.no_platforms') }}
        </p>
      </div>

      <!-- Search (skills tab only) -->
      <div v-if="appStore.currentTab === 'plugins'" class="p-2.5" style="border-top: 1px solid var(--hairline)">
        <input
          type="text"
          :placeholder="t('ui.search_placeholder')"
          class="ah-search-input"
          @input="handleSearch"
        />
      </div>

      <!-- Search (sessions tab only) -->
      <div v-if="appStore.currentTab === 'sessions' && sessionsStore.selectedPlatformId" class="p-2.5" style="border-top: 1px solid var(--hairline)">
        <input
          type="text"
          :placeholder="t('session.search_placeholder', { agent: sessionsStore.platforms.find(p => p.id === sessionsStore.selectedPlatformId)?.display_name || '' })"
          class="ah-search-input"
          :value="sessionsStore.searchQuery"
          @input="handleSessionSearch"
        />
      </div>

      <!-- Trash badge -->
      <div
        v-if="appStore.trashCount > 0"
        class="px-3 py-2 cursor-pointer flex items-center gap-2 text-xs"
        style="border-top: 1px solid var(--hairline); color: var(--ink-3)"
        @click="appStore.openTrash()"
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
        {{ t('trash.title') }} ({{ appStore.trashCount }})
      </div>

      <!-- Version (+ footer shortcuts: monitor panel, language, theme) -->
      <div
        class="relative px-3 py-2 text-center cursor-pointer select-none transition-colors hover:bg-[color:var(--sunken)]"
        style="border-top: 1px solid var(--hairline)"
        @click="appStore.openAbout"
        :title="appStore.isDownloading ? t('about.downloading_title', { percent: appStore.updateProgress }) : (appStore.availableUpdate ? `发现新版本 v${appStore.availableUpdate.version}，点击查看` : t('about.title'))"
      >
        <div class="sidebar-footer-actions">
          <button
            v-tooltip="t('ui.settings')"
            class="sidebar-footer-btn"
            @click.stop="openUsageSettings"
          >
            <Settings :size="12" />
          </button>
          <button
            v-tooltip="t('tray.open_usage')"
            class="sidebar-footer-btn"
            @click.stop="openUsageTray"
          >
            <Gauge :size="12" />
          </button>
          <button
            v-tooltip="t('ui.switch_language')"
            class="sidebar-footer-btn sidebar-footer-btn--text"
            @click.stop="appStore.switchLocale()"
          >
            {{ appStore.locale === 'en' ? 'EN' : '中' }}
          </button>
          <button
            v-tooltip="t('ui.toggle_theme')"
            class="sidebar-footer-btn sidebar-footer-btn--text"
            @click.stop="appStore.toggleTheme()"
          >
            {{ appStore.isNight ? '☾' : '☀' }}
          </button>
        </div>
        <!-- Downloading: spinner + percent -->
        <span
          v-if="appStore.isDownloading"
          class="inline-flex items-center justify-center gap-1.5"
          style="color: var(--accent); font-size: 11px"
        >
          <svg class="about-spin" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
          {{ appStore.updateProgress }}%
        </span>
        <span
          v-else
          :style="appStore.availableUpdate ? { color: 'var(--warning)' } : { color: 'var(--ink-4)' }"
          style="font-size: 11px"
        >
          v{{ appStore.appVersion }}
          <span v-if="appStore.availableUpdate" class="ml-1 text-[10px] font-semibold">{{ t('about.update_available_short') }}</span>
        </span>
      </div>
    </template>

    <!-- Usage settings: shared auto-refresh interval (1–10 min), synced with
         the tray popup through the backend settings snapshot. -->
    <AppModal
      :show="usageSettingsOpen"
      :title="t('usage_settings.title')"
      width-class="w-[22rem]"
      @close="usageSettingsOpen = false"
    >
      <div class="flex flex-col gap-2">
        <label class="text-xs font-semibold" style="color: var(--ink-2)">
          {{ t('usage_settings.refresh_interval') }}
        </label>
        <div class="flex items-center gap-3">
          <input
            type="range"
            class="usage-settings-slider flex-1"
            min="1"
            max="10"
            step="1"
            :value="refreshMinutesDraft ?? switchStore.refreshMinutes"
            :aria-label="t('usage_settings.refresh_interval')"
            @input="onRefreshMinutesInput"
            @change="persistRefreshMinutes"
          >
          <span class="usage-settings-value">
            {{ t('usage_settings.minutes', { n: refreshMinutesDraft ?? switchStore.refreshMinutes }) }}
          </span>
        </div>
      </div>
    </AppModal>
  </aside>
</template>

<style scoped>
.sidebar-footer-actions {
  position: absolute;
  left: 8px;
  top: 50%;
  transform: translateY(-50%);
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
.sidebar-footer-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  color: var(--ink-4);
  background: transparent;
  cursor: pointer;
  transition: color .15s ease, background-color .15s ease;
}
.sidebar-footer-btn--text {
  width: auto;
  min-width: 20px;
  padding: 0 3px;
  font-size: 10px;
  font-weight: 600;
}
.sidebar-footer-btn:hover {
  color: var(--accent);
  background: var(--sunken);
}

.ah-nav {
  padding: 8px 10px 7px;
  border-bottom: 1px solid var(--hairline);
  /* 2×2 grid: icon above label, one cell per main tab. */
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
}
.ah-nav-item {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px 4px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--ink-2);
  font-size: 12px;
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-soft), color var(--dur-fast) var(--ease-soft);
}
.ah-nav-item:hover { background: var(--hover); color: var(--ink); }
.ah-nav-item.is-active {
  background: var(--surface);
  color: var(--ink);
  font-weight: 600;
  box-shadow: inset 0 0 0 1px var(--hairline);
}
.ah-nav-item__icon { flex-shrink: 0; color: var(--ink-3); transition: color var(--dur-fast) var(--ease-soft); }
.ah-nav-item:hover .ah-nav-item__icon { color: var(--ink-2); }
.ah-nav-item.is-active .ah-nav-item__icon { color: var(--accent); }
.ah-scope-picker {
  padding: 10px;
  border-bottom: 1px solid var(--hairline);
}
.ah-scope-picker__label {
  margin: 0 4px 5px;
  color: var(--ink-4);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: .06em;
  text-transform: uppercase;
}
.ah-scope-picker__control {
  display: flex;
  align-items: stretch;
  min-width: 0;
  border: 1px solid var(--hairline);
  border-radius: 9px;
  background: var(--surface);
  overflow: hidden;
}
.ah-scope-picker__main {
  min-width: 0;
  flex: 1;
  padding: 8px 9px;
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--ink-2);
  text-align: left;
  cursor: pointer;
}
.ah-scope-picker__main:hover { background: var(--hover); }
.ah-scope-picker__main:disabled { cursor: wait; opacity: .6; }
.ah-scope-picker__text { min-width: 0; display: grid; gap: 1px; }
.ah-scope-picker__text strong,
.ah-scope-picker__text small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ah-scope-picker__text strong { color: var(--ink); font-size: 12px; font-weight: 600; }
.ah-scope-picker__text small { color: var(--ink-4); font-size: 10px; }
.ah-scope-picker__reset {
  width: 30px;
  flex: 0 0 30px;
  display: grid;
  place-items: center;
  border-left: 1px solid var(--hairline);
  color: var(--ink-4);
  cursor: pointer;
}
.ah-scope-picker__reset:hover { color: var(--ink); background: var(--hover); }
.about-spin {
  animation: about-spin 0.8s linear infinite;
}
@keyframes about-spin {
  to { transform: rotate(360deg); }
}

/* Usage settings modal slider (mirrors the tray popup's slider look). */
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
  background: var(--accent);
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
