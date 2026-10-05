<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Folder, Copy, ExternalLink, CircleHelp, Search } from 'lucide-vue-next'
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

interface ExternalSkillRef {
  id: string
  name: string
  projectOnly?: boolean
}

// Map of agents to the external skill directories they natively support reading
const EXTERNAL_SKILL_REFS: Record<string, ExternalSkillRef[]> = {
  opencode: [
    { id: 'codex', name: 'Codex' },
    { id: 'claude-code', name: 'Claude Code' },
  ],
  omp: [
    { id: 'claude-code', name: 'Claude Code' },
    { id: 'cursor', name: 'Cursor' },
  ],
  cursor: [
    { id: 'codex', name: 'Codex' },
  ],
  antigravity: [
    { id: 'codex', name: 'Codex', projectOnly: true },
  ],
  'grok-build': [
    { id: 'codex', name: 'Codex' },
  ],
  'kimi-code': [
    { id: 'codex', name: 'Codex' },
  ],
  qwen: [
    { id: 'codex', name: 'Codex' },
  ],
  zcode: [
    { id: 'codex', name: 'Codex' },
  ],
  dsh: [
    { id: 'codex', name: 'Codex' },
  ],
}

// Agents that natively read from Codex's ~/.agents/skills
const CODEX_CONSUMING_AGENTS: ExternalSkillRef[] = [
  { id: 'cursor', name: 'Cursor' },
  { id: 'antigravity', name: 'Antigravity', projectOnly: true },
  { id: 'grok-build', name: 'Grok Build' },
  { id: 'kimi-code', name: 'Kimi Code' },
  { id: 'qwen', name: 'Qwen Code' },
  { id: 'zcode', name: 'ZCode' },
  { id: 'dsh', name: 'DeepSeek Harness' },
  { id: 'opencode', name: 'OpenCode' },
]

const currentExternalSkillRefs = computed(() => {
  const currentId = pluginsStore.selectedPlatformId
  if (!currentId) return []
  return EXTERNAL_SKILL_REFS[currentId] || []
})

const showMcpSection = computed(() => Boolean(pluginsStore.selectedPlatform?.supports_mcp)
  && (pluginsStore.isGlobalScope || serverCount.value > 0))
const showClaudeSection = computed(() => isClaudeCode.value
  && (pluginsStore.isGlobalScope || claudePluginsStore.plugins.length > 0))
// ZCode 插件市场只有用户级数据，项目目录范围不展示该区块。
const showZCodeSection = computed(() => isZCode.value && pluginsStore.isGlobalScope)
// Qwen Code 扩展同理：只有用户级（~/.qwen/extensions）。
const showQwenSection = computed(() => isQwen.value && pluginsStore.isGlobalScope)

const showSkillsSection = computed(() => Boolean(pluginsStore.selectedPlatform?.supports_skills)
  && (pluginsStore.isGlobalScope || skillCount.value > 0))

const platformTitle = computed(() => pluginsStore.selectedPlatform?.display_name || '')

const currentPlatformMatches = computed(() => {
  if (!pluginsStore.searchQuery || !pluginsStore.selectedPlatformId) return 0
  return pluginsStore.platformMatchCounts[pluginsStore.selectedPlatformId] || 0
})

const totalMatchesAcrossAll = computed(() => {
  if (!pluginsStore.searchQuery) return 0
  return pluginsStore.searchResults.length
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

function jumpToPlatform(id: string) {
  pluginsStore.selectPlatform(id)
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
          <div v-if="isCodex" class="ah-plugin-shared-agents">
            <div class="ah-plugin-shared-agents__list">
              <button
                v-for="agent in CODEX_CONSUMING_AGENTS"
                :key="agent.id"
                type="button"
                class="ah-plugin-shared-agent-badge"
                v-tooltip="agent.projectOnly ? t('plugin.shared_skills_project_only_tooltip') : t('plugin.shared_skills_banner_desc')"
                @click="pluginsStore.selectPlatform(agent.id)"
              >
                <span>{{ agent.name }}</span>
                <CircleHelp v-if="agent.projectOnly" :size="11" class="ah-plugin-shared-agent-hint" />
              </button>
            </div>
          </div>
        </div>
      </header>

      <!-- Search Active Banner -->
      <div v-if="pluginsStore.searchQuery" class="ah-plugin-search-banner">
        <div class="flex items-center gap-2 min-w-0">
          <Search :size="13" class="ah-plugin-search-banner__icon shrink-0" />
          <span class="text-xs font-medium truncate">
            {{ t('plugin.search_active_banner', { query: pluginsStore.searchQuery }) }}
          </span>
          <span class="ah-plugin-search-banner__tag shrink-0">
            {{ currentPlatformMatches > 0
                ? t('plugin.search_matches_count', { count: currentPlatformMatches })
                : (totalMatchesAcrossAll > 0
                    ? t('plugin.search_no_matches_in_platform', { query: pluginsStore.searchQuery })
                    : t('plugin.search_no_matches_total'))
            }}
          </span>
        </div>
        <button
          type="button"
          class="btn btn-secondary btn-xs shrink-0"
          @click="pluginsStore.clearSearch()"
        >
          {{ t('plugin.search_clear') }}
        </button>
      </div>

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
                <Folder :size="12" class="ah-plugin-path-pill__folder shrink-0" />
                <span class="truncate">{{ shortenPath(pluginsStore.selectedPlatform.config_path) }}</span>
                <Copy :size="10.5" class="ah-plugin-path-pill__copy shrink-0" />
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
                <Folder :size="12" class="ah-plugin-path-pill__folder shrink-0" />
                <span class="truncate">{{ shortenPath(pluginsStore.selectedPlatform.skill_dir) }}</span>
                <Copy :size="10.5" class="ah-plugin-path-pill__copy shrink-0" />
              </button>
              <button
                v-for="ext in currentExternalSkillRefs"
                :key="ext.id"
                type="button"
                class="ah-plugin-shared-link"
                v-tooltip="t('plugin.external_skills_tooltip', { agent: ext.name })"
                @click.stop="jumpToPlatform(ext.id)"
              >
                <span>{{ ext.name }}</span>
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
  max-width: 280px;
  height: 24px;
  padding: 0 8px 0 7px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--ink) 4%, transparent);
  border: 1px solid var(--hairline);
  color: var(--ink-3);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1;
  cursor: pointer;
  user-select: none;
  transition: all var(--dur-fast) var(--ease-soft);
  min-width: 0;
}
.ah-plugin-path-pill:hover {
  background: color-mix(in srgb, var(--ink) 7%, transparent);
  border-color: var(--border);
  color: var(--ink);
}
.ah-plugin-path-pill__folder {
  color: var(--ink-3);
  opacity: 0.65;
  transition: opacity var(--dur-fast) var(--ease-soft), color var(--dur-fast) var(--ease-soft);
}
.ah-plugin-path-pill:hover .ah-plugin-path-pill__folder {
  opacity: 0.95;
  color: var(--ink);
}
.ah-plugin-path-pill__copy {
  opacity: 0;
  width: 0;
  margin-left: -5px;
  transform: scale(0.7);
  transition: opacity var(--dur-fast) var(--ease-soft),
              width var(--dur-fast) var(--ease-soft),
              margin-left var(--dur-fast) var(--ease-soft),
              transform var(--dur-fast) var(--ease-soft);
}
.ah-plugin-path-pill:hover .ah-plugin-path-pill__copy {
  opacity: 0.75;
  width: 10.5px;
  margin-left: 0;
  transform: scale(1);
}
.ah-plugin-shared-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 24px;
  padding: 0 8px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--accent) 8%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent) 20%, transparent);
  color: var(--accent);
  font-size: 11px;
  font-weight: 500;
  line-height: 1;
  cursor: pointer;
  transition: all var(--dur-fast) var(--ease-soft);
  flex-shrink: 0;
}
.ah-plugin-shared-link:hover {
  background: color-mix(in srgb, var(--accent) 15%, transparent);
  border-color: var(--accent);
}
.ah-plugin-readonly-badge {
  display: inline-flex;
  align-items: center;
  height: 24px;
  padding: 0 7.5px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--ink) 4%, transparent);
  border: 1px solid var(--hairline);
  color: var(--ink-4);
  font-size: 10.5px;
  line-height: 1;
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
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 24px;
  min-width: 24px;
  border: 1px solid var(--hairline);
  border-radius: var(--radius-pill);
  background: var(--surface);
  color: var(--ink-3);
  font-size: 11.5px;
  font-family: var(--font-mono);
  padding: 0 7.5px;
  line-height: 1;
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
  transition: border-color var(--dur-base) var(--ease-soft),
              box-shadow var(--dur-base) var(--ease-soft);
}
.ah-plugin-pane.is-collapsed {
  box-shadow: var(--shadow-mist);
}
.ah-plugin-pane.is-collapsed:hover {
  border-color: var(--border);
}
.ah-plugin-pane__header {
  height: 48px;
  min-height: 48px;
  padding: 0 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.ah-plugin-pane__header--collapsible {
  cursor: pointer;
  user-select: none;
  transition: background-color var(--dur-fast) var(--ease-soft);
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
  border-radius: var(--radius-md);
  border: 1px solid transparent;
  background: transparent;
  color: var(--ink-4);
  cursor: pointer;
  transition: all 0.15s ease;
  padding: 0;
  flex-shrink: 0;
}
.ah-plugin-pane__toggle:hover {
  background: var(--hover);
  border-color: var(--border);
  color: var(--ink);
}
.ah-plugin-pane__chevron {
  transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}
.ah-plugin-pane__chevron.is-collapsed {
  transform: rotate(-90deg);
}
.ah-plugin-pane__header h2 { font-size: 13.5px; font-weight: 600; line-height: 1.2; color: var(--ink); margin: 0; }
.ah-plugin-pane__body { min-width: 0; border-top: 1px solid var(--hairline); }
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
.ah-plugin-shared-agent-hint {
  color: var(--ink-4);
  transition: color 0.15s ease;
}
.ah-plugin-shared-agent-badge:hover .ah-plugin-shared-agent-hint {
  color: var(--ink-2);
}

:deep(.ah-embedded-view) { padding: 0; }
:deep(.ah-embedded-view .ah-view-content) { max-width: none; }
:deep(.ah-embedded-view .ah-table-wrap) { border: 0; border-radius: 0; }
:deep(.ah-embedded-view .ah-thead) {
  height: 38px;
  grid-template-columns: minmax(11rem, 1.35fr) minmax(12rem, 2.75fr) 5.25rem 2.25rem;
  column-gap: 16px;
  padding: 0 8px 0 16px;
  background: var(--surface);
}
:deep(.ah-embedded-view .ah-row) {
  min-height: 46px;
  height: 46px;
  grid-template-columns: minmax(11rem, 1.35fr) minmax(12rem, 2.75fr) 5.25rem 2.25rem;
  column-gap: 16px;
  padding: 0 8px 0 16px;
}
:deep(.ah-embedded-view .ah-row__desc) {
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.5;
  color: var(--ink-3);
  font-size: 13px;
  transition: color var(--dur-fast) var(--ease-soft);
}
:deep(.ah-embedded-view .ah-row:hover .ah-row__desc) {
  color: var(--ink-2);
}
:deep(.ah-embedded-view .ah-row__size) {
  font-variant-numeric: tabular-nums;
  font-size: 12px;
}
:deep(.ah-embedded-view .ah-config-view) {
  max-width: 100%;
  overflow-wrap: anywhere;
  word-break: break-word;
  white-space: pre-wrap;
}

.ah-plugin-search-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  margin-bottom: 12px;
  background: var(--accent-soft);
  border: 1px solid var(--accent);
  border-radius: var(--radius-sm);
  color: var(--ink);
}
.ah-plugin-search-banner__icon {
  color: var(--accent);
}
.ah-plugin-search-banner__tag {
  font-size: 11.5px;
  color: var(--ink-2);
  background: var(--surface);
  border: 1px solid var(--hairline);
  padding: 1px 7px;
  border-radius: var(--radius-pill);
}

@media (max-width: 1050px) {
  .ah-plugin-grid { flex: none; }
}
@media (max-width: 720px) {
  .ah-plugin-view { padding: 16px; }
  .ah-plugin-header { align-items: flex-start; flex-direction: column; gap: 10px; }
  :deep(.ah-embedded-view .ah-thead) { display: none; }
  :deep(.ah-embedded-view .ah-row) { grid-template-columns: 1fr auto; height: auto; min-height: 44px; padding: 10px 12px; }
}
</style>
