<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { usePluginsStore } from '@/stores/plugins'
import { useSessionsStore } from '@/stores/sessions'
import { useSwitchStore } from '@/stores/switch'
import { platform } from '@/lib/utils'

import { useToast } from '@/composables/useToast'
import { useHoverResetBool } from '@/composables/useHoverReset'

const { t } = useI18n()
const appStore = useAppStore()
const skillsStore = useSkillsStore()
const pluginsStore = usePluginsStore()
const sessionsStore = useSessionsStore()
const switchStore = useSwitchStore()
const { showToast } = useToast()

// Windows drops its native frame at startup (see lib.rs), so the toolbar
// doubles as the title bar and hosts custom min/max/close controls. macOS
// keeps its native traffic lights via the overlay title bar instead.
const isWindows = platform === 'windows'
const appWindow = isWindows ? getCurrentWindow() : null
const isMaximized = ref(false)
let unlistenResize: (() => void) | undefined

onMounted(async () => {
  if (!appWindow) return
  isMaximized.value = await appWindow.isMaximized()
  unlistenResize = await appWindow.onResized(async () => {
    isMaximized.value = await appWindow.isMaximized()
  })
})
onUnmounted(() => unlistenResize?.())

const minimizeWindow = () => { appWindow?.minimize().catch(() => {}) }
const toggleMaximizeWindow = () => { appWindow?.toggleMaximize().catch(() => {}) }
const closeWindow = () => { appWindow?.close().catch(() => {}) }

const breadcrumb = computed(() => {
  if (appStore.currentTab === 'monitor') {
    return `${t('session_monitor.title')} — Codex`
  }
  if (appStore.currentTab === 'sessions') {
    const p = sessionsStore.platforms.find(p => p.id === sessionsStore.selectedPlatformId)
    return p?.display_name || t('session.title')
  }
  if (appStore.currentTab === 'accounts') {
    const names: Record<string, string> = {
      codex: 'Codex',
      'claude-code': 'Claude Code',
      'grok-build': 'Grok Build',
      'kimi-code': 'Kimi Code',
      deepseek: 'DeepSeek Harness',
    }
    if (!switchStore.selectedAgent) return t('switch.title')
    const readOnly = ['codex', 'grok-build', 'kimi-code', 'deepseek'].includes(switchStore.selectedAgent)
    const title = readOnly ? t('switch.current_account_title') : t('switch.title')
    return `${title} — ${names[switchStore.selectedAgent] || ''}`
  }
  if (appStore.currentView === 'plugins') {
    return t('plugin.title')
  }
  if (skillsStore.selectedSkillName) {
    // Display-only join: match the native separator of the folder path so
    // Windows never shows a mixed `D:\Coding\skills/my-skill` title.
    if (skillsStore.selectedFolder) {
      const sep = skillsStore.selectedFolder.includes('\\') ? '\\' : '/'
      return `${skillsStore.selectedFolder}${sep}${skillsStore.selectedSkillName}`
    }
    return skillsStore.selectedSkillName
  }
  return ''
})

const showBack = computed(() => appStore.currentTab === 'plugins' && appStore.currentView !== 'plugins')
const showDiff = computed(() => pluginsStore.isGlobalScope
  && appStore.currentTab === 'plugins'
  && (appStore.currentView === 'detail' || appStore.currentView === 'diff'))
const showSync = computed(() => pluginsStore.isGlobalScope
  && appStore.currentTab === 'plugins'
  && appStore.currentView === 'detail')
const showDelete = showSync

const { armed: confirmingDelete, arm: armConfirmDelete, reset: resetConfirmDelete } = useHoverResetBool()

async function handleDeleteClick() {
  if (!skillsStore.selectedSkillName) return
  if (!confirmingDelete.value) {
    armConfirmDelete()
    return
  }
  resetConfirmDelete()
  try {
    await skillsStore.performDeleteSkill(skillsStore.selectedSkillName, skillsStore.selectedFolder)
    showToast(t('skill.deleted'), 'success')
    appStore.refreshTrashCount()
    appStore.setView('plugins')
  } catch (e: any) {
    showToast(t('skill.delete_failed', { error: e?.message || e?.SyncError || String(e) }), 'error')
  }
}

function handleBack() {
  skillsStore.backToList()
  appStore.setView('plugins')
}

async function handleDiffClick() {
  try {
    await skillsStore.loadDiffCandidates()
    if (skillsStore.diffCandidates.length === 0) {
      showToast(t('diff.no_other'), 'warning')
    } else {
      skillsStore.diffPlatformModalOpen = true
    }
  } catch (e: any) {
    showToast(String(e), 'error')
  }
}

async function handleSyncClick() {
  try {
    await skillsStore.loadSyncTargets()
    if (skillsStore.syncTargets.length === 0) {
      showToast(t('sync.no_targets') || 'No sync targets found', 'warning')
    } else {
      skillsStore.syncTargetPlatformId = skillsStore.syncTargets[0].id
      skillsStore.syncPlatformModalOpen = true
    }
  } catch (e: any) {
    showToast(String(e), 'error')
  }
}
</script>

<template>
  <div class="ah-toolbar" data-tauri-drag-region="deep">
    <button v-if="showBack" class="ah-toolbar__back" @click="handleBack">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
      {{ t('action.back') }}
    </button>

    <span class="ah-toolbar__breadcrumb">{{ breadcrumb }}</span>

    <div class="flex-1" />

    <button v-if="showDiff" class="btn btn-secondary btn-sm" @click="handleDiffClick">
      {{ t('action.diff') }}
    </button>

    <button v-if="showSync" class="btn btn-secondary btn-sm" @click="handleSyncClick">
      {{ t('action.sync') }}
    </button>

    <button
      v-if="showDelete"
      :class="['btn btn-sm', confirmingDelete ? 'btn-danger' : 'btn-secondary']"
      @click="handleDeleteClick"
      @mouseleave="resetConfirmDelete"
    >
      {{ confirmingDelete ? t('skill.confirm_delete') : t('skill.delete') }}
    </button>

    <!-- Custom window controls (Windows only; frame is removed at startup) -->
    <div v-if="isWindows" class="ah-win-controls">
      <button class="ah-win-controls__btn" aria-label="Minimize" @click="minimizeWindow">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1"><line x1="1.5" y1="6" x2="10.5" y2="6"/></svg>
      </button>
      <button
        class="ah-win-controls__btn"
        :aria-label="isMaximized ? 'Restore' : 'Maximize'"
        @click="toggleMaximizeWindow"
      >
        <svg v-if="isMaximized" width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1">
          <rect x="3.5" y="1.5" width="7" height="7"/>
          <path d="M8.5 8.5v2h-7v-7h2" fill="none"/>
        </svg>
        <svg v-else width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1">
          <rect x="1.5" y="1.5" width="9" height="9"/>
        </svg>
      </button>
      <button class="ah-win-controls__btn ah-win-controls__btn--close" aria-label="Close" @click="closeWindow">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1">
          <line x1="1.5" y1="1.5" x2="10.5" y2="10.5"/>
          <line x1="10.5" y1="1.5" x2="1.5" y2="10.5"/>
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
/* Windows-style caption buttons, flush with the toolbar's top-right corner. */
.ah-win-controls {
  display: flex;
  align-items: stretch;
  align-self: stretch;
  margin: 0 -18px 0 6px;
}
.ah-win-controls__btn {
  width: 46px;
  display: grid;
  place-items: center;
  border: none;
  background: transparent;
  color: var(--ink-2);
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-soft), color var(--dur-fast) var(--ease-soft);
}
.ah-win-controls__btn:hover { background: var(--hover); color: var(--ink); }
.ah-win-controls__btn--close:hover { background: #e81123; color: #fff; }
</style>
