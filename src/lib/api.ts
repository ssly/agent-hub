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
export const getPlatformSkills = (platformId: string, workspaceDir = '') =>
  invoke<any[]>('get_platform_skills', { platformId, workspaceDir: workspaceDir || null })
export const getSkillDetail = (platformId: string, skillName: string, folder: string, workspaceDir = '') =>
  invoke<any>('get_skill_detail', { platformId, skillName, folder, workspaceDir: workspaceDir || null })
export const openSkillFolder = (platformId: string, skillName: string, folder: string, workspaceDir = '') =>
  invoke<void>('open_skill_folder', { platformId, skillName, folder, workspaceDir: workspaceDir || null })
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
export const refreshPlatformSkills = (platformId: string, workspaceDir = '') =>
  invoke<any[]>('refresh_platform_skills', { platformId, workspaceDir: workspaceDir || null })
export const getLocale = () => invoke<string>('get_locale')
export const setLocale = (locale: string) => invoke<void>('set_locale', { locale })
export const searchSkills = (query: string, workspaceDir = '') =>
  invoke<any[]>('search_skills', { query, workspaceDir: workspaceDir || null })
export const readSkillFile = (platformId: string, skillName: string, folder: string, filePath: string, workspaceDir = '') =>
  invoke<string>('read_skill_file', { platformId, skillName, folder, filePath, workspaceDir: workspaceDir || null })
export const deleteSkill = (platformId: string, skillName: string, folder: string) =>
  invoke<void>('delete_skill_cmd', { platformId, skillName, folder })

// MCP
export const listMcpPlatforms = (workspaceDir = '') =>
  invoke<any[]>('list_mcp_platforms', { workspaceDir: workspaceDir || null })
export const getMcpServers = (platformId: string, workspaceDir = '') =>
  invoke<any[]>('get_mcp_servers', { platformId, workspaceDir: workspaceDir || null })
export const getMcpServer = (platformId: string, name: string, workspaceDir = '') =>
  invoke<any>('get_mcp_server', { platformId, name, workspaceDir: workspaceDir || null })
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
export const previewMcpChange = (platformId: string, serverName: string, configText?: string) =>
  invoke<any>('preview_mcp_change_cmd', { platformId, serverName, configText })
export const syncMcpServer = (sourcePlatformId: string, targetPlatformId: string, serverName: string) =>
  invoke<void>('sync_mcp_server_cmd', { sourcePlatformId, targetPlatformId, serverName })

// Claude Code native plugins
export const listClaudePlugins = (workspaceDir = '') =>
  invoke<any[]>('list_claude_plugins', { workspaceDir: workspaceDir || null })
export const setClaudePluginEnabled = (pluginId: string, scope: string, enabled: boolean) =>
  invoke<void>('set_claude_plugin_enabled', { pluginId, scope, enabled })

// Zcode marketplace plugins (read-only)
export const getZcodePlugins = () => invoke<any[]>('get_zcode_plugins')

export async function pickPluginDirectory(): Promise<string | null> {
  if (!isTauri) return '/Users/demo/projects/agent-hub'
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ directory: true, multiple: false })
  return typeof selected === 'string' ? selected : null
}

// Sessions
export const listSessionPlatforms = () => invoke<any[]>('list_session_platforms')
export const listSessions = (platformId: string, pathFilter: string, offset: number, limit: number) =>
  invoke<any>('list_sessions', { platformId, pathFilter, offset, limit })
export const listSessionTerminals = () => invoke<any[]>('list_session_terminals')
export const resumeSession = (platformId: string, sessionId: string, projectPath: string, terminalId: string) =>
  invoke<string>('resume_session', { platformId, sessionId, projectPath, terminalId })
export interface SessionResumePreview {
  command: string
  last_user_message: string | null
  last_assistant_message: string | null
}
export const getSessionResumePreview = (platformId: string, sessionId: string, projectPath: string) =>
  invoke<SessionResumePreview>('get_session_resume_preview', { platformId, sessionId, projectPath })
export const getSessionMessages = (platformId: string, sessionId: string, offset: number, limit: number) =>
  invoke<any[]>('get_session_messages', { platformId, sessionId, offset, limit })
export const deleteSession = (platformId: string, sessionId: string) =>
  invoke<void>('delete_session', { platformId, sessionId })
export const deleteSessions = (platformId: string, sessionIds: string[]) =>
  invoke<{ deleted: number; failed: Array<{ session_id: string; error: string }> }>(
    'delete_sessions',
    { platformId, sessionIds },
  )
export interface SessionExportResult {
  path: string
  session_count: number
  message_count: number
}
export async function exportSessionsHtml(
  platformId: string,
  sessionIds: string[],
  locale: string,
): Promise<SessionExportResult | null> {
  const date = new Date().toISOString().slice(0, 10).replace(/-/g, '')
  const filename = `Agent-Hub-Sessions-${date}.html`
  let outputPath = filename
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const selected = await save({
      defaultPath: filename,
      filters: [{ name: 'HTML', extensions: ['html'] }],
    })
    if (!selected) return null
    outputPath = selected
  }
  return invoke<SessionExportResult>('export_sessions_html', {
    platformId,
    sessionIds,
    outputPath,
    locale,
  })
}
export const searchSessionMessages = (platformId: string, query: string) =>
  invoke<any[]>('search_session_messages', { platformId, query })

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

// Session monitor
export const getCodexSessionMonitorSnapshot = () =>
  invoke<any>('get_codex_session_monitor_snapshot')
export const deleteCodexSessionMonitorSession = (sessionId: string) =>
  invoke<void>('delete_codex_session_monitor_session', { sessionId })
export const getCodexHookStatus = () => invoke<any>('get_codex_hook_status')
export const previewCodexHookChange = (action: 'install' | 'uninstall') =>
  invoke<any>('preview_codex_hook_change', { action })
export const applyCodexHookChange = (action: 'install' | 'uninstall', expectedBeforeHash: string) =>
  invoke<any>('apply_codex_hook_change', { action, expectedBeforeHash })

export const getClaudeSessionMonitorSnapshot = () =>
  invoke<any>('get_claude_session_monitor_snapshot')
export const deleteClaudeSessionMonitorSession = (sessionId: string) =>
  invoke<void>('delete_claude_session_monitor_session', { sessionId })
export const getClaudeHookStatus = () => invoke<any>('get_claude_hook_status')
export const previewClaudeHookChange = (action: 'install' | 'uninstall') =>
  invoke<any>('preview_claude_hook_change', { action })
export const applyClaudeHookChange = (action: 'install' | 'uninstall', expectedBeforeHash: string) =>
  invoke<any>('apply_claude_hook_change', { action, expectedBeforeHash })

export const getCursorSessionMonitorSnapshot = () =>
  invoke<any>('get_cursor_session_monitor_snapshot')
export const deleteCursorSessionMonitorSession = (sessionId: string) =>
  invoke<void>('delete_cursor_session_monitor_session', { sessionId })
export const getCursorHookStatus = () => invoke<any>('get_cursor_hook_status')
export const previewCursorHookChange = (action: 'install' | 'uninstall') =>
  invoke<any>('preview_cursor_hook_change', { action })
export const applyCursorHookChange = (action: 'install' | 'uninstall', expectedBeforeHash: string) =>
  invoke<any>('apply_cursor_hook_change', { action, expectedBeforeHash })

export const getGrokSessionMonitorSnapshot = () =>
  invoke<any>('get_grok_session_monitor_snapshot')
export const deleteGrokSessionMonitorSession = (sessionId: string) =>
  invoke<void>('delete_grok_session_monitor_session', { sessionId })
export const getGrokHookStatus = () => invoke<any>('get_grok_hook_status')
export const previewGrokHookChange = (action: 'install' | 'uninstall') =>
  invoke<any>('preview_grok_hook_change', { action })
export const applyGrokHookChange = (action: 'install' | 'uninstall', expectedBeforeHash: string) =>
  invoke<any>('apply_grok_hook_change', { action, expectedBeforeHash })

export const getKimiSessionMonitorSnapshot = () =>
  invoke<any>('get_kimi_session_monitor_snapshot')
export const deleteKimiSessionMonitorSession = (sessionId: string) =>
  invoke<void>('delete_kimi_session_monitor_session', { sessionId })
export const getKimiHookStatus = () => invoke<any>('get_kimi_hook_status')
export const previewKimiHookChange = (action: 'install' | 'uninstall') =>
  invoke<any>('preview_kimi_hook_change', { action })
export const applyKimiHookChange = (action: 'install' | 'uninstall', expectedBeforeHash: string) =>
  invoke<any>('apply_kimi_hook_change', { action, expectedBeforeHash })

export const getZcodeSessionMonitorSnapshot = () =>
  invoke<any>('get_zcode_session_monitor_snapshot')
export const deleteZcodeSessionMonitorSession = (sessionId: string) =>
  invoke<void>('delete_zcode_session_monitor_session', { sessionId })
export const getZcodeHookStatus = () => invoke<any>('get_zcode_hook_status')
export const previewZcodeHookChange = (action: 'install' | 'uninstall') =>
  invoke<any>('preview_zcode_hook_change', { action })
export const applyZcodeHookChange = (action: 'install' | 'uninstall', expectedBeforeHash: string) =>
  invoke<any>('apply_zcode_hook_change', { action, expectedBeforeHash })

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
// Delete the live auth file (e.g. ~/.codex/auth.json) WITHOUT backing it up.
// The account pool (~/.agent-hub/switch/<agent>/) is left untouched.
export const deleteActiveAuth = (agentType: string) => invoke<void>('delete_active_auth', { agentType })

// Codex usage windows for the currently active account.
// Window presence and ordering vary by account. Use window_seconds to label
// each returned window as 5h, 7d, 30d, or another duration.
export interface UsageWindow {
  used_percent: number
  remaining_percent: number
  reset_after_seconds: number
  reset_at: number
  // Duration of the window in seconds (5h=18000, 7d=604800, 30d=2592000).
  window_seconds: number
}
// "Rate-limit reset" credits — the one-click window reset button on the
// ChatGPT web UI draws from this pool. `available_count` = resets remaining.
export interface ResetCredits {
  available_count: number
}
export interface CodexUsage {
  // Email derived from the currently logged-in Codex CLI account.
  account_name: string | null
  plan_type: string
  // All returned quota windows, sorted by duration. New tray UI uses this so
  // 5h/7d/30d can coexist without being capped at primary + secondary.
  usage_windows: UsageWindow[]
  primary_window: UsageWindow | null
  secondary_window: UsageWindow | null
  reset_credits: ResetCredits | null
}
export const getCodexUsage = () => invoke<CodexUsage>('get_codex_usage')

// Codex rate-limit reset credits with validity period.
// Comes from a SEPARATE endpoint (/wham/rate-limit-reset-credits) because the
// usage endpoint only carries `available_count`, not the per-credit expiry.
// Each credit is valid ~30 days from grant; `next_expires_at` is the soonest.
export interface ResetCreditEntry {
  status: string                 // "available" | "redeemed" | ...
  expires_at: string | null      // ISO-8601, e.g. "2026-07-31T20:03:43Z"
  granted_at: string | null      // ISO-8601
  title: string | null           // e.g. "Full reset (Weekly + 5 hr)"
}
export interface CodexResetCredits {
  available_count: number
  next_expires_at: string | null // soonest-expiring available credit
  credits: ResetCreditEntry[]
}
export const getCodexResetCredits = () =>
  invoke<CodexResetCredits>('get_codex_reset_credits')

// Shared Codex quota snapshot used by both the Accounts view and tray popup.
// Backend caches for 10 minutes unless `force` is true (manual refresh).
export interface CodexTraySnapshot {
  usage: CodexUsage
  reset_credits: CodexResetCredits | null
  last_query_at: number
}
export const getCodexTrayUsage = (force = false) =>
  invoke<CodexTraySnapshot>('get_codex_tray_usage', { force })
export const resizeUsageTray = (height: number) =>
  invoke<void>('resize_usage_tray', { height })
export const setUsageTrayPinned = (pinned: boolean) =>
  invoke<void>('set_usage_tray_pinned', { pinned })
export const openUsageTray = () => invoke<void>('open_usage_tray')

// Grok Build uses only the CLI's current/default account. Agent Hub does not
// manage or switch Grok credentials; this endpoint is read-only.
// Backend caches for 10 minutes unless `force` is true (manual refresh).
export interface GrokUsage {
  account_name: string | null
  plan_type: string
  period_type: 'monthly' | 'weekly'
  usage_window: UsageWindow
  limit_value: number | null
  used_value: number | null
  prepaid_balance: number | null
  on_demand_cap: number | null
  on_demand_used: number | null
  on_demand_enabled: boolean | null
  source: 'live' | 'cache'
  fetched_at: number
  stale: boolean
}
export const getGrokUsage = (force = false) =>
  invoke<GrokUsage>('get_grok_usage', { force })

// Kimi Code uses the CLI's `sk-kimi-…` API key from ~/.kimi-code/config.toml.
// We never touch OAuth tokens (those are scoped to the kimi CLI itself).
// Backend caches for 10 minutes unless `force` is true (manual refresh).
export interface KimiUsage {
  account_name: string | null
  // `METHOD_API_KEY` for the long-lived Coding Plan key, `METHOD_OAUTH` for CLI.
  auth_method: string
  // The 5-hour rolling rate-limit window.
  window_5h: UsageWindow | null
  // The weekly quota window (resets every 7 days from subscription date).
  window_weekly: UsageWindow | null
  // Raw weekly limit/used values for "used / limit" display.
  weekly_limit: number | null
  weekly_used: number | null
  // Windows in ascending order, for generic iteration.
  usage_windows: UsageWindow[]
  fetched_at: number
}
export const getKimiUsage = (force = false) =>
  invoke<KimiUsage>('get_kimi_usage', { force })

// Claude Code official-login (OAuth subscription) usage. The backend reads
// the CLI's own credentials read-only (CLAUDE_CODE_OAUTH_TOKEN env → macOS
// Keychain "Claude Code-credentials" → ~/.claude/.credentials.json) and calls
// the same /api/oauth/usage endpoint as Claude Code's /usage. Tokens are never
// refreshed by us — an expired login surfaces a re-login hint instead.
// Backend caches for 10 minutes unless `force` is true (manual refresh).
export interface ClaudeUsage {
  account_name: string | null
  // subscriptionType from the OAuth credentials (pro / max / …), "unknown" fallback.
  plan_type: string
  window_5h: UsageWindow | null
  window_weekly: UsageWindow | null
  // Windows in ascending order, for generic iteration.
  usage_windows: UsageWindow[]
  fetched_at: number
}
export const getClaudeUsage = (force = false) =>
  invoke<ClaudeUsage>('get_claude_usage', { force })

export interface UsageProviderAvailability {
  codex: boolean
  grok_build: boolean
  kimi_code: boolean
  claude_code: boolean
}
export const getUsageProviderAvailability = () =>
  invoke<UsageProviderAvailability>('get_usage_provider_availability')

// App
export const getAppVersion = () => invoke<string>('get_app_version')
