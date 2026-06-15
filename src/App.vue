<script setup lang="ts">
import { onMounted, watch, ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { useToast } from '@/composables/useToast'
import { formatInt, formatSessionTime } from '@/lib/utils'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import AppToolbar from '@/components/layout/AppToolbar.vue'
import AppToast from '@/components/layout/AppToast.vue'
import SkillListView from '@/components/skills/SkillListView.vue'
import SkillDetailView from '@/components/skills/SkillDetailView.vue'
import McpListView from '@/components/mcp/McpListView.vue'
import SessionListView from '@/components/sessions/SessionListView.vue'
import SwitchView from '@/components/switch/SwitchView.vue'
import SearchResults from '@/components/search/SearchResults.vue'
import DiffView from '@/components/diff/DiffView.vue'
import AppModal from '@/components/ui/AppModal.vue'

// Detect Tauri context for plugin usage (updater works only in desktop build)
const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__

const appStore = useAppStore()
const skillsStore = useSkillsStore()
const { showToast } = useToast()
const { t, locale } = useI18n()

const copyLabel = ref('')
const displayCopyLabel = computed(() => copyLabel.value || t('action.copy'))

const fixPromptText = computed(() => {
  const paths = skillsStore.invalidSkills.map(item => item.path).join('\n')
  return t('scan_invalid.fix_prompt', { paths })
})

function handleCopyFixPrompt() {
  navigator.clipboard.writeText(fixPromptText.value).then(() => {
    copyLabel.value = t('action.copied')
    setTimeout(() => {
      copyLabel.value = ''
    }, 1500)
  })
}

async function handleDoSync() {
  if (!skillsStore.syncTargetPlatformId) return
  try {
    await skillsStore.startSync(skillsStore.syncTargetPlatformId, skillsStore.syncOverwrite)
    showToast(t('sync.done'), 'success')
  } catch (e: any) {
    showToast(t('sync.failed', { error: e?.message || e?.SyncError || String(e) }), 'error')
  }
}

async function handleDeleteTrashForever(id: string) {
  if (window.confirm(t('trash.confirm_delete_forever'))) {
    try {
      await appStore.deleteTrashForever(id)
      showToast(t('trash.deleted') || 'Permanently deleted', 'success')
    } catch (e: any) {
      showToast(String(e), 'error')
    }
  }
}

async function handleEmptyTrash() {
  if (window.confirm(t('trash.confirm_empty'))) {
    try {
      await appStore.emptyTrash()
      showToast(t('trash.deleted') || 'Trash emptied', 'success')
    } catch (e: any) {
      showToast(String(e), 'error')
    }
  }
}

// ----- About / Version modal + updater -----
const aboutUpdateStatus = ref<'idle' | 'checking' | 'available' | 'uptodate' | 'installing' | 'error'>('idle')
const aboutUpdateInfo = ref<any>(null)
const aboutProgress = ref(0)
const aboutError = ref('')

function resetAboutState() {
  aboutUpdateStatus.value = 'idle'
  aboutUpdateInfo.value = null
  aboutProgress.value = 0
  aboutError.value = ''
}

async function openExternal(url: string) {
  try {
    if (isTauri) {
      const { open } = await import('@tauri-apps/plugin-shell')
      await open(url)
      return
    }
  } catch (e) {
    console.warn('[about] shell open failed, using fallback', e)
  }
  window.open(url, '_blank', 'noopener,noreferrer')
}

async function handleCheckUpdates() {
  if (!isTauri) {
    showToast('Updater is only available in the desktop app.', 'error')
    return
  }
  aboutUpdateStatus.value = 'checking'
  aboutError.value = ''
  aboutUpdateInfo.value = null
  aboutProgress.value = 0

  try {
    const { check } = await import('@tauri-apps/plugin-updater')
    const update = await check()
    if (update) {
      aboutUpdateInfo.value = update
      aboutUpdateStatus.value = 'available'
      appStore.availableUpdate = update
    } else {
      aboutUpdateStatus.value = 'uptodate'
    }
  } catch (e: any) {
    aboutError.value = e?.message || String(e)
    aboutUpdateStatus.value = 'error'
    showToast(t('about.check_failed') + (aboutError.value ? `: ${aboutError.value}` : ''), 'error')
  }
}

async function checkForUpdatesSilently() {
  if (!isTauri) return
  try {
    const { check } = await import('@tauri-apps/plugin-updater')
    const update = await check()
    if (update) {
      appStore.availableUpdate = update
      aboutUpdateInfo.value = update
      aboutUpdateStatus.value = 'available'
      // Non-intrusive notification. User can click the version in the sidebar.
      showToast(t('about.new_version_toast', { version: update.version }), 'info')
    }
  } catch (e) {
    // Silent fail is intentional (dev mode, offline, rate limit, etc.)
    // Do not show error toasts for background check.
  }
}

async function handleInstallUpdate() {
  if (!aboutUpdateInfo.value || !isTauri) return

  aboutUpdateStatus.value = 'installing'
  aboutProgress.value = 0
  aboutError.value = ''

  try {
    let downloaded = 0
    let contentLength = 0

    await aboutUpdateInfo.value.downloadAndInstall((event: any) => {
      if (event.event === 'Started') {
        contentLength = event.data?.contentLength || 0
      } else if (event.event === 'Progress') {
        downloaded += event.data?.chunkLength || 0
        if (contentLength > 0) {
          aboutProgress.value = Math.min(100, Math.round((downloaded / contentLength) * 100))
        }
      } else if (event.event === 'Finished') {
        aboutProgress.value = 100
      }
    })

    showToast(t('about.update_complete'), 'success')

    const { relaunch } = await import('@tauri-apps/plugin-process')
    await relaunch()
  } catch (e: any) {
    aboutError.value = e?.message || String(e)
    aboutUpdateStatus.value = 'error'
    showToast((t('about.install_failed') || 'Install failed') + `: ${aboutError.value}`, 'error')
  }
}

function handleOpenGithub() {
  openExternal('https://github.com/ssly/agent-hub')
}

function handleOpenHomepage() {
  openExternal('https://liuxyz.com')
}

// Reset transient update state whenever the About modal is (re)opened
watch(() => appStore.aboutModalOpen, (isOpen) => {
  if (isOpen) {
    resetAboutState()
    // If we have a pending update from background check, pre-fill the modal
    if (appStore.availableUpdate) {
      aboutUpdateInfo.value = appStore.availableUpdate
      aboutUpdateStatus.value = 'available'
    }
  }
})

watch(() => appStore.locale, (newVal) => {
  locale.value = newVal
}, { immediate: true })

onMounted(async () => {
  await appStore.init()

  // Perform a silent background update check shortly after launch.
  // This provides the "update notification" the user expects.
  // Only runs in real Tauri desktop build.
  setTimeout(() => {
    checkForUpdatesSilently()
  }, 2500)
})
</script>

<template>
  <div class="flex h-full">
    <AppSidebar />
    <main class="flex-1 flex flex-col overflow-hidden" style="background: var(--canvas)">
      <AppToolbar />
      <div class="flex-1 overflow-y-auto">
        <template v-if="appStore.currentTab === 'skills'">
          <SkillListView v-if="appStore.currentView === 'skills'" />
          <SkillDetailView v-else-if="appStore.currentView === 'detail'" />
          <DiffView v-else-if="appStore.currentView === 'diff'" />
          <SearchResults v-else-if="appStore.currentView === 'search'" />
        </template>
        <McpListView v-else-if="appStore.currentTab === 'mcp'" />
        <SessionListView v-else-if="appStore.currentTab === 'sessions'" />
        <SwitchView v-else-if="appStore.currentTab === 'switch'" />
      </div>
    </main>
    <AppToast />

    <!-- Scan Invalid Skills Modal -->
    <AppModal
      :show="skillsStore.scanModalOpen"
      :title="t('scan_invalid.title', { count: skillsStore.invalidSkills.length })"
      @close="skillsStore.scanModalOpen = false"
    >
      <div class="space-y-4">
        <p class="text-xs" style="color: var(--ink-3)">{{ t('scan_invalid.subtitle') }}</p>
        <div class="space-y-1 max-h-[30vh] overflow-y-auto">
          <div
            v-for="item in skillsStore.invalidSkills"
            :key="item.path"
            class="flex items-start gap-2 p-3 rounded"
            style="background: var(--sunken)"
          >
            <span class="text-yellow-500 font-bold">⚠️</span>
            <div class="flex-1 min-w-0">
              <div class="text-sm truncate" style="color: var(--ink)" :title="item.path">{{ item.path }}</div>
              <div class="text-xs" style="color: var(--ink-3)">
                {{ item.platform_name }} · <span class="text-red-500">{{ item.reason }}</span>
              </div>
            </div>
          </div>
        </div>
        <div class="border-t pt-4" style="border-color: var(--hairline)">
          <div class="flex items-center justify-between mb-2">
            <span class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('scan_invalid.fix_prompt_label') }}</span>
            <button
              class="text-xs cursor-pointer flex items-center gap-1"
              style="color: var(--accent)"
              @click="handleCopyFixPrompt"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
              {{ displayCopyLabel }}
            </button>
          </div>
          <textarea
            readonly
            class="w-full h-24 text-xs rounded p-2 resize-none font-mono"
            style="background: var(--sunken); border: 1px solid var(--border); color: var(--ink)"
            :value="fixPromptText"
          />
          <p class="text-[11px] mt-1" style="color: var(--ink-3)">{{ t('scan_invalid.copy_hint') }}</p>
        </div>
      </div>
      <template #footer>
        <button class="btn btn-secondary" @click="skillsStore.scanModalOpen = false">{{ t('action.close') }}</button>
      </template>
    </AppModal>

    <!-- Diff Platform Selection Modal -->
    <AppModal
      :show="skillsStore.diffPlatformModalOpen"
      :title="t('diff.select_platform')"
      @close="skillsStore.diffPlatformModalOpen = false"
      width-class="w-[30rem]"
    >
      <div class="space-y-1.5">
        <button
          v-for="c in skillsStore.diffCandidates"
          :key="c.id"
          class="w-full text-left px-3 py-2 rounded cursor-pointer transition-colors border"
          style="background: var(--surface); color: var(--ink); border-color: var(--border);"
          @click="skillsStore.startDiff(c.id); appStore.setView('diff')"
        >
          {{ c.display_name }}
        </button>
      </div>
      <template #footer>
        <button class="btn btn-secondary" @click="skillsStore.diffPlatformModalOpen = false">{{ t('action.cancel') }}</button>
      </template>
    </AppModal>

    <!-- Sync Platform Selection Modal -->
    <AppModal
      :show="skillsStore.syncPlatformModalOpen"
      :title="t('sync.title')"
      @close="skillsStore.syncPlatformModalOpen = false"
      width-class="w-[32rem]"
    >
      <div class="space-y-4">
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('sync.select_target') }}</label>
          <select
            v-model="skillsStore.syncTargetPlatformId"
            class="ah-select w-full"
            style="height: 36px;"
          >
            <option v-for="target in skillsStore.syncTargets" :key="target.id" :value="target.id">
              {{ target.display_name }}
            </option>
          </select>
        </div>

        <div class="flex items-center gap-2">
          <input
            id="sync-overwrite-checkbox"
            type="checkbox"
            v-model="skillsStore.syncOverwrite"
            class="cursor-pointer"
          />
          <label for="sync-overwrite-checkbox" class="text-sm cursor-pointer select-none" style="color: var(--ink)">
            {{ t('action.overwrite') }}
          </label>
        </div>
      </div>
      <template #footer>
        <button class="btn btn-secondary" @click="skillsStore.syncPlatformModalOpen = false">{{ t('action.cancel') }}</button>
        <button
          class="btn btn-primary"
          :disabled="!skillsStore.syncTargetPlatformId"
          @click="handleDoSync"
        >
          {{ t('action.confirm') }}
        </button>
      </template>
    </AppModal>

    <!-- Trash/Recycle Bin Modal -->
    <AppModal
      :show="appStore.trashModalOpen"
      :title="t('trash.title')"
      @close="appStore.trashModalOpen = false"
      width-class="w-[36rem]"
    >
      <div v-if="appStore.trashLoading" class="loading-pulse text-center py-12" style="color: var(--ink-3)">
        Loading...
      </div>
      <div v-else-if="appStore.trashItems.length === 0" class="text-center py-12" style="color: var(--ink-3)">
        {{ t('trash.empty') }}
      </div>
      <div v-else class="space-y-1 max-h-[50vh] overflow-y-auto">
        <div
          v-for="item in appStore.trashItems"
          :key="item.id"
          class="flex items-center justify-between p-3 border-b"
          style="border-color: var(--hairline)"
        >
          <div>
            <div class="text-sm font-medium" style="color: var(--ink)">{{ item.name }}</div>
            <div class="text-xs" style="color: var(--ink-3)">
              {{ item.platform_id }} · {{ item.item_type === 'mcp' ? t('trash.type_mcp') : t('trash.type_skill') }}
            </div>
          </div>
          <div class="flex gap-2">
            <button class="btn btn-secondary btn-sm" @click="appStore.restoreTrash(item.id)">{{ t('trash.restore') }}</button>
            <button class="btn btn-danger btn-sm" @click="handleDeleteTrashForever(item.id)">{{ t('trash.delete_forever') }}</button>
          </div>
        </div>
      </div>
      <template #footer>
        <button
          v-if="appStore.trashItems.length > 0"
          class="btn btn-danger"
          @click="handleEmptyTrash"
        >
          {{ t('trash.empty_trash') }}
        </button>
        <button class="btn btn-secondary" @click="appStore.trashModalOpen = false">{{ t('action.close') }}</button>
      </template>
    </AppModal>

    <!-- About / Version + Check Updates Modal -->
    <AppModal
      :show="appStore.aboutModalOpen"
      :title="t('about.title')"
      @close="appStore.aboutModalOpen = false"
      width-class="w-[30rem]"
    >
      <div class="space-y-4 text-sm">
        <!-- Version -->
        <div>
          <div class="text-[11px] font-medium tracking-wide" style="color: var(--ink-3)">{{ t('about.version_label') }}</div>
          <div class="font-mono text-lg" style="color: var(--ink)">v{{ appStore.appVersion }}</div>
        </div>

        <!-- Author -->
        <div>
          <div class="text-[11px] font-medium tracking-wide" style="color: var(--ink-3)">{{ t('about.author_label') }}</div>
          <button
            class="text-left px-1 py-0.5 rounded hover:bg-[var(--sunken)] transition-colors w-fit"
            style="color: var(--accent)"
            @click="handleOpenHomepage"
          >
            yeqiyeluo
          </button>
        </div>

        <!-- Links -->
        <div>
          <div class="text-[11px] font-medium tracking-wide mb-1" style="color: var(--ink-3)">{{ t('about.links_label') }}</div>
          <div class="flex flex-col gap-0.5">
            <button
              class="text-left px-1 py-0.5 rounded hover:bg-[var(--sunken)] transition-colors w-fit"
              style="color: var(--accent)"
              @click="handleOpenGithub"
            >
              {{ t('about.github') }}
            </button>
          </div>
        </div>

        <!-- Check for Updates -->
        <div class="pt-3 border-t space-y-2" style="border-color: var(--hairline)">
          <button
            class="btn btn-primary w-full"
            :disabled="aboutUpdateStatus === 'checking' || aboutUpdateStatus === 'installing'"
            @click="handleCheckUpdates"
          >
            <span v-if="aboutUpdateStatus === 'checking'">{{ t('about.checking') }}</span>
            <span v-else>{{ t('about.check_updates') }}</span>
          </button>

          <!-- Status -->
          <div v-if="aboutUpdateStatus === 'uptodate'" class="text-center text-xs py-1" style="color: #16a34a">
            {{ t('about.up_to_date') }}
          </div>

          <div v-if="aboutUpdateStatus === 'available' && aboutUpdateInfo" class="space-y-2">
            <div class="text-xs" style="color: var(--ink-3)">
              {{ t('about.update_available') }}
              <span class="font-mono ml-1" style="color: var(--ink)">v{{ aboutUpdateInfo.version }}</span>
            </div>

            <div
              v-if="aboutUpdateInfo.body"
              class="text-[11px] p-2 rounded max-h-[120px] overflow-auto font-mono whitespace-pre-wrap"
              style="background: var(--sunken); color: var(--ink-2); border: 1px solid var(--border)"
            >
              {{ aboutUpdateInfo.body }}
            </div>

            <button
              class="btn btn-primary w-full"
              @click="handleInstallUpdate"
            >
              {{ t('about.install_restart') }}
            </button>
          </div>

          <div v-if="aboutUpdateStatus === 'installing'" class="space-y-1">
            <div class="text-xs" style="color: var(--ink-3)">{{ t('about.installing') }}</div>
            <div class="h-1.5 rounded bg-[var(--sunken)] overflow-hidden">
              <div
                class="h-1.5 rounded bg-[var(--accent)] transition-all duration-150"
                :style="{ width: aboutProgress + '%' }"
              ></div>
            </div>
            <div class="text-right text-[10px] font-mono tabular-nums" style="color: var(--ink-3)">
              {{ aboutProgress }}%
            </div>
          </div>

          <div v-if="aboutUpdateStatus === 'error' && aboutError" class="text-xs text-red-500 break-all">
            {{ t('about.install_failed') || t('about.check_failed') }}: {{ aboutError }}
          </div>
        </div>
      </div>

      <template #footer>
        <button class="btn btn-secondary" @click="appStore.aboutModalOpen = false">{{ t('action.close') }}</button>
      </template>
    </AppModal>
  </div>
</template>
