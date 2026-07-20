<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMcpStore } from '@/stores/mcp'
import { useAppStore } from '@/stores/app'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import { ref, computed, watch } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'

const props = withDefaults(defineProps<{ embedded?: boolean; readonly?: boolean }>(), {
  embedded: false,
  readonly: false,
})

const { t } = useI18n()
const store = useMcpStore()
const appStore = useAppStore()
const { showToast } = useToast()

const newServerName = ref('')
const newServerConfig = ref('')

// Edit modal state
const editModalOpen = ref(false)
const editServerName = ref('')
const editText = ref('')
// Tracks whether the current preview originated from the edit modal,
// so cancelling the preview re-opens the edit modal.
const previewFromEdit = ref(false)

// Detail modal: clicking a server row opens its config in a modal
// (same interaction pattern as the Accounts tab).
const detailModalOpen = ref(false)
const detailServerName = ref('')

async function openServerDetail(name: string) {
  detailServerName.value = name
  detailModalOpen.value = true
  if (!store.serverDetails[name] && store.selectedPlatformId) {
    try {
      const detail = await api.getMcpServer(store.selectedPlatformId, name, store.workspaceDirectory)
      store.serverDetails[name] = { config_text: detail.config_text, format: detail.format }
    } catch (e: any) {
      showToast(String(e?.message || e), 'error')
    }
  }
}

function closeServerDetail() {
  detailModalOpen.value = false
}

function handleDetailEdit() {
  const name = detailServerName.value
  closeServerDetail()
  handleEditClick(name)
}

function handleDetailSync() {
  const name = detailServerName.value
  closeServerDetail()
  handleSyncClick(name)
}

function handleDetailDelete() {
  const name = detailServerName.value
  closeServerDetail()
  handleDeleteClick(name)
}

// Detect selected platform format
const selectedFormat = computed(() => {
  const p = store.platforms.find(p => p.id === store.selectedPlatformId)
  return p?.format || 'json'
})

// Default config template based on platform format
function defaultConfigTemplate(format: string): string {
  if (format === 'toml') return 'command = ""\nargs = []\n'
  return '{\n  "command": "",\n  "args": [],\n  "env": {}\n}'
}

// Update default when platform changes
watch(() => store.selectedPlatformId, () => {
  newServerConfig.value = defaultConfigTemplate(selectedFormat.value)
}, { immediate: true })

// --- Add flow: add modal → preview ---
async function handleCreateServer() {
  const name = newServerName.value.trim()
  const config = newServerConfig.value.trim()
  if (!name) return
  try {
    await store.loadAddPreview(name, config)
    store.addModalOpen = false
  } catch (e: any) {
    const msg = String(e?.message || e)
    if (msg.includes('TOML')) {
      showToast(t('mcp.only_toml'), 'error')
    } else if (msg.includes('JSON')) {
      showToast(t('mcp.only_json'), 'error')
    } else {
      showToast(msg, 'error')
    }
  }
}

// --- Delete flow: delete button → preview ---
async function handleDeleteClick(name: string) {
  store.deleteConfirmServerName = null
  try {
    await store.loadDeletePreview(name)
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

// --- Preview confirm / cancel ---
async function handlePreviewConfirm() {
  try {
    const wasEdit = previewFromEdit.value
    const wasAdd = store.previewMode === 'add' && !previewFromEdit.value
    const wasDelete = store.previewMode === 'delete'
    const changedName = store.previewData?.server_name
    await store.confirmPreview()
    // Refresh the edited server's detail so the read-only view stays in sync.
    if (wasEdit && changedName && store.selectedPlatformId) {
      try {
        const newDetail = await api.getMcpServer(store.selectedPlatformId, changedName)
        store.serverDetails[changedName] = { config_text: newDetail.config_text, format: newDetail.format }
      } catch { /* ignore refresh error */ }
    }
    showToast(wasEdit || wasAdd ? t('mcp.saved') : t('mcp.deleted'), 'success')
    // Deleting an MCP server moves its config into the trash on the backend;
    // refresh the sidebar badge so the count reflects it immediately.
    if (wasDelete) {
      appStore.refreshTrashCount()
    }
    previewFromEdit.value = false
    // Reset add form if it was a fresh add
    if (wasAdd) {
      newServerName.value = ''
      newServerConfig.value = defaultConfigTemplate(selectedFormat.value)
    }
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

function handlePreviewCancel() {
  const fromEdit = previewFromEdit.value
  const wasAdd = store.previewMode === 'add' && !fromEdit
  store.cancelPreview()
  previewFromEdit.value = false
  if (fromEdit) {
    // Re-open edit modal with preserved inputs
    editModalOpen.value = true
  } else if (wasAdd) {
    // Re-open add modal with preserved inputs
    store.addModalOpen = true
  }
}

async function handleSyncClick(serverName: string) {
  try {
    await store.loadSyncTargets(serverName)
    if (store.syncTargets.length === 0) {
      showToast(t('diff.no_other'), 'warning')
    } else {
      store.syncModalOpen = true
    }
  } catch (e: any) {
    showToast(String(e), 'error')
  }
}

async function handleDoSync() {
  if (!store.syncTargetPlatformId) return
  try {
    await store.performSync(store.syncTargetPlatformId)
    showToast(t('mcp.sync_done'), 'success')
  } catch (e: any) {
    showToast(t('mcp.sync_failed', { error: e?.message || e }), 'error')
  }
}

// The backend now returns the config WITH the proper section header for TOML
// (e.g. `[mcp_servers.node_repl]` + `[mcp_servers.node_repl.env]`), so no
// client-side wrapping is needed.
function displayConfig(name: string): string {
  const detail = store.serverDetails[name]
  return detail?.config_text ?? ''
}

// --- Edit modal flow ---
function handleEditClick(name: string) {
  const detail = store.serverDetails[name]
  if (!detail) return
  editServerName.value = name
  editText.value = detail.config_text
  editModalOpen.value = true
}

async function handleEditSave() {
  const name = editServerName.value
  const text = editText.value.trim()
  if (!name || !text || !store.selectedPlatformId) return
  const detail = store.serverDetails[name]
  try {
    let saveText = text
    if (detail?.format !== 'toml') {
      // JSON: strip the `{ "<name>": ... }` wrapper if present
      const parsed = JSON.parse(text)
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed) && Object.keys(parsed).length === 1 && parsed[name]) {
        saveText = JSON.stringify(parsed[name], null, 2)
      } else {
        saveText = JSON.stringify(parsed, null, 2)
      }
    } else {
      // TOML: strip the outer `[mcp_servers.<name>]` wrapper lines so the
      // importer receives just the server's inner config.
      saveText = stripTomlHeader(text, name)
    }
    // Route through preview + confirmation before applying, same as add/delete.
    editModalOpen.value = false
    previewFromEdit.value = true
    await store.loadAddPreview(name, saveText)
  } catch (e: any) {
    const msg = String(e?.message || e)
    if (msg.includes('TOML')) {
      showToast(t('mcp.only_toml'), 'error')
    } else if (msg.includes('JSON')) {
      showToast(t('mcp.only_json'), 'error')
    } else {
      showToast(msg, 'error')
    }
  }
}

// Remove the leading `[mcp_servers.<name>]` line (and any blank line after it)
// from a TOML config snippet. The inner subtables like `[mcp_servers.<name>.env]`
// are preserved — the importer's `parse_server_config_input_with_format` knows
// how to unwrap nested keys via the mcp_key.
function stripTomlHeader(text: string, name: string): string {
  const header = `[mcp_servers.${name}]`
  const lines = text.split('\n')
  let i = 0
  // Skip the leading server header line + any immediately following blank lines.
  if (lines[i]?.trim() === header) {
    i++
    while (i < lines.length && lines[i].trim() === '') {
      i++
    }
  }
  return lines.slice(i).join('\n').trim()
}
</script>

<template>
  <div :class="[props.embedded ? 'ah-embedded-view' : 'p-6 view-enter']">
    <div class="ah-view-content">
      <div v-if="!store.selectedPlatformId" class="flex flex-col items-center justify-center py-20 text-center">
        <p style="color: var(--ink-3)">{{ t('mcp.title') }}</p>
      </div>

      <template v-else>
        <!-- Add button -->
        <div v-if="!props.embedded && !props.readonly" class="flex justify-end mb-4">
          <button class="btn btn-primary btn-sm" @click="store.addModalOpen = true">+ {{ t('mcp.add') }}</button>
        </div>

        <div v-if="store.servers.length === 0" class="flex flex-col items-center justify-center py-12 text-center">
          <p style="color: var(--ink-3)">{{ t('mcp.no_servers') }}</p>
        </div>

        <!-- Server List: click a row to open its config in a modal -->
        <div class="space-y-1">
          <div
            v-for="server in store.servers"
            :key="server.name"
            class="ah-server-row"
            @click="openServerDetail(server.name)"
          >
            <div class="flex-1 min-w-0">
              <div class="ah-server-row__name">{{ server.name }}</div>
              <div class="ah-server-row__summary">{{ server.summary }}</div>
            </div>
            <span class="ah-server-row__chevron">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg>
            </span>
          </div>
        </div>

        <!-- Server Detail Modal -->
        <AppModal
          :show="detailModalOpen"
          :title="detailServerName"
          width-class="w-[44rem]"
          @close="closeServerDetail"
        >
          <div v-if="!store.serverDetails[detailServerName]" class="loading-pulse flex items-center justify-center py-12" style="color: var(--ink-3)">
            {{ t('switch.content_loading') }}
          </div>
          <div v-else class="ah-config-view-wrap ah-config-view-wrap--fill">
            <span class="ah-config-view__badge">
              {{ store.serverDetails[detailServerName]?.format === 'toml' ? 'TOML' : 'JSON' }}
            </span>
            <pre class="ah-config-view">{{ displayConfig(detailServerName) }}</pre>
          </div>
          <template #footer>
            <div class="flex items-center gap-2 w-full">
              <template v-if="!props.readonly">
                <button class="btn btn-secondary" @click="handleDetailSync">{{ t('mcp.sync') }}</button>
                <button class="btn btn-danger" @click="handleDetailDelete">{{ t('mcp.delete') }}</button>
              </template>
              <div class="flex-1" />
              <button class="btn btn-secondary" @click="closeServerDetail">{{ t('action.close') }}</button>
              <button
                v-if="!props.readonly && store.serverDetails[detailServerName]"
                class="btn btn-primary"
                @click="handleDetailEdit"
              >
                {{ t('mcp.edit') }}
              </button>
            </div>
          </template>
        </AppModal>

        <!-- Add Server Modal -->
        <AppModal
          :show="store.addModalOpen"
          :title="t('mcp.add')"
          @close="store.addModalOpen = false"
          width-class="w-[36rem]"
        >
          <div class="flex flex-col gap-4">
            <div class="flex flex-col gap-1.5">
              <label class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('mcp.server_name') }}</label>
              <input
                v-model="newServerName"
                type="text"
                class="ah-search-input"
                placeholder="e.g. filesystem"
              />
            </div>
            <div class="flex flex-col gap-1.5">
              <label class="text-xs font-semibold" style="color: var(--ink-2)">
                {{ t('mcp.config') }}
                <span class="font-normal ml-1" style="color: var(--ink-4)">
                  {{ selectedFormat === 'toml' ? 'TOML' : 'JSON' }}
                </span>
              </label>
              <textarea
                v-model="newServerConfig"
                v-auto-resize
                class="ah-config-editor ah-config-editor--auto"
                :placeholder="selectedFormat === 'toml' ? 'command = &quot;npx&quot;' : '{}'"
              />
            </div>
          </div>
          <template #footer>
            <button class="btn btn-secondary" @click="store.addModalOpen = false">{{ t('action.cancel') }}</button>
            <button class="btn btn-primary" :disabled="!newServerName.trim()" @click="handleCreateServer">{{ t('action.confirm') }}</button>
          </template>
        </AppModal>

        <!-- Sync Server Modal -->
        <AppModal
          :show="store.syncModalOpen"
          :title="t('mcp.sync_title')"
          @close="store.syncModalOpen = false"
          width-class="w-[30rem]"
        >
          <div class="space-y-4">
            <div class="flex flex-col gap-1.5">
              <label class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('mcp.select_target') }}</label>
              <select
                v-model="store.syncTargetPlatformId"
                class="ah-select w-full"
                style="height: 36px;"
              >
                <option v-for="target in store.syncTargets" :key="target.id" :value="target.id">
                  {{ target.display_name }}
                </option>
              </select>
            </div>
          </div>
          <template #footer>
            <button class="btn btn-secondary" @click="store.syncModalOpen = false">{{ t('action.cancel') }}</button>
            <button class="btn btn-primary" :disabled="!store.syncTargetPlatformId" @click="handleDoSync">{{ t('action.confirm') }}</button>
          </template>
        </AppModal>

        <!-- Edit Server Modal -->
        <AppModal
          :show="editModalOpen"
          :title="t('mcp.edit') + ' · ' + editServerName"
          @close="editModalOpen = false"
          width-class="w-[36rem]"
        >
          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-semibold" style="color: var(--ink-2)">
              {{ t('mcp.config') }}
              <span class="font-normal ml-1" style="color: var(--ink-4)">
                {{ store.serverDetails[editServerName]?.format === 'toml' ? 'TOML' : 'JSON' }}
              </span>
            </label>
            <textarea
              v-model="editText"
              v-auto-resize
              class="ah-config-editor ah-config-editor--auto"
            />
          </div>
          <template #footer>
            <button class="btn btn-secondary" @click="editModalOpen = false">{{ t('action.cancel') }}</button>
            <button class="btn btn-primary" @click="handleEditSave">{{ t('mcp.save') }}</button>
          </template>
        </AppModal>

        <!-- Preview Diff Modal (Add / Delete) -->
        <AppModal
          :show="store.previewModalOpen"
          :title="store.previewMode === 'add' ? t('mcp.preview_title_add') : t('mcp.preview_title_delete')"
          @close="handlePreviewCancel"
          width-class="w-[40rem]"
        >
          <!-- Loading -->
          <div v-if="store.previewLoading" class="text-sm py-4" style="color: var(--ink-3)">
            {{ t('mcp.preview_loading') }}
          </div>

          <!-- Error -->
          <div v-else-if="store.previewData?.error" class="text-sm py-3" style="color: var(--danger)">
            {{ t('mcp.preview_error', { error: store.previewData.error }) }}
          </div>

          <!-- Preview content -->
          <template v-else-if="store.previewData">
            <!-- Conflict warning -->
            <div v-if="store.previewData.has_conflict" class="text-xs mb-3 px-3 py-2 rounded" style="background: color-mix(in srgb, var(--warning) 12%, transparent); color: var(--warning);">
              {{ t('mcp.preview_conflict') }}
            </div>

            <!-- Config path -->
            <div class="text-xs mb-2" style="color: var(--ink-4)">
              {{ store.previewData.target_config_path }}
              <span class="ml-2">{{ store.previewData.target_format?.toUpperCase() }}</span>
            </div>

            <!-- Stats -->
            <div class="text-xs mb-3 font-medium" style="color: var(--accent)">
              {{ t('mcp.preview_stats', { added: store.previewData.added, removed: store.previewData.removed }) }}
            </div>

            <!-- Diff lines -->
            <pre class="ah-config-editor" style="max-height: 360px; overflow-y: auto; font-size: 12px; line-height: 1.6;"><template v-for="(line, idx) in store.previewData.diff_lines" :key="idx"><span :style="{
                color: line.tag === 'added' ? 'var(--success)' : line.tag === 'removed' ? 'var(--danger)' : 'var(--ink-3)',
                display: 'block',
                background: line.tag === 'added' ? 'color-mix(in srgb, var(--success) 8%, transparent)' : line.tag === 'removed' ? 'color-mix(in srgb, var(--danger) 8%, transparent)' : 'transparent',
              }">{{ line.tag === 'added' ? '+ ' : line.tag === 'removed' ? '- ' : '  ' }}{{ line.content }}</span></template></pre>
          </template>

          <template #footer>
            <button class="btn btn-secondary" @click="handlePreviewCancel">{{ t('action.cancel') }}</button>
            <button
              class="btn"
              :class="store.previewMode === 'delete' ? 'btn-danger' : 'btn-primary'"
              :disabled="!store.previewData || !!store.previewData.error"
              @click="handlePreviewConfirm"
            >
              {{ store.previewMode === 'delete' ? t('mcp.delete') : t('action.confirm') }}
            </button>
          </template>
        </AppModal>
      </template>
    </div>
  </div>
</template>
