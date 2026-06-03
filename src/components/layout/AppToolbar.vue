<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { useMcpStore } from '@/stores/mcp'
import { useSessionsStore } from '@/stores/sessions'
import { useSwitchStore } from '@/stores/switch'

import { useToast } from '@/composables/useToast'

const { t } = useI18n()
const appStore = useAppStore()
const skillsStore = useSkillsStore()
const mcpStore = useMcpStore()
const sessionsStore = useSessionsStore()
const switchStore = useSwitchStore()
const { showToast } = useToast()

const breadcrumb = computed(() => {
  if (appStore.currentTab === 'mcp') {
    const p = mcpStore.platforms.find(p => p.id === mcpStore.selectedPlatformId)
    return p?.display_name || t('mcp.title')
  }
  if (appStore.currentTab === 'sessions') {
    const p = sessionsStore.platforms.find(p => p.id === sessionsStore.selectedPlatformId)
    return p?.display_name || t('session.title')
  }
  if (appStore.currentTab === 'switch') {
    const names: Record<string, string> = { codex: 'Codex', 'claude-code': 'Claude Code' }
    return switchStore.selectedAgent ? `${t('switch.title')} — ${names[switchStore.selectedAgent] || ''}` : t('switch.title')
  }
  if (appStore.currentView === 'skills') {
    return skillsStore.selectedPlatform?.display_name || ''
  }
  if (skillsStore.selectedSkillName) {
    return skillsStore.selectedFolder
      ? `${skillsStore.selectedFolder}/${skillsStore.selectedSkillName}`
      : skillsStore.selectedSkillName
  }
  return ''
})

const showBack = computed(() => appStore.currentTab === 'skills' && appStore.currentView !== 'skills')
const showDiff = computed(() => appStore.currentTab === 'skills' && (appStore.currentView === 'detail' || appStore.currentView === 'diff'))
const showSync = computed(() => appStore.currentTab === 'skills' && appStore.currentView === 'detail')
const showScan = computed(() => appStore.currentTab === 'skills')

function handleBack() {
  skillsStore.backToList()
  appStore.setView('skills')
}

async function handleScan() {
  try {
    await skillsStore.scanInvalidSkills()
  } catch (e: any) {
    showToast(t('scan_invalid.error', { error: e?.message || e?.SyncError || String(e) }), 'error')
  }
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
      skillsStore.syncOverwrite = false
      skillsStore.syncPlatformModalOpen = true
    }
  } catch (e: any) {
    showToast(String(e), 'error')
  }
}
</script>

<template>
  <div class="ah-toolbar">
    <button v-if="showBack" class="ah-toolbar__back" @click="handleBack">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
      {{ t('action.back') }}
    </button>

    <span class="ah-toolbar__breadcrumb">{{ breadcrumb }}</span>

    <div class="flex-1" />

    <button v-if="showScan" class="btn btn-primary btn-sm" @click="handleScan">
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
      {{ t('action.refresh') === 'Refresh' ? 'Check' : '检查' }}
    </button>

    <button v-if="showDiff" class="btn btn-secondary btn-sm" @click="handleDiffClick">
      {{ t('action.diff') }}
    </button>

    <button v-if="showSync" class="btn btn-secondary btn-sm" @click="handleSyncClick">
      {{ t('action.sync') }}
    </button>
  </div>
</template>
