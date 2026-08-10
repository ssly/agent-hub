<script setup lang="ts">
import { onMounted, onBeforeUnmount, watch, computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke, Channel } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useAppStore } from '@/stores/app'
import { useSkillsStore } from '@/stores/skills'
import { useToast } from '@/composables/useToast'
import { useHoverResetId, useHoverResetBool } from '@/composables/useHoverReset'
import { formatInt, formatSessionTime } from '@/lib/utils'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import AppToolbar from '@/components/layout/AppToolbar.vue'
import AppToast from '@/components/layout/AppToast.vue'
import SkillDetailView from '@/components/skills/SkillDetailView.vue'
import PluginView from '@/components/plugins/PluginView.vue'
import SessionListView from '@/components/sessions/SessionListView.vue'
import SessionMonitorView from '@/components/monitor/SessionMonitorView.vue'
import SwitchView from '@/components/switch/SwitchView.vue'
import SearchResults from '@/components/search/SearchResults.vue'
import DiffView from '@/components/diff/DiffView.vue'
import AppModal from '@/components/ui/AppModal.vue'
import AppLoading from '@/components/ui/AppLoading.vue'
import aboutHeroUrl from '@/assets/about-hero.png'

// Detect Tauri context for plugin usage (updater works only in desktop build)
const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__

const appStore = useAppStore()
const skillsStore = useSkillsStore()
const { showToast } = useToast()
const { t, locale } = useI18n()

// Keep the Monitor view alive after the first visit so re-entry is a cheap
// v-show toggle (no remount / no re-fetch critical path). First visit still
// paints a loading shell before any IPC.
const monitorVisited = ref(appStore.currentTab === 'monitor')
watch(
  () => appStore.currentTab,
  tab => {
    if (tab === 'monitor') monitorVisited.value = true
  },
)

async function handleDoSync() {
  if (!skillsStore.syncTargetPlatformId) return
  try {
    await skillsStore.startSync(skillsStore.syncTargetPlatformId, skillsStore.syncOverwrite)
    showToast(t('sync.done'), 'success')
  } catch (e: any) {
    showToast(t('sync.failed', { error: e?.message || e?.SyncError || String(e) }), 'error')
  }
}

// Trash deletion uses a two-step confirm: first click arms the danger chip,
// a second click deletes. The chip disarms the instant the pointer leaves the
// button (no timer), so users cancel by simply moving away. Tauri's webview
// has no native confirm() dialog, so we can't rely on window.confirm.
const { armedId: armedDeleteId, arm: armDelete, reset: resetDelete } = useHoverResetId()
const { armed: armedEmpty, arm: armEmpty, reset: resetEmpty } = useHoverResetBool()

function onTrashModalClose() {
  resetDelete()
  resetEmpty()
  appStore.trashModalOpen = false
}

async function handleRestoreTrash(id: string) {
  resetDelete()
  resetEmpty()
  try {
    await appStore.restoreTrash(id)
    showToast(t('trash.restored'), 'success')
  } catch (e: any) {
    showToast(t('trash.restore_failed', { error: e?.SyncError || e?.message || e }), 'error')
  }
}

async function handleDeleteTrashForever(id: string) {
  if (armedDeleteId.value !== id) {
    armDelete(id)
    return
  }
  resetDelete()
  try {
    await appStore.deleteTrashForever(id)
    showToast(t('trash.deleted'), 'success')
  } catch (e: any) {
    showToast(t('trash.delete_failed', { error: e?.SyncError || e?.message || e }), 'error')
  }
}

async function handleEmptyTrash() {
  if (!armedEmpty.value) {
    armEmpty()
    return
  }
  resetEmpty()
  try {
    await appStore.emptyTrash()
    showToast(t('trash.emptied'), 'success')
  } catch (e: any) {
    showToast(t('trash.empty_failed', { error: e?.SyncError || e?.message || e }), 'error')
  }
}

// ----- About / Version modal + updater -----
// Updater state lives in the store so the sidebar can surface download
// progress after the About modal is closed mid-download.
const aboutUpdateStatus = computed(() => appStore.updateStatus)
const aboutUpdateInfo = computed(() => appStore.updateInfo)
const aboutProgress = computed(() => appStore.updateProgress)
const aboutDownloaded = computed(() => appStore.updateDownloaded)
const aboutTotal = computed(() => appStore.updateTotal)
const aboutError = computed(() => appStore.updateError)
const aboutDownloadSource = computed(() => appStore.updateDownloadSource)
const UPDATE_TIMEOUT_MINUTES = 5

const aboutProgressText = computed(() => {
  const downloaded = formatBytes(aboutDownloaded.value)
  if (aboutTotal.value > 0) {
    return t('about.progress_detail', {
      percent: aboutProgress.value,
      downloaded,
      total: formatBytes(aboutTotal.value),
    })
  }
  return t('about.progress_unknown', { downloaded })
})

function resetAboutState() {
  appStore.resetUpdateState()
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / (1024 ** index)
  return `${value >= 100 || index === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`
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
  appStore.updateStatus = 'checking'
  appStore.updateError = ''
  appStore.updateInfo = null
  appStore.updateProgress = 0

  try {
    const update = await invoke<any>('plugin:updater|check')
    if (update) {
      const meta = { version: update.version, body: update.body, date: update.date }
      appStore.updateInfo = meta
      appStore.updateStatus = 'available'
      appStore.availableUpdate = meta
    } else {
      appStore.updateStatus = 'uptodate'
    }
  } catch (e: any) {
    appStore.updateError = e?.message || String(e)
    appStore.updateStatus = 'error'
    showToast(t('about.check_failed') + (appStore.updateError ? `: ${appStore.updateError}` : ''), 'error')
  }
}

async function checkForUpdatesSilently() {
  if (!isTauri) return
  try {
    const update = await invoke<any>('plugin:updater|check')
    if (update) {
      const meta = { version: update.version, body: update.body, date: update.date }
      appStore.availableUpdate = meta
      appStore.updateInfo = meta
      appStore.updateStatus = 'available'
      // Non-intrusive notification. User can click the version in the sidebar.
      showToast(t('about.new_version_toast', { version: update.version }), 'info')
    }
  } catch (e) {
    // Silent fail is intentional (dev mode, offline, rate limit, etc.)
    // Do not show error toasts for background check.
  }
}

async function handleInstallUpdate(useMirror = false, clearCache = false) {
  if (!aboutUpdateInfo.value || !isTauri) return

  appStore.updateStatus = 'installing'
  appStore.updateDownloadSource = useMirror ? 'mirror' : 'direct'
  appStore.updateProgress = 0
  appStore.updateDownloaded = 0
  appStore.updateTotal = 0
  appStore.updateError = ''

  try {
    // Re-check to get a fresh rid (the stored pendingUpdateRaw may be stale,
    // and the rid is a resource table handle that doesn't persist reliably).
    const update = await invoke<any>('plugin:updater|check')
    if (!update) {
      throw new Error('Update no longer available')
    }
    const rid = update.rid
    if (!rid) {
      throw new Error('Missing update resource id')
    }

    const channel = new Channel<any>()
    // Progress must be monotonic: mirrors sometimes flip Content-Length between
    // remaining-bytes and full size, which would otherwise make the bar jump
    // backwards (e.g. 80% → 70%).
    const applyDownloadProgress = (downloadedRaw: number, totalRaw?: number) => {
      const downloaded = Math.max(0, Number(downloadedRaw) || 0)
      const totalHint = Math.max(0, Number(totalRaw) || 0)
      if (totalHint > 0) {
        // Prefer a larger total once known; never shrink (avoids percent drops).
        if (appStore.updateTotal <= 0 || totalHint >= appStore.updateTotal) {
          appStore.updateTotal = totalHint
        }
      }
      if (downloaded > appStore.updateDownloaded) {
        appStore.updateDownloaded = downloaded
      }
      const total = appStore.updateTotal
      if (total > 0) {
        const next = Math.min(100, Math.round((appStore.updateDownloaded / total) * 100))
        if (next > appStore.updateProgress) {
          appStore.updateProgress = next
        }
      }
    }
    channel.onmessage = (event: any) => {
      const eventName = String(event?.event || '').toLowerCase()
      if (eventName === 'started') {
        // Prefer full-file total; contentLength alone may be remaining bytes on 206.
        const total = event.data?.total || 0
        const resumed = event.data?.resumedFrom || 0
        applyDownloadProgress(resumed, total)
      } else if (eventName === 'progress') {
        applyDownloadProgress(event.data?.downloaded || 0, event.data?.total || 0)
      } else if (eventName === 'finished') {
        const total = event.data?.total || appStore.updateTotal
        const downloaded = event.data?.downloaded || total
        if (total > 0) appStore.updateTotal = total
        appStore.updateDownloaded = Math.max(appStore.updateDownloaded, downloaded || 0)
        appStore.updateProgress = 100
      }
    }

    await invoke('download_and_install_update_resumable', {
      rid,
      onEvent: channel,
      useMirror,
      clearCache,
    })

    showToast(t('about.update_complete'), 'success')

    const { relaunch } = await import('@tauri-apps/plugin-process')
    await relaunch()
  } catch (e: any) {
    const rawError = e?.message || e?.SyncError || String(e)
    // 如果是用户主动取消（切换到镜像），不显示错误
    if (rawError.includes('__cancelled__')) {
      return
    }
    appStore.updateError = /timed?\s*out|timeout/i.test(rawError)
      ? t('about.timeout_error', { minutes: UPDATE_TIMEOUT_MINUTES })
      : rawError
    appStore.updateStatus = 'error'
    showToast((t('about.install_failed') || 'Install failed') + `: ${appStore.updateError}`, 'error')
  }
}

async function handleSwitchUpdateSource() {
  if (!aboutUpdateInfo.value || !isTauri) return
  const useMirror = aboutDownloadSource.value !== 'mirror'

  try {
    await invoke('cancel_update_download')
  } catch {
  }

  await new Promise(resolve => setTimeout(resolve, 300))

  await handleInstallUpdate(useMirror, true)
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
    // Don't clobber an in-flight download: closing & reopening the About modal
    // mid-download must preserve progress.
    if (appStore.updateStatus !== 'installing') {
      resetAboutState()
      // If we have a pending update from background check, pre-fill the modal
      if (appStore.availableUpdate) {
        appStore.updateInfo = appStore.availableUpdate
        appStore.updateStatus = 'available'
      }
    }
  }
})

watch(() => appStore.locale, (newVal) => {
  locale.value = newVal
}, { immediate: true })

// Unlisten handles for tray → main-window events.
let trayUnlisten: UnlistenFn | null = null

onMounted(async () => {
  await appStore.init()

  // Tray right-click "Check for Updates" opens About and runs the same flow
  // as the sidebar About modal (no separate updater path).
  if (isTauri) {
    try {
      trayUnlisten = await listen('tray-check-updates', async () => {
        appStore.aboutModalOpen = true
        if (appStore.updateStatus !== 'installing' && appStore.updateStatus !== 'checking') {
          await handleCheckUpdates()
        }
      })
    } catch (e) {
      console.warn('[tray] failed to listen for tray-check-updates', e)
    }
  }

  // Perform a silent background update check shortly after launch.
  // This provides the "update notification" the user expects.
  // Only runs in real Tauri desktop build.
  setTimeout(() => {
    checkForUpdatesSilently()
  }, 2500)
})

onBeforeUnmount(() => {
  trayUnlisten?.()
  trayUnlisten = null
})
</script>

<template>
  <div class="flex h-full">
    <AppSidebar />
    <main class="flex-1 flex flex-col overflow-hidden">
      <AppToolbar />
      <div class="flex-1 overflow-y-auto">
        <template v-if="appStore.currentTab === 'plugins'">
          <PluginView v-if="appStore.currentView === 'plugins'" />
          <SkillDetailView v-else-if="appStore.currentView === 'detail'" />
          <DiffView v-else-if="appStore.currentView === 'diff'" />
          <SearchResults v-else-if="appStore.currentView === 'search'" />
        </template>
        <SessionListView v-else-if="appStore.currentTab === 'sessions'" />
        <SwitchView v-else-if="appStore.currentTab === 'accounts'" />
        <!-- Monitor stays mounted after first visit (v-show) so re-entry is free;
             first visit still paints a loader shell before any IPC. -->
        <SessionMonitorView v-if="monitorVisited" v-show="appStore.currentTab === 'monitor'" />
      </div>
    </main>
    <AppToast />

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
      @close="onTrashModalClose"
      width-class="w-[36rem]"
    >
      <AppLoading v-if="appStore.trashLoading" class="py-12" />
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
          <div class="flex gap-2 items-center">
            <button class="btn btn-secondary btn-sm" :disabled="armedDeleteId === item.id" @click="handleRestoreTrash(item.id)">{{ t('trash.restore') }}</button>
            <button
              class="btn btn-sm trash-confirm-btn"
              :class="armedDeleteId === item.id ? 'trash-confirm-btn--armed' : 'btn-danger'"
              @click="handleDeleteTrashForever(item.id)"
              @mouseleave="resetDelete()"
            >
              {{ armedDeleteId === item.id ? t('trash.confirm_delete_hint') : t('trash.delete_forever') }}
            </button>
          </div>
        </div>
      </div>
      <template #footer>
        <button
          v-if="appStore.trashItems.length > 0"
          class="btn trash-confirm-btn"
          :class="armedEmpty ? 'trash-confirm-btn--armed' : 'btn-danger'"
          @click="handleEmptyTrash"
          @mouseleave="resetEmpty()"
        >
          {{ armedEmpty ? t('trash.confirm_empty_hint') : t('trash.empty_trash') }}
        </button>
        <button class="btn btn-secondary" @click="onTrashModalClose">{{ t('action.close') }}</button>
      </template>
    </AppModal>

    <!-- About / Version + Check Updates Modal -->
    <AppModal
      :show="appStore.aboutModalOpen"
      :title="t('about.title')"
      bare
      :close-on-outside="!aboutUpdateStatus || aboutUpdateStatus !== 'installing'"
      @close="appStore.aboutModalOpen = false"
      width-class="w-[26rem] about-modal-shell"
    >
      <div class="about-modal">
        <!-- Compact header: smaller brand mark + identity. -->
        <div class="about-head">
          <div class="about-head__text">
            <div class="about-head__title-row">
              <h2 class="about-name">Agent Hub</h2>
              <span class="about-version font-mono">v{{ appStore.appVersion }}</span>
            </div>
            <p class="about-tagline">{{ t('about.tagline') }}</p>
          </div>
          <div class="about-head__brand">
            <img :src="aboutHeroUrl" alt="" class="about-head__logo" draggable="false" />
          </div>
        </div>

        <dl class="about-meta">
          <div class="about-meta__row">
            <dt class="about-meta__label">{{ t('about.author_label') }}</dt>
            <dd class="about-meta__value">
              <button
                class="about-link"
                :title="t('about.homepage')"
                @click="handleOpenHomepage"
              >
                yeqiyeluo
                <svg class="about-link__ext" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
              </button>
            </dd>
          </div>
          <div class="about-meta__row">
            <dt class="about-meta__label">{{ t('about.links_label') }}</dt>
            <dd class="about-meta__value">
              <button class="about-link" @click="handleOpenGithub">
                GitHub
                <svg class="about-link__ext" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
              </button>
            </dd>
          </div>
        </dl>

        <div class="about-update">
          <button
            class="btn btn-primary w-full"
            :disabled="aboutUpdateStatus === 'checking' || aboutUpdateStatus === 'installing'"
            @click="handleCheckUpdates"
          >
            <span v-if="aboutUpdateStatus === 'checking'">{{ t('about.checking') }}</span>
            <span v-else>{{ t('about.check_updates') }}</span>
          </button>

          <div v-if="aboutUpdateStatus === 'uptodate'" class="about-update__status about-update__status--ok">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
            {{ t('about.up_to_date') }}
          </div>

          <div v-if="aboutUpdateStatus === 'available' && aboutUpdateInfo" class="about-update__available">
            <div class="about-update__headline">
              {{ t('about.update_available') }}
              <span class="about-update__version font-mono">v{{ aboutUpdateInfo.version }}</span>
            </div>

            <!-- Only the notes pane scrolls — avoids a second modal scrollbar. -->
            <div
              v-if="aboutUpdateInfo.body"
              class="about-update__body"
            >
              {{ aboutUpdateInfo.body }}
            </div>

            <button
              class="btn btn-primary w-full"
              @click="handleInstallUpdate(false)"
            >
              {{ t('about.install_restart') }}
            </button>

            <button
              class="btn btn-secondary w-full"
              @click="handleInstallUpdate(true)"
            >
              {{ t('about.use_mirror') }}
            </button>
          </div>

          <div v-if="aboutUpdateStatus === 'installing'" class="about-update__progress">
            <div class="about-update__headline">
              {{ t('about.installing', { minutes: UPDATE_TIMEOUT_MINUTES }) }}
            </div>
            <div class="about-progressbar">
              <div
                :class="[
                  'about-progressbar__fill',
                  aboutTotal > 0 ? 'about-progressbar__fill--known' : 'update-progress-indeterminate',
                ]"
                :style="aboutTotal > 0 ? { width: aboutProgress + '%' } : undefined"
              ></div>
            </div>
            <div class="about-update__meta font-mono tabular-nums">
              {{ aboutProgressText }}
            </div>
            <button
              class="btn btn-secondary w-full"
              @click="handleSwitchUpdateSource"
            >
              {{ t(aboutDownloadSource === 'mirror' ? 'about.switch_to_direct' : 'about.switch_to_mirror') }}
            </button>
          </div>

          <div v-if="aboutUpdateStatus === 'error' && aboutError" class="about-update__error">
            <div class="about-update__errtext">
              {{ t(aboutUpdateInfo ? 'about.install_failed' : 'about.check_failed') }}: {{ aboutError }}
            </div>
            <div v-if="aboutUpdateInfo" class="grid grid-cols-2 gap-2">
              <button class="btn btn-secondary" @click="handleInstallUpdate(false)">
                {{ t('about.retry_github') }}
              </button>
              <button class="btn btn-secondary" @click="handleInstallUpdate(true)">
                {{ t('about.retry_mirror') }}
              </button>
            </div>
            <div v-if="aboutUpdateInfo" class="about-update__meta about-update__meta--left">
              {{ t('about.mirror_notice') }}
            </div>
          </div>
        </div>
      </div>
      <!-- No footer close button: backdrop click (and Esc) closes the modal.
           close-on-outside stays disabled while an install is in flight. -->
    </AppModal>
  </div>
</template>

<style scoped>
.update-progress-indeterminate {
  width: 35%;
  animation: update-progress-slide 1.2s ease-in-out infinite;
}

@keyframes update-progress-slide {
  0% { transform: translateX(-110%); }
  100% { transform: translateX(320%); }
}

/* ----- About modal (compact; only release notes scroll) ----- */
.about-modal {
  display: flex;
  flex-direction: column;
  /* Cap height so the modal shell never needs a second scrollbar. */
  max-height: min(72vh, 34rem);
  overflow: hidden;
}

.about-head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 18px 6px;
}
.about-head__text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.about-head__title-row {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 8px;
}
.about-head__brand {
  flex: 0 0 auto;
  width: 72px;
  height: 72px;
  border-radius: var(--radius-sm);
  overflow: hidden;
  background: var(--sunken);
  box-shadow: var(--shadow-soft);
  pointer-events: none;
}
.about-head__logo {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.about-name {
  margin: 0;
  font-family: var(--font-serif);
  font-size: 17px;
  font-weight: 600;
  letter-spacing: -0.01em;
  color: var(--ink);
  line-height: 1.2;
}
.about-version {
  font-size: 11px;
  font-weight: 500;
  color: var(--ink-3);
}
.about-tagline {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.45;
  color: var(--ink-3);
}

.about-meta {
  margin: 0;
  padding: 4px 18px 2px;
  display: flex;
  flex-direction: column;
}
.about-meta__row {
  display: grid;
  grid-template-columns: 52px 1fr;
  align-items: center;
  column-gap: 10px;
  padding: 3px 0;
}
.about-meta__label {
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 0.02em;
  color: var(--ink-4);
}
.about-meta__value {
  margin: 0;
}
.about-link {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12.5px;
  font-weight: 500;
  color: var(--accent);
  background: transparent;
  border: none;
  padding: 1px 0;
  cursor: pointer;
  transition: color var(--dur-fast) var(--ease-soft);
}
.about-link:hover { color: var(--accent-strong); }
.about-link__ext { opacity: 0.55; }

.about-update {
  margin-top: 6px;
  padding: 10px 18px 14px;
  border-top: 1px solid var(--hairline);
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}
.about-update__status {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  font-size: 12px;
  padding: 1px 0;
}
.about-update__status--ok { color: var(--success); }

.about-update__available {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}
.about-update__headline {
  font-size: 12px;
  color: var(--ink-3);
  line-height: 1.35;
}
.about-update__version {
  margin-left: 4px;
  font-size: 12px;
  font-weight: 600;
  color: var(--ink);
}
.about-update__body {
  font-size: 11px;
  line-height: 1.5;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  /* Single scroll region for release notes. */
  max-height: 7.5rem;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  font-family: var(--font-mono, ui-monospace, monospace);
  white-space: pre-wrap;
  word-break: break-word;
  background: var(--sunken);
  color: var(--ink-2);
  border: 1px solid var(--border);
}

.about-update__progress,
.about-update__error {
  display: flex;
  flex-direction: column;
  gap: 7px;
}
.about-progressbar {
  height: 5px;
  border-radius: var(--radius-pill);
  background: var(--sunken);
  overflow: hidden;
}
.about-progressbar__fill {
  height: 100%;
  border-radius: var(--radius-pill);
  background: var(--accent);
}
.about-progressbar__fill--known {
  /* Only expand width — no shrink animation when percent is held/monotonic. */
  transition: width .2s linear;
}
.about-update__meta {
  font-size: 10px;
  text-align: right;
  color: var(--ink-4);
}
.about-update__meta--left { text-align: left; line-height: 1.4; }
.about-update__errtext {
  font-size: 12px;
  color: var(--danger);
  word-break: break-word;
  line-height: 1.4;
}

/* Two-step delete confirm: a quiet danger button turns into a solid danger
   chip on first click; moving the pointer away disarms it instantly. Tauri's
   webview has no native confirm() dialog, so we can't rely on window.confirm. */
.trash-confirm-btn--armed {
  background: var(--danger);
  color: var(--on-accent);
  border-color: var(--danger);
  white-space: nowrap;
}
</style>
