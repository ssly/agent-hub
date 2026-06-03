<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSwitchStore } from '@/stores/switch'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'

const { t } = useI18n()
const store = useSwitchStore()
const { showToast } = useToast()

const addNote = ref('')
const addContent = ref('')
const editingNoteValue = ref('')

async function handleSaveCurrent() {
  if (!store.selectedAgent) return
  try {
    await api.saveCurrentAuthProfile(store.selectedAgent, '')
    showToast(t('switch.saved_toast'), 'success')
    await store.loadProfiles()
  } catch (e: any) {
    if (e === 'duplicate_key' || e?.message === 'duplicate_key') showToast(t('switch.duplicate_key_error'), 'error')
    else if (e === 'no_active_auth' || e?.message === 'no_active_auth') showToast(t('switch.no_active_auth_error'), 'error')
    else showToast(String(e?.message || e), 'error')
  }
}

async function handleSwitch(id: string) {
  if (!store.selectedAgent) return
  if (store.switchConfirmId !== id) {
    store.switchConfirmId = id
    store.deleteConfirmId = null
    return
  }
  try {
    await api.switchAuthProfile(store.selectedAgent, id)
    store.switchConfirmId = null
    showToast(t('switch.switched_toast'), 'success')
    await store.loadProfiles()
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

async function handleDelete(id: string) {
  if (!store.selectedAgent) return
  if (store.deleteConfirmId !== id) {
    store.deleteConfirmId = id
    store.switchConfirmId = null
    return
  }
  try {
    await api.deleteAuthProfile(store.selectedAgent, id)
    store.deleteConfirmId = null
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

function startEditNote(profile: any) {
  store.editingNoteId = profile.id
  editingNoteValue.value = profile.note || ''
}

async function handleSaveNote(profile: any) {
  if (!store.selectedAgent) return
  const note = editingNoteValue.value.trim()
  try {
    await api.updateAuthProfileNote(store.selectedAgent, profile.id, note)
    store.editingNoteId = null
    await store.loadProfiles()
  } catch (e: any) {
    showToast(String(e?.message || e), 'error')
  }
}

async function startEditContent(profile: any) {
  store.editingContentId = profile.id
  if (!store.contentCache[profile.id]) {
    try {
      store.contentCache[profile.id] = await api.getAuthProfileContent(store.selectedAgent!, profile.id)
    } catch (e: any) {
      store.contentCache[profile.id] = ''
      showToast(String(e?.message || e), 'error')
    }
  }
}

async function handleSaveContent(profile: any) {
  if (!store.selectedAgent) return
  const content = store.contentCache[profile.id]?.trim()
  try {
    await api.updateAuthProfileContent(store.selectedAgent, profile.id, content)
    store.editingContentId = null
    showToast(t('switch.content_saved_toast'), 'success')
    await store.loadProfiles()
  } catch (e: any) {
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
              :class="['ah-card', profile.is_active ? 'is-active' : '']"
            >
              <div class="flex flex-col">
                <div class="flex items-start justify-between gap-2">
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2 mb-0.5">
                      <template v-if="store.editingNoteId === profile.id">
                        <input
                          v-model="editingNoteValue"
                          type="text"
                          class="ah-search-input py-0.5 text-xs"
                          style="max-width: 200px"
                          @blur="handleSaveNote(profile)"
                          @keydown.enter="handleSaveNote(profile)"
                          @keydown.esc="store.editingNoteId = null"
                        />
                        <button class="btn btn-primary btn-sm" style="height: 24px; padding: 0 6px" @click="handleSaveNote(profile)">✓</button>
                      </template>
                      <template v-else>
                        <span class="text-sm font-medium cursor-pointer" style="color: var(--ink)" @dblclick="startEditNote(profile)">
                          {{ profile.note || t('switch.account_fallback', { n: idx + 1 }) }}
                        </span>
                      </template>
                      <span
                        v-if="profile.is_active"
                        class="text-xs px-1.5 py-0.5 rounded"
                        style="background: var(--accent-soft); color: var(--accent); border: 1px solid var(--accent-mid)"
                      >{{ t('switch.active_badge') }}</span>
                    </div>
                    <div class="text-xs" style="color: var(--ink-3)">
                      {{ profile.saved_at ? profile.saved_at.substring(0, 19).replace('T', ' ') + ' UTC' : '' }}
                      {{ profile.key ? ` · ${profile.key}` : '' }}
                    </div>
                  </div>

                  <div class="flex gap-1.5 flex-shrink-0 items-center">
                    <template v-if="store.switchConfirmId === profile.id">
                      <span class="text-xs" style="color: var(--accent)">{{ t('switch.confirm_switch') }}</span>
                      <button class="btn btn-secondary btn-sm" @click="handleSwitch(profile.id)">{{ t('action.confirm') }}</button>
                      <button class="btn btn-ghost btn-sm" @click="store.switchConfirmId = null">{{ t('action.cancel') }}</button>
                    </template>
                    <template v-else-if="store.deleteConfirmId === profile.id">
                      <span class="text-xs" style="color: var(--danger)">{{ t('switch.confirm_delete') }}</span>
                      <button class="btn btn-sm" style="background: var(--danger); color: #fff; border-color: var(--danger)" @click="handleDelete(profile.id)">{{ t('action.confirm') }}</button>
                      <button class="btn btn-ghost btn-sm" @click="store.deleteConfirmId = null">{{ t('action.cancel') }}</button>
                    </template>
                    <template v-else>
                      <button class="btn btn-secondary btn-sm" @click="startEditNote(profile)">{{ t('switch.note_btn') }}</button>
                      <button class="btn btn-secondary btn-sm" @click="startEditContent(profile)">{{ t('switch.edit_content_btn') }}</button>
                      <button v-if="!profile.is_active" class="btn btn-secondary btn-sm" @click="handleSwitch(profile.id)">{{ t('switch.switch_btn') }}</button>
                      <button class="btn btn-danger btn-sm" @click="handleDelete(profile.id)">{{ t('switch.delete_btn') }}</button>
                    </template>
                  </div>
                </div>

                <!-- Edit key content block -->
                <div v-if="store.editingContentId === profile.id" class="mt-3 pt-3 border-t" style="border-color: var(--hairline)">
                  <textarea
                    v-model="store.contentCache[profile.id]"
                    class="ah-config-editor"
                    style="min-height: 100px"
                  />
                  <div class="mt-2 flex justify-end gap-2">
                    <button class="btn btn-secondary btn-sm" @click="store.editingContentId = null">{{ t('action.cancel') }}</button>
                    <button class="btn btn-primary btn-sm" @click="handleSaveContent(profile)">{{ t('switch.save_content') }}</button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>
