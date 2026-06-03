/**
 * API layer — auto-detects Tauri vs browser environment.
 *
 * In Tauri: uses `@tauri-apps/api/core` invoke() for real IPC.
 * In browser (npm run dev:web): uses mock data from mock-api.ts.
 */

// Static imports — Vite tree-shakes unused chunks in production
import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import * as mockApi from './mock-api'

// Check if we're running inside Tauri
const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__

if (typeof window !== 'undefined' && !isTauri) {
  console.log('%c🎨 Agent Hub — Web Debug Mode (mock data)', 'color: #3A6B8C; font-weight: bold; font-size: 14px')
}

function snakeToCamel(s: string): string {
  return s.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase())
}

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    return tauriInvoke<T>(cmd, args)
  }
  // Mock mode — dispatch to matching mock function
  const fnName = snakeToCamel(cmd)
  const fn = (mockApi as Record<string, unknown>)[fnName] as ((...a: unknown[]) => Promise<T>) | undefined
      || (mockApi as Record<string, unknown>)[cmd] as ((...a: unknown[]) => Promise<T>) | undefined
  if (fn) {
    const params = args ? Object.values(args) : []
    return fn(...params)
  }
  console.warn(`[mock-api] No mock for command: ${cmd}`)
  return Promise.resolve(null as T)
}

// Skills / Platforms
export const listPlatforms = () => invoke<any[]>('list_platforms')
export const getPlatformSkills = (platformId: string) => invoke<any[]>('get_platform_skills', { platformId })
export const getSkillDetail = (platformId: string, skillName: string, folder: string) =>
  invoke<any>('get_skill_detail', { platformId, skillName, folder })
export const getDiffCandidates = (platformId: string, skillName: string, folder: string) =>
  invoke<any[]>('get_diff_candidates', { platformId, skillName, folder })
export const diffSkills = (sourcePlatformId: string, targetPlatformId: string, skillName: string, folder: string) =>
  invoke<any>('diff_skills_cmd', { sourcePlatformId, targetPlatformId, skillName, folder })
export const getSyncTargets = (platformId: string, skillName: string, folder: string) =>
  invoke<any[]>('get_sync_targets', { platformId, skillName, folder })
export const syncSkill = (sourcePlatformId: string, targetPlatformId: string, skillName: string, folder: string, overwrite: boolean) =>
  invoke<any>('sync_skill_cmd', { sourcePlatformId, targetPlatformId, skillName, folder, overwrite })
export const syncFolder = (sourcePlatformId: string, targetPlatformId: string, folder: string) =>
  invoke<any>('sync_folder_cmd', { sourcePlatformId, targetPlatformId, folder })
export const refreshPlatforms = () => invoke<any[]>('refresh_platforms')
export const refreshPlatformSkills = (platformId: string) => invoke<any[]>('refresh_platform_skills', { platformId })
export const getLocale = () => invoke<string>('get_locale')
export const setLocale = (locale: string) => invoke<void>('set_locale', { locale })
export const searchSkills = (query: string) => invoke<any[]>('search_skills', { query })
export const readSkillFile = (platformId: string, skillName: string, folder: string, filePath: string) =>
  invoke<string>('read_skill_file', { platformId, skillName, folder, filePath })
export const deleteSkill = (platformId: string, skillName: string, folder: string) =>
  invoke<void>('delete_skill_cmd', { platformId, skillName, folder })
export const scanInvalidSkills = () => invoke<any[]>('scan_invalid_skills_cmd')

// MCP
export const listMcpPlatforms = () => invoke<any[]>('list_mcp_platforms')
export const getMcpServers = (platformId: string) => invoke<any[]>('get_mcp_servers', { platformId })
export const getMcpServer = (platformId: string, name: string) => invoke<any>('get_mcp_server', { platformId, name })
export const saveMcpServer = (platformId: string, name: string, configJson: string) =>
  invoke<void>('save_mcp_server_cmd', { platformId, name, configJson })
export const deleteMcpServer = (platformId: string, name: string) =>
  invoke<void>('delete_mcp_server_cmd', { platformId, name })
export const importMcpServer = (platformId: string, name: string, configText: string) =>
  invoke<void>('import_mcp_server_cmd', { platformId, name, configText })
export const getMcpSyncTargets = (platformId: string, serverName: string) =>
  invoke<any[]>('get_mcp_sync_targets', { platformId, serverName })
export const previewMcpSync = (sourcePlatformId: string, targetPlatformId: string, serverName: string) =>
  invoke<any>('preview_mcp_sync_cmd', { sourcePlatformId, targetPlatformId, serverName })
export const syncMcpServer = (sourcePlatformId: string, targetPlatformId: string, serverName: string) =>
  invoke<void>('sync_mcp_server_cmd', { sourcePlatformId, targetPlatformId, serverName })

// Sessions
export const listSessionPlatforms = () => invoke<any[]>('list_session_platforms')
export const listSessions = (platformId: string, pathFilter: string, offset: number, limit: number) =>
  invoke<any>('list_sessions', { platformId, pathFilter, offset, limit })
export const listSessionTerminals = () => invoke<any[]>('list_session_terminals')
export const resumeSession = (platformId: string, sessionId: string, projectPath: string, terminalId: string) =>
  invoke<string>('resume_session', { platformId, sessionId, projectPath, terminalId })
export const getSessionMessages = (platformId: string, sessionId: string, offset: number, limit: number) =>
  invoke<any[]>('get_session_messages', { platformId, sessionId, offset, limit })
export const deleteSession = (platformId: string, sessionId: string) =>
  invoke<void>('delete_session', { platformId, sessionId })

// Trash
export const listTrash = () => invoke<any[]>('list_trash_cmd')
export const restoreTrashItem = (id: string) => invoke<void>('restore_trash_item_cmd', { id })
export const permanentlyDeleteTrashItem = (id: string) => invoke<void>('permanently_delete_trash_item_cmd', { id })
export const emptyTrash = () => invoke<void>('empty_trash_cmd')

// Monitor
export const getActiveSessions = () => invoke<any[]>('get_active_sessions')
export const getMonitorConfig = () => invoke<any>('get_monitor_config')
export const setMonitorConfig = (opts: { notificationEnabled?: boolean; notificationCooldownSecs?: number }) =>
  invoke<any>('set_monitor_config', opts)
export const setMonitorPolling = (enabled: boolean) => invoke<void>('set_monitor_polling', { enabled })
export const forcePollMonitor = () => invoke<void>('force_poll_monitor')
export const configureHooks = (agentType: string) => invoke<void>('configure_hooks', { agentType })
export const removeHooks = (agentType: string) => invoke<void>('remove_hooks', { agentType })
export const getHooksStatus = () => invoke<Record<string, boolean>>('get_hooks_status')

// Switch
export const listSwitchProfiles = (agentType: string) => invoke<any>('list_switch_profiles', { agentType })
export const saveCurrentAuthProfile = (agentType: string, note: string) =>
  invoke<void>('save_current_auth_profile', { agentType, note })
export const addAuthProfile = (agentType: string, content: string, note: string) =>
  invoke<void>('add_auth_profile', { agentType, content, note })
export const switchAuthProfile = (agentType: string, id: string) =>
  invoke<void>('switch_auth_profile', { agentType, id })
export const updateAuthProfileNote = (agentType: string, id: string, note: string) =>
  invoke<void>('update_auth_profile_note', { agentType, id, note })
export const deleteAuthProfile = (agentType: string, id: string) =>
  invoke<void>('delete_auth_profile', { agentType, id })
export const getAuthProfileContent = (agentType: string, id: string) =>
  invoke<string>('get_auth_profile_content', { agentType, id })
export const updateAuthProfileContent = (agentType: string, id: string, content: string) =>
  invoke<void>('update_auth_profile_content', { agentType, id, content })
export const clearActiveAuth = (agentType: string) => invoke<void>('clear_active_auth', { agentType })

// App
export const getAppVersion = () => invoke<string>('get_app_version')
