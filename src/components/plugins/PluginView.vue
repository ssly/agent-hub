<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { usePluginsStore } from '@/stores/plugins'
import { useSkillsStore } from '@/stores/skills'
import { useMcpStore } from '@/stores/mcp'
import { useClaudePluginsStore } from '@/stores/claude-plugins'
import SkillListView from '@/components/skills/SkillListView.vue'
import McpListView from '@/components/mcp/McpListView.vue'
import ClaudePluginList from '@/components/plugins/ClaudePluginList.vue'

const { t } = useI18n()
const pluginsStore = usePluginsStore()
const skillsStore = useSkillsStore()
const mcpStore = useMcpStore()
const claudePluginsStore = useClaudePluginsStore()

const skillCount = computed(() => skillsStore.skills.length)
const serverCount = computed(() => mcpStore.servers.length)
const isClaudeCode = computed(() => pluginsStore.selectedPlatformId === 'claude-code')
const showMcpSection = computed(() => pluginsStore.isGlobalScope || serverCount.value > 0)
const showClaudeSection = computed(() => isClaudeCode.value
  && (pluginsStore.isGlobalScope || claudePluginsStore.plugins.length > 0))
const showSkillsSection = computed(() => pluginsStore.isGlobalScope || skillCount.value > 0)
</script>

<template>
  <div class="ah-plugin-view view-enter">
    <div v-if="!pluginsStore.selectedPlatform" class="ah-plugin-empty">
      <p>{{ t('plugin.no_agents') }}</p>
    </div>

    <template v-else>
      <header class="ah-plugin-header">
        <div class="min-w-0">
          <p class="ah-plugin-eyebrow">
            {{ pluginsStore.isGlobalScope ? t('plugin.workspace') : t('plugin.scope_project') }}
          </p>
          <h1 class="ah-page-title truncate">{{ pluginsStore.selectedPlatform.display_name }}</h1>
          <p class="ah-plugin-summary">
            {{ isClaudeCode
              ? t('plugin.summary_claude', { plugins: claudePluginsStore.plugins.length, skills: skillCount, servers: serverCount })
              : t('plugin.summary', { skills: skillCount, servers: serverCount }) }}
          </p>
          <div class="ah-plugin-paths">
            <div v-if="pluginsStore.selectedPlatform.skill_dir" class="ah-plugin-path">
              <span>{{ t('plugin.skills_path') }}</span>
              <code>{{ pluginsStore.selectedPlatform.skill_dir }}</code>
            </div>
            <div v-if="pluginsStore.selectedPlatform.config_path" class="ah-plugin-path">
              <span>{{ t('plugin.mcp_path') }}</span>
              <code>{{ pluginsStore.selectedPlatform.config_path }}</code>
            </div>
          </div>
        </div>
      </header>

      <div class="ah-plugin-grid">
        <section v-if="showMcpSection" class="ah-plugin-pane" aria-labelledby="plugin-mcp-heading">
          <div class="ah-plugin-pane__header">
            <div>
              <h2 id="plugin-mcp-heading">{{ t('plugin.mcp') }}</h2>
              <p>{{ t('plugin.mcp_hint') }}</p>
            </div>
            <div class="flex items-center gap-2">
              <span class="ah-plugin-count">{{ serverCount }}</span>
              <button
                v-if="pluginsStore.selectedPlatform.supports_mcp && pluginsStore.isGlobalScope"
                class="btn btn-primary btn-sm"
                @click="mcpStore.addModalOpen = true"
              >+ {{ t('mcp.add') }}</button>
            </div>
          </div>
          <div class="ah-plugin-pane__body ah-plugin-pane__body--mcp">
            <McpListView embedded :readonly="!pluginsStore.isGlobalScope" />
          </div>
        </section>

        <section v-if="showClaudeSection" class="ah-plugin-pane" aria-labelledby="plugin-claude-heading">
          <div class="ah-plugin-pane__header">
            <div>
              <h2 id="plugin-claude-heading">{{ t('plugin.claude_plugins') }}</h2>
              <p>{{ t('plugin.claude_plugins_hint') }}</p>
            </div>
            <span
              class="ah-plugin-count"
              :title="t('plugin.claude_enabled_count', { enabled: claudePluginsStore.enabledCount, total: claudePluginsStore.plugins.length })"
            >{{ claudePluginsStore.enabledCount }}/{{ claudePluginsStore.plugins.length }}</span>
          </div>
          <div class="ah-plugin-pane__body">
            <ClaudePluginList />
          </div>
        </section>

        <section v-if="showSkillsSection" class="ah-plugin-pane" aria-labelledby="plugin-skills-heading">
          <div class="ah-plugin-pane__header">
            <div>
              <h2 id="plugin-skills-heading">{{ t('plugin.skills') }}</h2>
              <p>{{ t('plugin.skills_hint') }}</p>
            </div>
            <span class="ah-plugin-count">{{ skillCount }}</span>
          </div>
          <div class="ah-plugin-pane__body">
            <SkillListView embedded :readonly="!pluginsStore.isGlobalScope" />
          </div>
        </section>
      </div>
    </template>
  </div>
</template>

<style scoped>
.ah-plugin-view {
  min-height: 100%;
  padding: 22px 24px 24px;
  display: flex;
  flex-direction: column;
}
.ah-plugin-empty {
  flex: 1;
  display: grid;
  place-items: center;
  color: var(--ink-3);
}
.ah-plugin-header {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 24px;
  margin-bottom: 18px;
}
.ah-plugin-eyebrow {
  margin-bottom: 2px;
  color: var(--ink-4);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: .08em;
  text-transform: uppercase;
}
.ah-plugin-summary {
  margin-top: 3px;
  color: var(--ink-3);
  font-size: 13px;
}
.ah-plugin-paths {
  display: flex;
  flex-wrap: wrap;
  gap: 5px 16px;
  margin-top: 7px;
}
.ah-plugin-path {
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: 7px;
  color: var(--ink-4);
  font-size: 11px;
}
.ah-plugin-path span { flex-shrink: 0; font-weight: 500; }
.ah-plugin-path code {
  color: var(--ink-3);
  font-family: var(--font-mono);
  overflow-wrap: anywhere;
  word-break: break-word;
}
.ah-plugin-count {
  border: 1px solid var(--hairline);
  border-radius: var(--radius-pill);
  background: var(--surface);
  color: var(--ink-3);
  font-size: 12px;
  padding: 4px 10px;
}
.ah-plugin-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 14px;
  align-items: start;
}
.ah-plugin-pane {
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--surface);
  border: 1px solid var(--hairline);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-mist);
  overflow: hidden;
}
.ah-plugin-pane__header {
  min-height: 66px;
  padding: 13px 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border-bottom: 1px solid var(--hairline);
}
.ah-plugin-pane__header h2 { font-size: 14px; font-weight: 600; color: var(--ink); }
.ah-plugin-pane__header p { font-size: 12px; color: var(--ink-4); margin-top: 1px; }
.ah-plugin-count { font-family: var(--font-mono); padding: 2px 8px; }
.ah-plugin-pane__body { min-width: 0; }

:deep(.ah-embedded-view) { padding: 0; }
:deep(.ah-embedded-view .ah-view-content) { max-width: none; }
:deep(.ah-embedded-view .ah-table-wrap) { border: 0; border-radius: 0; }
:deep(.ah-embedded-view .ah-thead),
:deep(.ah-embedded-view .ah-row) {
  grid-template-columns: minmax(9rem, 1.15fr) minmax(10rem, 1.65fr) 4.5rem 2rem;
  column-gap: 8px;
}
:deep(.ah-embedded-view .ah-row__desc) {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  white-space: normal;
  overflow: hidden;
  overflow-wrap: anywhere;
  line-height: 1.45;
}
:deep(.ah-embedded-view .ah-accordion__summary),
:deep(.ah-embedded-view .ah-config-view) {
  max-width: 100%;
  overflow-wrap: anywhere;
  word-break: break-word;
  white-space: pre-wrap;
}

@media (max-width: 1050px) {
  .ah-plugin-grid { flex: none; }
}
@media (max-width: 720px) {
  .ah-plugin-view { padding: 16px; }
  .ah-plugin-header { align-items: flex-start; flex-direction: column; gap: 10px; }
  :deep(.ah-embedded-view .ah-thead) { display: none; }
  :deep(.ah-embedded-view .ah-row) { grid-template-columns: 1fr auto; }
}
</style>
