<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { formatBytes } from '@/lib/utils'
import { useToast } from '@/composables/useToast'
import { useHoverResetId } from '@/composables/useHoverReset'

withDefaults(defineProps<{ embedded?: boolean; readonly?: boolean }>(), {
  embedded: false,
  readonly: false,
})

const { t } = useI18n()
const appStore = useAppStore()
const store = useSkillsStore()
const { showToast } = useToast()

const totalSkills = computed(() => store.skills.length)
const enabledSkills = computed(() => store.skills.filter((s: any) => s.version || s.description).length)
const totalSize = computed(() => formatBytes(store.skills.reduce((acc: number, s: any) => acc + (s.total_size || 0), 0)))

const groupedSkills = computed(() => {
  const groups = new Map<string, any[]>()
  groups.set('', [])
  for (const s of store.skills) {
    const key = s.folder || ''
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key)!.push(s)
  }
  if (groups.get('')!.length === 0) groups.delete('')

  if (store.skillSortBy === 'size') {
    const dir = store.skillSortDir === 'desc' ? -1 : 1
    for (const arr of groups.values()) {
      arr.sort((a: any, b: any) => dir * ((a.total_size || 0) - (b.total_size || 0)))
    }
  }
  return groups
})

function handleRowClick(name: string, folder: string) {
  store.selectSkill(name, folder)
  appStore.setView('detail')
}

const activeKebabSkill = ref<{ name: string; folder: string } | null>(null)
const { armedId: confirmingDeleteSkill, arm: armConfirmDelete, reset: resetConfirmDelete } = useHoverResetId()

function toggleKebab(skill: any) {
  if (activeKebabSkill.value?.name === skill.name && activeKebabSkill.value?.folder === skill.folder) {
    activeKebabSkill.value = null
  } else {
    activeKebabSkill.value = { name: skill.name, folder: skill.folder || '' }
  }
  resetConfirmDelete()
}

function handleKebabDiff(skill: any) {
  store.selectSkill(skill.name, skill.folder || '')
  activeKebabSkill.value = null
  handleDiffClick()
}

async function handleKebabSync(skill: any) {
  store.selectSkill(skill.name, skill.folder || '')
  activeKebabSkill.value = null
  try {
    await store.loadSyncTargets()
    if (store.syncTargets.length === 0) {
      showToast(t('error.no_target'), 'warning')
      return
    }
    store.syncTargetPlatformId = store.syncTargets[0].id
    store.syncOverwrite = false
    store.syncPlatformModalOpen = true
  } catch (e: any) {
    showToast(String(e), 'error')
  }
}

async function handleDiffClick() {
  try {
    await store.loadDiffCandidates()
    if (store.diffCandidates.length === 0) {
      showToast(t('diff.no_other'), 'warning')
    } else {
      store.diffPlatformModalOpen = true
    }
  } catch (e: any) {
    showToast(String(e), 'error')
  }
}

function handleKebabDelete(skill: any) {
  const key = `${skill.folder || ''}:${skill.name}`
  if (confirmingDeleteSkill.value !== key) {
    armConfirmDelete(key)
    return
  }

  resetConfirmDelete()
  activeKebabSkill.value = null
  store.performDeleteSkill(skill.name, skill.folder || '')
    .then(() => {
      showToast(t('skill.deleted'), 'success')
      appStore.refreshTrashCount()
    })
    .catch((e: any) => {
      showToast(t('skill.delete_failed', { error: e?.message || e?.SyncError || String(e) }), 'error')
    })
}

function closeKebab() {
  activeKebabSkill.value = null
  resetConfirmDelete()
}

onMounted(() => {
  window.addEventListener('click', closeKebab)
})

onUnmounted(() => {
  window.removeEventListener('click', closeKebab)
})
</script>

<template>
  <div :class="[embedded ? 'ah-embedded-view' : 'p-6 view-enter']">
    <div class="ah-view-content">
      <!-- Empty state -->
      <div v-if="store.skills.length === 0" class="flex flex-col items-center justify-center py-20 text-center">
        <p style="color: var(--ink-3)">{{ t('ui.no_skills') }}</p>
      </div>

      <template v-else>
        <!-- Page Header -->
        <div v-if="!embedded" class="ah-page-header">
          <h1 class="ah-page-title">{{ store.selectedPlatform?.display_name }}</h1>
        </div>

        <!-- KPI Row -->
        <div v-if="!embedded" class="ah-kpi-row">
          <article
            v-for="(kpi, i) in [
              { label: t('plugin.skills'), value: totalSkills, unit: t('plugin.skills') },
              { label: t('action.refresh'), value: enabledSkills, unit: t('plugin.skills') },
              { label: t('skill.size'), value: totalSize, unit: '' },
            ]"
            :key="i"
            class="ah-kpi"
          >
            <div class="ah-kpi__body">
              <p class="ah-kpi__label">{{ kpi.label }}</p>
              <p class="ah-kpi__value">
                <span class="ah-kpi__num">{{ kpi.value }}</span>
                <span v-if="kpi.unit" class="ah-kpi__unit">{{ kpi.unit }}</span>
              </p>
            </div>
          </article>
        </div>

        <!-- Table -->
        <div class="ah-table-wrap">
          <!-- Header -->
          <div class="ah-thead">
            <div class="ah-th">{{ t('skill.name') }}</div>
            <div class="ah-th">{{ t('skill.description') }}</div>
            <div
              :class="['ah-th ah-th--size sortable', store.skillSortBy === 'size' ? 'sorted' : '']"
              @click="store.toggleSort"
            >
              {{ t('skill.size') }}
              <svg class="ah-sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                <polyline v-if="store.skillSortDir === 'asc'" points="18 15 12 9 6 15"/>
                <polyline v-else points="6 9 12 15 18 9"/>
              </svg>
            </div>
            <div></div>
          </div>

          <!-- Rows -->
          <template v-for="[folder, items] in groupedSkills" :key="folder">
            <!-- Folder header -->
            <button
              v-if="folder !== ''"
              class="ah-folder-header"
              @click="store.toggleFolder(folder)"
            >
              <span class="ah-folder-arrow">{{ store.collapsedFolders.has(folder) ? '▶' : '▼' }}</span>
              <span class="ah-folder-name">{{ folder }}</span>
              <span class="ah-folder-count">({{ items.length }})</span>
            </button>

            <!-- Skill rows -->
            <template v-if="folder === '' || !store.collapsedFolders.has(folder)">
              <div
                v-for="(skill, idx) in items"
                :key="`${folder}/${skill.name}/${idx}`"
                class="ah-row"
                @click="handleRowClick(skill.name, skill.folder || '')"
              >
                <!-- Name -->
                <div class="ah-row__name">
                  <div class="ah-row__name-text">
                    <span class="ah-row__skill-name">{{ skill.name }}</span>
                    <span v-if="skill.version" class="ah-version-chip">v{{ skill.version }}</span>
                  </div>
                </div>

                <!-- Description -->
                <div class="ah-row__desc">{{ skill.description || '' }}</div>

                <!-- Size -->
                <div class="ah-row__size">{{ formatBytes(skill.total_size || 0) }}</div>

                <!-- Actions -->
                <div class="ah-row__actions relative">
                  <button v-if="!readonly" class="ah-kebab" @click.stop="toggleKebab(skill)">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="5" r="1" fill="currentColor"/><circle cx="12" cy="12" r="1" fill="currentColor"/><circle cx="12" cy="19" r="1" fill="currentColor"/></svg>
                  </button>

                  <!-- Kebab Dropdown Menu -->
                  <div
                    v-if="activeKebabSkill && activeKebabSkill.name === skill.name && activeKebabSkill.folder === (skill.folder || '')"
                    class="ah-kebab-menu ah-kebab-menu--inline"
                  >
                    <button
                      class="ah-kebab-item"
                      @click.stop="handleKebabSync(skill)"
                    >
                      {{ t('action.sync') }}
                    </button>
                    <button
                      class="ah-kebab-item"
                      @click.stop="handleKebabDiff(skill)"
                    >
                      {{ t('action.diff') }}
                    </button>
                    <button
                      class="ah-kebab-item font-medium"
                      :class="confirmingDeleteSkill === `${skill.folder || ''}:${skill.name}` ? 'is-confirming' : 'danger'"
                      @click.stop="handleKebabDelete(skill)"
                      @mouseleave="resetConfirmDelete()"
                    >
                      {{ confirmingDeleteSkill === `${skill.folder || ''}:${skill.name}` ? t('skill.confirm_delete') : t('skill.delete') }}
                    </button>
                  </div>
                </div>
              </div>
            </template>
          </template>
        </div>
      </template>
    </div>
  </div>
</template>
