<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useQwenPluginsStore, type QwenPlugin } from '@/stores/qwen-plugins'
import AppLoading from '@/components/ui/AppLoading.vue'

const { t } = useI18n()
const store = useQwenPluginsStore()

function componentsLabel(plugin: QwenPlugin) {
  const parts: string[] = []
  if (plugin.mcpServerCount > 0) parts.push(t('plugin.qwen_component_mcp', { count: plugin.mcpServerCount }))
  if (plugin.skillCount > 0) parts.push(t('plugin.qwen_component_skills', { count: plugin.skillCount }))
  if (plugin.commandCount > 0) parts.push(t('plugin.qwen_component_commands', { count: plugin.commandCount }))
  if (plugin.agentCount > 0) parts.push(t('plugin.qwen_component_agents', { count: plugin.agentCount }))
  return parts.join(' · ')
}
</script>

<template>
  <div class="ah-qwen-plugins">
    <div v-if="store.loading && store.plugins.length === 0" class="ah-qwen-plugins__state">
      <AppLoading :size="40">{{ t('plugin.qwen_loading') }}</AppLoading>
    </div>

    <div v-else-if="store.error" class="ah-qwen-plugins__state">
      <span>{{ t('plugin.qwen_load_failed', { error: store.error }) }}</span>
      <button class="btn btn-secondary btn-sm" @click="store.loadPlugins()">{{ t('action.refresh') }}</button>
    </div>

    <div v-else-if="store.plugins.length === 0" class="ah-qwen-plugins__state">
      {{ t('plugin.qwen_empty') }}
    </div>

    <template v-else>
      <div class="ah-qwen-plugin-list">
        <article
          v-for="plugin in store.plugins"
          :key="plugin.id"
          class="ah-qwen-plugin"
        >
          <div class="ah-qwen-plugin__main">
            <div class="ah-qwen-plugin__title-row">
              <strong class="ah-qwen-plugin__name">{{ plugin.name }}</strong>
              <span v-if="plugin.version" class="ah-version-chip">v{{ plugin.version }}</span>
            </div>
            <p v-if="plugin.description" class="ah-qwen-plugin__description">{{ plugin.description }}</p>
            <div class="ah-qwen-plugin__meta">
              <span v-if="componentsLabel(plugin)">{{ componentsLabel(plugin) }}</span>
              <span v-if="plugin.installPath">{{ plugin.installPath }}</span>
            </div>
          </div>
        </article>
      </div>
      <p class="ah-qwen-plugins__note">{{ t('plugin.qwen_readonly_note') }}</p>
    </template>
  </div>
</template>

<style scoped>
.ah-qwen-plugins__state {
  min-height: 76px;
  padding: 18px 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--ink-3);
  font-size: 13px;
  text-align: center;
}
.ah-qwen-plugin-list { display: grid; }
.ah-qwen-plugin {
  min-width: 0;
  min-height: 74px;
  padding: 11px 16px;
  display: flex;
  align-items: center;
  border-bottom: 1px solid var(--hairline);
  transition: background var(--dur-fast) var(--ease-soft);
}
.ah-qwen-plugin:hover { background: var(--hover); }
.ah-qwen-plugin__main { min-width: 0; }
.ah-qwen-plugin__title-row { display: flex; align-items: baseline; gap: 7px; min-width: 0; }
.ah-qwen-plugin__name {
  min-width: 0;
  color: var(--ink);
  font-size: 13.5px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ah-qwen-plugin__description {
  margin-top: 2px;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  color: var(--ink-2);
  font-size: 12.5px;
  line-height: 1.4;
}
.ah-qwen-plugin__meta {
  margin-top: 4px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  color: var(--ink-4);
  font-family: var(--font-mono);
  font-size: 10.5px;
}
.ah-qwen-plugins__note {
  padding: 9px 16px 11px;
  color: var(--ink-4);
  font-size: 11.5px;
  line-height: 1.55;
}

@media (max-width: 720px) {
  .ah-qwen-plugin { padding-inline: 12px; }
}
</style>
