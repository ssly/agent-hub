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
  const diffResult = ref<any>(null)
  const searchResults = ref<any[]>([])
  const searchLoading = ref(false)
  const searchQuery = ref('')

  // Modal States
  const diffPlatformModalOpen = ref(false)
  const diffCandidates = ref<any[]>([])
  const syncPlatformModalOpen = ref(false)
  const syncTargets = ref<any[]>([])
  const syncOverwrite = ref(false)
  const syncTargetPlatformId = ref<string | null>(null)

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
    diffResult.value = null
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

  async function loadDiffCandidates() {
    if (!selectedPlatformId.value || !selectedSkillName.value) return
    diffCandidates.value = await api.getDiffCandidates(selectedPlatformId.value, selectedSkillName.value, selectedFolder.value)
  }

  async function startDiff(targetPlatformId: string) {
    if (!selectedPlatformId.value || !selectedSkillName.value) return
    diffResult.value = await api.diffSkills(selectedPlatformId.value, targetPlatformId, selectedSkillName.value, selectedFolder.value)
    diffPlatformModalOpen.value = false
  }

  async function loadSyncTargets() {
    if (!selectedPlatformId.value || !selectedSkillName.value) return
    syncTargets.value = await api.getSyncTargets(selectedPlatformId.value, selectedSkillName.value, selectedFolder.value)
  }

  async function startSync(targetPlatformId: string, overwrite: boolean) {
    if (!selectedPlatformId.value || !selectedSkillName.value) return
    await api.syncSkill(selectedPlatformId.value, targetPlatformId, selectedSkillName.value, selectedFolder.value, overwrite)
    syncPlatformModalOpen.value = false
    await refreshPlatforms()
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
    skillSortBy, skillSortDir, collapsedFolders, diffResult,
    searchResults, searchLoading, searchQuery,
    diffPlatformModalOpen, diffCandidates,
    syncPlatformModalOpen, syncTargets, syncOverwrite, syncTargetPlatformId,
    selectedPlatform,
    refreshPlatforms, reloadPlatforms, loadSkills, selectPlatform, clearPlatform, selectSkill,
    backToList, toggleFolder, toggleSort, doSearch,
    loadDiffCandidates, startDiff, loadSyncTargets, startSync, performDeleteSkill,
  }
})
