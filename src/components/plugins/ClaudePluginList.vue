<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useToast } from '@/composables/useToast'
import { useClaudePluginsStore, type ClaudeCodePlugin } from '@/stores/claude-plugins'

const { t } = useI18n()
const { showToast } = useToast()
const store = useClaudePluginsStore()

function scopeLabel(scope: string) {
  const key = `plugin.claude_scope_${scope}`
  const translated = t(key)
  return translated === key ? scope : translated
}

async function handleToggle(plugin: ClaudeCodePlugin) {
  if (!plugin.manageable || store.togglingIds.has(plugin.id)) return
  const nextEnabled = !plugin.enabled
  try {
    await store.setPluginEnabled(plugin, nextEnabled)
    showToast(
      t(nextEnabled ? 'plugin.claude_enabled_toast' : 'plugin.claude_disabled_toast', { name: plugin.name }),
      'success',
    )
  } catch (e: any) {
    showToast(t('plugin.claude_toggle_failed', { error: e?.message || String(e) }), 'error')
  }
}
</script>

<template>
  <div class="ah-cc-plugins">
    <div v-if="store.loading && store.plugins.length === 0" class="ah-cc-plugins__state loading-pulse">
      {{ t('plugin.claude_loading') }}
    </div>

    <div v-else-if="store.error" class="ah-cc-plugins__state">
      <span>{{ t('plugin.claude_load_failed', { error: store.error }) }}</span>
      <button class="btn btn-secondary btn-sm" @click="store.loadPlugins()">{{ t('action.refresh') }}</button>
    </div>

    <div v-else-if="store.plugins.length === 0" class="ah-cc-plugins__state">
      {{ t(store.workspaceDirectory ? 'plugin.claude_project_empty' : 'plugin.claude_empty') }}
    </div>

    <div v-else class="ah-cc-plugin-list">
      <article
        v-for="plugin in store.plugins"
        :key="plugin.id"
        :class="['ah-cc-plugin', plugin.enabled ? 'is-enabled' : 'is-disabled']"
      >
        <span class="ah-cc-plugin__status" aria-hidden="true"></span>
        <div class="ah-cc-plugin__main">
          <div class="ah-cc-plugin__title-row">
            <strong class="ah-cc-plugin__name">{{ plugin.name }}</strong>
            <span v-if="plugin.version && plugin.version !== 'unknown'" class="ah-version-chip">v{{ plugin.version }}</span>
          </div>
          <p v-if="plugin.description" class="ah-cc-plugin__description">{{ plugin.description }}</p>
          <div class="ah-cc-plugin__meta">
            <span v-if="plugin.marketplace">{{ plugin.marketplace }}</span>
            <span class="ah-cc-plugin__scope">{{ scopeLabel(plugin.scope) }}</span>
            <span v-if="!plugin.manageable" :title="t('plugin.claude_scope_read_only')">
              {{ t('plugin.claude_read_only') }}
            </span>
          </div>
        </div>
        <button
          type="button"
          role="switch"
          class="ah-plugin-switch"
          :class="{ 'is-on': plugin.enabled, 'is-busy': store.togglingIds.has(plugin.id) }"
          :aria-checked="plugin.enabled"
          :aria-label="t(plugin.enabled ? 'plugin.claude_disable' : 'plugin.claude_enable', { name: plugin.name })"
          :title="plugin.manageable
            ? t(plugin.enabled ? 'plugin.claude_disable' : 'plugin.claude_enable', { name: plugin.name })
            : t('plugin.claude_scope_read_only')"
          :disabled="!plugin.manageable || store.togglingIds.has(plugin.id)"
          @click="handleToggle(plugin)"
        >
          <span class="ah-plugin-switch__thumb"></span>
        </button>
      </article>
    </div>
  </div>
</template>

<style scoped>
.ah-cc-plugins__state {
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
.ah-cc-plugin-list { display: grid; }
.ah-cc-plugin {
  min-width: 0;
  min-height: 74px;
  padding: 11px 16px;
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr) auto;
  align-items: center;
  gap: 11px;
  border-bottom: 1px solid var(--hairline);
  transition: background var(--dur-fast) var(--ease-soft), opacity var(--dur-fast) var(--ease-soft);
}
.ah-cc-plugin:last-child { border-bottom: 0; }
.ah-cc-plugin:hover { background: var(--hover); }
.ah-cc-plugin.is-disabled { opacity: .68; }
.ah-cc-plugin__status {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--ink-4);
}
.ah-cc-plugin.is-enabled .ah-cc-plugin__status { background: var(--success); }
.ah-cc-plugin__main { min-width: 0; }
.ah-cc-plugin__title-row { display: flex; align-items: baseline; gap: 7px; min-width: 0; }
.ah-cc-plugin__name {
  min-width: 0;
  color: var(--ink);
  font-size: 13.5px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ah-cc-plugin__description {
  margin-top: 2px;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  color: var(--ink-2);
  font-size: 12.5px;
  line-height: 1.4;
}
.ah-cc-plugin__meta {
  margin-top: 4px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  color: var(--ink-4);
  font-family: var(--font-mono);
  font-size: 10.5px;
}
.ah-cc-plugin__scope {
  padding: 0 5px;
  border: 1px solid var(--hairline);
  border-radius: var(--radius-pill);
  background: var(--sunken);
}
.ah-plugin-switch {
  position: relative;
  width: 34px;
  height: 20px;
  flex-shrink: 0;
  border: 0;
  border-radius: var(--radius-pill);
  background: var(--border);
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-soft), opacity var(--dur-fast) var(--ease-soft);
}
.ah-plugin-switch.is-on { background: var(--accent); }
.ah-plugin-switch:disabled { cursor: not-allowed; opacity: .55; }
.ah-plugin-switch.is-busy { opacity: .55; }
.ah-plugin-switch__thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--surface);
  box-shadow: 0 1px 2px rgba(0, 0, 0, .18);
  transition: transform var(--dur-fast) var(--ease-soft);
}
.ah-plugin-switch.is-on .ah-plugin-switch__thumb { transform: translateX(14px); }

@media (max-width: 720px) {
  .ah-cc-plugin { padding-inline: 12px; }
}
</style>
