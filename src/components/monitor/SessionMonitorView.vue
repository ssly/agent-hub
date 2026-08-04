<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Activity, CheckCircle2, CircleStop } from 'lucide-vue-next'
import AppModal from '@/components/ui/AppModal.vue'
import SessionCard from '@/components/sessions/SessionCard.vue'
import SessionMessagesModal from '@/components/sessions/SessionMessagesModal.vue'
import SessionResumeModal from '@/components/sessions/SessionResumeModal.vue'
import { useToast } from '@/composables/useToast'
import {
  useSessionMonitorStore,
  HOOK_AGENTS,
  MONITOR_AGENTS,
  MONITOR_AGENT_PLATFORM,
  type MonitorAgent,
  type HookAgent,
  type AgentSessionState,
  type SessionState,
} from '@/stores/session-monitor'

const { t, locale } = useI18n()
const store = useSessionMonitorStore()
const { showToast } = useToast()

const supportsHooks = computed(() => (HOOK_AGENTS as string[]).includes(store.activeAgent))
const isAll = computed(() => store.activeAgent === 'all')
const hookAction = computed(() => store.hookStatus?.installed ? 'uninstall' : 'install')
const HOOK_CONFIG_PATHS: Record<string, string> = {
  codex: '~/.codex/hooks.json',
  claude: '~/.claude/settings.json',
  cursor: '~/.cursor/hooks.json',
  grok: '~/.grok/hooks/agent-hub.json',
  kimi: '~/.kimi-code/config.toml',
  zcode: '~/.zcode/cli/config.json',
}
const defaultConfigPath = computed(() => HOOK_CONFIG_PATHS[store.activeAgent] ?? '')
const runningCount = computed(
  () => store.displaySessions.filter(session => session.status === 'running').length,
)

// Outdated hook installs: managed handlers exist but no longer match the
// current expected set (e.g. StopFailure/Interrupt were added in a later
// version). Only a reinstall brings them up to date, so one banner covers
// every tab — including the merged "all" view, which has no hook card.
const outdatedHookAgents = computed(() =>
  HOOK_AGENTS.filter(agent => {
    const status = store.hookStatuses[agent]
    return status ? !status.installed && status.managedHandlerCount > 0 : false
  }),
)
const outdatedHookAgentNames = computed(() =>
  outdatedHookAgents.value
    .map(agent => agentLabel(agent))
    .join(locale.value === 'en' ? ', ' : '、'),
)

/** One banner per page, highest priority first — warnings never stack.
 *  Priority: query errors > hook-detected issues > outdated installs >
 *  Codex trust reminder > new-session reload note. */
const primaryNotice = computed<{ kind: 'info' | 'warning' | 'error'; text: string } | null>(() => {
  if (store.error) return { kind: 'error', text: store.error }
  if (store.hookStatus?.issue) return { kind: 'warning', text: store.hookStatus.issue }
  if (outdatedHookAgents.value.length) {
    return { kind: 'warning', text: t('session_monitor.hook_upgrade_hint', { agents: outdatedHookAgentNames.value }) }
  }
  if (store.activeAgent === 'codex' && store.hookStatus?.installed) {
    return { kind: 'warning', text: t('session_monitor.trust_hint') }
  }
  if ((['cursor', 'grok', 'kimi', 'zcode'] as string[]).includes(store.activeAgent) && store.hookStatus?.installed) {
    return { kind: 'info', text: t('session_monitor.hook_reload_hint') }
  }
  return null
})

// One-line enablement tags for the merged view: green when the agent's hook
// is installed, gray otherwise — a quiet reminder that agents only report
// after a proper install. Clicking a tag jumps to that agent's tab for
// install/repair.
const agentTags = computed(() =>
  MONITOR_AGENTS.map(agent => ({
    agent,
    enabled: !!store.hookStatuses[agent as HookAgent]?.installed,
  })),
)

function agentLabel(agent: MonitorAgent): string {
  return t(`session_monitor.agent_${agent}`)
}

/** Card badge: mark the concrete client when the capture channel proves it.
 *  Codex rows carry hook-detected provenance: the ChatGPT desktop / IDE
 *  client is marked as such, everything else stays "Codex". */
function agentBadgeLabel(session: AgentSessionState): string {
  if (session.agent === 'codex' && session.source === 'chatgpt') {
    return t('session_monitor.source_chatgpt')
  }
  return agentLabel(session.agent)
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
  if (session.source === 'chatgpt') return t('session_monitor.source_chatgpt')
  if (session.source === 'cursor') return t('session_monitor.source_cursor')
  return t('session_monitor.source_terminal')
}

/** Platform id of the sessions adapter backing a monitor row's full history.
 *  Null when the agent has no sessions adapter. */
function sessionPlatform(session: { agent: MonitorAgent } | null): string | null {
  return session ? MONITOR_AGENT_PLATFORM[session.agent] ?? null : null
}

function emptyHint(): string {
  if (isAll.value) {
    return t('session_monitor.empty_all_hint')
  }
  return store.hookStatus?.installed
    ? t('session_monitor.empty_installed_hint', { agent: agentLabel(store.activeAgent as MonitorAgent) })
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
        @click="store.openHookPreview(store.activeAgent as HookAgent, hookAction)"
      >
        {{ hookAction === 'uninstall' ? t('session_monitor.uninstall_hook') : t('session_monitor.install_hook') }}
      </button>
    </div>

    <div v-if="isAll" class="monitor-tag-row">
      <button
        v-for="tag in agentTags"
        :key="tag.agent"
        class="monitor-tag"
        :class="tag.enabled ? 'is-on' : 'is-off'"
        v-tooltip="t(tag.enabled ? 'session_monitor.tag_on' : 'session_monitor.tag_off')"
        @click="store.activeAgent = tag.agent"
      >
        <span class="monitor-tag__dot" />
        {{ agentLabel(tag.agent) }}
      </button>
    </div>

    <section v-if="supportsHooks" class="hook-card ah-card">
      <div class="hook-card__status">
        <CheckCircle2 v-if="store.hookStatus?.installed" :size="18" class="hook-card__ok" />
        <CircleStop v-else :size="18" class="hook-card__missing" />
        <div class="min-w-0">
          <div class="hook-card__title">
            {{ store.hookStatus?.installed
              ? t('session_monitor.hook_installed', { agent: agentLabel(store.activeAgent as MonitorAgent) })
              : t('session_monitor.hook_missing', { agent: agentLabel(store.activeAgent as MonitorAgent) }) }}
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

    <p
      v-if="primaryNotice"
      class="ah-notice"
      :class="{
        'ah-notice--warning': primaryNotice.kind === 'warning',
        'ah-notice--error': primaryNotice.kind === 'error',
      }"
    >
      {{ primaryNotice.text }}
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
      <SessionCard
        v-for="session in store.displaySessions"
        :key="`${session.agent}-${session.sessionId}`"
        :badge="agentBadgeLabel(session)"
        :source="session.source"
        :source-label="sourceLabel(session)"
        :status="session.status"
        :time="formatTime(session.updatedAt)"
        :delete-note="t('session_monitor.delete_note')"
        :title="session.userPrompt || t('session_monitor.no_prompt')"
        :subtitle="session.cwd || undefined"
        :resumable="sessionPlatform(session) !== null"
        @open="sessionPlatform(session) !== null && store.openMessages(session)"
        @resume="store.openResume(session)"
        @delete="store.deleteSession(session.sessionId, session.agent)"
      >
        <div class="session-row__line">
          <span>{{ t('session_monitor.assistant_reply') }}</span>
          <p>{{ session.assistantReply || (session.status === 'running' ? t('session_monitor.waiting_reply', { agent: agentLabel(session.agent) }) : t('session_monitor.no_reply')) }}</p>
        </div>
      </SessionCard>
    </div>

    <SessionMessagesModal
      :show="store.messagesModalOpen"
      :platform-id="sessionPlatform(store.modalSession)"
      :session-id="store.modalSession?.sessionId"
      :title="store.modalSession?.userPrompt || undefined"
      :project-path="store.modalSession?.cwd"
      :started-at="store.modalSession?.updatedAt ? store.modalSession.updatedAt / 1000 : null"
      @close="store.messagesModalOpen = false"
    />

    <SessionResumeModal
      :show="store.resumeModalOpen"
      :platform-id="sessionPlatform(store.resumeSession)"
      :session-id="store.resumeSession?.sessionId"
      :project-path="store.resumeSession?.cwd"
      :title="store.resumeSession?.userPrompt || undefined"
      @close="store.resumeModalOpen = false"
    />

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
      <div v-else-if="store.previewError" class="ah-notice ah-notice--error">
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

        <p v-if="store.preview.action === 'install' && store.previewAgent === 'codex'" class="ah-notice ah-notice--warning">
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

/* Enablement tags under the merged view header: one quiet line showing which
   agents are live (green dot) and which still need a hook install (gray). */
.monitor-tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 2px 0 12px;
}
.monitor-tag {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2px 9px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface);
  color: var(--ink-3);
  font-size: 11px;
  cursor: pointer;
  transition: border-color var(--dur-fast) var(--ease-soft), color var(--dur-fast) var(--ease-soft);
}
.monitor-tag:hover { border-color: var(--border-strong); color: var(--ink-2); }
.monitor-tag.is-on { color: var(--ink-2); }
.monitor-tag__dot { width: 6px; height: 6px; border-radius: 999px; }
.monitor-tag.is-on .monitor-tag__dot { background: var(--success); }
.monitor-tag.is-off .monitor-tag__dot { background: var(--ink-4); }
.session-list-header { display: flex; align-items: center; justify-content: space-between; margin: 24px 0 10px; }
.session-list-header h2 { color: var(--ink); font: 600 15px/1.2 var(--font-serif); }
.session-monitor-list { display: flex; flex-direction: column; gap: 9px; }
/* Q&A line inside the shared card's default slot (monitor-specific body). */
.session-row__line { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 8px; min-width: 0; margin-top: 4px; font-size: 12.5px; line-height: 1.8; }
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
