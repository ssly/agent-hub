<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSwitchStore } from '@/stores/switch'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'
import AppModal from '@/components/ui/AppModal.vue'

const { t } = useI18n()
const store = useSwitchStore()
const { showToast } = useToast()

const AGENT_DISPLAY_NAMES: Record<string, string> = {
  codex: 'Codex',
  'claude-code': 'Claude Code',
}
const agentName = computed(
  () => AGENT_DISPLAY_NAMES[store.selectedAgent ?? ''] ?? store.selectedAgent ?? ''
)

const addNote = ref('')
const addContent = ref('')

async function handleSaveCurrent() {
  if (!store.selectedAgent) return
  try {
    await api.saveCurrentAuthProfile(store.selectedAgent, '')
    showToast(t('switch.saved_toast'), 'success')
    await store.loadProfiles()
  } catch (e: any) {
    if (e === 'duplicate_key' || e?.message === 'duplicate_key') showToast(t('switch.duplicate_key_error'), 'error')
    else if (e === 'no_active_auth' || e?.message === 'no_active_auth') showToast(t('switch.no_active_auth_error', { agent: agentName.value }), 'error')
    else showToast(String(e?.message || e), 'error')
  }
}

// Clicking a card toggles the inline switch-confirm state.
// The active card never enters the flow — it just hints it's in use.
function handleCardClick(profile: any) {
  if (profile.is_active) {
    showToast(t('switch.already_active_hint'), 'info')
    return
  }
  // Clicking the already-confirming card keeps it in place (no toggle-off);
  // clicking another card switches the confirm target to it.
  if (store.switchConfirmId === profile.id) return
  store.switchConfirmId = profile.id
}

// Dismiss the inline switch-confirm when clicking anywhere outside the cards.
// Cards stop propagation (@click.stop) so clicking inside a card won't dismiss.
function handleOutsideClick() {
  if (store.switchConfirmId) store.switchConfirmId = null
}

onMounted(() => window.addEventListener('click', handleOutsideClick))
onUnmounted(() => window.removeEventListener('click', handleOutsideClick))

async function doSwitch(id: string) {
  if (!store.selectedAgent) return
  try {
    await api.switchAuthProfile(store.selectedAgent, id)
    store.switchConfirmId = null
    showToast(t('switch.switched_toast', { agent: agentName.value }), 'success')
    await store.loadProfiles()
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

async function handleConfirmAdd() {
  if (!store.selectedAgent) return
  const content = addContent.value.trim()
  const note = addNote.value.trim()
  if (!content) {
    showToast(t('switch.invalid_json_error') || 'Content cannot be empty', 'error')
    return
  }
  try {
    await api.addAuthProfile(store.selectedAgent, content, note)
    addNote.value = ''
    addContent.value = ''
    store.addFormOpen = false
    showToast(t('switch.added_toast'), 'success')
    await store.loadProfiles()
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

async function openEditModal(profile: any) {
  try {
    await store.openEditModal(profile)
  } catch {
    showToast(t('switch.content_load_failed'), 'error')
  }
}

function closeEditModal() {
  store.closeEditModal()
}

async function handleSaveEdit() {
  if (!store.selectedAgent || !store.editingProfileId || store.editSaving) return
  store.editSaving = true
  const id = store.editingProfileId
  const note = store.editNote.trim()
  const content = store.editContent.trim()
  // Save note first; if either step fails, keep the modal open so the user can retry.
  try {
    await api.updateAuthProfileNote(store.selectedAgent, id, note)
  } catch (e: any) {
    store.editSaving = false
    showToast(String(e?.message || e), 'error')
    return
  }
  try {
    await api.updateAuthProfileContent(store.selectedAgent, id, content)
  } catch (e: any) {
    store.editSaving = false
    showToast(String(e?.message || e), 'error')
    return
  }
  store.editSaving = false
  showToast(t('switch.content_saved_toast'), 'success')
  store.closeEditModal()
  await store.loadProfiles()
}

function armDelete() {
  store.deleteArmed = true
}

async function confirmDelete() {
  if (!store.selectedAgent || !store.editingProfileId) return
  try {
    await api.deleteAuthProfile(store.selectedAgent, store.editingProfileId)
    showToast(t('switch.deleted_toast'), 'success')
    store.closeEditModal()
    await store.loadProfiles()
  } catch (e: any) {
    // keep armed so the user can retry or click away to cancel
    showToast(String(e?.message || e), 'error')
  }
}
</script>

<template>
  <div class="p-6 view-enter">
    <div class="ah-view-content">
      <div v-if="!store.selectedAgent" class="flex flex-col items-center justify-center py-20">
        <p style="color: var(--ink-3)">{{ t('switch.select_agent') }}</p>
      </div>

      <template v-else>
        <div class="max-w-2xl mx-auto">
          <!-- Toolbar -->
          <div class="flex gap-2 mb-4 flex-wrap items-center">
            <button class="btn btn-primary" @click="handleSaveCurrent">{{ t('switch.save_current') }}</button>
            <button class="btn btn-secondary" @click="store.addFormOpen = !store.addFormOpen">{{ t('switch.add_account') }}</button>
            <div class="flex-1" />
            <span v-if="store.currentKey" class="text-xs truncate max-w-[200px]" style="color: var(--ink-3); font-family: var(--font-mono)">
              {{ t('switch.current_key') }}: {{ store.currentKey }}
            </span>
          </div>

          <!-- Add Form Card -->
          <div
            v-if="store.addFormOpen"
            class="ah-card mb-4 space-y-3"
            style="background: var(--surface); border-color: var(--border)"
          >
            <h3 class="text-sm font-semibold" style="color: var(--ink)">{{ t('switch.add_account') }}</h3>
            <div class="flex flex-col gap-1">
              <label class="text-xs" style="color: var(--ink-3)">{{ t('switch.note_placeholder') }}</label>
              <input
                v-model="addNote"
                type="text"
                class="ah-search-input"
                :placeholder="t('switch.note_placeholder')"
              />
            </div>
            <div class="flex flex-col gap-1">
              <label class="text-xs" style="color: var(--ink-3)">{{ t('mcp.config') }} (auth.json key / contents)</label>
              <textarea
                v-model="addContent"
                class="ah-config-editor"
                placeholder="Paste key content or JSON..."
                style="min-height: 120px"
              />
            </div>
            <div class="flex justify-end gap-2">
              <button class="btn btn-secondary btn-sm" @click="store.addFormOpen = false">{{ t('action.cancel') }}</button>
              <button class="btn btn-primary btn-sm" @click="handleConfirmAdd">{{ t('action.confirm') }}</button>
            </div>
          </div>

          <!-- Profiles -->
          <div v-if="store.profiles.length === 0" class="text-center py-12 text-sm" style="color: var(--ink-4)">
            {{ t('switch.empty') }}
          </div>

          <div class="space-y-2">
            <div
              v-for="(profile, idx) in store.profiles"
              :key="profile.id"
              :class="['ah-card', 'switch-card', profile.is_active ? 'switch-card--active' : '']"
              @click.stop="handleCardClick(profile)"
            >
              <div class="flex items-start justify-between gap-2">
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2 mb-0.5">
                    <span class="text-sm font-medium" style="color: var(--ink)">
                      {{ profile.note || t('switch.account_fallback', { n: idx + 1 }) }}
                    </span>
                    <span v-if="profile.is_active" class="switch-active-badge">{{ t('switch.active_badge') }}</span>
                  </div>
                  <div class="text-xs" style="color: var(--ink-3)">
                    {{ profile.saved_at ? profile.saved_at.substring(0, 19).replace('T', ' ') + ' UTC' : '' }}
                    {{ profile.key ? ` · ${profile.key}` : '' }}
                  </div>
                </div>

                <div class="flex-shrink-0">
                  <button class="btn btn-secondary btn-sm" @click.stop="openEditModal(profile)">
                    {{ t('switch.edit_content_btn') }}
                  </button>
                </div>
              </div>

              <!-- Inline switch confirmation -->
              <div
                v-if="store.switchConfirmId === profile.id && !profile.is_active"
                class="switch-confirm"
                @click.stop
              >
                <span class="text-xs" style="color: var(--accent)">{{ t('switch.confirm_switch', { agent: agentName }) }}</span>
                <button class="btn btn-primary btn-sm" @click.stop="doSwitch(profile.id)">{{ t('action.confirm') }}</button>
                <button class="btn btn-ghost btn-sm" @click.stop="store.switchConfirmId = null">{{ t('action.cancel') }}</button>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- Edit Modal -->
    <AppModal
      :show="store.editModalOpen"
      :title="t('switch.edit_modal_title')"
      width-class="w-[44rem]"
      @close="closeEditModal"
    >
      <!-- body: clicking anywhere here disarms the delete confirmation -->
      <div class="space-y-4" @click="store.deleteArmed = false">
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('switch.note_label') }}</label>
          <input
            v-model="store.editNote"
            type="text"
            class="ah-search-input"
            :placeholder="t('switch.note_placeholder')"
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-semibold" style="color: var(--ink-2)">{{ t('switch.content_label') }}</label>
          <textarea
            v-if="!store.editContentLoading"
            v-model="store.editContent"
            class="ah-config-editor"
            style="min-height: 200px"
          />
          <div
            v-else
            class="ah-config-editor flex items-center justify-center text-xs"
            style="min-height: 200px; color: var(--ink-3)"
          >
            {{ t('switch.content_loading') }}
          </div>
        </div>
      </div>

      <template #footer>
        <div class="flex items-center gap-2 w-full" @click="store.deleteArmed = false">
          <button
            :class="store.deleteArmed ? 'btn btn-sm' : 'btn btn-danger btn-sm'"
            :style="store.deleteArmed ? { background: 'var(--danger)', color: '#fff', borderColor: 'var(--danger)' } : {}"
            @click.stop="store.deleteArmed ? confirmDelete() : armDelete()"
          >
            {{ store.deleteArmed ? t('action.confirm') : t('switch.delete_btn') }}
          </button>
          <div class="flex-1" />
          <button class="btn btn-secondary" @click="closeEditModal">{{ t('action.cancel') }}</button>
          <button
            class="btn btn-primary"
            :disabled="store.editSaving || store.editContentLoading"
            @click="handleSaveEdit"
          >
            {{ t('switch.save_note') }}
          </button>
        </div>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.switch-card {
  cursor: pointer;
  position: relative;
}
.switch-card:hover {
  border-color: var(--border);
  box-shadow: var(--shadow-soft);
}
.switch-card--active {
  background: var(--accent-soft);
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent-mid) inset;
  padding-left: 18px;
}
.switch-card--active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 4px;
  background: var(--accent);
  border-radius: 0 2px 2px 0;
}
.switch-active-badge {
  background: var(--accent);
  color: #fff;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
}
.switch-confirm {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px dashed var(--hairline);
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
</style>
