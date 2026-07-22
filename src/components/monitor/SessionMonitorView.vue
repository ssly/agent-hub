<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Activity, CheckCircle2, CircleStop, Monitor, Terminal } from 'lucide-vue-next'
import AppModal from '@/components/ui/AppModal.vue'
import { useToast } from '@/composables/useToast'
import { useSessionMonitorStore, type CodexSessionState } from '@/stores/session-monitor'

const { t, locale } = useI18n()
const store = useSessionMonitorStore()
const { showToast } = useToast()

const hookAction = computed(() => store.hookStatus?.installed ? 'uninstall' : 'install')
const runningCount = computed(
  () => store.snapshot.sessions.filter(session => session.status === 'running').length,
)

function formatTime(timestamp: number): string {
  if (!timestamp) return ''
  return new Intl.DateTimeFormat(locale.value === 'en' ? 'en-US' : 'zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(new Date(timestamp))
}

function sourceLabel(session: CodexSessionState): string {
  return session.source === 'chatgpt'
    ? t('session_monitor.source_chatgpt')
    : t('session_monitor.source_terminal')
}

async function handleApplyHook() {
  const action = store.preview?.action
  await store.applyHookPreview()
  if (!store.previewError && action) {
    showToast(
      action === 'install'
        ? t('session_monitor.install_success')
        : t('session_monitor.uninstall_success'),
      'success',
    )
  }
}

onMounted(() => store.initialize())
onUnmounted(() => store.dispose())
</script>

<template>
  <div class="session-monitor-page">
    <div class="ah-page-header session-monitor-heading">
      <div>
        <h1 class="ah-page-title">{{ t('session_monitor.title') }}</h1>
        <p class="session-monitor-subtitle">{{ t('session_monitor.subtitle') }}</p>
      </div>
      <button
        class="btn"
        :class="hookAction === 'uninstall' ? 'btn-danger' : 'btn-primary'"
        :disabled="store.previewLoading || store.hookLoading"
        @click="store.openHookPreview(hookAction)"
      >
        {{ hookAction === 'uninstall' ? t('session_monitor.uninstall_hook') : t('session_monitor.install_hook') }}
      </button>
    </div>

    <section class="hook-card ah-card">
      <div class="hook-card__status">
        <CheckCircle2 v-if="store.hookStatus?.installed" :size="18" class="hook-card__ok" />
        <CircleStop v-else :size="18" class="hook-card__missing" />
        <div class="min-w-0">
          <div class="hook-card__title">
            {{ store.hookStatus?.installed ? t('session_monitor.hook_installed') : t('session_monitor.hook_missing') }}
          </div>
          <div class="hook-card__path">
            {{ store.hookStatus?.configPath || '~/.codex/hooks.json' }}
          </div>
        </div>
      </div>
      <div class="hook-card__meta">
        <span>{{ t('session_monitor.running_summary', { count: runningCount }) }}</span>
        <span>{{ t('session_monitor.total_summary', { count: store.snapshot.sessions.length }) }}</span>
      </div>
    </section>

    <p v-if="store.hookStatus?.issue" class="monitor-notice monitor-notice--warning">
      {{ store.hookStatus.issue }}
    </p>
    <p v-if="store.error" class="monitor-notice monitor-notice--error">
      {{ store.error }}
    </p>

    <div class="session-list-header">
      <h2>{{ t('session_monitor.sessions_title') }}</h2>
      <button class="btn btn-secondary btn-sm" :disabled="store.loading" @click="store.refresh">
        {{ t('session_monitor.refresh') }}
      </button>
    </div>

    <div v-if="store.loading && store.snapshot.sessions.length === 0" class="monitor-empty">
      {{ t('session_monitor.loading') }}
    </div>
    <div v-else-if="store.snapshot.sessions.length === 0" class="monitor-empty">
      <Activity :size="30" />
      <strong>{{ t('session_monitor.empty') }}</strong>
      <span>{{ store.hookStatus?.installed ? t('session_monitor.empty_installed_hint') : t('session_monitor.empty_hook_hint') }}</span>
    </div>
    <div v-else class="session-monitor-list">
      <article
        v-for="session in store.snapshot.sessions"
        :key="session.sessionId"
        class="session-row ah-card"
        :class="{ 'session-row--running': session.status === 'running' }"
      >
        <div class="session-row__top">
          <div class="session-source">
            <Monitor v-if="session.source === 'chatgpt'" :size="15" />
            <Terminal v-else :size="15" />
            <span>{{ sourceLabel(session) }}</span>
          </div>
          <div class="session-status" :class="`session-status--${session.status}`">
            <span class="session-status__dot" />
            {{ session.status === 'running' ? t('session_monitor.status_running') : t('session_monitor.status_ended') }}
          </div>
          <time class="session-time">{{ formatTime(session.updatedAt) }}</time>
        </div>

        <div class="session-row__line" :title="session.userPrompt || ''">
          <span>{{ t('session_monitor.user_question') }}</span>
          <p>{{ session.userPrompt || t('session_monitor.no_prompt') }}</p>
        </div>
        <div class="session-row__line" :title="session.assistantReply || ''">
          <span>{{ t('session_monitor.assistant_reply') }}</span>
          <p>{{ session.assistantReply || (session.status === 'running' ? t('session_monitor.waiting_reply') : t('session_monitor.no_reply')) }}</p>
        </div>
      </article>
    </div>

    <AppModal
      :show="store.previewOpen"
      :title="store.preview?.action === 'uninstall' ? t('session_monitor.uninstall_preview_title') : t('session_monitor.install_preview_title')"
      width-class="w-[44rem]"
      :close-on-outside="!store.hookLoading"
      @close="store.closeHookPreview"
    >
      <div v-if="store.previewLoading" class="preview-loading">
        {{ t('session_monitor.preview_loading') }}
      </div>
      <div v-else-if="store.previewError" class="monitor-notice monitor-notice--error">
        {{ store.previewError }}
      </div>
      <template v-else-if="store.preview">
        <p class="preview-explanation">
          {{ store.preview.action === 'install' ? t('session_monitor.install_explanation') : t('session_monitor.uninstall_explanation') }}
        </p>

        <dl class="preview-fields">
          <div>
            <dt>{{ t('session_monitor.config_file') }}</dt>
            <dd>{{ store.preview.configPath }}</dd>
          </div>
          <div>
            <dt>{{ t('session_monitor.hook_command') }}</dt>
            <dd>{{ store.preview.command }}</dd>
          </div>
        </dl>

        <p v-if="store.preview.action === 'install'" class="monitor-notice monitor-notice--warning">
          {{ t('session_monitor.trust_hint') }}
        </p>

        <div class="preview-stats">
          {{ t('session_monitor.preview_stats', { added: store.preview.added, removed: store.preview.removed }) }}
        </div>
        <pre v-if="store.preview.diffLines.length" class="hook-diff"><template v-for="(line, index) in store.preview.diffLines" :key="index"><span :class="`hook-diff__${line.tag}`">{{ line.tag === 'added' ? '+ ' : line.tag === 'removed' ? '- ' : '  ' }}{{ line.content }}</span></template></pre>
        <p v-else class="preview-loading">{{ t('session_monitor.no_changes') }}</p>
      </template>

      <template #footer>
        <button class="btn btn-secondary" :disabled="store.hookLoading" @click="store.closeHookPreview">
          {{ t('action.cancel') }}
        </button>
        <button
          class="btn"
          :class="store.preview?.action === 'uninstall' ? 'btn-danger' : 'btn-primary'"
          :disabled="!store.preview?.changed || store.hookLoading || store.previewLoading"
          @click="handleApplyHook"
        >
          {{ store.hookLoading
            ? t('session_monitor.applying')
            : store.preview?.action === 'uninstall'
              ? t('session_monitor.confirm_uninstall')
              : t('session_monitor.confirm_install') }}
        </button>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.session-monitor-page { max-width: 960px; margin: 0 auto; padding: 28px 32px 48px; }
.session-monitor-heading { align-items: flex-end; }
.session-monitor-subtitle { margin-top: 5px; color: var(--ink-3); font-size: 13px; }
.hook-card { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin-bottom: 14px; }
.hook-card__status { display: flex; align-items: center; gap: 10px; min-width: 0; }
.hook-card__ok { color: var(--success); flex: none; }
.hook-card__missing { color: var(--warning); flex: none; }
.hook-card__title { color: var(--ink); font-size: 14px; font-weight: 600; }
.hook-card__path { margin-top: 2px; color: var(--ink-4); font: 11px/1.4 var(--font-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.hook-card__meta { display: flex; gap: 14px; flex: none; color: var(--ink-3); font-size: 12px; }
.monitor-notice { margin: 10px 0 14px; padding: 9px 11px; border-radius: var(--radius-sm); font-size: 12px; line-height: 1.5; }
.monitor-notice--warning { color: var(--warning); background: color-mix(in srgb, var(--warning) 10%, transparent); }
.monitor-notice--error { color: var(--danger); background: var(--danger-soft); }
.session-list-header { display: flex; align-items: center; justify-content: space-between; margin: 24px 0 10px; }
.session-list-header h2 { color: var(--ink); font: 600 15px/1.2 var(--font-serif); }
.session-monitor-list { display: flex; flex-direction: column; gap: 9px; }
.session-row { padding: 13px 15px; }
.session-row--running { border-color: color-mix(in srgb, var(--success) 45%, var(--hairline)); box-shadow: 0 0 0 1px color-mix(in srgb, var(--success) 7%, transparent); }
.session-row__top { display: flex; align-items: center; gap: 10px; margin-bottom: 10px; }
.session-source { display: inline-flex; align-items: center; gap: 6px; color: var(--ink-2); font-size: 12px; font-weight: 600; }
.session-status { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; }
.session-status__dot { width: 7px; height: 7px; border-radius: 999px; background: currentColor; }
.session-status--running { color: var(--success); }
.session-status--ended { color: var(--ink-4); }
.session-time { margin-left: auto; color: var(--ink-4); font: 11px/1 var(--font-mono); }
.session-row__line { display: grid; grid-template-columns: 58px minmax(0, 1fr); align-items: center; gap: 7px; min-width: 0; font-size: 12.5px; line-height: 1.8; }
.session-row__line > span { color: var(--ink-4); }
.session-row__line > p { min-width: 0; overflow: hidden; color: var(--ink-2); text-overflow: ellipsis; white-space: nowrap; }
.monitor-empty { min-height: 230px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--ink-4); text-align: center; }
.monitor-empty strong { color: var(--ink-2); font-size: 14px; }
.monitor-empty span { font-size: 12px; }
.preview-loading { padding: 16px 0; color: var(--ink-3); font-size: 13px; }
.preview-explanation { color: var(--ink-2); font-size: 13px; line-height: 1.65; }
.preview-fields { display: grid; gap: 9px; margin: 14px 0; }
.preview-fields > div { display: grid; grid-template-columns: 76px minmax(0, 1fr); gap: 10px; }
.preview-fields dt { color: var(--ink-4); font-size: 12px; }
.preview-fields dd { min-width: 0; overflow-wrap: anywhere; color: var(--ink-2); font: 11.5px/1.55 var(--font-mono); }
.preview-stats { margin: 14px 0 7px; color: var(--accent); font-size: 12px; font-weight: 600; }
.hook-diff { max-height: 340px; overflow: auto; padding: 11px 0; border: 1px solid var(--hairline); border-radius: var(--radius-sm); background: var(--sunken); font: 11.5px/1.55 var(--font-mono); }
.hook-diff span { display: block; min-height: 18px; padding: 0 11px; white-space: pre-wrap; overflow-wrap: anywhere; }
.hook-diff__added { color: var(--success); background: color-mix(in srgb, var(--success) 8%, transparent); }
.hook-diff__removed { color: var(--danger); background: color-mix(in srgb, var(--danger) 8%, transparent); }
.hook-diff__context { color: var(--ink-3); }
@media (max-width: 760px) {
  .session-monitor-page { padding: 20px; }
  .hook-card { align-items: flex-start; flex-direction: column; }
  .session-monitor-heading { align-items: flex-start; gap: 14px; }
}
</style>
