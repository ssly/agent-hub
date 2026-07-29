<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Activity, CheckCircle2, CircleStop, Monitor, Radar, Terminal, Trash2 } from 'lucide-vue-next'
import AppModal from '@/components/ui/AppModal.vue'
import { useToast } from '@/composables/useToast'
import { useSessionMonitorStore, type MonitorAgent, type SessionState } from '@/stores/session-monitor'

const { t, locale } = useI18n()
const store = useSessionMonitorStore()
const { showToast } = useToast()

const supportsHooks = computed(() => store.activeAgent === 'codex' || store.activeAgent === 'claude')
const isKiro = computed(() => store.activeAgent === 'kiro')
const isAll = computed(() => store.activeAgent === 'all')
const hookAction = computed(() => store.hookStatus?.installed ? 'uninstall' : 'install')
const defaultConfigPath = computed(() =>
  store.activeAgent === 'claude' ? '~/.claude/settings.json' : '~/.codex/hooks.json',
)
const runningCount = computed(
  () => store.displaySessions.filter(session => session.status === 'running').length,
)

function agentLabel(agent: MonitorAgent): string {
  return t(`session_monitor.agent_${agent}`)
}

/** Title of the sessions section: the merged tab has no single agent name. */
function sessionsTitle(): string {
  return isAll.value
    ? t('session_monitor.agent_all')
    : t('session_monitor.sessions_title', { agent: agentLabel(store.activeAgent as MonitorAgent) })
}

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

function sourceLabel(session: SessionState): string {
  return session.source === 'chatgpt'
    ? t('session_monitor.source_chatgpt')
    : t('session_monitor.source_terminal')
}

function emptyHint(): string {
  if (isAll.value) {
    return t('session_monitor.empty_all_hint')
  }
  if (isKiro.value) {
    return store.kiroStatus?.enabled
      ? t('session_monitor.empty_installed_hint', { agent: agentLabel('kiro') })
      : t('session_monitor.empty_monitor_hint')
  }
  return store.hookStatus?.installed
    ? t('session_monitor.empty_installed_hint', { agent: agentLabel(store.activeAgent as 'codex' | 'claude') })
    : t('session_monitor.empty_hook_hint')
}

function previewExplanation(): string {
  if (!store.preview) return ''
  return store.preview.action === 'install'
    ? t('session_monitor.install_explanation')
    : t('session_monitor.uninstall_explanation')
}

async function handleApplyHook() {
  const action = store.preview?.action
  const agent = store.previewAgent
  await store.applyHookPreview()
  if (!store.previewError && action) {
    showToast(
      action === 'install'
        ? t('session_monitor.install_success', { agent: agentLabel(agent) })
        : t('session_monitor.uninstall_success', { agent: agentLabel(agent) }),
      'success',
    )
  }
}

async function handleToggleKiroMonitor() {
  const next = !store.kiroStatus?.enabled
  await store.setKiroEnabled(next)
  if (!store.error) {
    showToast(
      next
        ? t('session_monitor.monitor_enabled_toast')
        : t('session_monitor.monitor_disabled_toast'),
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
        v-if="supportsHooks"
        class="btn"
        :class="hookAction === 'uninstall' ? 'btn-danger' : 'btn-primary'"
        :disabled="store.previewLoading || store.hookLoading"
        @click="store.openHookPreview(store.activeAgent as 'codex' | 'claude', hookAction)"
      >
        {{ hookAction === 'uninstall' ? t('session_monitor.uninstall_hook') : t('session_monitor.install_hook') }}
      </button>
      <button
        v-else-if="isKiro"
        class="btn"
        :class="store.kiroStatus?.enabled ? 'btn-danger' : 'btn-primary'"
        @click="handleToggleKiroMonitor"
      >
        {{ store.kiroStatus?.enabled ? t('session_monitor.disable_monitor') : t('session_monitor.enable_monitor') }}
      </button>
    </div>

    <section v-if="supportsHooks" class="hook-card ah-card">
      <div class="hook-card__status">
        <CheckCircle2 v-if="store.hookStatus?.installed" :size="18" class="hook-card__ok" />
        <CircleStop v-else :size="18" class="hook-card__missing" />
        <div class="min-w-0">
          <div class="hook-card__title">
            {{ store.hookStatus?.installed
              ? t('session_monitor.hook_installed', { agent: agentLabel(store.activeAgent as 'codex' | 'claude') })
              : t('session_monitor.hook_missing', { agent: agentLabel(store.activeAgent as 'codex' | 'claude') }) }}
          </div>
          <div class="hook-card__path">
            {{ store.hookStatus?.configPath || defaultConfigPath }}
          </div>
        </div>
      </div>
      <div class="hook-card__meta">
        <span>{{ t('session_monitor.running_summary', { count: runningCount }) }}</span>
        <span>{{ t('session_monitor.total_summary', { count: store.snapshot.sessions.length }) }}</span>
      </div>
    </section>

    <section v-else-if="isKiro" class="hook-card ah-card">
      <div class="hook-card__status">
        <Radar v-if="store.kiroStatus?.enabled && store.kiroStatus?.available" :size="18" class="hook-card__ok" />
        <CircleStop v-else :size="18" class="hook-card__missing" />
        <div class="min-w-0">
          <div class="hook-card__title">
            {{ store.kiroStatus?.enabled
              ? (store.kiroStatus?.available ? t('session_monitor.kiro_watch_active') : t('session_monitor.kiro_watch_unavailable'))
              : t('session_monitor.kiro_watch_off') }}
          </div>
          <div class="hook-card__path">
            {{ store.kiroStatus?.sessionsDir || '~/.kiro/sessions/cli' }}
          </div>
        </div>
      </div>
      <div class="hook-card__meta">
        <span>{{ t('session_monitor.running_summary', { count: runningCount }) }}</span>
        <span>{{ t('session_monitor.total_summary', { count: store.snapshot.sessions.length }) }}</span>
      </div>
    </section>

    <p v-if="isKiro" class="monitor-notice">
      {{ t('session_monitor.kiro_thread_note') }}
    </p>
    <p v-if="store.activeAgent === 'codex' && store.hookStatus?.installed" class="monitor-notice monitor-notice--warning">
      {{ t('session_monitor.trust_hint') }}
    </p>
    <p v-if="store.hookStatus?.issue" class="monitor-notice monitor-notice--warning">
      {{ store.hookStatus.issue }}
    </p>
    <p v-if="store.error" class="monitor-notice monitor-notice--error">
      {{ store.error }}
    </p>

    <div class="session-list-header">
      <h2>{{ sessionsTitle() }}</h2>
      <button class="btn btn-secondary btn-sm" :disabled="store.loading" @click="store.refresh">
        {{ t('session_monitor.refresh') }}
      </button>
    </div>

    <div v-if="store.loading && store.displaySessions.length === 0" class="monitor-empty">
      {{ t('session_monitor.loading') }}
    </div>
    <div v-else-if="store.displaySessions.length === 0" class="monitor-empty">
      <Activity :size="30" />
      <strong>{{ t('session_monitor.empty') }}</strong>
      <span>{{ emptyHint() }}</span>
    </div>
    <div v-else class="session-monitor-list">
      <article
        v-for="session in store.displaySessions"
        :key="`${session.agent}-${session.sessionId}`"
        class="session-row ah-card"
        :class="{ 'session-row--running': session.status === 'running' }"
      >
        <div class="session-row__top">
          <span v-if="isAll" class="session-agent-badge">{{ agentLabel(session.agent) }}</span>
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
          <button
            v-if="session.status === 'ended'"
            v-tooltip="t('session_monitor.delete_session')"
            class="session-delete"
            @click="store.deleteSession(session.sessionId, session.agent)"
          >
            <Trash2 :size="13" />
          </button>
        </div>

        <div class="session-row__line">
          <span>{{ t('session_monitor.user_question') }}</span>
          <p>{{ session.userPrompt || t('session_monitor.no_prompt') }}</p>
        </div>
        <div class="session-row__line">
          <span>{{ t('session_monitor.assistant_reply') }}</span>
          <p>{{ session.assistantReply || (session.status === 'running' ? t('session_monitor.waiting_reply', { agent: agentLabel(session.agent) }) : t('session_monitor.no_reply')) }}</p>
        </div>
      </article>
    </div>

    <AppModal
      :show="store.previewOpen"
      :title="store.preview?.action === 'uninstall'
        ? t('session_monitor.uninstall_preview_title', { agent: agentLabel(store.previewAgent) })
        : t('session_monitor.install_preview_title', { agent: agentLabel(store.previewAgent) })"
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
          {{ previewExplanation() }}
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

        <p v-if="store.preview.action === 'install' && store.previewAgent === 'codex'" class="monitor-notice monitor-notice--warning">
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
.monitor-notice { margin: 10px 0 14px; padding: 9px 11px; border-radius: var(--radius-sm); font-size: 12px; line-height: 1.5; color: var(--ink-3); background: var(--sunken); }
.monitor-notice--warning { color: var(--warning); background: color-mix(in srgb, var(--warning) 10%, transparent); }
.monitor-notice--error { color: var(--danger); background: var(--danger-soft); }
.session-list-header { display: flex; align-items: center; justify-content: space-between; margin: 24px 0 10px; }
.session-list-header h2 { color: var(--ink); font: 600 15px/1.2 var(--font-serif); }
.session-monitor-list { display: flex; flex-direction: column; gap: 9px; }
.session-row { padding: 13px 15px; }
.session-row--running { border-color: color-mix(in srgb, var(--success) 45%, var(--hairline)); box-shadow: 0 0 0 1px color-mix(in srgb, var(--success) 7%, transparent); }
.session-row__top { display: flex; align-items: center; gap: 10px; margin-bottom: 10px; }
.session-source { display: inline-flex; align-items: center; gap: 6px; color: var(--ink-2); font-size: 12px; font-weight: 600; }
.session-agent-badge { display: inline-flex; align-items: center; padding: 2px 8px; border-radius: 999px; background: var(--sunken); color: var(--ink-2); font-size: 11px; font-weight: 600; }
.session-status { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; }
.session-status__dot { width: 7px; height: 7px; border-radius: 999px; background: currentColor; }
.session-status--running { color: var(--success); }
.session-status--ended { color: var(--ink-4); }
.session-time { margin-left: auto; color: var(--ink-4); font: 11px/1 var(--font-mono); }
.session-delete { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: none; border-radius: var(--radius-sm); background: transparent; color: var(--ink-4); cursor: pointer; transition: color .15s, background .15s; }
.session-delete:hover { color: var(--danger); background: var(--danger-soft); }
.session-row__line { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 8px; min-width: 0; font-size: 12.5px; line-height: 1.8; }
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
