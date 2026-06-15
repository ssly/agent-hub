<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { useMcpStore } from '@/stores/mcp'
import { useSessionsStore } from '@/stores/sessions'
import { useSwitchStore } from '@/stores/switch'

const { t } = useI18n()
const appStore = useAppStore()
const skillsStore = useSkillsStore()
const mcpStore = useMcpStore()
const sessionsStore = useSessionsStore()
const switchStore = useSwitchStore()

const tabs = [
  { id: 'skills' as const, labelKey: 'ui.skills_tab' },
  { id: 'mcp' as const, labelKey: 'ui.mcp_tab' },
  { id: 'sessions' as const, labelKey: 'ui.sessions_tab' },
  { id: 'switch' as const, labelKey: 'ui.switch_tab' },
]

function handleTabClick(tabId: typeof tabs[number]['id']) {
  appStore.switchTab(tabId)
  if (tabId === 'sessions') {
    sessionsStore.isLoading = true
    Promise.all([
      sessionsStore.refreshPlatforms(),
      sessionsStore.refreshTerminals(),
    ]).finally(() => {
      sessionsStore.isLoading = false
    })
  } else if (tabId === 'switch') {
    if (!switchStore.selectedAgent) {
      switchStore.selectAgent(localStorage.getItem('ah-switch-agent') || 'codex')
    } else {
      // Always refresh on tab entry. selectedAgent is persisted in localStorage, so without
      // this the list would rely on stale store.profiles and could render empty (e.g. after
      // an earlier load failure or a startup race). Mirrors how the sessions tab always refreshes.
      switchStore.loadProfiles()
    }
  }
}

async function handleRefresh() {
  if (appStore.currentTab === 'mcp') {
    await mcpStore.refreshPlatforms()
  } else if (appStore.currentTab === 'sessions') {
    sessionsStore.isLoading = true
    await Promise.all([sessionsStore.refreshPlatforms(true), sessionsStore.refreshTerminals()])
    sessionsStore.isLoading = false
  } else if (appStore.currentTab === 'switch') {
    if (switchStore.selectedAgent) await switchStore.loadProfiles()
  } else {
    await skillsStore.refreshPlatforms()
  }
}

function getSidebarItems() {
  if (appStore.currentTab === 'mcp') return mcpStore.platforms
  if (appStore.currentTab === 'sessions') return sessionsStore.platforms
  if (appStore.currentTab === 'switch') return [
    { id: 'codex', display_name: 'Codex' },
    { id: 'claude-code', display_name: 'Claude Code' },
  ]
  return skillsStore.platforms
}

function getSelectedId() {
  if (appStore.currentTab === 'mcp') return mcpStore.selectedPlatformId
  if (appStore.currentTab === 'sessions') return sessionsStore.selectedPlatformId
  if (appStore.currentTab === 'switch') return switchStore.selectedAgent
  return skillsStore.selectedPlatformId
}

function handleItemClick(id: string) {
  if (appStore.currentTab === 'mcp') mcpStore.selectPlatform(id)
  else if (appStore.currentTab === 'sessions') sessionsStore.selectPlatform(id)
  else if (appStore.currentTab === 'switch') switchStore.selectAgent(id)
  else {
    skillsStore.selectPlatform(id)
    appStore.setView('skills')
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
      appStore.setView('skills')
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
        {{ t('ui.title') }}<span class="ah-sidebar__brand-dot"> ·</span>
      </span>
      <div class="flex gap-1 items-center">
        <button v-show="!appStore.sidebarCollapsed" class="ah-sidebar__header-btn" @click="appStore.switchLocale()">
          {{ appStore.locale === 'en' ? 'EN' : '中文' }}
        </button>
        <button v-show="!appStore.sidebarCollapsed" class="ah-sidebar__header-btn" @click="appStore.toggleTheme()">
          {{ appStore.isNightTheme() ? '☀' : '☾' }}
        </button>
        <button v-show="!appStore.sidebarCollapsed" class="ah-sidebar__header-btn" @click="handleRefresh">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
        </button>
        <button
          class="ah-sidebar__header-btn"
          :style="appStore.sidebarCollapsed ? { transform: 'rotate(180deg)' } : {}"
          @click="appStore.toggleSidebar()"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
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
            <span v-if="item.server_count != null" class="ah-platform-item__count">{{ item.server_count }}</span>
            <span v-if="item.session_count != null" class="ah-platform-item__count">{{ item.session_count }}</span>
          </div>
          <div
            v-if="getSelectedId() === item.id && item.skill_dir"
            class="ah-platform-item__path"
          >
            {{ item.skill_dir }}
          </div>
        </button>
        <p v-if="getSidebarItems().length === 0" class="text-sm p-3" style="color: var(--ink-3)">
          {{ t('ui.no_platforms') }}
        </p>
      </div>

      <!-- Search (skills tab only) -->
      <div v-if="appStore.currentTab === 'skills'" class="p-2.5" style="border-top: 1px solid var(--hairline)">
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
        {{ t('ui.trash') }} ({{ appStore.trashCount }})
      </div>

      <!-- Version -->
      <div
        class="px-3 py-2 text-center cursor-pointer select-none transition-colors hover:bg-[color:var(--sunken)]"
        style="border-top: 1px solid var(--hairline)"
        @click="appStore.openAbout"
        :title="appStore.availableUpdate ? `发现新版本 v${appStore.availableUpdate.version}，点击查看` : t('about.title')"
      >
        <span
          :style="appStore.availableUpdate ? { color: '#f59e0b' } : { color: 'var(--ink-4)' }"
          style="font-size: 11px"
        >
          v{{ appStore.appVersion }}
          <span v-if="appStore.availableUpdate" class="ml-1 text-[10px] font-semibold">{{ t('about.update_available_short') }}</span>
        </span>
      </div>
    </template>
  </aside>
</template>
