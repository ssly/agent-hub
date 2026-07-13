import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { useSkillsStore } from './skills'
import { useMcpStore } from './mcp'
import { useClaudePluginsStore } from './claude-plugins'

export const usePluginsStore = defineStore('plugins', () => {
  const skillsStore = useSkillsStore()
  const mcpStore = useMcpStore()
  const claudePluginsStore = useClaudePluginsStore()
  const selectedPlatformId = ref<string | null>(localStorage.getItem('ah-plugin-platform'))
  const workspaceDirectory = ref(localStorage.getItem('ah-plugin-workspace-dir') || '')
  const isLoading = ref(false)

  const isGlobalScope = computed(() => !workspaceDirectory.value)

  function workspaceSkillPath(platformId: string) {
    const relativeByPlatform: Record<string, string> = {
      'claude-code': '.claude/skills',
      codex: '.codex/skills',
      cursor: '.cursor/skills-cursor',
      gemini: '.gemini/skills',
      openclaw: '.openclaw/skills',
      hermes: '.hermes/skills',
      trae: '.trae/skills',
      kiro: '.kiro/skills',
      'shared-pool': '.agents/skills',
    }
    const relative = relativeByPlatform[platformId]
    if (!relative || !workspaceDirectory.value) return ''
    const separator = workspaceDirectory.value.includes('\\') ? '\\' : '/'
    return `${workspaceDirectory.value.replace(/[\\/]+$/, '')}${separator}${relative.replaceAll('/', separator)}`
  }

  const platforms = computed(() => {
    const merged = new Map<string, any>()

    for (const platform of skillsStore.platforms) {
      merged.set(platform.id, {
        ...platform,
        skill_count: platform.skill_count ?? 0,
        server_count: 0,
        supports_skills: true,
        supports_mcp: false,
      })
    }

    for (const platform of mcpStore.platforms) {
      const existing = merged.get(platform.id)
      merged.set(platform.id, {
        ...existing,
        ...platform,
        skill_count: existing?.skill_count ?? 0,
        server_count: platform.server_count ?? 0,
        supports_skills: Boolean(existing?.supports_skills),
        supports_mcp: true,
      })
    }

    return Array.from(merged.values())
  })

  const selectedPlatform = computed(() => {
    const platform = platforms.value.find(item => item.id === selectedPlatformId.value)
    if (!platform || isGlobalScope.value) return platform
    return {
      ...platform,
      skill_dir: platform.supports_skills ? workspaceSkillPath(platform.id) : '',
    }
  })

  async function loadPlatform(id: string) {
    selectedPlatformId.value = id
    localStorage.setItem('ah-plugin-platform', id)

    const hasSkills = skillsStore.platforms.some(platform => platform.id === id)
    const hasMcp = mcpStore.platforms.some(platform => platform.id === id)

    await Promise.all([
      hasSkills
        ? skillsStore.selectPlatform(id, workspaceDirectory.value)
        : skillsStore.clearPlatform(id, workspaceDirectory.value),
      hasMcp
        ? mcpStore.selectPlatform(id, workspaceDirectory.value)
        : mcpStore.clearPlatform(id, workspaceDirectory.value),
      id === 'claude-code'
        ? claudePluginsStore.loadPlugins(workspaceDirectory.value)
        : Promise.resolve(claudePluginsStore.clear()),
    ])
  }

  async function refreshPlatforms() {
    isLoading.value = true
    try {
      await Promise.all([
        skillsStore.refreshPlatforms(),
        mcpStore.refreshPlatforms(workspaceDirectory.value),
      ])

      const nextId = platforms.value.some(platform => platform.id === selectedPlatformId.value)
        ? selectedPlatformId.value
        : platforms.value[0]?.id ?? null

      if (nextId) await loadPlatform(nextId)
    } finally {
      isLoading.value = false
    }
  }

  async function selectPlatform(id: string) {
    if (id === selectedPlatformId.value
      && skillsStore.selectedPlatformId === id
      && (mcpStore.selectedPlatformId === id || !selectedPlatform.value?.supports_mcp)) return
    await loadPlatform(id)
  }

  async function setWorkspaceDirectory(directory: string | null) {
    const next = directory?.trim() || ''
    if (next === workspaceDirectory.value) return
    workspaceDirectory.value = next
    if (next) localStorage.setItem('ah-plugin-workspace-dir', next)
    else localStorage.removeItem('ah-plugin-workspace-dir')

    isLoading.value = true
    try {
      await mcpStore.refreshPlatforms(next)
      const nextId = platforms.value.some(platform => platform.id === selectedPlatformId.value)
        ? selectedPlatformId.value
        : platforms.value[0]?.id ?? null
      if (nextId) await loadPlatform(nextId)
    } finally {
      isLoading.value = false
    }
  }

  return {
    platforms,
    selectedPlatformId,
    selectedPlatform,
    workspaceDirectory,
    isGlobalScope,
    isLoading,
    refreshPlatforms,
    selectPlatform,
    setWorkspaceDirectory,
  }
})
