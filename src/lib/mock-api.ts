/**
 * Mock API for browser-only debugging (no Tauri backend).
 * Provides fake data for all API functions so the UI can be
 * developed and tested in a regular browser via `npm run dev:web`.
 */

// ---- Fake data generators ----

const PLATFORMS = [
  {
    id: 'shared-pool',
    display_name: 'Shared Pool',
    skill_dir: '~/.agents/skills',
    skill_count: 8,
  },
  {
    id: 'codex',
    display_name: 'Codex',
    skill_dir: '~/.agents/skills',
    skill_count: 8,
  },
  {
    id: 'claude-code',
    display_name: 'Claude Code',
    skill_dir: '~/.claude/skills',
    skill_count: 12,
  },
  {
    id: 'antigravity',
    display_name: 'Antigravity',
    skill_dir: '~/.gemini/config/skills',
    skill_count: 4,
  },
  {
    id: 'grok-build',
    display_name: 'Grok Build',
    skill_dir: '~/.grok/skills',
    skill_count: 2,
  },
  {
    id: 'kimi-code',
    display_name: 'Kimi Code',
    skill_dir: '~/.kimi-code/skills',
    skill_count: 6,
  },
  {
    id: 'cursor',
    display_name: 'Cursor',
    skill_dir: '~/.cursor/skills',
    skill_count: 3,
  },
]

function makeSkills(platformId: string) {
  const base = [
    { name: 'code-review', folder: '', description: 'Automated code review with best practices', version: '1.2.0', total_size: 4521, is_symlink: false },
    { name: 'refactor', folder: '', description: 'Intelligent code refactoring suggestions', version: '0.8.1', total_size: 12040, is_symlink: false },
    { name: 'test-gen', folder: '', description: 'Generate unit tests from code', version: '2.0.0', total_size: 8765, is_symlink: false },
    { name: 'doc-writer', folder: '', description: 'Auto-generate documentation from code', version: '1.0.3', total_size: 3201, is_symlink: false },
    { name: 'translate', folder: 'i18n', description: 'Translate strings to multiple languages', version: '0.5.0', total_size: 2150, is_symlink: false },
    { name: 'locale-check', folder: 'i18n', description: 'Check for missing locale keys', version: null, total_size: 980, is_symlink: false },
    { name: 'api-scaffold', folder: 'backend', description: 'Scaffold REST API endpoints', version: '3.1.0', total_size: 45000, is_symlink: false },
    { name: 'db-migrate', folder: 'backend', description: 'Generate database migration files', version: '1.4.2', total_size: 6700, is_symlink: true },
  ]
  if (platformId === 'codex') return base.slice(0, 5)
  if (platformId === 'cursor') return base.slice(0, 3)
  return base
}

function makeSkillDetail(name: string) {
  return {
    name,
    platform_id: 'claude-code',
    description: `This is a mock description for the "${name}" skill. It provides various automation capabilities.`,
    version: '1.2.0',
    total_size: 12345,
    is_symlink: false,
    files: ['SKILL.md', 'index.ts', 'config.json', 'utils/helper.ts'],
  }
}

const MCP_PLATFORMS = [
  { id: 'codex', display_name: 'Codex', server_count: 3, config_path: '~/.codex/config.toml', format: 'toml' },
  { id: 'claude-code', display_name: 'Claude Code', server_count: 4, config_path: '~/.claude.json', format: 'json' },
  { id: 'antigravity', display_name: 'Antigravity', server_count: 2, config_path: '~/.gemini/config/mcp_config.json', format: 'json' },
  { id: 'grok-build', display_name: 'Grok Build', server_count: 1, config_path: '~/.grok/config.toml', format: 'toml' },
  { id: 'kimi-code', display_name: 'Kimi Code', server_count: 2, config_path: '~/.kimi-code/mcp.json', format: 'json' },
  { id: 'cursor', display_name: 'Cursor', server_count: 2, config_path: '~/.cursor/mcp.json', format: 'json' },
]

const CLAUDE_PLUGINS = [
  { id: 'frontend-design@claude-plugins-official', name: 'frontend-design', marketplace: 'claude-plugins-official', version: '1.0.0', scope: 'user', enabled: true, manageable: true, description: 'Frontend design skill for UI/UX implementation', install_path: '~/.claude/plugins/cache/claude-plugins-official/frontend-design/1.0.0' },
  { id: 'rust-analyzer-lsp@claude-plugins-official', name: 'rust-analyzer-lsp', marketplace: 'claude-plugins-official', version: '1.0.0', scope: 'user', enabled: true, manageable: true, description: 'Rust language server for code intelligence and analysis', install_path: '~/.claude/plugins/cache/claude-plugins-official/rust-analyzer-lsp/1.0.0' },
  { id: 'codex@openai-codex', name: 'codex', marketplace: 'openai-codex', version: '1.0.2', scope: 'user', enabled: true, manageable: true, description: 'Use Codex from Claude Code to review code or delegate tasks.', install_path: '~/.claude/plugins/cache/openai-codex/codex/1.0.2' },
  { id: 'claude-mem@thedotmack', name: 'claude-mem', marketplace: 'thedotmack', version: '13.8.0', scope: 'user', enabled: false, manageable: true, description: 'Memory compression system for Claude Code', install_path: '~/.claude/plugins/cache/thedotmack/claude-mem/13.8.0' },
  { id: 'team-review@company', name: 'team-review', marketplace: 'company', version: '2.4.0', scope: 'managed', enabled: true, manageable: false, description: 'Organization-managed review policies and hooks', install_path: '' },
]

const PROJECT_CLAUDE_PLUGINS = [
  { id: 'review-workflow@team-tools', name: 'review-workflow', marketplace: 'team-tools', version: '1.1.0', scope: 'project', enabled: true, manageable: false, description: 'Project-specific review commands and hooks', install_path: '/Users/demo/.claude/plugins/review-workflow' },
]

function makeMcpServers(platformId: string) {
  const servers = [
    { name: 'github', summary: 'GitHub API integration' },
    { name: 'filesystem', summary: 'Local filesystem access' },
    { name: 'postgres', summary: 'PostgreSQL database queries' },
    { name: 'web-search', summary: 'Web search via DuckDuckGo' },
  ]
  return platformId === 'cursor' ? servers.slice(0, 2) : servers
}

const SESSION_PLATFORMS = [
  { id: 'codex', display_name: 'Codex', session_count: 5 },
  { id: 'claude-code', display_name: 'Claude Code', session_count: 28 },
  { id: 'grok', display_name: 'Grok Build', session_count: 2 },
  { id: 'kimi', display_name: 'Kimi Code', session_count: 4 },
  { id: 'kiro', display_name: 'Kiro CLI', session_count: 3 },
]

function makeSessions(offset: number, limit: number) {
  const total = 28
  const sessions = []
  for (let i = offset; i < Math.min(offset + limit, total); i++) {
    sessions.push({
      id: `session-${i}`,
      title: i === 0 ? 'Vue 3 frontend refactor' : i === 1 ? 'Fix auth token refresh' : `Session #${i + 1}`,
      project_path: i % 3 === 0 ? '/Users/demo/projects/agent-hub' : i % 3 === 1 ? '/Users/demo/projects/api-server' : '',
      model: i % 2 === 0 ? 'claude-sonnet-4' : 'claude-opus-4',
      tokens_used: Math.floor(Math.random() * 50000) + 1000,
      started_at: Date.now() / 1000 - Math.floor(Math.random() * 86400 * 7),
      updated_at: Date.now() / 1000 - Math.floor(Math.random() * 3600 * 24),
    })
  }
  return {
    sessions,
    total,
    offset,
    has_more: offset + limit < total,
    paths: ['all', 'unknown', '/Users/demo/projects/agent-hub', '/Users/demo/projects/api-server'],
  }
}

const SWITCH_PROFILES = {
  profiles: [
    { id: 'prof-1', note: 'Personal account', key: 'sk-ant-...Xm3k', is_active: true, saved_at: '2025-12-01T10:30:00Z' },
    { id: 'prof-2', note: 'Work account', key: 'sk-ant-...9pBz', is_active: false, saved_at: '2025-11-15T08:20:00Z' },
    { id: 'prof-3', note: '', key: 'sk-ant-...qR7n', is_active: false, saved_at: '2025-10-22T14:45:00Z' },
  ],
  current_key: 'sk-ant-...Xm3k',
}

// ---- Simulate async delay ----
function delay(ms = 150): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms + Math.random() * 100))
}

// ---- Mock implementations of all API functions ----

// Skills / Platforms
export async function listPlatforms() { await delay(); return PLATFORMS }
export async function getPlatformSkills(platformId: string, workspaceDir?: string) {
  await delay()
  return workspaceDir ? makeSkills(platformId).slice(0, 2) : makeSkills(platformId)
}
export async function getSkillDetail(_platformId: string, skillName: string, _folder: string, workspaceDir?: string) {
  await delay(200); return makeSkillDetail(skillName)
}
export async function openSkillFolder() { await delay(200) }
export async function getDiffCandidates() { await delay(); return PLATFORMS.map(p => ({ id: p.id, display_name: p.display_name })) }
export async function diffSkills() { await delay(); return { skill_name: 'code-review', source_platform: 'claude-code', target_platform: 'codex', file_diffs: [] } }
export async function getSyncTargets() { await delay(); return PLATFORMS.map(p => ({ id: p.id, display_name: p.display_name })) }
export async function syncSkill() { await delay(500); return { success: true } }
export async function syncFolder() { await delay(500); return { success: true } }
export async function refreshPlatforms() { await delay(); return PLATFORMS }
export async function refreshPlatformSkills(platformId: string) { await delay(); return makeSkills(platformId) }
export async function getLocale() { return localStorage.getItem('ah-locale') || 'zh-CN' }
export async function setLocale(locale: string) { localStorage.setItem('ah-locale', locale) }
export async function searchSkills(query: string, workspaceDir?: string) {
  await delay()
  return makeSkills('claude-code')
    .filter(s => s.name.includes(query) || (s.description || '').includes(query))
    .map(s => ({ skill_name: s.name, folder: s.folder, platform_id: 'claude-code', platform_name: 'Claude Code', description: s.description }))
}
export async function readSkillFile(_platformId: string, _skillName: string, _folder: string, filePath: string, _workspaceDir?: string) {
  await delay()
  if (filePath.endsWith('.md')) {
    return `# ${_skillName}\n\nThis is a mock SKILL.md file.\n\n## Usage\n\nRun the skill with:\n\n\`\`\`bash\n/skill ${_skillName}\n\`\`\`\n\n## Configuration\n\nNo configuration required.\n`
  }
  if (filePath.endsWith('.json')) {
    return JSON.stringify({ name: _skillName, version: '1.0.0', description: 'Mock config' }, null, 2)
  }
  return `// Mock content for ${filePath}\nexport function main() {\n  console.log("Hello from ${_skillName}");\n}\n`
}
export async function deleteSkill() { await delay() }

// MCP
export async function listMcpPlatforms(workspaceDir?: string) {
  await delay()
  if (!workspaceDir) return MCP_PLATFORMS
  return MCP_PLATFORMS.map(platform => ({
    ...platform,
    server_count: 1,
    config_path: platform.id === 'claude-code'
      ? `${workspaceDir}/.mcp.json`
      : `${workspaceDir}/.cursor/mcp.json`,
  }))
}
export async function getMcpServers(platformId: string, workspaceDir?: string) {
  await delay()
  return workspaceDir ? makeMcpServers(platformId).slice(0, 1) : makeMcpServers(platformId)
}
export async function getMcpServer(_platformId: string, name: string, _workspaceDir?: string) {
  await delay()
  const configs: Record<string, any> = {
    github: { command: 'npx', args: ['-y', '@modelcontextprotocol/server-github'], env: { GITHUB_TOKEN: 'ghp_xxx' } },
    filesystem: { command: 'npx', args: ['-y', '@modelcontextprotocol/server-filesystem', '/home/user'] },
    postgres: { command: 'npx', args: ['-y', '@modelcontextprotocol/server-postgres', 'postgresql://localhost/mydb'] },
    'web-search': { command: 'npx', args: ['-y', '@anthropic/mcp-server-web-search'] },
  }
  return { config_text: JSON.stringify(configs[name] || {}, null, 2), format: 'json' }
}
export async function saveMcpServer() { await delay() }
export async function deleteMcpServer() { await delay() }
export async function importMcpServer() { await delay() }
export async function getMcpSyncTargets() { await delay(); return MCP_PLATFORMS.map(p => ({ id: p.id, display_name: p.display_name })) }
export async function previewMcpSync() { await delay(); return { changes: [] } }
export async function previewMcpChange() { await delay(); return { server_name: 'mock', target_format: 'json', target_config_path: '/mock/config.json', has_conflict: false, diff_lines: [{ tag: 'added', content: '  "mock": { "command": "echo" }\n' }], added: 1, removed: 0 } }
export async function syncMcpServer() { await delay(500) }

// Claude Code native plugins
export async function listClaudePlugins(workspaceDir?: string) {
  await delay()
  return (workspaceDir ? PROJECT_CLAUDE_PLUGINS : CLAUDE_PLUGINS).map(plugin => ({ ...plugin }))
}
export async function setClaudePluginEnabled(pluginId: string, scope: string, enabled: boolean) {
  await delay(400)
  const plugin = CLAUDE_PLUGINS.find(item => item.id === pluginId && item.scope === scope)
  if (!plugin || !plugin.manageable) throw new Error('Plugin scope is read-only')
  plugin.enabled = enabled
}

// Sessions
export async function listSessionPlatforms() { await delay(); return SESSION_PLATFORMS }
export async function listSessions(_platformId: string, _pathFilter: string, offset: number, limit: number) {
  await delay(200); return makeSessions(offset, limit)
}
export async function listSessionTerminals() {
  await delay()
  return [
    { id: 'iterm2', display_name: 'iTerm2', available: true },
    { id: 'terminal-default', display_name: 'Terminal', available: true },
    { id: 'wezterm', display_name: 'WezTerm', available: false },
  ]
}
export async function resumeSession() { await delay(300); return 'claude --resume session-0' }
export async function getSessionResumePreview(platformId: string, sessionId: string) {
  await delay(300)
  return {
    command: `cd '/Users/demo/projects/agent-hub' && ${platformId === 'codex' ? `codex resume ${sessionId}` : `claude --resume ${sessionId}`}`,
    last_user_message: '帮我把这个 Vue 3 组件重构成组合式函数,逻辑复用性太差了。',
    last_assistant_message: '已完成重构,逻辑抽到了 useSessionList 里,组件只负责渲染。',
  }
}
export async function getSessionMessages() {
  await delay();
  const now = Date.now();
  return [
    { role: 'user', content: '帮我把这个 Vue 3 组件重构成组合式函数,逻辑复用性太差了。', timestamp: now - 3600 * 1000 },
    { role: 'assistant', content: '好的,我先看一下现有组件的结构。\n\n建议拆成三个 composable:\n1. useFetchData — 负责数据加载与缓存\n2. usePagination — 分页状态\n3. useSelection — 多选逻辑\n\n这样每个函数职责单一,测试也方便。', timestamp: now - 3540 * 1000 },
    { role: 'user', content: 'useFetchData 里要不要加 AbortController?页面切换时旧请求还在跑。', timestamp: now - 3400 * 1000 },
    { role: 'assistant', content: '要加。在 composable 内部维护一个 controller,onScopeDispose 时 abort,新请求进来前先取消上一个:\n\nlet ctrl: AbortController | null = null\nasync function load() {\n  ctrl?.abort()\n  ctrl = new AbortController()\n  const res = await fetch(url, { signal: ctrl.signal })\n}', timestamp: now - 3300 * 1000 },
  ]
}
export async function deleteSession() { await delay(200) }
export async function deleteSessions(_platformId: string, sessionIds: string[]) {
  await delay(300)
  return { deleted: sessionIds.length, failed: [] }
}
export async function exportSessionsHtml(
  platformId: string,
  sessionIds: string[],
  outputPath: string,
  locale: string,
) {
  await delay(300)
  const isZh = locale.toLowerCase().startsWith('zh')
  const title = isZh ? '会话记录' : 'Session Transcript'
  const sessions = sessionIds.map((id, index) => `
    <section>
      <h2>${isZh ? '示例会话' : 'Sample session'} ${index + 1}</h2>
      <p><strong>${isZh ? '用户' : 'User'}：</strong>${id}</p>
      <p><strong>Agent：</strong>${isZh ? '这是浏览器调试模式生成的导出预览。' : 'This export preview was generated in web debug mode.'}</p>
    </section>`).join('')
  const html = `<!doctype html><html lang="${isZh ? 'zh-CN' : 'en'}"><meta charset="utf-8"><title>${title}</title><style>body{max-width:900px;margin:40px auto;padding:0 20px;font:16px/1.7 system-ui;color:#1e2a32}section{margin:20px 0;padding:20px;border:1px solid #ddd;border-radius:14px;background:#fff}</style><body><h1>${title}</h1><p>${platformId}</p>${sessions}</body></html>`
  if (typeof document !== 'undefined') {
    const url = URL.createObjectURL(new Blob([html], { type: 'text/html;charset=utf-8' }))
    const link = document.createElement('a')
    link.href = url
    link.download = outputPath
    document.body.appendChild(link)
    link.click()
    link.remove()
    window.setTimeout(() => URL.revokeObjectURL(url), 0)
  }
  return { path: outputPath, session_count: sessionIds.length, message_count: sessionIds.length * 2 }
}
export async function searchSessionMessages(platformId: string, query: string) {
  await delay();
  const q = query.toLowerCase();
  return [
    {
      session_id: 'session-0',
      session_title: 'Vue 3 frontend refactor',
      project_path: '/Users/demo/projects/agent-hub',
      platform_id: platformId,
      message: {
        role: 'user',
        content: `I have a question about how to implement ${query} and test it in Vue 3.`,
        timestamp: Date.now() - 3600 * 1000
      }
    },
    {
      session_id: 'session-0',
      session_title: 'Vue 3 frontend refactor',
      project_path: '/Users/demo/projects/agent-hub',
      platform_id: platformId,
      message: {
        role: 'assistant',
        content: `Here is the solution to implement ${query} using the existing component library.`,
        timestamp: Date.now() - 3500 * 1000
      }
    }
  ].filter(item => item.message.content.toLowerCase().includes(q));
}

// Trash
export async function listTrash() { await delay(); return [{ id: 'trash-1', name: 'old-skill', platform_id: 'claude-code' }] }
export async function restoreTrashItem() { await delay() }
export async function permanentlyDeleteTrashItem() { await delay() }
export async function emptyTrash() { await delay() }

// Monitor
export async function getActiveSessions() { await delay(); return [] }
export async function getMonitorConfig() { await delay(); return { notificationEnabled: false, notificationCooldownSecs: 300 } }
export async function setMonitorConfig() { await delay() }
export async function setMonitorPolling() { await delay() }
export async function forcePollMonitor() { await delay() }
export async function configureHooks() { await delay() }
export async function removeHooks() { await delay() }
export async function getHooksStatus() { await delay(); return {} }

// Switch
export async function listSwitchProfiles() { await delay(); return SWITCH_PROFILES }
export async function saveCurrentAuthProfile() { await delay() }
export async function addAuthProfile() { await delay() }
export async function switchAuthProfile() { await delay() }
export async function updateAuthProfileNote() { await delay() }
export async function deleteAuthProfile() { await delay() }
export async function getAuthProfileContent() { await delay(); return 'sk-ant-api03-mock-key-content...' }
export async function updateAuthProfileContent() { await delay() }
export async function clearActiveAuth() { await delay() }
export async function deleteActiveAuth() { await delay() }
export async function getCodexUsage() {
  await delay()
  // Simulate a Free plan: only a monthly primary window, no 5h/7d secondary.
  return {
    account_name: 'codex@example.com',
    plan_type: 'free',
    usage_windows: [
      { used_percent: 5, remaining_percent: 95, reset_after_seconds: 2505600, reset_at: 1782799999, window_seconds: 2592000 },
    ],
    primary_window: { used_percent: 5, remaining_percent: 95, reset_after_seconds: 2505600, reset_at: 1782799999, window_seconds: 2592000 },
    secondary_window: null,
    reset_credits: { available_count: 1 },
  }
}
export async function getCodexResetCredits() {
  await delay()
  // Simulate multiple banked credits to exercise the per-card layout:
  //   - one available, expiring in ~28d (the soonest one)
  //   - one already redeemed
  const availGranted = new Date(Date.now() - 2 * 86400_000).toISOString()
  const availExpires = new Date(Date.now() + 28 * 86400_000).toISOString()
  const usedGranted = new Date(Date.now() - 10 * 86400_000).toISOString()
  return {
    available_count: 1,
    next_expires_at: availExpires,
    credits: [
      {
        status: 'available',
        expires_at: availExpires,
        granted_at: availGranted,
        title: 'Full reset (Weekly + 5 hr)',
      },
      {
        status: 'redeemed',
        expires_at: null,
        granted_at: usedGranted,
        title: 'Full reset (Weekly + 5 hr)',
      },
    ],
  }
}
// In-memory mock cache mirrors the real 10-minute backend TTL.
const MOCK_USAGE_TTL_MS = 10 * 60 * 1000
let mockCodexTray: { at: number; data: any } | null = null
let mockGrokUsage: { at: number; data: any } | null = null
let mockKimiUsage: { at: number; data: any } | null = null

function mockCacheFresh(entry: { at: number } | null) {
  return Boolean(entry && Date.now() - entry.at < MOCK_USAGE_TTL_MS)
}

export async function getCodexTrayUsage(force = false) {
  if (!force && mockCacheFresh(mockCodexTray)) {
    return structuredClone(mockCodexTray!.data)
  }
  await delay()
  const now = Math.floor(Date.now() / 1000)
  const expiries = [46, 118, 190, 262].map(hours => new Date(Date.now() + hours * 3600_000).toISOString())
  const payload = {
    usage: {
      account_name: 'codex@example.com',
      plan_type: 'plus',
      usage_windows: [
        { used_percent: 39, remaining_percent: 61, reset_after_seconds: 7740, reset_at: now + 7740, window_seconds: 18000 },
        { used_percent: 61, remaining_percent: 39, reset_after_seconds: 291600, reset_at: now + 291600, window_seconds: 604800 },
        { used_percent: 12, remaining_percent: 88, reset_after_seconds: 1209600, reset_at: now + 1209600, window_seconds: 2592000 },
      ],
      primary_window: { used_percent: 39, remaining_percent: 61, reset_after_seconds: 7740, reset_at: now + 7740, window_seconds: 18000 },
      secondary_window: { used_percent: 61, remaining_percent: 39, reset_after_seconds: 291600, reset_at: now + 291600, window_seconds: 604800 },
      reset_credits: { available_count: 4 },
    },
    reset_credits: {
      available_count: 4,
      next_expires_at: expiries[0],
      credits: expiries.map(expiresAt => ({
        status: 'available',
        expires_at: expiresAt,
        granted_at: null,
        title: 'Full reset (Weekly + 5 hr)',
      })),
    },
    last_query_at: now,
  }
  mockCodexTray = { at: Date.now(), data: payload }
  return structuredClone(payload)
}
export async function getGrokUsage(force = false) {
  if (!force && mockCacheFresh(mockGrokUsage)) {
    return structuredClone(mockGrokUsage!.data)
  }
  await delay()
  const now = Math.floor(Date.now() / 1000)
  const payload = {
    account_name: 'default@grok.build',
    plan_type: 'SuperGrok',
    period_type: 'weekly',
    usage_window: {
      used_percent: 4,
      remaining_percent: 96,
      reset_after_seconds: 345600,
      reset_at: now + 345600,
      window_seconds: 604800,
    },
    limit_value: null,
    used_value: null,
    prepaid_balance: 0,
    on_demand_cap: 0,
    on_demand_used: 0,
    on_demand_enabled: false,
    source: 'live',
    fetched_at: now,
    stale: false,
  }
  mockGrokUsage = { at: Date.now(), data: payload }
  return structuredClone(payload)
}
export async function getUsageProviderAvailability() {
  return { codex: true, grok_build: true, kimi_code: true, claude_code: true }
}
export async function resizeUsageTray() {}
export async function setUsageTrayPinned() {}
export async function openUsageTray() {}

let mockClaudeUsage: { at: number; data: any } | null = null
export async function getClaudeUsage(force = false) {
  if (!force && mockCacheFresh(mockClaudeUsage)) {
    return structuredClone(mockClaudeUsage!.data)
  }
  await delay()
  const now = Math.floor(Date.now() / 1000)
  const window5h = {
    used_percent: 12,
    remaining_percent: 88,
    reset_after_seconds: 3_600 * 2,
    reset_at: now + 3_600 * 2,
    window_seconds: 18_000,
  }
  const windowWeekly = {
    used_percent: 31,
    remaining_percent: 69,
    reset_after_seconds: 86_400 * 5,
    reset_at: now + 86_400 * 5,
    window_seconds: 604_800,
  }
  const payload = {
    account_name: 'demo@anthropic.com',
    plan_type: 'max',
    window_5h: window5h,
    window_weekly: windowWeekly,
    usage_windows: [window5h, windowWeekly],
    fetched_at: now,
  }
  mockClaudeUsage = { at: Date.now(), data: payload }
  return structuredClone(payload)
}

export async function getKimiUsage(force = false) {
  if (!force && mockCacheFresh(mockKimiUsage)) {
    return structuredClone(mockKimiUsage!.data)
  }
  await delay()
  const now = Math.floor(Date.now() / 1000)
  const window5h = {
    used_percent: 40,
    remaining_percent: 60,
    reset_after_seconds: 3_600 * 3,
    reset_at: now + 3_600 * 3,
    window_seconds: 18_000,
  }
  const windowWeekly = {
    used_percent: 43,
    remaining_percent: 57,
    reset_after_seconds: 86_400 * 4,
    reset_at: now + 86_400 * 4,
    window_seconds: 604_800,
  }
  const payload = {
    account_name: 'demo@kimi.com',
    auth_method: 'METHOD_API_KEY',
    window_5h: window5h,
    window_weekly: windowWeekly,
    weekly_limit: 100,
    weekly_used: 43,
    usage_windows: [window5h, windowWeekly],
    fetched_at: now,
  }
  mockKimiUsage = { at: Date.now(), data: payload }
  return structuredClone(payload)
}

// Session monitor
let codexHookInstalled = false
let claudeHookInstalled = false
let grokHookInstalled = false
let kimiHookInstalled = false

let codexMonitorSessions = [
  {
    sessionId: '019f85-chatgpt',
    turnId: 'turn-running',
    source: 'chatgpt',
    status: 'running',
    cwd: '/Users/demo/projects/agent-hub',
    userPrompt: '实现 Codex 会话监听，并确保 Hook 安装过程不会影响已有配置。',
    assistantReply: null,
    updatedAt: Date.now() - 12_000,
  },
  {
    sessionId: '019f84-terminal',
    turnId: 'turn-ended',
    source: 'terminal',
    status: 'ended',
    cwd: '/Users/demo/projects/api-server',
    userPrompt: '检查登录接口偶发 401 的原因，并给出修复建议。',
    assistantReply: '问题来自刷新令牌并发更新，已增加单飞锁并补充回归测试。',
    updatedAt: Date.now() - 180_000,
  },
]

let claudeMonitorSessions = [
  {
    sessionId: 'c4f2a1-terminal',
    turnId: 'prompt-1',
    source: 'terminal',
    status: 'running',
    cwd: '/Users/demo/projects/web-app',
    userPrompt: '帮我把登录页改成暗色主题。',
    assistantReply: null,
    updatedAt: Date.now() - 30_000,
  },
]

let kiroMonitorSessions = [
  {
    sessionId: 'kiro-7d21',
    turnId: 'turn-1',
    source: 'terminal',
    status: 'ended',
    cwd: '/Users/demo/projects/data-pipeline',
    userPrompt: '优化定时任务的失败重试逻辑。',
    assistantReply: '已把固定间隔重试改成指数退避，并加了最大重试次数上限。',
    updatedAt: Date.now() - 600_000,
  },
]

let kiroMonitorEnabled = true

let grokMonitorSessions = [
  {
    sessionId: 'grok-9b3c',
    turnId: 'turn-1',
    source: 'terminal',
    status: 'running',
    cwd: '/Users/demo/projects/recommender',
    userPrompt: '把推荐接口的分页改成游标式，注意兼容旧参数。',
    assistantReply: null,
    updatedAt: Date.now() - 45_000,
  },
]

let kimiMonitorSessions = [
  {
    sessionId: 'kimi-51af',
    turnId: 'turn-1',
    source: 'terminal',
    status: 'ended',
    cwd: '/Users/demo/projects/notes-app',
    userPrompt: '给设置页加上导出全部笔记的入口。',
    assistantReply: '已在设置页新增导出按钮，支持 Markdown 打包下载。',
    updatedAt: Date.now() - 900_000,
  },
]

function makeHookStatus(installed: boolean, configPath: string, command: string) {
  return {
    installed,
    configPath,
    command,
    managedHandlerCount: installed ? 2 : 0,
    issue: null,
  }
}

function makeHookPreview(action: 'install' | 'uninstall', configPath: string, command: string) {
  const adding = action === 'install'
  const tag = adding ? 'added' : 'removed'
  return {
    action,
    configPath,
    command,
    beforeHash: 'mock-before-hash',
    added: adding ? 14 : 0,
    removed: adding ? 0 : 14,
    changed: true,
    diffLines: [
      { tag: 'context', content: '{' },
      { tag: 'context', content: '  "hooks": {' },
      { tag, content: `    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "…", "timeout": 10 }] }],` },
      { tag, content: `    "Stop": [{ "hooks": [{ "type": "command", "command": "…", "timeout": 10 }] }]` },
      { tag: 'context', content: '  }' },
      { tag: 'context', content: '}' },
    ],
  }
}

export async function getCodexSessionMonitorSnapshot() {
  await delay()
  return {
    revision: 2,
    sessions: codexMonitorSessions,
  }
}

export async function deleteCodexSessionMonitorSession(sessionId: string) {
  await delay()
  codexMonitorSessions = codexMonitorSessions.filter(
    session => session.sessionId !== sessionId,
  )
}

const CODEX_HOOK_COMMAND = "'/Applications/AGENT HUB.app/Contents/MacOS/agent-hub' --agent-hub-codex-hook"
const CLAUDE_HOOK_COMMAND = "'/Applications/AGENT HUB.app/Contents/MacOS/agent-hub' --agent-hub-claude-hook"

export async function getCodexHookStatus() {
  await delay()
  return makeHookStatus(codexHookInstalled, '~/.codex/hooks.json', CODEX_HOOK_COMMAND)
}

export async function previewCodexHookChange(action: 'install' | 'uninstall') {
  await delay()
  return makeHookPreview(action, '~/.codex/hooks.json', CODEX_HOOK_COMMAND)
}

export async function applyCodexHookChange(action: 'install' | 'uninstall', _expectedBeforeHash: string) {
  await delay(300)
  codexHookInstalled = action === 'install'
  return getCodexHookStatus()
}

export async function getClaudeSessionMonitorSnapshot() {
  await delay()
  return {
    revision: 1,
    sessions: claudeMonitorSessions,
  }
}

export async function deleteClaudeSessionMonitorSession(sessionId: string) {
  await delay()
  claudeMonitorSessions = claudeMonitorSessions.filter(
    session => session.sessionId !== sessionId,
  )
}

export async function getClaudeHookStatus() {
  await delay()
  return makeHookStatus(claudeHookInstalled, '~/.claude/settings.json', CLAUDE_HOOK_COMMAND)
}

export async function previewClaudeHookChange(action: 'install' | 'uninstall') {
  await delay()
  return makeHookPreview(action, '~/.claude/settings.json', CLAUDE_HOOK_COMMAND)
}

export async function applyClaudeHookChange(action: 'install' | 'uninstall', _expectedBeforeHash: string) {
  await delay(300)
  claudeHookInstalled = action === 'install'
  return getClaudeHookStatus()
}

export async function getKiroSessionMonitorSnapshot() {
  await delay()
  return {
    revision: 1,
    sessions: kiroMonitorSessions,
  }
}

export async function deleteKiroSessionMonitorSession(sessionId: string) {
  await delay()
  kiroMonitorSessions = kiroMonitorSessions.filter(
    session => session.sessionId !== sessionId,
  )
}

export async function getKiroMonitorStatus() {
  await delay()
  return {
    available: true,
    sessionsDir: '~/.kiro/sessions/cli',
    enabled: kiroMonitorEnabled,
  }
}

export async function setKiroMonitorEnabled(enabled: boolean) {
  await delay(200)
  kiroMonitorEnabled = enabled
  return getKiroMonitorStatus()
}

const GROK_HOOK_COMMAND = "'/Applications/AGENT HUB.app/Contents/MacOS/agent-hub' --agent-hub-grok-hook"
const KIMI_HOOK_COMMAND = "'/Applications/AGENT HUB.app/Contents/MacOS/agent-hub' --agent-hub-kimi-hook"

export async function getGrokSessionMonitorSnapshot() {
  await delay()
  return {
    revision: 1,
    sessions: grokMonitorSessions,
  }
}

export async function deleteGrokSessionMonitorSession(sessionId: string) {
  await delay()
  grokMonitorSessions = grokMonitorSessions.filter(
    session => session.sessionId !== sessionId,
  )
}

export async function getGrokHookStatus() {
  await delay()
  return makeHookStatus(grokHookInstalled, '~/.grok/hooks/agent-hub.json', GROK_HOOK_COMMAND)
}

export async function previewGrokHookChange(action: 'install' | 'uninstall') {
  await delay()
  return makeHookPreview(action, '~/.grok/hooks/agent-hub.json', GROK_HOOK_COMMAND)
}

export async function applyGrokHookChange(action: 'install' | 'uninstall', _expectedBeforeHash: string) {
  await delay(300)
  grokHookInstalled = action === 'install'
  return getGrokHookStatus()
}

export async function getKimiSessionMonitorSnapshot() {
  await delay()
  return {
    revision: 1,
    sessions: kimiMonitorSessions,
  }
}

export async function deleteKimiSessionMonitorSession(sessionId: string) {
  await delay()
  kimiMonitorSessions = kimiMonitorSessions.filter(
    session => session.sessionId !== sessionId,
  )
}

export async function getKimiHookStatus() {
  await delay()
  return makeHookStatus(kimiHookInstalled, '~/.kimi-code/config.toml', KIMI_HOOK_COMMAND)
}

export async function previewKimiHookChange(action: 'install' | 'uninstall') {
  await delay()
  return makeHookPreview(action, '~/.kimi-code/config.toml', KIMI_HOOK_COMMAND)
}

export async function applyKimiHookChange(action: 'install' | 'uninstall', _expectedBeforeHash: string) {
  await delay(300)
  kimiHookInstalled = action === 'install'
  return getKimiHookStatus()
}

// App
export async function getAppVersion() { return '0.9.3-dev' }
