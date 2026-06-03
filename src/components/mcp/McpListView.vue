<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMcpStore } from '@/stores/mcp'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import { ref } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'

const { t } = useI18n()
const store = useMcpStore()
const { showToast } = useToast()
const editingText = ref<Record<string, string>>({})

const newServerName = ref('')
const newServerConfig = ref('{\n  "command": "",\n  "args": [],\n  "env": {}\n}')

async function handleCreateServer() {
  const name = newServerName.value.trim()
  const config = newServerConfig.value.trim()
  if (!name) return
  try {
    await store.createServer(name, config)
    newServerName.value = ''
    newServerConfig.value = '{\n  "command": "",\n  "args": [],\n  "env": {}\n}'
    store.addModalOpen = false
    showToast(t('mcp.saved'), 'success')
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

async function handleConfirmDelete(name: string) {
  try {
    await store.deleteServer(name)
    store.deleteConfirmServerName = null
    showToast(t('mcp.deleted'), 'success')
  } catch (e: any) {
    showToast(t('mcp.delete_failed', { error: e?.message || e }), 'error')
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

function getWrappedConfig(name: string): string {
  const detail = store.serverDetails[name]
  if (!detail) return ''
  if (detail.format === 'toml') return `[mcp_servers.${name}]\n${detail.config_text}`
  try {
    const obj = JSON.parse(detail.config_text)
    return JSON.stringify({ [name]: obj }, null, 2)
  } catch {
    return detail.config_text
  }
}

async function handleSave(name: string) {
  const text = editingText.value[name]?.trim()
  if (!text || !store.selectedPlatformId) return
  const detail = store.serverDetails[name]
  try {
    let saveText = text
    if (detail?.format !== 'toml') {
      const parsed = JSON.parse(text)
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed) && Object.keys(parsed).length === 1 && parsed[name]) {
        saveText = JSON.stringify(parsed[name], null, 2)
      } else {
        saveText = JSON.stringify(parsed, null, 2)
      }
    }
    await api.importMcpServer(store.selectedPlatformId, name, saveText)
    const newDetail = await api.getMcpServer(store.selectedPlatformId, name)
    store.serverDetails[name] = { config_text: newDetail.config_text, format: newDetail.format }
    showToast(t('mcp.saved'), 'success')
  } catch (e: any) {
    showToast(e?.SyncError || String(e), 'error')
  }
}
</script>

<template>
  <div class="p-6 view-enter">
    <div class="ah-view-content">
      <div v-if="!store.selectedPlatformId" class="flex flex-col items-center justify-center py-20 text-center">
        <p style="color: var(--ink-3)">{{ t('mcp.title') }}</p>
      </div>

      <template v-else>
        <!-- Add button -->
        <div class="flex justify-end mb-4">
          <button class="btn btn-primary btn-sm" @click="store.addModalOpen = true">+ {{ t('mcp.add') }}</button>
        </div>

        <div v-if="store.servers.length === 0" class="flex flex-col items-center justify-center py-12 text-center">
          <p style="color: var(--ink-3)">{{ t('mcp.no_servers') }}</p>
        </div>

        <!-- Server List -->
        <div class="space-y-1">
          <div
            v-for="server in store.servers"
            :key="server.name"
            :class="['ah-accordion', store.expandedServer === server.name ? 'is-expanded' : '']"
          >
            <!-- Server Header -->
            <div class="ah-accordion__header group">
              <button class="flex-1 flex items-center gap-2" @click="store.toggleServer(server.name)">
                <span :class="['ah-accordion__arrow', store.expandedServer === server.name ? 'is-open' : '']">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg>
                </span>
                <div class="flex-1 min-w-0">
                  <div class="ah-accordion__name">{{ server.name }}</div>
                  <div class="ah-accordion__summary">{{ server.summary }}</div>
                </div>
              </button>
              <div class="ah-accordion__actions">
                <button class="btn btn-ghost btn-sm" @click.stop="handleSyncClick(server.name)">{{ t('mcp.sync') }}</button>
                
                <template v-if="store.deleteConfirmServerName !== server.name">
                  <button
                    class="btn btn-ghost btn-sm"
                    style="color: var(--ink-4)"
                    @click.stop="store.deleteConfirmServerName = server.name"
                  >
                    {{ t('mcp.delete') }}
                  </button>
                </template>
                <template v-else>
                  <span class="text-xs mr-1" style="color: var(--danger)">{{ t('mcp.confirm_delete') }}</span>
                  <button
                    class="btn btn-sm text-white mr-1"
                    style="background: var(--danger); border-color: var(--danger)"
                    @click.stop="handleConfirmDelete(server.name)"
                  >
                    {{ t('action.confirm') }}
                  </button>
                  <button
                    class="btn btn-ghost btn-sm text-xs"
                    @click.stop="store.deleteConfirmServerName = null"
                  >
                    {{ t('action.cancel') }}
                  </button>
                </template>
              </div>
            </div>

            <!-- Expanded Content -->
            <div v-if="store.expandedServer === server.name && store.serverDetails[server.name]" class="ah-accordion__content">
              <div class="text-xs mt-2 mb-2" style="color: var(--ink-3)">
                {{ store.serverDetails[server.name]?.format === 'toml' ? 'TOML' : 'JSON' }}
              </div>
              <textarea
                :value="editingText[server.name] ?? getWrappedConfig(server.name)"
                @input="editingText[server.name] = ($event.target as HTMLTextAreaElement).value"
                @blur="handleSave(server.name)"
                class="ah-config-editor"
              />
            </div>
          </div>
        </div>

        <!-- Add Server Modal -->
        <AppModal
          :show="store.addModalOpen"
          :title="t('mcp.add')"
          @close="store.addModalOpen = false"
          width-class="w-[36rem]"
        >
          <div class="space-y-4">
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
              <label class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('mcp.config') }}</label>
              <textarea
                v-model="newServerConfig"
                class="ah-config-editor"
                placeholder="{}"
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
      </template>
    </div>
  </div>
</template>
