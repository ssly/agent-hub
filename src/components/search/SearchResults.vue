<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { truncate } from '@/lib/utils'

const { t } = useI18n()
const appStore = useAppStore()
const store = useSkillsStore()

function handleClick(result: any) {
  store.selectedPlatformId = result.platform_id
  store.selectSkill(result.skill_name, result.folder || '')
  store.loadSkills()
  appStore.setView('detail')
}
</script>

<template>
  <div class="p-6 view-enter">
    <div class="ah-view-content">
      <div v-if="store.searchLoading" class="flex items-center justify-center py-16 gap-2" style="color: var(--ink-2)">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="animate-spin"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
        <span class="text-sm">{{ t('ui.searching') }}</span>
      </div>

      <div v-else-if="store.searchResults.length === 0" class="flex flex-col items-center justify-center py-20">
        <p style="color: var(--ink-3)">{{ t('ui.no_results') }}</p>
      </div>

      <template v-else>
        <h2 class="ah-section-title">{{ t('ui.search_results') }}</h2>
        <div class="ah-table-wrap">
          <button
            v-for="r in store.searchResults"
            :key="`${r.platform_id}/${r.folder}/${r.skill_name}`"
            class="w-full text-left px-4 py-3 cursor-pointer transition-colors flex items-center gap-3"
            style="border-bottom: 1px solid var(--hairline)"
            @click="handleClick(r)"
          >
            <span style="color: var(--accent); font-weight: 500">{{ r.skill_name }}</span>
            <span v-if="r.folder" class="text-xs" style="color: var(--warning)">{{ r.folder }}/</span>
            <span class="text-xs" style="color: var(--ink-3)">{{ r.platform_name }}</span>
            <span class="text-sm" style="color: var(--ink-4)">{{ truncate(r.description, 50) }}</span>
          </button>
        </div>
      </template>
    </div>
  </div>
</template>
