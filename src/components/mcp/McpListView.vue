<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMcpStore } from '@/stores/mcp'
import { useAppStore } from '@/stores/app'
import { usePluginsStore } from '@/stores/plugins'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import { ref, computed, watch } from 'vue'
import { highlightText } from '@/lib/utils'
import AppModal from '@/components/ui/AppModal.vue'
import AppLoading from '@/components/ui/AppLoading.vue'

const props = withDefaults(defineProps<{ embedded?: boolean; readonly?: boolean }>(), {
  embedded: false,
  readonly: false,
})

const { t } = useI18n()
const store = useMcpStore()
const appStore = useAppStore()
const pluginsStore = usePluginsStore()
const { showToast } = useToast()

const displayedServers = computed(() => {
  const q = pluginsStore.searchQuery.toLowerCase().trim()
  if (!q) return store.servers
  return store.servers.filter((server: any) => {
    const nameMatch = (server.name || '').toLowerCase().includes(q)
    const summaryMatch = (server.summary || '').toLowerCase().includes(q)
    return nameMatch || summaryMatch
  })
})

const newServerName = ref('')
const newServerConfig = ref('')

// Edit modal state
const editModalOpen = ref(false)
const editServerName = ref('')
const editText = ref('')
const deleteConfirmName = ref<string | null>(null)

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

const isReadOnlyPlatform = computed(() => props.readonly)

// Default config template based on platform format
function defaultConfigTemplate(format: string, platformId?: string): string {
  if (platformId === 'opencode') {
    return '{\n  "type": "local",\n  "command": [\n    "npx",\n    "-y",\n    "@modelcontextprotocol/server-everything"\n  ],\n  "environment": {}\n}'
  }
  if (format === 'toml') return 'command = ""\nargs = []\n'
  return '{\n  "command": "",\n  "args": [],\n  "env": {}\n}'
}

// Update default when platform changes
watch(() => store.selectedPlatformId, () => {
  newServerConfig.value = defaultConfigTemplate(selectedFormat.value, store.selectedPlatformId || undefined)
}, { immediate: true })

// --- Add flow: direct create without preview ---
async function handleCreateServer() {
  const name = newServerName.value.trim()
  const config = newServerConfig.value.trim()
  if (!name) return
  try {
    await store.createServer(name, config)
    store.addModalOpen = false
    newServerName.value = ''
    newServerConfig.value = defaultConfigTemplate(selectedFormat.value, store.selectedPlatformId || undefined)
    showToast(t('mcp.saved'), 'success')
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

// --- Delete flow: confirm modal → delete ---
function handleDeleteClick(name: string) {
  deleteConfirmName.value = name
}

async function handleConfirmDelete() {
  if (!deleteConfirmName.value) return
  const name = deleteConfirmName.value
  deleteConfirmName.value = null
  try {
    await store.deleteServer(name)
    appStore.refreshTrashCount()
    showToast(t('mcp.deleted'), 'success')
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
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
    await store.createServer(name, saveText)
    editModalOpen.value = false
    // Refresh the edited server's detail so the read-only view stays in sync.
    try {
      const newDetail = await api.getMcpServer(store.selectedPlatformId, name)
      store.serverDetails[name] = { config_text: newDetail.config_text, format: newDetail.format }
    } catch { /* ignore refresh error */ }
    showToast(t('mcp.saved'), 'success')
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
        <!-- Add button (hidden for read-only platforms) -->
        <div v-if="!props.embedded && !isReadOnlyPlatform" class="flex justify-end mb-4">
          <button class="btn btn-primary btn-sm" @click="store.addModalOpen = true">+ {{ t('mcp.add') }}</button>
        </div>
        <div v-if="store.servers.length === 0" class="flex flex-col items-center justify-center py-12 text-center">
          <p style="color: var(--ink-3)">{{ t('mcp.no_servers') }}</p>
        </div>
        <div v-else-if="displayedServers.length === 0" class="flex flex-col items-center justify-center py-12 text-center">
          <p style="color: var(--ink-3)">{{ t('mcp.no_matching_servers') }}</p>
        </div>

        <!-- Server List: click a row to open its config in a modal -->
        <div v-else class="ah-server-list">
          <div
            v-for="server in displayedServers"
            :key="server.name"
            class="ah-server-row"
            @click="openServerDetail(server.name)"
          >
            <div class="flex-1 min-w-0">
              <div class="ah-server-row__name" v-html="highlightText(server.name, pluginsStore.searchQuery)"></div>
              <div class="ah-server-row__summary" v-html="highlightText(server.summary || '', pluginsStore.searchQuery)"></div>
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
          <AppLoading v-if="!store.serverDetails[detailServerName]" class="py-12" :size="40">
            {{ t('switch.content_loading') }}
          </AppLoading>
          <div v-else class="ah-config-view-wrap ah-config-view-wrap--fill">
            <span class="ah-config-view__badge">
              {{ store.serverDetails[detailServerName]?.format === 'toml' ? 'TOML' : 'JSON' }}
            </span>
            <pre class="ah-config-view">{{ displayConfig(detailServerName) }}</pre>
          </div>
          <template #footer>
            <div class="flex items-center gap-2 w-full">
              <template v-if="!isReadOnlyPlatform">
                <button class="btn btn-danger" @click="handleDetailDelete">{{ t('mcp.delete') }}</button>
              </template>
              <div class="flex-1" />
              <button class="btn btn-secondary" @click="closeServerDetail">{{ t('action.close') }}</button>
              <button
                v-if="!isReadOnlyPlatform && store.serverDetails[detailServerName]"
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

        <!-- Delete Confirm Modal -->
        <AppModal
          :show="deleteConfirmName !== null"
          :title="t('mcp.confirm_delete')"
          @close="deleteConfirmName = null"
          width-class="w-[26rem]"
        >
          <p class="text-sm" style="color: var(--ink-2)">
            {{ t('mcp.delete_confirm', { name: deleteConfirmName }) }}
          </p>
          <template #footer>
            <button class="btn btn-secondary" @click="deleteConfirmName = null">{{ t('action.cancel') }}</button>
            <button class="btn btn-danger" @click="handleConfirmDelete">{{ t('mcp.delete') }}</button>
          </template>
        </AppModal>
      </template>
    </div>
  </div>
</template>
