<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSkillsStore } from '@/stores/skills'
import { useToast } from '@/composables/useToast'
import { formatBytes, avatarToneFromName } from '@/lib/utils'
import * as api from '@/lib/api'
import { FolderOpen } from 'lucide-vue-next'

const { t } = useI18n()
const store = useSkillsStore()
const { showToast } = useToast()
const detail = ref<any>(null)
const activeFile = ref<string | null>(null)
const fileContent = ref('')
const loading = ref(true)
const openingFolder = ref(false)

async function loadDetail() {
  if (!store.selectedPlatformId || !store.selectedSkillName) return
  loading.value = true
  try {
    detail.value = await api.getSkillDetail(
      store.selectedPlatformId,
      store.selectedSkillName,
      store.selectedFolder,
      store.workspaceDirectory,
    )
    const defaultFile = detail.value.files.find((f: string) => /(^|\/)SKILL\.md$/i.test(f)) || detail.value.files[0] || null
    if (defaultFile) await loadFile(defaultFile)
  } catch (e) {
    detail.value = null
  } finally {
    loading.value = false
  }
}

async function loadFile(path: string) {
  activeFile.value = path
  fileContent.value = ''
  if (!store.selectedPlatformId || !store.selectedSkillName) return
  try {
    fileContent.value = await api.readSkillFile(
      store.selectedPlatformId,
      store.selectedSkillName,
      store.selectedFolder,
      path,
      store.workspaceDirectory,
    )
  } catch (e) {
    fileContent.value = `Error: ${e}`
  }
}

// Reveal this skill's directory in the OS file manager.
async function handleOpenFolder() {
  if (!store.selectedPlatformId || !store.selectedSkillName || openingFolder.value) return
  openingFolder.value = true
  try {
    await api.openSkillFolder(
      store.selectedPlatformId,
      store.selectedSkillName,
      store.selectedFolder,
      store.workspaceDirectory,
    )
  } catch (e: any) {
    showToast(t('skill.open_folder_failed'), 'error')
  } finally {
    openingFolder.value = false
  }
}

onMounted(loadDetail)
watch(() => [store.selectedSkillName, store.selectedFolder], loadDetail)
</script>

<template>
  <div class="p-6 view-enter">
    <div class="ah-view-content ah-skill-detail">
      <div v-if="loading" class="loading-pulse" style="color: var(--ink-3)">Loading...</div>

      <template v-else-if="detail">
        <!-- Hero -->
        <header class="ah-hero">
          <div :class="['ah-hero__icon', `ah-hero__icon--${avatarToneFromName(detail.name)}`]">
            {{ (detail.name || '?').charAt(0).toUpperCase() }}
          </div>
          <div>
            <h1 class="ah-hero__title">{{ detail.name }}</h1>
            <p v-if="detail.description" class="ah-hero__subtitle">{{ detail.description }}</p>
          </div>
          <button
            class="btn btn-secondary flex items-center gap-1.5 flex-shrink-0"
            :disabled="openingFolder"
            :title="t('skill.open_folder')"
            @click="handleOpenFolder"
          >
            <FolderOpen :size="15" />
            {{ t('skill.open_folder') }}
          </button>
        </header>

        <!-- Skill location path + open button -->
        <div class="ah-skill-path">
          <span class="ah-skill-path__label">{{ t('skill.location') }}</span>
          <code class="ah-skill-path__value">{{ detail.path }}</code>
        </div>

        <!-- Metadata Row -->
        <div class="ah-meta-row">
          <article class="ah-meta">
            <p class="ah-meta__label">{{ t('skill.platform') }}</p>
            <p class="ah-meta__value">{{ detail.platform_id }}</p>
          </article>
          <article class="ah-meta">
            <p class="ah-meta__label">{{ t('skill.version') }}</p>
            <p class="ah-meta__value ah-meta__value--mono">{{ detail.version || '—' }}</p>
          </article>
          <article class="ah-meta">
            <p class="ah-meta__label">{{ t('skill.size') }}</p>
            <p class="ah-meta__value ah-meta__value--mono">{{ formatBytes(detail.total_size) }}</p>
          </article>
          <article class="ah-meta">
            <p class="ah-meta__label">{{ t('skill.files') }}</p>
            <p class="ah-meta__value">{{ detail.files.length }}</p>
          </article>
          <article class="ah-meta">
            <p class="ah-meta__label">{{ detail.is_symlink ? 'Symlink' : 'Type' }}</p>
            <p class="ah-meta__value">{{ detail.is_symlink ? '→ ...' : 'Directory' }}</p>
          </article>
        </div>

        <!-- Files -->
        <section v-if="detail.files.length > 0">
          <h2 class="ah-section-title">{{ t('skill.files') }}</h2>
          <div class="ah-file-workspace">
            <nav class="ah-files-list" :aria-label="t('skill.files')">
              <button
                v-for="file in detail.files"
                :key="file"
                :class="['ah-file', activeFile === file ? 'is-active' : '']"
                :title="file"
                @click="loadFile(file)"
              >
                <div class="ah-file__icon">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
                </div>
                <span class="ah-file__name">{{ file }}</span>
              </button>
            </nav>

            <div v-if="activeFile" class="ah-file-content">
              <div class="ah-file-viewer__header">
                <span class="ah-file-viewer__path">{{ activeFile }}</span>
              </div>
              <pre class="ah-file-viewer">{{ fileContent }}</pre>
            </div>
          </div>
        </section>
      </template>

      <div v-else class="py-20 text-center" style="color: var(--danger)">Failed to load skill detail.</div>
    </div>
  </div>
</template>
