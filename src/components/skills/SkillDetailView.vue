<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSkillsStore } from '@/stores/skills'
import { formatBytes } from '@/lib/utils'
import * as api from '@/lib/api'
import { Link2, FileText, ChevronDown, Check, FileCode } from 'lucide-vue-next'
import AppLoading from '@/components/ui/AppLoading.vue'

const { t } = useI18n()
const store = useSkillsStore()
const detail = ref<any>(null)
const activeFile = ref<string | null>(null)
const fileContent = ref('')
const loading = ref(true)

const isDropdownOpen = ref(false)
let leaveTimer: ReturnType<typeof setTimeout> | null = null

function handleMouseEnter() {
  if (leaveTimer) {
    clearTimeout(leaveTimer)
    leaveTimer = null
  }
  isDropdownOpen.value = true
}

function handleMouseLeave() {
  leaveTimer = setTimeout(() => {
    isDropdownOpen.value = false
    leaveTimer = null
  }, 160)
}

function toggleDropdown() {
  isDropdownOpen.value = !isDropdownOpen.value
}

async function handleSelectFile(file: string) {
  isDropdownOpen.value = false
  if (activeFile.value === file) return
  await loadFile(file)
}

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

onMounted(loadDetail)

onBeforeUnmount(() => {
  if (leaveTimer) clearTimeout(leaveTimer)
})

watch(() => [store.selectedSkillName, store.selectedFolder], loadDetail)
</script>

<template>
  <div class="p-6 view-enter">
    <div class="ah-view-content ah-skill-detail">
      <AppLoading v-if="loading" class="py-16" />

      <template v-else-if="detail">
        <!-- Hero -->
        <header class="ah-hero">
          <div class="flex items-center gap-2.5 flex-wrap">
            <h1 class="ah-hero__title mb-0">{{ detail.name }}</h1>
            <span v-if="detail.version" class="ah-version-chip ah-version-chip--hero">v{{ detail.version }}</span>
          </div>
          <p v-if="detail.description" class="ah-hero__subtitle mt-1.5">{{ detail.description }}</p>
        </header>

        <!-- Skill location path + symlink target if present -->
        <div class="ah-skill-path">
          <div class="ah-skill-path__item">
            <span class="ah-skill-path__label">{{ t('skill.location') }}</span>
            <code class="ah-skill-path__value">{{ detail.path }}</code>
            <span v-if="detail.is_symlink" class="ah-symlink-badge">
              <Link2 :size="11" />
              <span>{{ t('skill.symlink') }}</span>
            </span>
          </div>
          <div v-if="detail.is_symlink && detail.symlink_target" class="ah-skill-path__item">
            <span class="ah-skill-path__label">{{ t('skill.symlink_target') }}</span>
            <code class="ah-skill-path__value ah-skill-path__value--symlink" v-tooltip="detail.symlink_target">
              → {{ detail.symlink_target }}
            </code>
          </div>
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
        </div>

        <!-- Files -->
        <section v-if="detail.files.length > 0">
          <h2 class="ah-section-title">{{ t('skill.files') }}</h2>
          <div class="ah-file-workspace">
            <!-- Full-width detail content -->
            <div v-if="activeFile" class="ah-file-content">
              <div class="ah-file-viewer__header">
                <div class="flex items-center gap-2 min-w-0 pr-4">
                  <FileCode :size="14" class="text-accent flex-shrink-0" />
                  <span class="ah-file-viewer__path">{{ activeFile }}</span>
                </div>
              </div>
              <pre class="ah-file-viewer">{{ fileContent }}</pre>
            </div>

            <!-- Right-side current file selector dropdown -->
            <div class="ah-file-dropdown-anchor">
              <div
                class="ah-file-dropdown-wrap"
                @mouseenter="handleMouseEnter"
                @mouseleave="handleMouseLeave"
              >
                <!-- Displays the current file name -->
                <button
                  type="button"
                  class="ah-file-select-btn"
                  :class="{ 'is-active': isDropdownOpen }"
                  :title="activeFile || ''"
                  @click="toggleDropdown"
                >
                  <FileText :size="13" class="ah-file-select-btn__icon" />
                  <span class="ah-file-select-btn__name">{{ activeFile }}</span>
                  <span v-if="detail.files.length > 1" class="ah-file-select-btn__badge">
                    {{ detail.files.length }}
                  </span>
                  <ChevronDown v-if="detail.files.length > 1" :size="12" class="ah-file-select-btn__arrow" />
                </button>

                <!-- Dropdown list directly underneath -->
                <transition name="dropdown-fade">
                  <div
                    v-if="isDropdownOpen && detail.files.length > 1"
                    class="ah-file-dropdown-menu"
                    @click.stop
                  >
                    <button
                      v-for="file in detail.files"
                      :key="file"
                      type="button"
                      :class="['ah-file-dropdown-item', activeFile === file ? 'is-active' : '']"
                      :title="file"
                      @click="handleSelectFile(file)"
                    >
                      <FileText :size="13" class="ah-file-dropdown-item__icon" />
                      <span class="ah-file-dropdown-item__name">{{ file }}</span>
                      <Check v-if="activeFile === file" :size="13" class="ah-file-dropdown-item__check ml-auto" />
                    </button>
                  </div>
                </transition>
              </div>
            </div>
          </div>
        </section>
      </template>

      <div v-else class="py-20 text-center" style="color: var(--danger)">Failed to load skill detail.</div>
    </div>
  </div>
</template>
