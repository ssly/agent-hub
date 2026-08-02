<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import AppModal from '@/components/ui/AppModal.vue'
import AppLoading from '@/components/ui/AppLoading.vue'
import { useToast } from '@/composables/useToast'
import * as api from '@/lib/api'

// Self-fetching resume modal (copy-command flow) shared by the Sessions
// browser and the Monitor. Parents pass the platform/session identity and the
// project path used for the terminal `cd`.
const props = defineProps<{
  show: boolean
  platformId?: string | null
  sessionId?: string | null
  projectPath?: string | null
  title?: string
}>()

const emit = defineEmits<{ close: [] }>()

const { t } = useI18n()
const { showToast } = useToast()

const preview = ref<api.SessionResumePreview | null>(null)
const loading = ref(false)
const loadError = ref('')
const copied = ref(false)

async function copyResumeCommand() {
  const command = preview.value?.command
  if (!command) return
  try {
    await navigator.clipboard.writeText(command)
    copied.value = true
    showToast(t('action.copied'), 'success')
    setTimeout(() => { copied.value = false }, 2000)
  } catch (e: any) {
    showToast(t('session.copy_failed', { error: e?.message || e }), 'error')
  }
}

watch(() => props.show, async open => {
  if (!open || !props.platformId || !props.sessionId) return
  preview.value = null
  loadError.value = ''
  loading.value = true
  try {
    preview.value = await api.getSessionResumePreview(
      props.platformId,
      props.sessionId,
      props.projectPath || '',
    )
  } catch (e: any) {
    loadError.value = e?.SyncError || e?.message || String(e)
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <AppModal
    :show="show"
    :title="t('session.resume')"
    width-class="w-[36rem]"
    @close="emit('close')"
  >
    <AppLoading v-if="loading" class="py-8">{{ t('session.loading_messages') }}</AppLoading>
    <div v-else-if="loadError" class="py-8 text-center" style="color: var(--danger)">
      {{ t('session.resume_failed', { error: loadError }) }}
    </div>
    <div v-else-if="preview" class="flex flex-col gap-2.5">
      <div class="ah-resume-row">
        <span class="ah-resume-row__label">{{ t('session.resume_field_title') }}</span>
        <span class="ah-resume-row__value select-text">
          {{ title || t('session.untitled') }}
        </span>
      </div>
      <div class="ah-resume-row">
        <span class="ah-resume-row__label">{{ t('session.resume_last_question') }}</span>
        <span class="ah-resume-row__value select-text">
          {{ preview.last_user_message || t('session.resume_empty') }}
        </span>
      </div>
      <div class="ah-resume-row">
        <span class="ah-resume-row__label">{{ t('session.resume_last_answer') }}</span>
        <span class="ah-resume-row__value select-text">
          {{ preview.last_assistant_message || t('session.resume_empty') }}
        </span>
      </div>
      <div class="ah-resume-row ah-resume-row--command">
        <div class="flex items-center justify-between gap-2">
          <span class="ah-resume-row__label">{{ t('session.resume_command_label') }}</span>
          <span class="ah-resume-row__hint">{{ t('session.resume_command_hint') }}</span>
        </div>
        <div class="ah-resume-command">
          <code class="ah-resume-command__text select-text">{{ preview.command }}</code>
          <button class="btn btn-secondary btn-sm shrink-0" @click="copyResumeCommand">
            {{ copied ? t('action.copied') : t('action.copy') }}
          </button>
        </div>
      </div>
    </div>
    <template #footer>
      <button class="btn btn-secondary" @click="emit('close')">{{ t('action.close') }}</button>
    </template>
  </AppModal>
</template>

<style scoped>
/* Resume modal rows: fixed-width label + single-line truncated value */
.ah-resume-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}
.ah-resume-row--command {
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  margin-top: 4px;
}
.ah-resume-row__label {
  flex-shrink: 0;
  font-size: 12px;
  font-weight: 600;
  color: var(--ink-2);
  white-space: nowrap;
}
.ah-resume-row__hint {
  font-size: 11px;
  color: var(--ink-4);
  white-space: nowrap;
}
.ah-resume-row__value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
  color: var(--ink);
}
.ah-resume-command {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 10px;
  background: var(--sunken);
  border: 1px solid var(--hairline);
  border-radius: var(--radius-sm);
}
.ah-resume-command__text {
  min-width: 0;
  flex: 1;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  line-height: 1.6;
  color: var(--ink);
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
