<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useZcodePluginsStore, type ZcodePlugin } from '@/stores/zcode-plugins'
import AppLoading from '@/components/ui/AppLoading.vue'

const { t } = useI18n()
const store = useZcodePluginsStore()

function componentsLabel(plugin: ZcodePlugin) {
  const parts: string[] = []
  if (plugin.skill_count > 0) parts.push(t('plugin.zcode_component_skills', { count: plugin.skill_count }))
  if (plugin.command_count > 0) parts.push(t('plugin.zcode_component_commands', { count: plugin.command_count }))
  if (plugin.hook_count > 0) parts.push(t('plugin.zcode_component_hooks', { count: plugin.hook_count }))
  return parts.join(' · ')
}
</script>

<template>
  <div class="ah-zc-plugins">
    <div v-if="store.loading && store.plugins.length === 0" class="ah-zc-plugins__state">
      <AppLoading :size="40">{{ t('plugin.zcode_loading') }}</AppLoading>
    </div>

    <div v-else-if="store.error" class="ah-zc-plugins__state">
      <span>{{ t('plugin.zcode_load_failed', { error: store.error }) }}</span>
      <button class="btn btn-secondary btn-sm" @click="store.loadPlugins()">{{ t('action.refresh') }}</button>
    </div>

    <div v-else-if="store.plugins.length === 0" class="ah-zc-plugins__state">
      {{ t('plugin.zcode_empty') }}
    </div>

    <template v-else>
      <div class="ah-zc-plugin-list">
        <article
          v-for="plugin in store.plugins"
          :key="plugin.id"
          :class="['ah-zc-plugin', plugin.installed ? 'is-installed' : 'is-missing']"
        >
          <span class="ah-zc-plugin__status" aria-hidden="true"></span>
          <div class="ah-zc-plugin__main">
            <div class="ah-zc-plugin__title-row">
              <strong class="ah-zc-plugin__name">{{ plugin.name }}</strong>
              <span v-if="plugin.version" class="ah-version-chip">v{{ plugin.version }}</span>
              <span
                :class="['ah-zc-plugin__badge', plugin.installed ? 'is-installed' : 'is-missing']"
                :title="t('plugin.zcode_installed_hint')"
              >{{ t(plugin.installed ? 'plugin.zcode_installed' : 'plugin.zcode_not_installed') }}</span>
            </div>
            <p v-if="plugin.description" class="ah-zc-plugin__description">{{ plugin.description }}</p>
            <div class="ah-zc-plugin__meta">
              <span v-if="plugin.marketplace">{{ plugin.marketplace }}</span>
              <span v-if="plugin.author">{{ plugin.author }}</span>
              <span v-if="componentsLabel(plugin)">{{ componentsLabel(plugin) }}</span>
            </div>
          </div>
        </article>
      </div>
      <p class="ah-zc-plugins__note">{{ t('plugin.zcode_readonly_note') }}</p>
    </template>
  </div>
</template>

<style scoped>
.ah-zc-plugins__state {
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
.ah-zc-plugin-list { display: grid; }
.ah-zc-plugin {
  min-width: 0;
  min-height: 74px;
  padding: 11px 16px;
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr);
  align-items: center;
  gap: 11px;
  border-bottom: 1px solid var(--hairline);
  transition: background var(--dur-fast) var(--ease-soft), opacity var(--dur-fast) var(--ease-soft);
}
.ah-zc-plugin:hover { background: var(--hover); }
.ah-zc-plugin.is-missing { opacity: .68; }
.ah-zc-plugin__status {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--ink-4);
}
.ah-zc-plugin.is-installed .ah-zc-plugin__status { background: var(--success); }
.ah-zc-plugin__main { min-width: 0; }
.ah-zc-plugin__title-row { display: flex; align-items: baseline; gap: 7px; min-width: 0; }
.ah-zc-plugin__name {
  min-width: 0;
  color: var(--ink);
  font-size: 13.5px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ah-zc-plugin__badge {
  flex-shrink: 0;
  padding: 0 5px;
  border: 1px solid var(--hairline);
  border-radius: var(--radius-pill);
  background: var(--sunken);
  color: var(--ink-4);
  font-size: 10.5px;
}
.ah-zc-plugin__badge.is-installed { color: var(--success); }
.ah-zc-plugin__description {
  margin-top: 2px;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  color: var(--ink-2);
  font-size: 12.5px;
  line-height: 1.4;
}
.ah-zc-plugin__meta {
  margin-top: 4px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  color: var(--ink-4);
  font-family: var(--font-mono);
  font-size: 10.5px;
}
.ah-zc-plugins__note {
  padding: 9px 16px 11px;
  color: var(--ink-4);
  font-size: 11.5px;
  line-height: 1.55;
}

@media (max-width: 720px) {
  .ah-zc-plugin { padding-inline: 12px; }
}
</style>
