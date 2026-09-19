import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as api from '@/lib/api'

export const useSkillsStore = defineStore('skills', () => {
  const platforms = ref<any[]>([])
  const skills = ref<any[]>([])
  const selectedPlatformId = ref<string | null>(null)
  const workspaceDirectory = ref('')
  const selectedSkillName = ref<string | null>(null)
  const selectedFolder = ref('')
  const skillSortBy = ref<'size' | 'name'>('name')
  const skillSortDir = ref<'asc' | 'desc'>('asc')
  const collapsedFolders = ref(new Set<string>())
  const searchResults = ref<any[]>([])
  const searchLoading = ref(false)
  const searchQuery = ref('')

  // Modal States
  const syncPlatformModalOpen = ref(false)
  const skillsToSync = ref<Array<{ name: string; folder: string }>>([])
  const syncTargets = ref<any[]>([])
  const syncTargetPlatformIds = ref<string[]>([])
  const syncMode = ref<'symlink' | 'copy'>('copy')

  // Backward compatibility alias for single-target selection
  const syncTargetPlatformId = computed({
    get: () => syncTargetPlatformIds.value[0] || null,
    set: (val: string | null) => {
      syncTargetPlatformIds.value = val ? [val] : []
    },
  })

  const selectedPlatform = computed(() =>
    platforms.value.find(p => p.id === selectedPlatformId.value)
  )

  async function refreshPlatforms() {
    platforms.value = await api.refreshPlatforms()
    reconcileSelection()
    if (selectedPlatformId.value) {
      await loadSkills()
    }
  }

  async function reloadPlatforms() {
    platforms.value = await api.listPlatforms()
    reconcileSelection()
    if (selectedPlatformId.value) {
      await loadSkills()
    }
  }

  function reconcileSelection() {
    if (platforms.value.length === 0) {
      selectedPlatformId.value = null
      selectedSkillName.value = null
      selectedFolder.value = ''
      return
    }
    const exists = platforms.value.some(p => p.id === selectedPlatformId.value)
    if (!exists) {
      selectedPlatformId.value = platforms.value[0].id
      selectedSkillName.value = null
      selectedFolder.value = ''
    }
  }

  async function loadSkills() {
    if (!selectedPlatformId.value) return
    skills.value = await api.getPlatformSkills(selectedPlatformId.value, workspaceDirectory.value)
  }

  async function selectPlatform(id: string, workspaceDir = '') {
    selectedPlatformId.value = id
    workspaceDirectory.value = workspaceDir
    selectedSkillName.value = null
    selectedFolder.value = ''
    await loadSkills()
  }

  function clearPlatform(id: string, workspaceDir = '') {
    selectedPlatformId.value = id
    workspaceDirectory.value = workspaceDir
    selectedSkillName.value = null
    selectedFolder.value = ''
    skills.value = []
  }

  function selectSkill(name: string, folder: string = '') {
    selectedSkillName.value = name
    selectedFolder.value = folder
  }

  function backToList() {
    selectedSkillName.value = null
    selectedFolder.value = ''
  }

  function toggleFolder(folder: string) {
    if (collapsedFolders.value.has(folder)) {
      collapsedFolders.value.delete(folder)
    } else {
      collapsedFolders.value.add(folder)
    }
  }

  function toggleSort(field: 'size' | 'name' = 'size') {
    if (skillSortBy.value === field) {
      skillSortDir.value = skillSortDir.value === 'asc' ? 'desc' : 'asc'
    } else {
      skillSortBy.value = field
      skillSortDir.value = field === 'name' ? 'asc' : 'desc'
    }
  }

  async function doSearch(query: string) {
    if (!query.trim()) {
      searchResults.value = []
      searchLoading.value = false
      searchQuery.value = ''
      return
    }
    searchQuery.value = query
    searchLoading.value = true
    try {
      const results = await api.searchSkills(query, workspaceDirectory.value)
      if (searchQuery.value === query) {
        searchResults.value = results
      }
    } finally {
      if (searchQuery.value === query) {
        searchLoading.value = false
      }
    }
  }

  async function openSingleSync(skill: { name: string; folder?: string }) {
    selectSkill(skill.name, skill.folder || '')
    skillsToSync.value = [{ name: skill.name, folder: skill.folder || '' }]
    syncTargetPlatformIds.value = []
    await loadSyncTargets()
    syncPlatformModalOpen.value = true
  }

  async function openBatchSync(skillsList: Array<{ name: string; folder?: string }>) {
    skillsToSync.value = skillsList.map(s => ({ name: s.name, folder: s.folder || '' }))
    syncTargetPlatformIds.value = []
    await loadSyncTargets()
    syncPlatformModalOpen.value = true
  }

  async function loadSyncTargets() {
    if (!selectedPlatformId.value) return
    const skillsToInspect = skillsToSync.value.length > 0
      ? skillsToSync.value
      : (selectedSkillName.value ? [{ name: selectedSkillName.value, folder: selectedFolder.value }] : [])
    if (skillsToInspect.length === 0) return

    const results = await Promise.all(
      skillsToInspect.map(s => api.getSyncTargets(selectedPlatformId.value!, s.name, s.folder))
    )

    if (results.length === 0 || !results[0] || results[0].length === 0) {
      syncTargets.value = []
      return
    }

    const targetMap = new Map<string, { id: string; display_name: string; has_skill: boolean; conflict_skills: string[] }>()

    for (const target of results[0]) {
      targetMap.set(target.id, {
        id: target.id,
        display_name: target.display_name,
        has_skill: false,
        conflict_skills: [],
      })
    }

    results.forEach((targetList, idx) => {
      const skillName = skillsToInspect[idx].name
      for (const t of targetList) {
        const entry = targetMap.get(t.id)
        if (entry && t.has_skill) {
          entry.has_skill = true
          if (!entry.conflict_skills.includes(skillName)) {
            entry.conflict_skills.push(skillName)
          }
        }
      }
    })

    syncTargets.value = Array.from(targetMap.values())
  }

  async function startSync(
    targetPlatformId: string,
    overwrite: boolean,
    mode: 'symlink' | 'copy' = syncMode.value,
    skillName?: string,
    folder?: string
  ) {
    if (!selectedPlatformId.value) return
    const name = skillName || selectedSkillName.value
    const f = folder !== undefined ? folder : selectedFolder.value
    if (!name) return
    await api.syncSkill(selectedPlatformId.value, targetPlatformId, name, f, overwrite, mode)
  }

  async function performDeleteSkill(name: string, folder: string) {
    if (!selectedPlatformId.value) return
    await api.deleteSkill(selectedPlatformId.value, name, folder)
    if (selectedSkillName.value === name && selectedFolder.value === folder) {
      backToList()
    }
    await reloadPlatforms()
  }

  return {
    platforms, skills, selectedPlatformId, workspaceDirectory, selectedSkillName, selectedFolder,
    skillSortBy, skillSortDir, collapsedFolders,
    searchResults, searchLoading, searchQuery,
    syncPlatformModalOpen, skillsToSync, syncTargets, syncTargetPlatformIds, syncTargetPlatformId, syncMode,
    selectedPlatform,
    refreshPlatforms, reloadPlatforms, loadSkills, selectPlatform, clearPlatform, selectSkill,
    backToList, toggleFolder, toggleSort, doSearch,
    openSingleSync, loadSyncTargets, startSync, performDeleteSkill,
  }
})
