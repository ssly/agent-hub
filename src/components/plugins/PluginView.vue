<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Folder, Copy, ExternalLink } from 'lucide-vue-next'
import { useToast } from '@/composables/useToast'
import { usePluginsStore } from '@/stores/plugins'
import { useSkillsStore } from '@/stores/skills'
import { useMcpStore } from '@/stores/mcp'
import { useClaudePluginsStore } from '@/stores/claude-plugins'
import { useZCodePluginsStore } from '@/stores/zcode-plugins'
import { useQwenPluginsStore } from '@/stores/qwen-plugins'
import SkillListView from '@/components/skills/SkillListView.vue'
import McpListView from '@/components/mcp/McpListView.vue'
import ClaudePluginList from '@/components/plugins/ClaudePluginList.vue'
import ZCodePluginList from '@/components/plugins/ZCodePluginList.vue'
import QwenPluginList from '@/components/plugins/QwenPluginList.vue'

const { t } = useI18n()
const { showToast } = useToast()
const pluginsStore = usePluginsStore()
const skillsStore = useSkillsStore()
const mcpStore = useMcpStore()
const claudePluginsStore = useClaudePluginsStore()
const zcodePluginsStore = useZCodePluginsStore()
const qwenPluginsStore = useQwenPluginsStore()

const skillCount = computed(() => skillsStore.skills.length)
const serverCount = computed(() => mcpStore.servers.length)
const isClaudeCode = computed(() => pluginsStore.selectedPlatformId === 'claude-code')
const isCodex = computed(() => pluginsStore.selectedPlatformId === 'codex')
const isZCode = computed(() => pluginsStore.selectedPlatformId === 'zcode')
const isQwen = computed(() => pluginsStore.selectedPlatformId === 'qwen')
const isShared = computed(() => pluginsStore.selectedPlatformId === 'shared')

interface SharedPlatformInfo {
  id: string
  name: string
  projectOnly?: boolean
}

// Platforms supporting .agents/skills in official registry order
const SHARED_PLATFORMS: SharedPlatformInfo[] = [
  { id: 'codex', name: 'Codex' },
  { id: 'cursor', name: 'Cursor' },
  { id: 'antigravity', name: 'Antigravity', projectOnly: true },
  { id: 'grok-build', name: 'Grok Build' },
  { id: 'kimi-code', name: 'Kimi Code' },
  { id: 'qwen', name: 'Qwen Code' },
  { id: 'zcode', name: 'ZCode' },
  { id: 'dsh', name: 'DeepSeek Harness' },
]

const currentSharedPlatform = computed(() =>
  SHARED_PLATFORMS.find(p => p.id === pluginsStore.selectedPlatformId)
)

const showMcpSection = computed(() => Boolean(pluginsStore.selectedPlatform?.supports_mcp)
  && (pluginsStore.isGlobalScope || serverCount.value > 0))
const showClaudeSection = computed(() => isClaudeCode.value
  && (pluginsStore.isGlobalScope || claudePluginsStore.plugins.length > 0))
// ZCode 插件市场只有用户级数据，项目目录范围不展示该区块。
const showZCodeSection = computed(() => isZCode.value && pluginsStore.isGlobalScope)
// Qwen Code 扩展同理：只有用户级（~/.qwen/extensions）。
const showQwenSection = computed(() => isQwen.value && pluginsStore.isGlobalScope)
// Codex officially keeps user-level skills in Shared; show a jump
// note instead of a duplicated skills section.
const showSkillsSection = computed(() => !isCodex.value
  && (pluginsStore.isGlobalScope || skillCount.value > 0))
const platformTitle = computed(() => {
  const p = pluginsStore.selectedPlatform
  if (!p) return ''
  return p.id === 'shared' ? t('plugin.platform_shared') : p.display_name
})

function shortenPath(path: string): string {
  if (!path) return ''
  return path.replace(/^(\/Users\/[^/]+|C:\\Users\\[^\\]+)/, '~')
}

async function copyPath(path: string) {
  if (!path) return
  try {
    await navigator.clipboard.writeText(path)
    showToast(t('plugin.path_copied'), 'success')
  } catch {
    // fallback
  }
}

function jumpToShared() {
  pluginsStore.selectPlatform('shared')
}

function loadCollapsedPanes(): Set<string> {
  try {
    const raw = localStorage.getItem('ah-plugin-collapsed-panes')
    if (raw) {
      const arr = JSON.parse(raw)
      if (Array.isArray(arr)) return new Set(arr)
    }
  } catch {}
  return new Set()
}

function saveCollapsedPanes(panes: Set<string>) {
  try {
    localStorage.setItem('ah-plugin-collapsed-panes', JSON.stringify(Array.from(panes)))
  } catch {}
}

const collapsedPanes = ref<Set<string>>(loadCollapsedPanes())

function getPaneKey(section: string): string {
  return `${pluginsStore.selectedPlatformId || 'default'}:${section}`
}

function isPaneCollapsed(section: string): boolean {
  return collapsedPanes.value.has(getPaneKey(section))
}

function togglePane(section: string) {
  const key = getPaneKey(section)
  if (collapsedPanes.value.has(key)) {
    collapsedPanes.value.delete(key)
  } else {
    collapsedPanes.value.add(key)
  }
  saveCollapsedPanes(collapsedPanes.value)
}
</script>

<template>
  <div class="ah-plugin-view view-enter">
    <div v-if="!pluginsStore.selectedPlatform" class="ah-plugin-empty">
      <p>{{ t('plugin.no_agents') }}</p>
    </div>

    <template v-else>
      <header class="ah-plugin-header">
        <div class="min-w-0">
          <h1 class="ah-page-title truncate">{{ platformTitle }}</h1>
          <div v-if="isShared" class="ah-plugin-shared-agents">
            <div class="ah-plugin-shared-agents__list">
              <button
                v-for="agent in SHARED_PLATFORMS"
                :key="agent.id"
                type="button"
                class="ah-plugin-shared-agent-badge"
                v-tooltip="agent.projectOnly ? t('plugin.shared_skills_banner_desc_antigravity') : t('plugin.shared_skills_banner_desc')"
                @click="pluginsStore.selectPlatform(agent.id)"
              >
                <span>{{ agent.name }}</span>
                <span v-if="agent.projectOnly" class="ah-plugin-shared-agent-tag">{{ t('plugin.project_only_tag') }}</span>
              </button>
            </div>
          </div>
        </div>
      </header>

      <div class="ah-plugin-grid">
        <!-- MCP Section -->
        <section
          v-if="showMcpSection"
          class="ah-plugin-pane"
          :class="{ 'is-collapsed': isPaneCollapsed('mcp') }"
          aria-labelledby="plugin-mcp-heading"
        >
          <div
            class="ah-plugin-pane__header ah-plugin-pane__header--collapsible"
            @click="togglePane('mcp')"
          >
            <div class="flex items-center gap-2 min-w-0 flex-1">
              <h2 id="plugin-mcp-heading" class="shrink-0">{{ t('plugin.mcp') }}</h2>
              <button
                v-if="pluginsStore.selectedPlatform.config_path"
                type="button"
                class="ah-plugin-path-pill"
                v-tooltip="`${pluginsStore.selectedPlatform.config_path} · ${t('plugin.copy_path')}`"
                @click.stop="copyPath(pluginsStore.selectedPlatform.config_path)"
              >
                <Folder :size="12" class="shrink-0 opacity-60" />
                <span class="truncate">{{ shortenPath(pluginsStore.selectedPlatform.config_path) }}</span>
                <Copy :size="10" class="ah-plugin-path-pill__copy shrink-0" />
              </button>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <span class="ah-plugin-count">{{ serverCount }}</span>
              <button
                v-if="pluginsStore.selectedPlatform.supports_mcp && pluginsStore.isGlobalScope"
                class="btn btn-primary btn-sm"
                @click.stop="mcpStore.addModalOpen = true"
              >+ {{ t('mcp.add') }}</button>
              <button
                type="button"
                class="ah-plugin-pane__toggle"
                :aria-label="isPaneCollapsed('mcp') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                :title="isPaneCollapsed('mcp') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                @click.stop="togglePane('mcp')"
              >
                <svg
                  class="ah-plugin-pane__chevron"
                  :class="{ 'is-collapsed': isPaneCollapsed('mcp') }"
                  width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                  stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
          <div v-show="!isPaneCollapsed('mcp')" class="ah-plugin-pane__body ah-plugin-pane__body--mcp">
            <McpListView embedded :readonly="!pluginsStore.isGlobalScope" />
          </div>
        </section>

        <!-- Claude Code Plugins Section -->
        <section
          v-if="showClaudeSection"
          class="ah-plugin-pane"
          :class="{ 'is-collapsed': isPaneCollapsed('claude_plugins') }"
          aria-labelledby="plugin-claude-heading"
        >
          <div
            class="ah-plugin-pane__header ah-plugin-pane__header--collapsible"
            @click="togglePane('claude_plugins')"
          >
            <div class="flex items-center gap-2 min-w-0 flex-1">
              <h2 id="plugin-claude-heading" class="shrink-0">{{ t('plugin.claude_plugins') }}</h2>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <span
                class="ah-plugin-count"
                :title="t('plugin.claude_enabled_count', { enabled: claudePluginsStore.enabledCount, total: claudePluginsStore.plugins.length })"
              >{{ claudePluginsStore.enabledCount }}/{{ claudePluginsStore.plugins.length }}</span>
              <button
                type="button"
                class="ah-plugin-pane__toggle"
                :aria-label="isPaneCollapsed('claude_plugins') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                :title="isPaneCollapsed('claude_plugins') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                @click.stop="togglePane('claude_plugins')"
              >
                <svg
                  class="ah-plugin-pane__chevron"
                  :class="{ 'is-collapsed': isPaneCollapsed('claude_plugins') }"
                  width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                  stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
          <div v-show="!isPaneCollapsed('claude_plugins')" class="ah-plugin-pane__body">
            <ClaudePluginList />
          </div>
        </section>

        <!-- ZCode Plugins Section -->
        <section
          v-if="showZCodeSection"
          class="ah-plugin-pane"
          :class="{ 'is-collapsed': isPaneCollapsed('zcode_plugins') }"
          aria-labelledby="plugin-zcode-heading"
        >
          <div
            class="ah-plugin-pane__header ah-plugin-pane__header--collapsible"
            @click="togglePane('zcode_plugins')"
          >
            <div class="flex items-center gap-2 min-w-0 flex-1">
              <h2 id="plugin-zcode-heading" class="shrink-0">{{ t('plugin.zcode_plugins') }}</h2>
              <span
                class="ah-plugin-readonly-badge"
                v-tooltip="t('plugin.zcode_readonly_tooltip')"
              >
                {{ t('plugin.claude_read_only') }}
              </span>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <span
                class="ah-plugin-count"
                :title="t('plugin.zcode_installed_count', { installed: zcodePluginsStore.installedCount, total: zcodePluginsStore.plugins.length })"
              >{{ zcodePluginsStore.installedCount }}/{{ zcodePluginsStore.plugins.length }}</span>
              <button
                type="button"
                class="ah-plugin-pane__toggle"
                :aria-label="isPaneCollapsed('zcode_plugins') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                :title="isPaneCollapsed('zcode_plugins') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                @click.stop="togglePane('zcode_plugins')"
              >
                <svg
                  class="ah-plugin-pane__chevron"
                  :class="{ 'is-collapsed': isPaneCollapsed('zcode_plugins') }"
                  width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                  stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
          <div v-show="!isPaneCollapsed('zcode_plugins')" class="ah-plugin-pane__body">
            <ZCodePluginList />
          </div>
        </section>

        <!-- Qwen Code Extensions Section -->
        <section
          v-if="showQwenSection"
          class="ah-plugin-pane"
          :class="{ 'is-collapsed': isPaneCollapsed('qwen_plugins') }"
          aria-labelledby="plugin-qwen-heading"
        >
          <div
            class="ah-plugin-pane__header ah-plugin-pane__header--collapsible"
            @click="togglePane('qwen_plugins')"
          >
            <div class="flex items-center gap-2 min-w-0 flex-1">
              <h2 id="plugin-qwen-heading" class="shrink-0">{{ t('plugin.qwen_plugins') }}</h2>
              <span
                class="ah-plugin-readonly-badge"
                v-tooltip="t('plugin.qwen_readonly_tooltip')"
              >
                {{ t('plugin.claude_read_only') }}
              </span>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <span class="ah-plugin-count">{{ qwenPluginsStore.plugins.length }}</span>
              <button
                type="button"
                class="ah-plugin-pane__toggle"
                :aria-label="isPaneCollapsed('qwen_plugins') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                :title="isPaneCollapsed('qwen_plugins') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                @click.stop="togglePane('qwen_plugins')"
              >
                <svg
                  class="ah-plugin-pane__chevron"
                  :class="{ 'is-collapsed': isPaneCollapsed('qwen_plugins') }"
                  width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                  stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
          <div v-show="!isPaneCollapsed('qwen_plugins')" class="ah-plugin-pane__body">
            <QwenPluginList />
          </div>
        </section>

        <!-- Codex Special Skills Banner -->
        <section v-if="isCodex" class="ah-plugin-codex-banner">
          <div class="flex items-center gap-2.5 min-w-0">
            <span class="ah-plugin-codex-banner__title">{{ t('plugin.skills') }}</span>
            <span class="ah-plugin-codex-banner__desc truncate">{{ t('plugin.codex_shared_hint') }}</span>
          </div>
          <button class="btn btn-secondary btn-sm shrink-0 flex items-center gap-1.5" @click="jumpToShared">
            <span>{{ t('plugin.jump_to_pool') }}</span>
            <ExternalLink :size="12" />
          </button>
        </section>

        <!-- Skills Section -->
        <section
          v-if="showSkillsSection"
          class="ah-plugin-pane"
          :class="{ 'is-collapsed': isPaneCollapsed('skills') }"
          aria-labelledby="plugin-skills-heading"
        >
          <div
            class="ah-plugin-pane__header ah-plugin-pane__header--collapsible"
            @click="togglePane('skills')"
          >
            <div class="flex items-center gap-2 min-w-0 flex-1">
              <h2 id="plugin-skills-heading" class="shrink-0">{{ t('plugin.skills') }}</h2>
              <button
                v-if="pluginsStore.selectedPlatform.skill_dir"
                type="button"
                class="ah-plugin-path-pill"
                v-tooltip="`${pluginsStore.selectedPlatform.skill_dir} · ${t('plugin.copy_path')}`"
                @click.stop="copyPath(pluginsStore.selectedPlatform.skill_dir)"
              >
                <Folder :size="12" class="shrink-0 opacity-60" />
                <span class="truncate">{{ shortenPath(pluginsStore.selectedPlatform.skill_dir) }}</span>
                <Copy :size="10" class="ah-plugin-path-pill__copy shrink-0" />
              </button>
              <button
                v-if="currentSharedPlatform"
                type="button"
                class="ah-plugin-shared-link"
                v-tooltip="currentSharedPlatform.projectOnly ? t('plugin.shared_skills_banner_desc_antigravity') : t('plugin.shared_skills_tooltip')"
                @click.stop="jumpToShared"
              >
                <span>{{ t('plugin.shared_skills_link') }}</span>
                <ExternalLink :size="11" />
              </button>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <span class="ah-plugin-count">{{ skillCount }}</span>
              <button
                type="button"
                class="ah-plugin-pane__toggle"
                :aria-label="isPaneCollapsed('skills') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                :title="isPaneCollapsed('skills') ? t('plugin.expand_pane') : t('plugin.collapse_pane')"
                @click.stop="togglePane('skills')"
              >
                <svg
                  class="ah-plugin-pane__chevron"
                  :class="{ 'is-collapsed': isPaneCollapsed('skills') }"
                  width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                  stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
          <div v-show="!isPaneCollapsed('skills')" class="ah-plugin-pane__body">
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
  padding: 20px 24px 24px;
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
  margin-bottom: 14px;
}
.ah-plugin-path-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 260px;
  padding: 2px 7px;
  border-radius: var(--radius-sm);
  background: var(--sunken);
  border: 1px solid var(--hairline);
  color: var(--ink-3);
  font-family: var(--font-mono);
  font-size: 11px;
  cursor: pointer;
  transition: all var(--dur-fast) var(--ease-soft);
  min-width: 0;
}
.ah-plugin-path-pill:hover {
  background: var(--hover);
  color: var(--ink);
  border-color: var(--border);
}
.ah-plugin-path-pill__copy {
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-soft);
}
.ah-plugin-path-pill:hover .ah-plugin-path-pill__copy {
  opacity: 0.8;
}
.ah-plugin-shared-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--accent) 9%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent) 22%, transparent);
  color: var(--accent);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: all var(--dur-fast) var(--ease-soft);
  flex-shrink: 0;
}
.ah-plugin-shared-link:hover {
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  border-color: var(--accent);
}
.ah-plugin-readonly-badge {
  display: inline-flex;
  align-items: center;
  padding: 1px 6px;
  border-radius: var(--radius-pill);
  background: var(--sunken);
  border: 1px solid var(--hairline);
  color: var(--ink-4);
  font-size: 10.5px;
  cursor: help;
  flex-shrink: 0;
}
.ah-plugin-codex-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 16px;
  border-radius: var(--radius-md);
  background: var(--surface);
  border: 1px solid var(--hairline);
  box-shadow: var(--shadow-mist);
}
.ah-plugin-codex-banner__title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--ink);
  flex-shrink: 0;
}
.ah-plugin-codex-banner__desc {
  font-size: 12.5px;
  color: var(--ink-3);
}
.ah-plugin-count {
  border: 1px solid var(--hairline);
  border-radius: var(--radius-pill);
  background: var(--surface);
  color: var(--ink-3);
  font-size: 12px;
  font-family: var(--font-mono);
  padding: 2px 8px;
}
.ah-plugin-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 12px;
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
  min-height: 46px;
  padding: 9px 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border-bottom: 1px solid var(--hairline);
}
.ah-plugin-pane.is-collapsed .ah-plugin-pane__header {
  border-bottom: none;
}
.ah-plugin-pane__header--collapsible {
  cursor: pointer;
  user-select: none;
  transition: background-color 0.15s ease;
}
.ah-plugin-pane__header--collapsible:hover {
  background: var(--hover);
}
.ah-plugin-pane__toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  border: 1px solid transparent;
  background: transparent;
  color: var(--ink-4);
  cursor: pointer;
  transition: all 0.15s ease;
  padding: 0;
  flex-shrink: 0;
}
.ah-plugin-pane__toggle:hover {
  background: var(--surface);
  border-color: var(--border);
  color: var(--ink);
}
.ah-plugin-pane__chevron {
  transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}
.ah-plugin-pane__chevron.is-collapsed {
  transform: rotate(-90deg);
}
.ah-plugin-pane__header h2 { font-size: 13.5px; font-weight: 600; color: var(--ink); margin: 0; }
.ah-plugin-pane__body { min-width: 0; }
.ah-plugin-shared-agents {
  margin-top: 8px;
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 12px;
}
.ah-plugin-shared-agents__list {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}
.ah-plugin-shared-agent-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2.5px 9px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  color: var(--ink-2);
  font-size: 11.5px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}
.ah-plugin-shared-agent-badge:hover {
  background: var(--hover);
  border-color: var(--border-strong);
  color: var(--ink);
}
.ah-plugin-shared-agent-tag {
  font-size: 10px;
  padding: 0 4px;
  line-height: 16px;
  background: var(--sunken);
  border: 1px solid var(--hairline);
  border-radius: 2px;
  color: var(--ink-4);
}

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
