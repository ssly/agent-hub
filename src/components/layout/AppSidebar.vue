<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderOpen, Globe2, X } from 'lucide-vue-next'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { usePluginsStore } from '@/stores/plugins'
import { useSessionsStore } from '@/stores/sessions'
import { useSwitchStore } from '@/stores/switch'
import { useToast } from '@/composables/useToast'
import { pickPluginDirectory } from '@/lib/api'

const { t } = useI18n()
const appStore = useAppStore()
const skillsStore = useSkillsStore()
const pluginsStore = usePluginsStore()
const sessionsStore = useSessionsStore()
const switchStore = useSwitchStore()
const { showToast } = useToast()
const isPickingDirectory = ref(false)

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
  { id: 'plugins' as const, labelKey: 'ui.plugins_tab' },
  { id: 'sessions' as const, labelKey: 'ui.sessions_tab' },
  { id: 'accounts' as const, labelKey: 'ui.accounts_tab' },
]

async function handleTabClick(tabId: typeof tabs[number]['id']) {
  appStore.switchTab(tabId)
  if (tabId === 'sessions') {
    sessionsStore.isLoading = true
    Promise.all([
      sessionsStore.refreshPlatforms(),
      sessionsStore.refreshTerminals(),
    ]).finally(() => {
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
    await Promise.all([sessionsStore.refreshPlatforms(true), sessionsStore.refreshTerminals()])
    sessionsStore.isLoading = false
  } else if (appStore.currentTab === 'accounts') {
    if (switchStore.selectedAgent) await switchStore.loadSelectedAgent()
  }
}

function getSidebarItems() {
  if (appStore.currentTab === 'sessions') return sessionsStore.platforms
  if (appStore.currentTab === 'accounts') return [
    { id: 'codex', display_name: 'Codex' },
    { id: 'claude-code', display_name: 'Claude Code' },
    { id: 'grok-build', display_name: 'Grok Build' },
  ]
  return pluginsStore.platforms
}

function getSelectedId() {
  if (appStore.currentTab === 'sessions') return sessionsStore.selectedPlatformId
  if (appStore.currentTab === 'accounts') return switchStore.selectedAgent
  return pluginsStore.selectedPlatformId
}

async function handleItemClick(id: string) {
  if (appStore.currentTab === 'sessions') sessionsStore.selectPlatform(id)
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
    <!-- Header -->
    <div class="ah-sidebar__header">
      <span v-show="!appStore.sidebarCollapsed" class="ah-sidebar__brand">
        {{ t('ui.title') }}
      </span>
      <div class="flex gap-1 items-center">
        <button v-show="!appStore.sidebarCollapsed" class="ah-sidebar__header-btn" @click="appStore.switchLocale()">
          {{ appStore.locale === 'en' ? 'EN' : '中' }}
        </button>
        <button v-show="!appStore.sidebarCollapsed" class="ah-sidebar__header-btn" @click="appStore.toggleTheme()">
          {{ appStore.isNightTheme() ? '☀' : '☾' }}
        </button>
        <button v-show="!appStore.sidebarCollapsed" class="ah-sidebar__header-btn" @click="handleRefresh">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
        </button>
      </div>
    </div>

    <!-- Content (hidden when collapsed) -->
    <template v-if="!appStore.sidebarCollapsed">
      <!-- Tab Bar -->
      <div class="ah-tab-bar">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          :class="['ah-tab', appStore.currentTab === tab.id ? 'is-active' : '']"
          @click="handleTabClick(tab.id)"
        >
          {{ t(tab.labelKey) }}
        </button>
      </div>

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
            <span class="ah-platform-item__name">{{ item.display_name }}</span>
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

      <!-- Version -->
      <div
        class="px-3 py-2 text-center cursor-pointer select-none transition-colors hover:bg-[color:var(--sunken)]"
        style="border-top: 1px solid var(--hairline)"
        @click="appStore.openAbout"
        :title="appStore.isDownloading ? t('about.downloading_title', { percent: appStore.updateProgress }) : (appStore.availableUpdate ? `发现新版本 v${appStore.availableUpdate.version}，点击查看` : t('about.title'))"
      >
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
  </aside>
</template>

<style scoped>
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
</style>
