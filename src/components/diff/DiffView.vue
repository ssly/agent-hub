<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSkillsStore } from '@/stores/skills'

const { t } = useI18n()
const store = useSkillsStore()
</script>

<template>
  <div class="p-6 view-enter">
    <div class="ah-view-content">
      <div v-if="!store.diffResult" class="py-20 text-center" style="color: var(--ink-3)">
        No diff data.
      </div>
      <template v-else>
        <h2 class="ah-page-title mb-4">{{ t('diff.title') }}: {{ store.diffResult.skill_name }}</h2>
        <div class="text-sm mb-4">
          <span style="color: var(--accent)">{{ t('diff.source_label') }}:</span> {{ store.diffResult.source_platform }}
          <span class="mx-2" style="color: var(--ink-4)">→</span>
          <span style="color: var(--accent)">{{ t('diff.target_label') }}:</span> {{ store.diffResult.target_platform }}
        </div>
        <div v-for="fd in store.diffResult.file_diffs" :key="fd.file_path" class="mb-6">
          <div class="font-bold mb-1" style="color: var(--accent)">
            {{ fd.file_path }}
            <span style="color: var(--ink-3)">+{{ fd.stats.added }} -{{ fd.stats.removed }}</span>
          </div>
          <pre class="ah-file-viewer">{{ fd.lines?.map((l: any) => l.Context || l.Added || l.Removed || '').join('\n') }}</pre>
        </div>
        <p v-if="store.diffResult.file_diffs.length === 0" style="color: var(--ink-3)">{{ t('diff.no_diff') }}</p>
      </template>
    </div>
  </div>
</template>
