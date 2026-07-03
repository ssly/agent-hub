/**
 * Mock API for browser-only debugging (no Tauri backend).
 * Provides fake data for all API functions so the UI can be
 * developed and tested in a regular browser via `npm run dev:web`.
 */

// ---- Fake data generators ----

const PLATFORMS = [
  {
    id: 'claude-code',
    display_name: 'Claude Code',
    skill_dir: '~/.claude/commands',
    skill_count: 12,
  },
  {
    id: 'codex',
    display_name: 'Codex',
    skill_dir: '~/.codex/skills',
    skill_count: 5,
  },
  {
    id: 'cursor',
    display_name: 'Cursor',
    skill_dir: '~/.cursor/commands',
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
  { id: 'claude-code', display_name: 'Claude Code', server_count: 4 },
  { id: 'cursor', display_name: 'Cursor', server_count: 2 },
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
  { id: 'claude-code', display_name: 'Claude Code', session_count: 28 },
  { id: 'codex', display_name: 'Codex', session_count: 5 },
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
export async function getPlatformSkills(platformId: string) { await delay(); return makeSkills(platformId) }
export async function getSkillDetail(_platformId: string, skillName: string, _folder: string) {
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
export async function searchSkills(query: string) {
  await delay()
  return makeSkills('claude-code')
    .filter(s => s.name.includes(query) || (s.description || '').includes(query))
    .map(s => ({ skill_name: s.name, folder: s.folder, platform_id: 'claude-code', platform_name: 'Claude Code', description: s.description }))
}
export async function readSkillFile(_platformId: string, _skillName: string, _folder: string, filePath: string) {
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
export async function listMcpPlatforms() { await delay(); return MCP_PLATFORMS }
export async function getMcpServers(platformId: string) { await delay(); return makeMcpServers(platformId) }
export async function getMcpServer(_platformId: string, name: string) {
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
export async function getSessionMessages() { await delay(); return [] }
export async function deleteSession() { await delay(200) }
export async function deleteSessions(_platformId: string, sessionIds: string[]) {
  await delay(300)
  return { deleted: sessionIds.length, failed: [] }
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
    plan_type: 'free',
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

// App
export async function getAppVersion() { return '0.9.3-dev' }
