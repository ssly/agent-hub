import { I18n } from './i18n.js';
import * as Api from './api.js';

// --- Tauri Updater helpers (no bundler) ---
const tauriInvoke = window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
const transformCallback = window.__TAURI_INTERNALS__.transformCallback.bind(window.__TAURI_INTERNALS__);

class TauriChannel {
    constructor() {
        this._onmessage = () => {};
        this._nextIndex = 0;
        this._pending = {};
        this._endIndex = undefined;
        this.id = transformCallback((raw) => {
            const idx = raw.index;
            if ('end' in raw) {
                if (idx === this._nextIndex) {
                    this._cleanup();
                } else {
                    this._endIndex = idx;
                }
                return;
            }
            const msg = raw.message;
            if (idx === this._nextIndex) {
                this._onmessage(msg);
                this._nextIndex++;
                while (this._nextIndex in this._pending) {
                    this._onmessage(this._pending[this._nextIndex]);
                    delete this._pending[this._nextIndex];
                    this._nextIndex++;
                }
                if (this._nextIndex === this._endIndex) {
                    this._cleanup();
                }
            } else {
                this._pending[idx] = msg;
            }
        });
    }
    _cleanup() {
        window.__TAURI_INTERNALS__.unregisterCallback?.(this.id);
    }
    set onmessage(handler) {
        this._onmessage = handler;
    }
    get onmessage() {
        return this._onmessage;
    }
    __TAURI_TO_IPC_KEY__() {
        return `__CHANNEL__:${this.id}`;
    }
    toJSON() {
        return this.__TAURI_TO_IPC_KEY__();
    }
}

async function checkUpdate() {
    try {
        return await tauriInvoke('plugin:updater|check');
    } catch { return null; }
}

// Use the custom backend command (download_and_install_update_resumable) so we
// receive fine-grained progress events that include `total` and `downloaded`
// on every Progress event. The built-in `plugin:updater|download_and_install`
// only emits `chunkLength`, which is not enough to show a real progress bar.
async function downloadAndInstall(rid, onProgress) {
    const channel = new TauriChannel();
    if (onProgress) {
        channel.onmessage = onProgress;
    }
    await tauriInvoke('download_and_install_update_resumable', { rid, onEvent: channel });
}

function getErrorMessage(error) {
    if (error == null) return 'Unknown error';
    if (typeof error === 'string') return error;
    if (error instanceof Error) {
        return error.message || String(error);
    }
    if (typeof error === 'object') {
        // Tauri CommandError is serialized as externally-tagged enum,
        // e.g. { SyncError: "..." } / { NotFound: "..." } / { General: "..." }.
        for (const key of Object.keys(error)) {
            const value = error[key];
            if (typeof value === 'string' && value.length > 0) {
                return value;
            }
        }
        // Common shapes that other libraries may produce.
        if (typeof error.message === 'string') return error.message;
        if (typeof error.error === 'string') return error.error;
        try {
            return JSON.stringify(error);
        } catch {
            return String(error);
        }
    }
    return String(error);
}

function classifyUpdateError(rawError) {
    const msg = String(rawError ?? '').toLowerCase();
    if (!msg) return 'other';
    if (msg.includes('timeout') || msg.includes('timed out') || msg.includes('超时')) {
        return 'timeout';
    }
    if (msg.includes('signature') || msg.includes('minisign') || msg.includes('签名')) {
        return 'signature';
    }
    if (
        msg.includes('dns') ||
        msg.includes('getaddrinfo') ||
        msg.includes('connection') ||
        msg.includes('connect ') ||
        msg.includes('connect:') ||
        msg.includes('unreachable') ||
        msg.includes('tls') ||
        msg.includes('ssl') ||
        msg.includes('certificate') ||
        msg.includes('offline') ||
        msg.includes('error sending request') ||
        msg.includes('network')
    ) {
        return 'network';
    }
    if (msg.includes('status:') || msg.includes('http')) {
        return 'http';
    }
    return 'other';
}

function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes < 0) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(1)} KB`;
    const mb = kb / 1024;
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    const gb = mb / 1024;
    return `${gb.toFixed(2)} GB`;
}

function parseNonNegativeNumber(value) {
    const n = Number(value);
    return Number.isFinite(n) && n >= 0 ? n : null;
}

function parseOptionalBoolean(value) {
    if (typeof value === 'boolean') return value;
    return null;
}

function normalizeDownloadEventKind(value) {
    const normalized = String(value ?? '').trim().toLowerCase();
    if (normalized === 'started' || normalized === 'start') return 'started';
    if (normalized === 'progress' || normalized === 'downloadprogress') return 'progress';
    if (normalized === 'finished' || normalized === 'finish' || normalized === 'done') return 'finished';
    return '';
}

function formatInt(value) {
    const n = Number(value);
    if (!Number.isFinite(n)) return '--';
    return Math.round(n).toLocaleString();
}

function parseDownloadProgressEvent(event) {
    if (event == null) {
        return {
            kind: '',
            chunkLength: null,
            contentLength: null,
            total: null,
            downloaded: null,
            resumedFrom: null,
            usedResume: null,
        };
    }

    let root = event;
    if (typeof root === 'string') {
        try {
            root = JSON.parse(root);
        } catch {
            root = {};
        }
    }
    if (Array.isArray(root)) {
        root = root.length > 0 ? root[root.length - 1] : {};
    }
    if (!root || typeof root !== 'object') {
        return {
            kind: '',
            chunkLength: null,
            contentLength: null,
            total: null,
            downloaded: null,
            resumedFrom: null,
            usedResume: null,
        };
    }

    // Some channel bridges wrap payload under `payload`.
    if (
        root.payload &&
        typeof root.payload === 'object' &&
        root.event == null &&
        root.kind == null &&
        root.type == null
    ) {
        root = root.payload;
    }

    let kind = normalizeDownloadEventKind(root.event ?? root.kind ?? root.type);
    let payload = root.data && typeof root.data === 'object' ? root.data : root;

    // Compatibility for externally-tagged shapes like { Started: { ... } }.
    if (!kind) {
        const variantKey = ['started', 'progress', 'finished', 'Started', 'Progress', 'Finished']
            .find((key) => root[key] && typeof root[key] === 'object');
        if (variantKey) {
            kind = normalizeDownloadEventKind(variantKey);
            payload = root[variantKey];
        }
    }

    if (!kind) {
        const hasProgressShape = parseNonNegativeNumber(payload.chunkLength ?? payload.chunk_length) !== null
            || parseNonNegativeNumber(payload.downloaded) !== null;
        const hasStartShape = parseNonNegativeNumber(payload.resumedFrom ?? payload.resumed_from) !== null
            || parseNonNegativeNumber(payload.contentLength ?? payload.content_length) !== null;
        if (hasProgressShape) {
            kind = 'progress';
        } else if (hasStartShape) {
            kind = 'started';
        }
    }

    return {
        kind,
        chunkLength: parseNonNegativeNumber(payload.chunkLength ?? payload.chunk_length),
        contentLength: parseNonNegativeNumber(payload.contentLength ?? payload.content_length),
        total: parseNonNegativeNumber(payload.total),
        downloaded: parseNonNegativeNumber(payload.downloaded),
        resumedFrom: parseNonNegativeNumber(payload.resumedFrom ?? payload.resumed_from),
        usedResume: parseOptionalBoolean(payload.usedResume ?? payload.used_resume),
    };
}

function relaunchApp() {
    return tauriInvoke('plugin:process|restart');
}

// SVG Icons (replacing emoji)
const Icons = {
    trash: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>',
    symlink: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>',
    warning: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>',
    sync: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/></svg>',
    arrowRight: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>',
    dot: '<svg width="8" height="8" viewBox="0 0 8 8"><circle cx="4" cy="4" r="4" fill="currentColor"/></svg>',
    plus: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>',
    back: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>',
    search: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>',
    folder: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>',
    refresh: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>',
    diff: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>',
    globe: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>',
    download: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>',
    // v1.1 icons
    grid22: '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>',
    code22: '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>',
    clock22: '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>',
    cube22: '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>',
    tag18: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>',
    cube18: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>',
    layers18: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 2 7 12 12 22 7 12 2"/><polyline points="2 17 12 22 22 17"/><polyline points="2 12 12 17 22 12"/></svg>',
    folder18: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>',
    file18: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>',
    fileSmall: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>',
    moreVert: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="5" r="1" fill="currentColor"/><circle cx="12" cy="12" r="1" fill="currentColor"/><circle cx="12" cy="19" r="1" fill="currentColor"/></svg>',
    sortAsc: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="18 15 12 9 6 15"/></svg>',
    sortDesc: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>',
    sortBoth: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="18 8 12 2 6 8"/><polyline points="6 16 12 22 18 16"/></svg>',
    code32: '<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>',
};

function avatarToneFromName(name) {
    const s = (name || '').toLowerCase();
    let h = 2166136261 >>> 0;
    for (let i = 0; i < s.length; i++) {
        h ^= s.charCodeAt(i);
        h = Math.imul(h, 16777619) >>> 0;
    }
    return ((h % 8) + 1);
}

function skillIconSvg(name) {
    const n = (name || '').toLowerCase();
    if (n.includes('code') || n.includes('dev') || n.includes('program')) return Icons.code32;
    return `<span style="font-size:15px;font-weight:700;line-height:1">${esc((name||'?').charAt(0).toUpperCase())}</span>`;
}

class App {
    constructor() {
        this.platforms = [];
        this.skills = [];
        this.selectedPlatformId = null;
        this.selectedSkillName = null;
        this.selectedFolder = '';
        this.currentView = 'skills';
        this.currentTab = 'skills'; // 'skills' | 'mcp' | 'sessions' | 'switch'
        this.diffResult = null;
        this.searchResults = [];
        this.searchLoading = false;
        this.searchQuery = '';
        this.fileViewing = null;
        this.i18n = new I18n();
        this.collapsedFolders = new Set();
        this.pendingSyncTarget = null;
        this.pendingDeleteTarget = null; // { type: 'skill'|'mcp', key: string }
        // MCP state
        this.mcpPlatforms = [];
        this.mcpServers = [];
        this.mcpServerDetails = {};  // name -> { config_text, format, editing }
        this.expandedMcpServer = null;  // name of currently expanded server
        this.selectedMcpPlatform = null;
        // Session state
        this.sessionPlatforms = [];
        this.selectedSessionPlatform = null;
        this.sessions = [];
        this.sessionTotal = 0;
        this.sessionOffset = 0;
        this.sessionHasMore = false;
        this.sessionLoadingMore = false;
        this.isSessionsLoading = false;
        this.sessionsLoadError = '';
        this.selectedSessionPathFilter = 'all';
        this.sessionPathOptions = ['all', 'unknown'];
        this.sessionPageSize = 50;
        this.sessionMessagePageSize = 50;
        this.sessionTerminals = [];
        this.selectedSessionTerminal = 'terminal-default';
        this.resumingSessionId = null;
        this.confirmingSessionDeleteId = null;
        this.sessionDeleteConfirmTimer = null;
        this.deletingSessionId = null;
        this.sidebarCollapsed = false;
        // Skill table state (v1.1)
        this.skillPage = 0;
        this.skillPageSize = 10;
        this.skillSortBy = 'size';
        this.skillSortDir = 'desc';
        this.openKebabRow = null; // name of skill with open kebab menu
        // Monitor state
        this.monitorSessions = [];
        this.monitorConfig = null;
        this.selectedMonitorAgent = 'all';
        this.monitorUnlisten = null;
        this.hooksStatus = {};
        // Switch state
        this.switchSelectedAgent = null; // 'codex' | 'claude-code'
        this.switchProfiles = [];
        this.switchCurrentKey = null;
        this.switchAddFormOpen = false;
        this.switchClearConfirmOpen = false;
        this.switchEditingId = null;
        this.switchEditingContentId = null;
        this.switchContentCache = {}; // id -> json string
        this.switchDeleteConfirmId = null; // id of profile pending delete confirmation
        this.switchAccountConfirmId = null; // id of profile pending switch confirmation
        this._switchOutsideClickHandler = null;
        // Trash state
        this.trashCount = 0;
        // Update state
        this.update = null; // Update object when available
        this.appVersion = '...';
    }

    async init() {
        this.i18n.locale = await Api.getLocale();
        await this.i18n.load();
        await this.refreshPlatforms();
        await this.refreshMcpPlatforms();
        await this.refreshTrashCount();
        await this.loadAppVersion();
        this.bindEvents();
        this.render();
        this.checkForUpdate();
    }

    async loadAppVersion() {
        try {
            this.appVersion = await Api.getAppVersion();
        } catch {
            this.appVersion = '0.0.0';
        }
    }

    _reconcilePlatformSelection() {
        if (this.platforms.length === 0) {
            this.selectedPlatformId = null;
            this.selectedSkillName = null;
            this.selectedFolder = '';
            this.currentView = 'skills';
            return;
        }
        const stillExists = this.platforms.some(p => p.id === this.selectedPlatformId);
        if (!stillExists) {
            this.selectedPlatformId = this.platforms[0].id;
            this.selectedSkillName = null;
            this.selectedFolder = '';
            this.currentView = 'skills';
        }
    }

    async reloadPlatforms() {
        this.platforms = await Api.listPlatforms();
        this._reconcilePlatformSelection();
        if (this.selectedPlatformId) {
            await this.loadSkills();
        }
        this.renderSidebar();
    }

    async refreshPlatforms() {
        this.platforms = await Api.refreshPlatforms();
        this._reconcilePlatformSelection();
        if (this.selectedPlatformId) {
            await this.loadSkills();
        }
        this.renderSidebar();
    }

    async loadSkills() {
        this.skills = await Api.getPlatformSkills(this.selectedPlatformId);
    }

    selectPlatform(id) {
        this.selectedPlatformId = id;
        this.selectedSkillName = null;
        this.selectedFolder = '';
        this.currentView = 'skills';
        this.skillPage = 0;
        this.loadSkills().then(() => this.render());
    }

    async selectSkill(name, folder) {
        this.selectedSkillName = name;
        this.selectedFolder = folder || '';
        this.currentView = 'detail';
        this.render();
    }

    backToList() {
        this.selectedSkillName = null;
        this.selectedFolder = '';
        this.currentView = 'skills';
        this.diffResult = null;
        this.render();
    }

    async doDiff(targetPlatformId) {
        this.diffResult = await Api.diffSkills(this.selectedPlatformId, targetPlatformId, this.selectedSkillName, this.selectedFolder);
        this.currentView = 'diff';
        this.closeModal();
        this.render();
    }

    async doSync(targetPlatformId, overwrite) {
        try {
            await Api.syncSkill(this.selectedPlatformId, targetPlatformId, this.selectedSkillName, this.selectedFolder, overwrite);
            this.closeModal();
            await this.reloadPlatforms();
            this.currentView = 'skills';
            this.selectedSkillName = null;
            this.selectedFolder = '';
            this.render();
        } catch (e) {
            this.showToast(this.i18n.tWith('sync.failed', { error: e.SyncError || e }), 'error');
        }
    }

    async doFolderSync(targetPlatformId, folder) {
        try {
            const result = await Api.syncFolder(this.selectedPlatformId, targetPlatformId, folder);
            this.closeModal();
            await this.reloadPlatforms();
            this.currentView = 'skills';
            this.render();
            const i = this.i18n;
            this.showToast(i.tWith('sync.done') + ` (${result.synced}/${result.total})`, 'success');        } catch (e) {
            this.showToast(this.i18n.tWith('sync.failed', { error: e.SyncError || e }), 'error');
        }
    }

    async deleteSkill(name, folder, btn) {
        const i = this.i18n;
        const key = `${this.selectedPlatformId}:${folder}:${name}`;
        if (btn.dataset.confirming === 'true') {
            try {
                await Api.deleteSkill(this.selectedPlatformId, name, folder);
                if (this.selectedSkillName === name && this.selectedFolder === folder) {
                    this.selectedSkillName = null;
                    this.selectedFolder = '';
                    this.currentView = 'skills';
                }
                await this.reloadPlatforms();
                await this.refreshTrashCount();
                this.render();
            } catch (e) {
                this.showToast(i.tWith('skill.delete_failed', { error: e.SyncError || e }), 'error');
            }
            return;
        }
        btn.dataset.confirming = 'true';
        btn.textContent = i.t('skill.confirm_delete');
        btn.classList.remove('text-red-600', 'hover:text-red-400');
        btn.classList.add('bg-red-700', 'hover:bg-red-600', 'text-white', 'rounded', 'px-2', 'py-0.5');
        const reset = () => {
            btn.dataset.confirming = 'false';
            btn.textContent = i.t('skill.delete');
            btn.classList.add('text-red-600', 'hover:text-red-400');
            btn.classList.remove('bg-red-700', 'hover:bg-red-600', 'text-white', 'rounded', 'px-2', 'py-0.5');
        };
        setTimeout(reset, 3000);
        const onClickOutside = (e) => {
            if (!btn.contains(e.target)) { reset(); document.removeEventListener('click', onClickOutside); }
        };
        setTimeout(() => document.addEventListener('click', onClickOutside), 0);
    }

    async doSearch(query) {
        if (!query.trim()) {
            this.currentView = 'skills';
            this.searchResults = [];
            this.searchLoading = false;
            this.searchQuery = '';
            this.render();
            return;
        }
        this.searchQuery = query;
        this.searchLoading = true;
        this.currentView = 'search';
        this.render();
        try {
            const results = await Api.searchSkills(query);
            // Ignore stale results if the user typed again while we were loading
            if (this.searchQuery !== query) return;
            this.searchResults = results;
        } finally {
            if (this.searchQuery === query) {
                this.searchLoading = false;
                this.render();
            }
        }
    }

    // --- Update ---
    async checkForUpdate() {
        try {
            const update = await checkUpdate();
            if (update) {
                this.update = update;
                this.renderUpdateBadge();
            }
        } catch {}
    }

    renderVersion() {
        const el = document.getElementById('version-label');
        if (el) el.textContent = `v${this.appVersion}`;
    }

    renderUpdateBadge() {
        if (!this.update) return;
        const el = document.getElementById('update-badge');
        const i = this.i18n;
        el.className = 'p-2 border-t border-gray-700 cursor-pointer hover:bg-gray-700/50';
        el.innerHTML = `<div class="flex items-center gap-1.5 px-1">
            <span class="text-green-400 flex-shrink-0">${Icons.dot}</span>
            <span class="text-xs text-gray-400">${i.t('update.badge')}</span>
            <span class="text-xs text-gray-500">v${esc(this.update.version)}</span>
        </div>`;
        el.onclick = () => this.showUpdateModal();
    }

    showUpdateModal() {
        const i = this.i18n;
        const update = this.update;
        const transition = i.tWith('update.transition', { current: update.currentVersion, latest: update.version });
        const html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-1">${i.t('update.title')}</h2>
            <p class="text-sm text-green-400 font-mono mb-4">${esc(transition)}</p>
            <div id="update-progress-wrap" class="mb-4 hidden">
                <div style="height:8px;background:#111827;border-radius:9999px;overflow:hidden;border:1px solid #374151;">
                    <div id="update-progress-bar" style="height:100%;width:0%;background:linear-gradient(90deg,#34d399,#10b981);transition:width .2s ease;"></div>
                </div>
                <div id="update-progress-text" class="text-xs text-gray-400 mt-2 font-mono">0%</div>
            </div>
            <div id="update-error" class="text-sm text-red-400 mb-3 hidden whitespace-pre-wrap break-words"></div>
            <div id="update-actions" class="flex gap-3 justify-end">
                <button class="px-4 py-2 bg-green-700 hover:bg-green-600 rounded text-sm cursor-pointer update-confirm-btn">${i.t('update.confirm')}</button>
                <button class="px-4 py-2 text-gray-400 hover:text-white text-sm cursor-pointer update-close-btn">${i.t('action.cancel')}</button>
            </div>
        </div>`;
        this.openModal(html);
        this.bindUpdateConfirm();
        this.bindUpdateClose();
    }

    bindUpdateConfirm() {
        const btn = this.modalEl().querySelector('.update-confirm-btn');
        if (btn) btn.addEventListener('click', () => this.doUpdate());
    }

    bindUpdateClose() {
        const btn = this.modalEl().querySelector('.update-close-btn');
        if (btn) btn.addEventListener('click', () => this.closeModal());
    }

    async doUpdate() {
        const i = this.i18n;
        const progressWrapEl = document.getElementById('update-progress-wrap');
        const progressBarEl = document.getElementById('update-progress-bar');
        const progressTextEl = document.getElementById('update-progress-text');
        const errorEl = document.getElementById('update-error');
        const actionsEl = document.getElementById('update-actions');
        const confirmBtn = this.modalEl().querySelector('.update-confirm-btn');
        const closeBtn = this.modalEl().querySelector('.update-close-btn');
        const DOWNLOAD_TIMEOUT_MS = 10 * 60 * 1000; // 10 minutes
        let downloadTimer = null;
        let totalBytes = 0;

        const setProgress = (percent, downloaded, total) => {
            const p = Math.max(0, Math.min(100, percent));
            if (progressBarEl) progressBarEl.style.width = `${p.toFixed(1)}%`;
            if (!progressTextEl) return;
            if (Number.isFinite(total) && total > 0) {
                progressTextEl.textContent = `${p.toFixed(1)}%  ${formatBytes(downloaded)} / ${formatBytes(total)}`;
            } else if (Number.isFinite(downloaded) && downloaded > 0) {
                progressTextEl.textContent = formatBytes(downloaded);
            } else {
                progressTextEl.textContent = `${p.toFixed(1)}%`;
            }
        };

        const showError = (rawError) => {
            const baseMsg = getErrorMessage(rawError);
            const category = classifyUpdateError(baseMsg);
            const reasonKey = {
                network: 'update.reason_network',
                signature: 'update.reason_signature',
                timeout: 'update.reason_timeout',
                http: 'update.reason_http',
                other: 'update.reason_other',
            }[category] || 'update.reason_other';
            const friendlyMsg = i.tWith('update.failed_with_reason', {
                reason: i.t(reasonKey),
                error: baseMsg,
            });
            if (errorEl) {
                errorEl.textContent = friendlyMsg;
                errorEl.classList.remove('hidden');
            }
            if (progressTextEl) {
                progressTextEl.textContent = '';
            }
            if (actionsEl) {
                actionsEl.innerHTML = `
                    <button class="px-4 py-2 bg-green-700 hover:bg-green-600 rounded text-sm cursor-pointer update-retry-btn">${i.t('update.retry')}</button>
                    <button class="px-4 py-2 text-gray-400 hover:text-white text-sm cursor-pointer update-close-btn">${i.t('action.close')}</button>`;
                actionsEl.querySelector('.update-retry-btn').addEventListener('click', () => {
                    // Reset UI state and kick off the download again.
                    if (errorEl) {
                        errorEl.textContent = '';
                        errorEl.classList.add('hidden');
                    }
                    actionsEl.innerHTML = `
                        <button class="px-4 py-2 bg-green-700 hover:bg-green-600 rounded text-sm cursor-pointer update-confirm-btn opacity-60" disabled>${i.t('update.confirm')}</button>
                        <button class="px-4 py-2 text-gray-400 hover:text-white text-sm cursor-pointer update-close-btn">${i.t('action.close')}</button>`;
                    this.bindUpdateClose();
                    this.doUpdate();
                });
                this.bindUpdateClose();
            }
        };

        try {
            const updateRid = this.update?.rid;
            if (typeof updateRid !== 'number') {
                throw new Error('Missing update rid from updater check result');
            }
            if (confirmBtn) {
                confirmBtn.disabled = true;
                confirmBtn.classList.add('opacity-60');
            }
            if (closeBtn) {
                // Disallow closing mid-download; we cannot abort the backend yet.
                closeBtn.classList.add('hidden');
            }
            if (errorEl) {
                errorEl.textContent = '';
                errorEl.classList.add('hidden');
            }
            progressWrapEl?.classList.remove('hidden');
            setProgress(0, 0, 0);

            const onProgress = (event) => {
                const parsed = parseDownloadProgressEvent(event);
                if (!parsed.kind) return;
                if (parsed.kind === 'started') {
                    const total = parsed.total ?? parsed.contentLength ?? 0;
                    totalBytes = total || 0;
                    const downloaded = parsed.downloaded ?? 0;
                    const percent = totalBytes > 0 ? (downloaded / totalBytes) * 100 : 0;
                    setProgress(percent, downloaded, totalBytes);
                } else if (parsed.kind === 'progress') {
                    const total = parsed.total ?? totalBytes;
                    if (total && total > totalBytes) totalBytes = total;
                    const downloaded = parsed.downloaded ?? 0;
                    const percent = totalBytes > 0 ? (downloaded / totalBytes) * 100 : 0;
                    setProgress(percent, downloaded, totalBytes);
                } else if (parsed.kind === 'finished') {
                    const downloaded = parsed.downloaded ?? totalBytes;
                    setProgress(100, downloaded, totalBytes || downloaded);
                }
            };

            // 5-minute hard timeout guard.
            const timeoutPromise = new Promise((_, reject) => {
                downloadTimer = setTimeout(() => {
                    reject(new Error(i.t('update.timeout_error')));
                }, DOWNLOAD_TIMEOUT_MS);
            });

            await Promise.race([
                downloadAndInstall(updateRid, onProgress),
                timeoutPromise,
            ]);

            clearTimeout(downloadTimer);
            setProgress(100, totalBytes, totalBytes);
            await relaunchApp();
        } catch (e) {
            if (downloadTimer) clearTimeout(downloadTimer);
            showError(e);
        }
    }

    async switchLang() {
        await this.i18n.switchLocale();
        await Api.setLocale(this.i18n.locale);
        this.render();
    }

    // --- Events ---
    bindEvents() {
        document.getElementById('btn-refresh').addEventListener('click', () => this.handleRefreshClick());
        document.getElementById('btn-lang').addEventListener('click', () => this.switchLang());
        document.getElementById('btn-back').addEventListener('click', () => this.backToList());
        document.getElementById('btn-diff').addEventListener('click', () => this.showDiffModal());
        document.getElementById('btn-sync').addEventListener('click', () => this.showSyncModal());
        document.getElementById('btn-scan-invalid').addEventListener('click', () => this.scanInvalidSkills());
        document.getElementById('btn-sidebar-toggle').addEventListener('click', () => {
            this.sidebarCollapsed = !this.sidebarCollapsed;
            document.getElementById('sidebar').classList.toggle('collapsed', this.sidebarCollapsed);
        });

        let debounce;
        document.getElementById('search-input').addEventListener('input', (e) => {
            clearTimeout(debounce);
            debounce = setTimeout(() => this.doSearch(e.target.value), 300);
        });

        document.getElementById('modal-overlay').addEventListener('click', (e) => {
            if (e.target.id === 'modal-overlay') this.closeModal();
        });
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') this.closeModal();
        });

        // Tab switching
        document.getElementById('tab-skills').addEventListener('click', () => this.switchTab('skills'));
        document.getElementById('tab-mcp').addEventListener('click', () => this.switchTab('mcp'));
        document.getElementById('tab-sessions').addEventListener('click', () => this.switchTab('sessions'));
        document.getElementById('tab-monitor').addEventListener('click', () => this.switchTab('monitor'));
        document.getElementById('tab-switch').addEventListener('click', () => this.switchTab('switch'));
    }

    async handleRefreshClick() {
        const btn = document.getElementById('btn-refresh');
        const icon = btn?.querySelector('svg');
        if (!btn || !icon || btn.disabled) return;

        btn.disabled = true;
        icon.classList.add('animate-spin');

        try {
            if (this.currentTab === 'mcp') {
                await this.refreshMcpPlatforms();
                this.render();
            } else if (this.currentTab === 'sessions') {
                this.isSessionsLoading = true;
                this.render();
                await Promise.all([
                    this.refreshSessionPlatforms({ keepCurrentPathFilter: true }),
                    this.refreshSessionTerminals(),
                ]);
                this.isSessionsLoading = false;
                this.render();
            } else {
                await this.refreshPlatforms();
                this.render();
            }
        } finally {
            this.isSessionsLoading = false;
            icon.classList.remove('animate-spin');
            btn.disabled = false;
        }
    }

    switchTab(tab) {
        // Monitor tab is currently disabled — ignore activation requests.
        if (tab === 'monitor') return;
        this.currentTab = tab;
        if (tab === 'mcp') {
            this.stopMonitorListener();
            Api.setMonitorPolling(false);
            this.refreshMcpPlatforms().then(() => this.render());
            return;
        }
        if (tab === 'sessions') {
            this.stopMonitorListener();
            Api.setMonitorPolling(false);
            this.isSessionsLoading = true;
            this.render();
            Promise.all([
                this.refreshSessionPlatforms(),
                this.refreshSessionTerminals(),
            ]).finally(() => {
                this.isSessionsLoading = false;
                this.render();
            });
            return;
        }
        if (tab === 'monitor') {
            Api.setMonitorPolling(true);
            this.startMonitorListener();
            this.refreshMonitor().finally(() => this.render());
            return;
        }
        if (tab === 'switch') {
            this.stopMonitorListener();
            Api.setMonitorPolling(false);
            this.switchAddFormOpen = false;
            this.switchClearConfirmOpen = false;
            this.switchEditingId = null;
            this.switchEditingContentId = null;
            this.switchContentCache = {};
            this.switchDeleteConfirmId = null;
            this.switchAccountConfirmId = null;
            if (!this.switchSelectedAgent) {
                this.switchSelectedAgent = 'codex';
            }
            this.loadSwitchProfiles();
            return;
        }
        this.stopMonitorListener();
        Api.setMonitorPolling(false);
        this.currentView = 'skills';
        this.render();
    }

    // --- Invalid Skill Scanner ---
    async scanInvalidSkills() {
        const i = this.i18n;
        const btn = document.getElementById('btn-scan-invalid');
        const originalText = btn.innerHTML;
        btn.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="animate-spin"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>`;
        btn.disabled = true;
        try {
            const invalid = await Api.scanInvalidSkills();
            this.showInvalidSkillsModal(invalid);
        } catch (e) {
            console.error('Scan invalid skills error:', e);
            this.showToast(i.tWith('scan_invalid.error', { error: e.message || e.SyncError || e }), 'error');
        } finally {
            btn.innerHTML = originalText;
            btn.disabled = false;
        }
    }

    showInvalidSkillsModal(invalid) {
        const i = this.i18n;
        if (invalid.length === 0) {
            this.showToast(i.t('scan_invalid.all_good'), 'success');
            return;
        }
        const fixPrompt = this.buildFixPrompt(invalid);
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-1">${i.tWith('scan_invalid.title', { count: invalid.length })}</h2>
            <p class="text-xs text-gray-500 mb-4">${i.t('scan_invalid.subtitle')}</p>
            <div class="space-y-1 mb-4 max-h-[40vh] overflow-y-auto">`;
        for (const item of invalid) {
            html += `<div class="flex items-start gap-2 px-3 py-2 rounded bg-gray-900/50">
                <span class="text-yellow-500 flex-shrink-0 mt-0.5">${Icons.warning}</span>
                <div class="flex-1 min-w-0">
                    <div class="text-sm text-gray-200 truncate" title="${esc(item.path)}">${esc(item.path)}</div>
                    <div class="text-xs text-gray-500">${esc(item.platform_name)} · <span class="text-red-400">${esc(item.reason)}</span></div>
                </div>
            </div>`;
        }
        html += `</div>
            <div class="border-t border-gray-700 pt-4">
                <div class="flex items-center justify-between mb-2">
                    <span class="text-xs text-gray-400">${i.t('scan_invalid.fix_prompt_label')}</span>
                    <button class="text-xs text-cyan-400 hover:text-cyan-300 cursor-pointer flex items-center gap-1 scan-invalid-copy-btn">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                        ${i.t('action.copy')}
                    </button>
                </div>
                <textarea id="scan-invalid-prompt" readonly class="w-full h-32 bg-gray-900 text-sm text-gray-300 font-mono rounded p-3 border border-gray-600 resize-none cursor-text select-all">${esc(fixPrompt)}</textarea>
                <p class="text-xs text-gray-500 mt-2">${i.t('scan_invalid.copy_hint')}</p>
            </div>
            <div class="flex justify-end mt-4">
                <button class="px-4 py-2 text-gray-400 hover:text-white text-sm cursor-pointer modal-cancel">${i.t('action.close')}</button>
            </div>
        </div>`;
        this.openModal(html);
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
        this.modalEl().querySelector('.scan-invalid-copy-btn').addEventListener('click', () => {
            const ta = document.getElementById('scan-invalid-prompt');
            ta.select();
            navigator.clipboard.writeText(ta.value).then(() => {
                const copyBtn = this.modalEl().querySelector('.scan-invalid-copy-btn');
                const original = copyBtn.innerHTML;
                copyBtn.innerHTML = `<span class="text-green-400">${i.t('action.copied')}</span>`;
                setTimeout(() => copyBtn.innerHTML = original, 1500);
            });
        });
    }

    buildFixPrompt(invalid) {
        const i = this.i18n;
        let paths = invalid.map(item => item.path).join('\n');
        return i.tWith('scan_invalid.fix_prompt', { paths });
    }

    // --- Sessions ---
    clearSessionDeleteConfirmation() {
        if (this.sessionDeleteConfirmTimer) {
            clearTimeout(this.sessionDeleteConfirmTimer);
            this.sessionDeleteConfirmTimer = null;
        }
        this.confirmingSessionDeleteId = null;
    }

    startSessionDeleteConfirmation(sessionId) {
        this.clearSessionDeleteConfirmation();
        this.confirmingSessionDeleteId = sessionId;
        this.sessionDeleteConfirmTimer = setTimeout(() => {
            if (this.confirmingSessionDeleteId === sessionId) {
                this.confirmingSessionDeleteId = null;
                this.sessionDeleteConfirmTimer = null;
                this.render();
            }
        }, 3000);
    }

    async refreshSessionPlatforms(options = {}) {
        const keepCurrentPathFilter = options.keepCurrentPathFilter === true;
        this.clearSessionDeleteConfirmation();
        this.deletingSessionId = null;
        this.sessionsLoadError = '';
        try {
            this.sessionPlatforms = await Api.listSessionPlatforms();
        } catch (e) {
            this.sessionPlatforms = [];
            this.sessionsLoadError = e?.SyncError || e?.message || String(e);
        }

        if (this.sessionPlatforms.length === 0) {
            this.selectedSessionPlatform = null;
            this.selectedSessionPathFilter = 'all';
            this.sessionPathOptions = ['all', 'unknown'];
            this.sessions = [];
            this.sessionOffset = 0;
            this.sessionTotal = 0;
            this.sessionHasMore = false;
            this.deletingSessionId = null;
            return;
        }

        const exists = this.sessionPlatforms.some(p => p.id === this.selectedSessionPlatform);
        if (!exists) {
            this.selectedSessionPlatform = this.sessionPlatforms[0].id;
        }
        if (!keepCurrentPathFilter) {
            this.selectedSessionPathFilter = 'all';
        }
        await this.loadSessionsForPlatform(this.selectedSessionPlatform, { append: false });
    }

    async refreshSessionTerminals() {
        try {
            const terminals = await Api.listSessionTerminals();
            this.sessionTerminals = Array.isArray(terminals) ? terminals : [];
        } catch {
            this.sessionTerminals = [];
        }
        if (this.sessionTerminals.length === 0) {
            this.selectedSessionTerminal = 'terminal-default';
            return;
        }
        const active = this.sessionTerminals.find(item => item.id === this.selectedSessionTerminal && item.available);
        if (active) return;
        const firstAvailable = this.sessionTerminals.find(item => item.available);
        this.selectedSessionTerminal = firstAvailable ? firstAvailable.id : this.sessionTerminals[0].id;
    }

    async loadSessionsForPlatform(platformId, options = {}) {
        const append = options.append === true;
        const allowResetFilter = options.allowResetFilter !== false;
        if (!platformId) {
            this.sessionPathOptions = ['all', 'unknown'];
            this.selectedSessionPathFilter = 'all';
            this.sessions = [];
            this.sessionOffset = 0;
            this.sessionTotal = 0;
            this.sessionHasMore = false;
            this.clearSessionDeleteConfirmation();
            this.deletingSessionId = null;
            return;
        }
        const offset = append ? this.sessionOffset : 0;
        try {
            const page = await Api.listSessions(
                platformId,
                this.selectedSessionPathFilter || 'all',
                offset,
                this.sessionPageSize
            );
            const pagePaths = Array.isArray(page?.paths) && page.paths.length > 0
                ? page.paths
                : ['all', 'unknown'];
            this.sessionPathOptions = pagePaths;
            if (!this.sessionPathOptions.includes(this.selectedSessionPathFilter)) {
                if (this.selectedSessionPathFilter !== 'all') {
                    this.selectedSessionPathFilter = 'all';
                    if (!append && allowResetFilter) {
                        await this.loadSessionsForPlatform(platformId, { append: false, allowResetFilter: false });
                        return;
                    }
                } else {
                    this.selectedSessionPathFilter = 'all';
                }
            }
            const pageSessions = Array.isArray(page?.sessions) ? page.sessions : [];
            if (append) {
                this.sessions = [...this.sessions, ...pageSessions];
            } else {
                this.sessions = pageSessions;
            }
            this.sessionTotal = Number(page?.total) || this.sessions.length;
            this.sessionOffset = Number(page?.offset ?? offset) + pageSessions.length;
            this.sessionHasMore = Boolean(page?.has_more) && this.sessionOffset < this.sessionTotal;
            this.sessionsLoadError = '';
            if (this.confirmingSessionDeleteId && !this.sessions.some(s => s.id === this.confirmingSessionDeleteId)) {
                this.clearSessionDeleteConfirmation();
            }
            if (this.deletingSessionId && !this.sessions.some(s => s.id === this.deletingSessionId)) {
                this.deletingSessionId = null;
            }
        } catch (e) {
            const errorText = e?.SyncError || e?.message || String(e);
            if (!append) {
                this.sessionPathOptions = ['all', 'unknown'];
                this.selectedSessionPathFilter = 'all';
                this.sessions = [];
                this.sessionOffset = 0;
                this.sessionTotal = 0;
                this.sessionHasMore = false;
                this.clearSessionDeleteConfirmation();
                this.deletingSessionId = null;
                this.sessionsLoadError = errorText;
            } else {
                this.sessionsLoadError = '';
                this.showToast(this.i18n.tWith('session.load_failed', { error: errorText }), 'error');
            }
        }
    }

    async selectSessionPlatform(id) {
        this.clearSessionDeleteConfirmation();
        this.deletingSessionId = null;
        this.selectedSessionPlatform = id;
        this.selectedSessionPathFilter = 'all';
        this.isSessionsLoading = true;
        this.render();
        try {
            await this.loadSessionsForPlatform(id, { append: false });
        } finally {
            this.isSessionsLoading = false;
            this.render();
        }
    }

    async loadMoreSessions() {
        if (this.sessionLoadingMore || !this.sessionHasMore || !this.selectedSessionPlatform) {
            return;
        }
        this.sessionLoadingMore = true;
        this.render();
        try {
            await this.loadSessionsForPlatform(this.selectedSessionPlatform, { append: true });
        } finally {
            this.sessionLoadingMore = false;
            this.render();
        }
    }

    async reloadSessionsAfterDelete(platformId, targetLoadedCount) {
        const errorText = (value) => value?.SyncError || value?.message || String(value);
        try {
            this.sessionPlatforms = await Api.listSessionPlatforms();
        } catch (e) {
            this.sessionsLoadError = errorText(e);
            return;
        }

        if (this.sessionPlatforms.length === 0) {
            this.selectedSessionPlatform = null;
            this.selectedSessionPathFilter = 'all';
            this.sessionPathOptions = ['all', 'unknown'];
            this.sessions = [];
            this.sessionOffset = 0;
            this.sessionTotal = 0;
            this.sessionHasMore = false;
            return;
        }

        const keepCurrentPlatform = this.sessionPlatforms.some(p => p.id === platformId);
        const nextPlatformId = keepCurrentPlatform ? platformId : this.sessionPlatforms[0].id;
        const platformChanged = this.selectedSessionPlatform !== nextPlatformId;
        this.selectedSessionPlatform = nextPlatformId;
        if (platformChanged) {
            this.selectedSessionPathFilter = 'all';
        }

        await this.loadSessionsForPlatform(this.selectedSessionPlatform, { append: false });

        const desiredCount = Math.max(targetLoadedCount, 0);
        while (this.sessionHasMore && this.sessions.length < desiredCount) {
            const before = this.sessions.length;
            await this.loadSessionsForPlatform(this.selectedSessionPlatform, { append: true });
            if (this.sessions.length <= before) break;
        }
    }

    async deleteSessionRecord(session) {
        if (!session || !session.id || this.deletingSessionId) {
            return;
        }

        const i = this.i18n;
        const platformId = session.platform_id || this.selectedSessionPlatform;
        if (!platformId) return;

        if (this.confirmingSessionDeleteId !== session.id) {
            this.startSessionDeleteConfirmation(session.id);
            this.render();
            return;
        }

        this.clearSessionDeleteConfirmation();
        this.deletingSessionId = session.id;
        this.render();

        const targetLoadedCount = Math.max(this.sessions.length - 1, 0);
        try {
            await Api.deleteSession(platformId, session.id);
            await this.reloadSessionsAfterDelete(platformId, targetLoadedCount);
            this.showToast(i.t('session.deleted'), 'success');
        } catch (e) {
            this.showToast(i.tWith('session.delete_failed', { error: e?.SyncError || e?.message || e }), 'error');
        } finally {
            this.deletingSessionId = null;
            this.render();
        }
    }

    async changeSessionPathFilter(pathFilter) {
        const nextFilter = typeof pathFilter === 'string' && pathFilter.trim()
            ? pathFilter.trim()
            : 'all';
        if (!this.selectedSessionPlatform || this.selectedSessionPathFilter === nextFilter) {
            return;
        }
        this.selectedSessionPathFilter = nextFilter;
        this.clearSessionDeleteConfirmation();
        this.deletingSessionId = null;
        this.isSessionsLoading = true;
        this.render();
        try {
            await this.loadSessionsForPlatform(this.selectedSessionPlatform, { append: false });
        } finally {
            this.isSessionsLoading = false;
            this.render();
        }
    }

    async resumeSessionInTerminal(session) {
        const i = this.i18n;
        const terminalId = this.selectedSessionTerminal || 'terminal-default';
        this.resumingSessionId = session.id;
        this.render();
        try {
            const launched = await Api.resumeSession(
                session.platform_id || this.selectedSessionPlatform,
                session.id,
                session.project_path || '',
                terminalId
            );
            this.showToast(i.tWith('session.resume_started', { command: launched }), 'success', 5000);
        } catch (e) {
            this.showToast(i.tWith('session.resume_failed', { error: e.SyncError || e.message || e }), 'error');
        } finally {
            this.resumingSessionId = null;
            this.render();
        }
    }

    async showSessionMessagesModal(session) {
        const i = this.i18n;
        const summary = [
            session.project_path ? `<span class="text-gray-500">${esc(session.project_path)}</span>` : '',
            session.model ? `<span class="text-cyan-400">${esc(session.model)}</span>` : '',
            session.tokens_used != null ? `<span class="text-yellow-500">${i.tWith('session.tokens_value', { count: formatInt(session.tokens_used) })}</span>` : '',
        ].filter(Boolean).join(' · ');

        const html = `<div class="p-5">
            <div class="flex items-start justify-between gap-3 mb-3">
                <div>
                    <h2 class="text-lg font-bold text-green-400">${esc(session.title || i.t('session.untitled'))}</h2>
                    <div class="text-xs text-gray-500 mt-1">${summary || i.t('session.no_metadata')}</div>
                    <div class="text-xs text-gray-600 mt-0.5">${i.tWith('session.started_at', { time: this.formatSessionTime(session.started_at) })}</div>
                </div>
                <button class="text-gray-400 hover:text-white text-sm cursor-pointer modal-cancel">${i.t('action.close')}</button>
            </div>
            <div data-session-messages class="bg-gray-900 rounded-lg border border-gray-700 h-[60vh] overflow-y-auto p-3 space-y-3"></div>
            <div data-session-loading class="text-xs text-gray-500 mt-2">${i.t('session.loading_messages')}</div>
        </div>`;
        this.openModal(html);
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());

        const listEl = this.modalEl().querySelector('[data-session-messages]');
        const loadingEl = this.modalEl().querySelector('[data-session-loading]');
        let offset = 0;
        let loading = false;
        let hasMore = true;

        const appendMessages = (messages) => {
            if (messages.length === 0 && offset === 0) {
                listEl.innerHTML = `<p class="text-sm text-gray-500">${i.t('session.no_messages')}</p>`;
                return;
            }
            for (const msg of messages) {
                const isUser = msg.role === 'user';
                const roleLabel = isUser ? i.t('session.role_user') : i.t('session.role_assistant');
                const roleClass = isUser ? 'text-cyan-400' : 'text-green-400';
                const bubbleClass = isUser ? 'border-cyan-800/40 bg-cyan-900/10' : 'border-green-800/40 bg-green-900/10';
                const timeLabel = msg.timestamp ? this.formatSessionTime(msg.timestamp) : '-';
                const node = document.createElement('div');
                node.className = `rounded border ${bubbleClass} p-3`;
                node.innerHTML = `<div class="flex items-center justify-between mb-2">
                    <span class="text-xs font-semibold ${roleClass}">${roleLabel}</span>
                    <span class="text-[11px] text-gray-600">${esc(timeLabel)}</span>
                </div>
                <pre class="text-sm text-gray-200 whitespace-pre-wrap break-words font-sans">${esc(msg.content || '')}</pre>`;
                listEl.appendChild(node);
            }
        };

        const loadMore = async () => {
            if (loading || !hasMore) return;
            loading = true;
            loadingEl.textContent = i.t('session.loading_messages');
            try {
                const platformId = session.platform_id || this.selectedSessionPlatform;
                const page = await Api.getSessionMessages(platformId, session.id, offset, this.sessionMessagePageSize || 50);
                appendMessages(page);
                offset += page.length;
                hasMore = page.length === (this.sessionMessagePageSize || 50);
                loadingEl.textContent = hasMore ? i.t('session.scroll_for_more') : i.t('session.no_more_messages');
            } catch (e) {
                loadingEl.textContent = i.tWith('session.load_failed', { error: e.SyncError || e.message || e });
                hasMore = false;
            } finally {
                loading = false;
            }
        };

        listEl.addEventListener('scroll', () => {
            if (listEl.scrollTop + listEl.clientHeight >= listEl.scrollHeight - 100) {
                loadMore();
            }
        });

        await loadMore();
    }

    // --- MCP ---
    async refreshMcpPlatforms() {
        this.mcpPlatforms = await Api.listMcpPlatforms();
        const exists = this.mcpPlatforms.some(p => p.id === this.selectedMcpPlatform);
        if (!exists) {
            this.selectedMcpPlatform = this.mcpPlatforms.length > 0 ? this.mcpPlatforms[0].id : null;
            if (this.selectedMcpPlatform) {
                await this.selectMcpPlatform(this.selectedMcpPlatform);
            }
        }
    }

    async selectMcpPlatform(id) {
        this.selectedMcpPlatform = id;
        this.expandedMcpServer = null;
        this.mcpServerDetails = {};
        try {
            this.mcpServers = await Api.getMcpServers(id);
        } catch {
            this.mcpServers = [];
        }
        this.render();
    }

    async showMcpAdd() {
        const i = this.i18n;
        const placeholderDemo = `# Codex / TOML style:
[mcp_servers.mcp-server-time]
command = "uvx"
args = ["mcp-server-time"]

[mcp_servers.mcp-server-time.env]
API_KEY = "your-key"

# Other Agent / JSON style:
{
  "mcpServers": {
    "mcp-server-time": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-time"]
    }
  }
}`;
        let html = `<div class="p-5">
            <div class="mb-3">
                <label class="text-xs text-gray-400">${i.t('mcp.server_name')}</label>
                <input id="mcp-add-name" class="w-full bg-gray-900 text-sm text-gray-200 rounded px-3 py-1.5 border border-gray-600 focus:border-cyan-500 outline-none" />
            </div>
            <div class="mb-2">
                <label class="text-xs text-gray-400">${i.t('mcp.config')} (${i.t('mcp.format_json')} / ${i.t('mcp.format_toml')})</label>
                <textarea id="mcp-add-area" class="w-full h-56 bg-gray-900 text-sm text-gray-200 font-mono rounded p-3 border border-gray-600 focus:border-cyan-500 outline-none resize-y" placeholder="${esc(placeholderDemo)}"></textarea>
            </div>
            <div class="flex gap-3 justify-end mt-3">
                <button class="px-4 py-2 bg-cyan-700 hover:bg-cyan-600 rounded text-sm cursor-pointer mcp-add-save">${i.t('mcp.save')}</button>
                <button class="px-4 py-2 text-gray-400 hover:text-white text-sm cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
            </div></div>`;
        this.openModal(html);
        this.modalEl().querySelector('.mcp-add-save').addEventListener('click', async () => {
            const name = document.getElementById('mcp-add-name').value.trim();
            const text = document.getElementById('mcp-add-area').value.trim();
            if (!name) { this.showToast('Server name required', 'warning'); return; }
            try {
                await Api.importMcpServer(this.selectedMcpPlatform, name, text);
                this.closeModal();
                await this.selectMcpPlatform(this.selectedMcpPlatform);
            } catch (e) {
                this.showToast(i.tWith('mcp.parse_error', { error: e.SyncError || e }), 'error');
            }
        });
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    async deleteMcpServer(name, btn) {
        const i = this.i18n;
        if (btn.dataset.confirming === 'true') {
            try {
                await Api.deleteMcpServer(this.selectedMcpPlatform, name);
                this.expandedMcpServer = null;
                delete this.mcpServerDetails[name];
                await this.refreshMcpPlatforms();
                await this.selectMcpPlatform(this.selectedMcpPlatform);
                await this.refreshTrashCount();
            } catch (e) {
                this.showToast(i.tWith('mcp.delete_failed', { error: e.SyncError || e }), 'error');
            }
            return;
        }
        btn.dataset.confirming = 'true';
        btn.textContent = i.t('mcp.confirm_delete');
        btn.classList.remove('text-red-600', 'hover:text-red-400');
        btn.classList.add('bg-red-700', 'hover:bg-red-600', 'text-white', 'rounded', 'px-2', 'py-0.5');
        const reset = () => {
            btn.dataset.confirming = 'false';
            btn.textContent = i.t('mcp.delete');
            btn.classList.add('text-red-600', 'hover:text-red-400');
            btn.classList.remove('bg-red-700', 'hover:bg-red-600', 'text-white', 'rounded', 'px-2', 'py-0.5');
        };
        setTimeout(reset, 3000);
        const onClickOutside = (e) => {
            if (!btn.contains(e.target)) { reset(); document.removeEventListener('click', onClickOutside); }
        };
        setTimeout(() => document.addEventListener('click', onClickOutside), 0);
    }

    // --- Trash ---

    async refreshTrashCount() {
        try {
            const items = await Api.listTrash();
            this.trashCount = items.length;
        } catch {
            this.trashCount = 0;
        }
        this.renderTrashBadge();
    }

    renderTrashBadge() {
        const el = document.getElementById('trash-bin');
        if (!el) return;
        if (this.trashCount === 0) {
            el.classList.add('hidden');
            return;
        }
        el.classList.remove('hidden');
        const i = this.i18n;
        el.innerHTML = `<div class="flex items-center gap-2 px-2 py-2">
            <span class="text-gray-500 flex-shrink-0">${Icons.trash}</span>
            <span class="text-xs text-gray-400">${i.tWith('trash.item_count', { count: this.trashCount })}</span>
        </div>`;
        el.onclick = () => this.showTrashModal();
    }

    async showTrashModal() {
        const i = this.i18n;
        let items;
        try {
            items = await Api.listTrash();
        } catch {
            items = [];
        }
        if (items.length === 0) {
            this.showToast(i.t('trash.empty'), 'info');
            return;
        }
        const now = Math.floor(Date.now() / 1000);
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-4">${i.t('trash.title')} (${items.length})</h2>
            <div class="space-y-1 mb-3 max-h-[60vh] overflow-y-auto">`;
        for (const item of items) {
            const secondsLeft = (item.deleted_at + 7 * 24 * 3600) - now;
            const daysLeft = Math.max(0, Math.ceil(secondsLeft / 86400));
            const daysStr = daysLeft > 0 ? i.tWith('trash.days_left', { n: daysLeft }) : `<span class="text-red-400">${i.t('trash.expired')}</span>`;
            const typeLabel = item.item_type === 'skill' ? i.t('trash.type_skill') : i.t('trash.type_mcp');
            const typeColor = item.item_type === 'skill' ? 'text-cyan-400' : 'text-purple-400';
            html += `<div class="flex items-center gap-2 px-3 py-2 rounded hover:bg-gray-700 group trash-item" data-trash-id="${esc(item.id)}">
                <div class="flex-1">
                    <div><span class="${typeColor} text-xs font-bold">[${typeLabel}]</span> <span class="text-gray-200">${esc(item.name)}</span></div>
                    <div class="text-xs text-gray-500">${esc(item.platform_id)}${item.folder ? ' / ' + esc(item.folder) : ''} · ${daysStr}</div>
                </div>
                <button class="text-xs text-green-600 hover:text-green-400 px-2 py-1 cursor-pointer trash-restore-btn">${i.t('trash.restore')}</button>
                <button class="text-xs text-red-600 hover:text-red-400 px-2 py-1 cursor-pointer trash-delete-btn" data-confirming="false">${i.t('trash.delete_forever')}</button>
            </div>`;
        }
        html += `</div>
            <div id="trash-status" class="text-sm text-gray-500 mb-3"></div>
            <div class="flex gap-3 justify-between">
                <button class="px-4 py-2 bg-red-800 hover:bg-red-700 rounded text-sm cursor-pointer trash-empty-btn" data-confirming="false">${i.t('trash.empty_trash')}</button>
                <button class="px-4 py-2 text-gray-400 hover:text-white cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
            </div></div>`;
        this.openModal(html);

        const setTrashStatus = (message, isError = false) => {
            const statusEl = this.modalEl().querySelector('#trash-status');
            if (!statusEl) return;
            statusEl.textContent = message || '';
            statusEl.classList.toggle('text-red-400', isError);
            statusEl.classList.toggle('text-gray-500', !isError);
        };
        const resetDeleteButton = (btn) => {
            if (!btn) return;
            btn.dataset.confirming = 'false';
            btn.textContent = i.t('trash.delete_forever');
            btn.classList.remove('bg-red-900', 'text-white', 'rounded');
            btn.classList.add('text-red-600', 'hover:text-red-400');
        };

        // Restore buttons
        this.modalEl().querySelectorAll('.trash-restore-btn').forEach(btn => {
            btn.addEventListener('click', async (e) => {
                e.stopPropagation();
                const row = btn.closest('[data-trash-id]');
                if (!row || !row.dataset.trashId) return;
                setTrashStatus('');
                await this.restoreTrashItem(row.dataset.trashId);
            });
        });

        // Permanent delete buttons
        this.modalEl().querySelectorAll('.trash-delete-btn').forEach(btn => {
            btn.addEventListener('click', async (e) => {
                e.stopPropagation();
                const row = btn.closest('[data-trash-id]');
                if (!row || !row.dataset.trashId) return;
                if (btn.dataset.confirming !== 'true') {
                    this.modalEl().querySelectorAll('.trash-delete-btn').forEach(other => resetDeleteButton(other));
                    btn.dataset.confirming = 'true';
                    btn.textContent = i.t('action.confirm');
                    btn.classList.add('bg-red-900', 'text-white', 'rounded');
                    btn.classList.remove('text-red-600', 'hover:text-red-400');
                    setTrashStatus(i.t('trash.confirm_delete_forever'));
                    setTimeout(() => {
                        if (document.body.contains(btn) && btn.dataset.confirming === 'true') {
                            resetDeleteButton(btn);
                            setTrashStatus('');
                        }
                    }, 4000);
                    return;
                }
                resetDeleteButton(btn);
                setTrashStatus('');
                await this.permanentlyDeleteTrashItem(row.dataset.trashId);
            });
        });

        // Empty trash button
        this.modalEl().querySelector('.trash-empty-btn').addEventListener('click', async (e) => {
            e.stopPropagation();
            const btn = e.currentTarget;
            if (btn.dataset.confirming !== 'true') {
                btn.dataset.confirming = 'true';
                btn.textContent = i.t('action.confirm');
                btn.classList.add('bg-red-900');
                setTrashStatus(i.t('trash.confirm_empty'));
                setTimeout(() => {
                    if (document.body.contains(btn) && btn.dataset.confirming === 'true') {
                        btn.dataset.confirming = 'false';
                        btn.textContent = i.t('trash.empty_trash');
                        btn.classList.remove('bg-red-900');
                        setTrashStatus('');
                    }
                }, 4000);
                return;
            }
            btn.dataset.confirming = 'false';
            btn.textContent = i.t('trash.empty_trash');
            btn.classList.remove('bg-red-900');
            setTrashStatus('');
            await this.emptyTrash();
        });

        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    async restoreTrashItem(id) {
        const i = this.i18n;
        try {
            await Api.restoreTrashItem(id);
            this.closeModal();
            await this.reloadPlatforms();
            await this.refreshMcpPlatforms();
            await this.refreshTrashCount();
            // If MCP tab and was viewing a platform, re-select it
            if (this.currentTab === 'mcp' && this.selectedMcpPlatform) {
                await this.selectMcpPlatform(this.selectedMcpPlatform);
            }
            this.render();
        } catch (e) {
            this.showToast(i.tWith('trash.restore_failed', { error: e.SyncError || e }), 'error');
        }
    }

    async permanentlyDeleteTrashItem(id) {
        try {
            await Api.permanentlyDeleteTrashItem(id);
            this.closeModal();
            await this.refreshTrashCount();
            if (this.trashCount > 0) this.showTrashModal();
        } catch (e) {
            console.error('Delete forever error:', e);
            this.showToast('Error: ' + (e.message || e.SyncError || (typeof e === 'object' ? JSON.stringify(e) : e)), 'error');
        }
    }

    async emptyTrash() {
        try {
            await Api.emptyTrash();
            this.closeModal();
            await this.refreshTrashCount();
        } catch (e) {
            console.error('Empty trash error:', e);
            this.showToast('Error: ' + (e.message || e.SyncError || (typeof e === 'object' ? JSON.stringify(e) : e)), 'error');
        }
    }

    async showMcpSyncModal(serverName) {
        const targets = await Api.getMcpSyncTargets(this.selectedMcpPlatform, serverName);
        if (targets.length === 0) {
            this.showToast(this.i18n.t('error.no_target'), 'warning');
            return;
        }
        this.pendingSyncTarget = null;
        const i = this.i18n;
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-4">${i.t('mcp.sync_title')} - ${i.t('mcp.select_target')}</h2>
            <div class="space-y-1">`;
        for (const t of targets) {
            const badge = t.has_server
                ? `<span class="text-yellow-500 text-xs ml-2">[${i.t('mcp.has_server')}]</span>`
                : `<span class="text-green-500 text-xs ml-2">[${i.t('mcp.new_server')}]</span>`;
            const fmt = t.format === 'toml' ? ' <span class="text-gray-600 text-xs">TOML</span>' : '';
            html += `<button class="w-full text-left px-3 py-2 rounded hover:bg-gray-700 text-gray-200 cursor-pointer mcp-sync-target"
                data-id="${t.id}" data-has="${t.has_server}">${esc(t.display_name)}${fmt}${badge}</button>`;
        }
        html += `</div><div class="mt-4 flex justify-end">
            <button class="px-4 py-2 text-gray-400 hover:text-white cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
        </div></div>`;
        this.openModal(html);
        this.modalEl().querySelectorAll('.mcp-sync-target').forEach(btn => {
            btn.addEventListener('click', async () => {
                const targetId = btn.dataset.id;
                if (this.pendingSyncTarget !== targetId) {
                    this.modalEl().querySelectorAll('.mcp-sync-target').forEach(b => {
                        b.classList.remove('bg-red-900', 'hover:bg-red-800');
                        b.classList.add('hover:bg-gray-700');
                        const hint = b.querySelector('.sync-confirm-hint');
                        if (hint) hint.remove();
                    });
                    this.pendingSyncTarget = targetId;
                    btn.classList.remove('hover:bg-gray-700');
                    btn.classList.add('bg-red-900', 'hover:bg-red-800');
                    const hint = document.createElement('span');
                    hint.className = 'text-red-400 text-xs ml-2 font-bold sync-confirm-hint';
                    hint.textContent = i.t('action.confirm');
                    btn.appendChild(hint);
                    return;
                }
                this.pendingSyncTarget = null;
                await this.showMcpSyncPreview(this.selectedMcpPlatform, targetId, serverName);
            });
        });
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    async showMcpSyncPreview(sourcePlatformId, targetPlatformId, serverName) {
        const preview = await Api.previewMcpSync(sourcePlatformId, targetPlatformId, serverName);
        const i = this.i18n;
        const srcName = this.mcpPlatforms.find(p => p.id === sourcePlatformId)?.display_name || sourcePlatformId;
        const tgtName = this.mcpPlatforms.find(p => p.id === targetPlatformId)?.display_name || targetPlatformId;

        const diffHtml = renderSideBySide(toSideBySide(preview.diff_lines));

        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-3">${i.t('mcp.sync_title')}</h2>
            <p class="text-sm mb-1"><span class="text-cyan-400">${i.t('sync.source')}:</span> ${esc(srcName)} / ${esc(serverName)}</p>
            <p class="text-sm mb-1"><span class="text-cyan-400">${i.t('sync.target')}:</span> ${esc(tgtName)} (${preview.target_format.toUpperCase()})</p>
            <p class="text-sm mb-2"><span class="text-gray-500">${esc(preview.target_config_path)}</span></p>
            ${preview.has_conflict ? `<p class="text-yellow-400 text-sm mb-3 flex items-center gap-1">${Icons.warning} ${i.t('mcp.conflict_warning')}</p>` : ''}
            <div class="text-sm mb-2"><span class="text-green-400">+${preview.added}</span> <span class="text-red-400">-${preview.removed}</span></div>
            <div style="max-height:50vh;overflow-y:auto">${diffHtml}</div>
            <div class="flex gap-3 justify-end mt-4">
                <button class="px-4 py-2 bg-cyan-700 hover:bg-cyan-600 rounded text-sm cursor-pointer mcp-sync-confirm">${i.t('mcp.confirm_sync')}</button>
                <button class="px-4 py-2 text-gray-400 hover:text-white cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
            </div>
        </div>`;
        this.modalEl().innerHTML = html;
        this.modalEl().querySelector('.mcp-sync-confirm').addEventListener('click', async () => {
            try {
                await Api.syncMcpServer(sourcePlatformId, targetPlatformId, serverName);
                this.closeModal();
                await this.refreshMcpPlatforms();
                await this.selectMcpPlatform(this.selectedMcpPlatform);
            } catch (e) {
                this.showToast(i.tWith('mcp.sync_failed', { error: e.SyncError || e }), 'error');
            }
        });
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    // --- Modals ---
    async showDiffModal() {
        const candidates = await Api.getDiffCandidates(this.selectedPlatformId, this.selectedSkillName, this.selectedFolder);
        if (candidates.length === 0) {
            this.showToast(this.i18n.t('diff.no_other'), 'warning');
            return;
        }
        const i = this.i18n;
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-4">${i.t('diff.select_platform')}</h2>
            <div class="space-y-1">`;
        for (const c of candidates) {
            html += `<button class="w-full text-left px-3 py-2 rounded hover:bg-gray-700 text-gray-200 cursor-pointer diff-target" data-id="${c.id}">
                ${c.display_name}</button>`;
        }
        html += `</div><div class="mt-4 flex justify-end">
            <button class="px-4 py-2 text-gray-400 hover:text-white cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
        </div></div>`;
        this.openModal(html);
        this.modalEl().querySelectorAll('.diff-target').forEach(btn => {
            btn.addEventListener('click', () => this.doDiff(btn.dataset.id));
        });
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    async showSyncModal() {
        const targets = await Api.getSyncTargets(this.selectedPlatformId, this.selectedSkillName, this.selectedFolder);
        if (targets.length === 0) {
            this.showToast(this.i18n.t('error.no_target'), 'warning');
            return;
        }
        this.pendingSyncTarget = null;
        const i = this.i18n;
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-4">${i.t('sync.title')} - ${i.t('sync.select_target')}</h2>
            <div class="space-y-1">`;
        for (const t of targets) {
            const badge = t.has_skill
                ? `<span class="text-yellow-500 text-xs ml-2">[${i.t('sync.has_skill')}]</span>`
                : `<span class="text-green-500 text-xs ml-2">[${i.t('sync.new')}]</span>`;
            html += `<button class="w-full text-left px-3 py-2 rounded hover:bg-gray-700 text-gray-200 cursor-pointer sync-target"
                data-id="${t.id}" data-has="${t.has_skill}">${t.display_name}${badge}</button>`;
        }
        html += `</div><div class="mt-4 flex justify-end">
            <button class="px-4 py-2 text-gray-400 hover:text-white cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
        </div></div>`;
        this.openModal(html);
        this.modalEl().querySelectorAll('.sync-target').forEach(btn => {
            btn.addEventListener('click', async () => {
                const targetId = btn.dataset.id;
                const hasSkill = btn.dataset.has === 'true';

                // Two-click confirmation: first click turns button red
                if (this.pendingSyncTarget !== targetId) {
                    // Reset any other pending buttons
                    this.modalEl().querySelectorAll('.sync-target').forEach(b => {
                        b.classList.remove('bg-red-900', 'hover:bg-red-800');
                        b.classList.add('hover:bg-gray-700');
                        const hint = b.querySelector('.sync-confirm-hint');
                        if (hint) hint.remove();
                    });

                    this.pendingSyncTarget = targetId;
                    btn.classList.remove('hover:bg-gray-700');
                    btn.classList.add('bg-red-900', 'hover:bg-red-800');
                    const hint = document.createElement('span');
                    hint.className = 'text-red-400 text-xs ml-2 font-bold sync-confirm-hint';
                    hint.textContent = i.t('action.confirm');
                    btn.appendChild(hint);
                    return;
                }

                // Second click — confirmed
                this.pendingSyncTarget = null;
                if (hasSkill) {
                    await this.showSyncConflict(targetId);
                } else {
                    await this.doSync(targetId, false);
                }
            });
        });
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    async showSyncConflict(targetPlatformId) {
        const diff = await Api.diffSkills(this.selectedPlatformId, targetPlatformId, this.selectedSkillName, this.selectedFolder);
        const i = this.i18n;
        const targetName = this.platforms.find(p => p.id === targetPlatformId)?.display_name || targetPlatformId;

        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-3">${i.t('sync.title')}</h2>
            <p class="text-sm mb-1"><span class="text-cyan-400">${i.t('sync.source')}:</span> ${this.selectedPlatformId} / ${this.selectedSkillName}</p>
            <p class="text-sm mb-2"><span class="text-cyan-400">${i.t('sync.target')}:</span> ${targetName}</p>
            <p class="text-yellow-400 text-sm mb-3 flex items-center gap-1">${Icons.warning} ${i.t('sync.conflict_warning')}</p>
            <div class="text-sm mb-4 space-y-1">`;
        for (const fd of diff.file_diffs) {
            html += `<div class="text-purple-400">${fd.file_path} <span class="text-gray-500">+${fd.stats.added} -${fd.stats.removed}</span></div>`;
        }
        html += `</div><div class="flex gap-3 justify-end">
            <button class="px-4 py-2 bg-cyan-700 hover:bg-cyan-600 rounded text-sm cursor-pointer sync-overwrite">${i.t('action.overwrite')}</button>
            <button class="px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm cursor-pointer sync-keep">${i.t('action.keep_target')}</button>
            <button class="px-4 py-2 text-gray-400 hover:text-white text-sm cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
        </div></div>`;
        this.modalEl().innerHTML = html;
        this.modalEl().querySelector('.sync-overwrite').addEventListener('click', () => this.doSync(targetPlatformId, true));
        this.modalEl().querySelector('.sync-keep').addEventListener('click', () => this.closeModal());
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    showFolderSyncModal(folder, count) {
        const targets = this.platforms.filter(p => p.id !== this.selectedPlatformId);
        if (targets.length === 0) {
            this.showToast(this.i18n.t('error.no_target'), 'warning');
            return;
        }
        this.pendingSyncTarget = null;
        const i = this.i18n;
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-4">${i.tWith('folder.sync_all', { count })}</h2>
            <p class="text-sm text-gray-400 mb-3">${esc(folder)}/</p>
            <div class="space-y-1">`;
        for (const t of targets) {
            html += `<button class="w-full text-left px-3 py-2 rounded hover:bg-gray-700 text-gray-200 cursor-pointer folder-sync-target"
                data-id="${t.id}">${t.display_name}</button>`;
        }
        html += `</div><div class="mt-4 flex justify-end">
            <button class="px-4 py-2 text-gray-400 hover:text-white cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
        </div></div>`;
        this.openModal(html);
        this.modalEl().querySelectorAll('.folder-sync-target').forEach(btn => {
            btn.addEventListener('click', async () => {
                const targetId = btn.dataset.id;
                if (this.pendingSyncTarget !== targetId) {
                    this.modalEl().querySelectorAll('.folder-sync-target').forEach(b => {
                        b.classList.remove('bg-red-900', 'hover:bg-red-800');
                        b.classList.add('hover:bg-gray-700');
                        const hint = b.querySelector('.sync-confirm-hint');
                        if (hint) hint.remove();
                    });
                    this.pendingSyncTarget = targetId;
                    btn.classList.remove('hover:bg-gray-700');
                    btn.classList.add('bg-red-900', 'hover:bg-red-800');
                    const hint = document.createElement('span');
                    hint.className = 'text-red-400 text-xs ml-2 font-bold sync-confirm-hint';
                    hint.textContent = i.t('action.confirm');
                    btn.appendChild(hint);
                    return;
                }
                this.pendingSyncTarget = null;
                await this.doFolderSync(targetId, folder);
            });
        });
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    openModal(html) {
        const overlay = document.getElementById('modal-overlay');
        overlay.style.display = 'flex';
        this.modalEl().innerHTML = html;
    }

    closeModal() {
        this.pendingSyncTarget = null;
        document.getElementById('modal-overlay').style.display = 'none';
    }

    modalEl() { return document.getElementById('modal-content'); }

    showToast(message, type = 'info', duration = 3000) {
        const container = document.getElementById('toast-container');
        if (!container) return;
        const toast = document.createElement('div');
        toast.className = `toast-item toast-${type}`;
        toast.textContent = message;
        container.appendChild(toast);
        setTimeout(() => {
            toast.classList.add('toast-exit');
            toast.addEventListener('animationend', () => toast.remove());
        }, duration);
    }

    renderEmptyState(message, icon = 'search') {
        const iconSvg = (Icons[icon] || Icons.search).replace(/width="\d+"/, 'width="32"').replace(/height="\d+"/, 'height="32"');
        return `<div class="empty-state"><div class="empty-state-icon">${iconSvg}</div><p>${message}</p></div>`;
    }

    // --- Render ---
    render() {
        this.renderTabBar();
        this.renderSidebar();
        this.renderTrashBadge();
        this.renderToolbar();
        this.renderView();
        this.renderVersion();
        const langLabel = document.getElementById('btn-lang-label');
        if (langLabel) langLabel.textContent = this.i18n.locale === 'en' ? 'EN' : '中文';
        const titleEl = document.querySelector('aside h1');
        if (titleEl) titleEl.textContent = this.i18n.t('ui.title');
        document.title = this.i18n.t('ui.title');
    }

    renderTabBar() {
        const i = this.i18n;
        const skillsTab = document.getElementById('tab-skills');
        const mcpTab = document.getElementById('tab-mcp');
        const sessionsTab = document.getElementById('tab-sessions');
        const switchTab = document.getElementById('tab-switch');
        const monitorTab = document.getElementById('tab-monitor');
        // Monitor tab disabled — keep hidden defensively
        if (monitorTab) { monitorTab.hidden = true; monitorTab.style.display = 'none'; }
        skillsTab.textContent = i.t('ui.skills_tab');
        mcpTab.textContent = i.t('ui.mcp_tab');
        sessionsTab.textContent = i.t('ui.sessions_tab');
        switchTab.textContent = i.t('ui.switch_tab');
        skillsTab.className = `flex-1 py-2 text-sm text-center cursor-pointer border-b-2 ${this.currentTab === 'skills' ? 'text-gray-300 border-cyan-500' : 'text-gray-500 hover:text-white border-transparent'}`;
        mcpTab.className = `flex-1 py-2 text-sm text-center cursor-pointer border-b-2 ${this.currentTab === 'mcp' ? 'text-gray-300 border-cyan-500' : 'text-gray-500 hover:text-white border-transparent'}`;
        sessionsTab.className = `flex-1 py-2 text-sm text-center cursor-pointer border-b-2 ${this.currentTab === 'sessions' ? 'text-gray-300 border-cyan-500' : 'text-gray-500 hover:text-white border-transparent'}`;
        switchTab.className = `flex-1 py-2 text-sm text-center cursor-pointer border-b-2 ${this.currentTab === 'switch' ? 'text-gray-300 border-cyan-500' : 'text-gray-500 hover:text-white border-transparent'}`;
        if (monitorTab) monitorTab.className = `flex-1 py-2 text-sm text-center cursor-pointer border-b-2 ${this.currentTab === 'monitor' ? 'text-gray-300 border-cyan-500' : 'text-gray-500 hover:text-white border-transparent'}`;
    }

    renderSidebar() {
        const i = this.i18n;
        const el = document.getElementById('platform-list');
        const searchEl = document.getElementById('search-input');

        if (this.currentTab === 'mcp') {
            searchEl.classList.add('hidden');
            if (this.mcpPlatforms.length === 0) {
                el.innerHTML = `<p class="text-gray-500 text-sm p-3">No MCP-capable platforms.</p>`;
                return;
            }
            el.innerHTML = this.mcpPlatforms.map(p => {
                const active = p.id === this.selectedMcpPlatform;
                return `<button class="w-full text-left px-3 py-2 rounded cursor-pointer ${active ? 'bg-gray-700 text-green-400 font-bold' : 'text-gray-300 hover:bg-gray-700/50'}"
                    data-mcp-platform="${p.id}">
                    <div class="flex items-center justify-between">
                        <span class="text-sm">${esc(p.display_name)}</span>
                        <span class="text-xs text-gray-500">${p.server_count}</span>
                    </div>
                    ${active ? `<div class="text-xs text-gray-600 truncate mt-0.5">${esc(p.config_path)}</div>` : ''}
                </button>`;
            }).join('');
            el.querySelectorAll('button[data-mcp-platform]').forEach(btn => {
                btn.addEventListener('click', () => this.selectMcpPlatform(btn.dataset.mcpPlatform));
            });
            return;
        }

        if (this.currentTab === 'sessions') {
            searchEl.classList.add('hidden');
            if (this.sessionPlatforms.length === 0) {
                el.innerHTML = `<p class="text-gray-500 text-sm p-3">${i.t('session.no_platforms')}</p>`;
                return;
            }
            el.innerHTML = this.sessionPlatforms.map(p => {
                const active = p.id === this.selectedSessionPlatform;
                return `<button class="w-full text-left px-3 py-2 rounded cursor-pointer ${active ? 'bg-gray-700 text-green-400 font-bold' : 'text-gray-300 hover:bg-gray-700/50'}"
                    data-session-platform="${p.id}">
                    <div class="flex items-center justify-between">
                        <span class="text-sm">${esc(p.display_name)}</span>
                        <span class="text-xs text-gray-500">${p.session_count}</span>
                    </div>
                </button>`;
            }).join('');
            el.querySelectorAll('button[data-session-platform]').forEach(btn => {
                btn.addEventListener('click', () => this.selectSessionPlatform(btn.dataset.sessionPlatform));
            });
            return;
        }

        if (this.currentTab === 'monitor') {
            searchEl.classList.add('hidden');
            const activeSessions = this.monitorSessions.filter(s => s.status !== 'ended');
            const groups = [
                { id: 'kiro', name: 'Kiro' },
                { id: 'claude-code', name: 'Claude Code' },
                { id: 'codex', name: 'Codex' },
            ];
            const withSessions = groups.filter(g => activeSessions.some(s => s.agent_type === g.id));
            const totalCount = activeSessions.length;
            let html = '';
            // "All" option
            const allActive = this.selectedMonitorAgent === 'all';
            html += `<button class="w-full text-left px-3 py-2 rounded cursor-pointer ${allActive ? 'bg-gray-700 text-green-400 font-bold' : 'text-gray-300 hover:bg-gray-700/50'}"
                data-monitor-agent="all">
                <div class="flex items-center justify-between">
                    <span class="text-sm">${i.t('monitor.all_agents')}</span>
                    <span class="text-xs ${allActive ? 'text-green-400' : 'text-gray-500'}">${totalCount}</span>
                </div>
            </button>`;
            // Individual agents
            html += withSessions.map(g => {
                const count = activeSessions.filter(s => s.agent_type === g.id).length;
                const active = this.selectedMonitorAgent === g.id;
                return `<button class="w-full text-left px-3 py-2 rounded cursor-pointer ${active ? 'bg-gray-700 text-green-400 font-bold' : 'text-gray-300 hover:bg-gray-700/50'}"
                    data-monitor-agent="${g.id}">
                    <div class="flex items-center justify-between">
                        <span class="text-sm">${esc(g.name)}</span>
                        <span class="text-xs ${active ? 'text-green-400' : 'text-gray-500'}">${count}</span>
                    </div>
                </button>`;
            }).join('');
            el.innerHTML = html;
            el.querySelectorAll('button[data-monitor-agent]').forEach(btn => {
                btn.addEventListener('click', () => {
                    this.selectedMonitorAgent = btn.dataset.monitorAgent;
                    this.render();
                });
            });

            // Hooks configuration section
            const hooksAgents = [
                { id: 'claude-code', name: 'Claude Code' },
                { id: 'codex', name: 'Codex' },
                { id: 'kiro', name: 'Kiro' },
            ];
            const hs = this.hooksStatus || {};
            const hooksHtml = hooksAgents.map(a => {
                const configured = hs[a.id] || false;
                const btnClass = configured
                    ? 'text-xs px-2 py-0.5 rounded bg-red-900/30 text-red-400 hover:bg-red-900/50 cursor-pointer'
                    : 'text-xs px-2 py-0.5 rounded bg-green-900/30 text-green-400 hover:bg-green-900/50 cursor-pointer';
                const btnText = configured ? i.t('monitor.hooks_clear') : i.t('monitor.hooks_import');
                const dot = configured
                    ? '<span class="w-1.5 h-1.5 rounded-full bg-green-500 inline-block"></span>'
                    : '<span class="w-1.5 h-1.5 rounded-full bg-gray-600 inline-block"></span>';
                return `<div class="flex items-center justify-between px-3 py-1.5">
                    <div class="flex items-center gap-1.5">
                        ${dot}
                        <span class="text-xs text-gray-400">${esc(a.name)}</span>
                    </div>
                    <button class="${btnClass}" data-hooks-agent="${a.id}">${btnText}</button>
                </div>`;
            }).join('');

            el.insertAdjacentHTML('beforeend', `<div class="mt-4 pt-3 border-t border-gray-700">
                <div class="px-3 pb-1.5 text-xs text-gray-500 font-medium">${i.t('monitor.hooks_title')}</div>
                ${hooksHtml}
            </div>`);

            el.querySelectorAll('button[data-hooks-agent]').forEach(btn => {
                btn.addEventListener('click', async () => {
                    const agentId = btn.dataset.hooksAgent;
                    const configured = this.hooksStatus[agentId] || false;
                    btn.disabled = true;
                    btn.textContent = '...';
                    try {
                        if (configured) {
                            await Api.removeHooks(agentId);
                            this.showToast(i.t('monitor.hooks_removed'), 'success');
                        } else {
                            await Api.configureHooks(agentId);
                            this.showToast(i.t('monitor.hooks_configured'), 'success');
                        }
                        this.hooksStatus = await Api.getHooksStatus();
                    } catch (err) {
                        this.showToast(err.message || err.General || String(err), 'error');
                    }
                    this.render();
                });
            });
            return;
        }

        if (this.currentTab === 'switch') {
            searchEl.classList.add('hidden');
            const agents = [
                { id: 'codex', name: 'Codex', count: null },
                { id: 'claude-code', name: 'Claude Code', count: null },
            ];
            el.innerHTML = agents.map(a => {
                const active = this.switchSelectedAgent === a.id;
                return `<button class="w-full text-left px-3 py-2 rounded cursor-pointer ${active ? 'bg-gray-700 text-green-400 font-bold' : 'text-gray-300 hover:bg-gray-700/50'}"
                    data-switch-agent="${a.id}">
                    <div class="flex items-center justify-between">
                        <span class="text-sm">${esc(a.name)}</span>
                    </div>
                </button>`;
            }).join('');
            el.querySelectorAll('button[data-switch-agent]').forEach(btn => {
                btn.addEventListener('click', () => this.selectSwitchAgent(btn.dataset.switchAgent));
            });
            return;
        }

        // Skills tab (default)
        searchEl.classList.remove('hidden');
        if (this.platforms.length === 0) {
            el.innerHTML = `<p class="text-gray-500 text-sm p-3">${i.t('ui.no_platforms')}</p>`;
            return;
        }
        el.innerHTML = this.platforms.map(p => {
            const active = p.id === this.selectedPlatformId;
            const desc = p.description ? `<div class="text-xs text-gray-500 mt-0.5">${esc(p.description)}</div>` : '';
            const dir = `<div class="text-xs text-gray-600 truncate mt-0.5" title="${esc(p.skill_dir)}">${esc(p.skill_dir)}</div>`;
            return `<button class="w-full text-left px-3 py-2 rounded cursor-pointer ${active ? 'bg-gray-700 text-green-400 font-bold' : 'text-gray-300 hover:bg-gray-700/50'}"
                data-platform-id="${p.id}">
                <div class="flex items-center justify-between">
                    <span class="text-sm">${esc(p.display_name)}</span>
                </div>
                ${active ? desc + dir : ''}
            </button>`;
        }).join('');
        el.querySelectorAll('button[data-platform-id]').forEach(btn => {
            btn.addEventListener('click', () => this.selectPlatform(btn.dataset.platformId));
        });
    }

    renderToolbar() {
        const i = this.i18n;
        const back = document.getElementById('btn-back');
        const diff = document.getElementById('btn-diff');
        const sync = document.getElementById('btn-sync');
        const breadcrumb = document.getElementById('breadcrumb');
        const scanInvalid = document.getElementById('btn-scan-invalid');

        if (this.currentTab === 'mcp') {
            back.classList.add('hidden');
            diff.classList.add('hidden');
            sync.classList.add('hidden');
            scanInvalid.classList.add('hidden');
            breadcrumb.textContent = this.selectedMcpPlatform
                ? (this.mcpPlatforms.find(p => p.id === this.selectedMcpPlatform)?.display_name || '')
                : i.t('mcp.title');
            return;
        }

        if (this.currentTab === 'sessions') {
            back.classList.add('hidden');
            diff.classList.add('hidden');
            sync.classList.add('hidden');
            scanInvalid.classList.add('hidden');
            breadcrumb.textContent = this.selectedSessionPlatform
                ? (this.sessionPlatforms.find(p => p.id === this.selectedSessionPlatform)?.display_name || '')
                : i.t('session.title');
            return;
        }

        if (this.currentTab === 'switch') {
            back.classList.add('hidden');
            diff.classList.add('hidden');
            sync.classList.add('hidden');
            scanInvalid.classList.add('hidden');
            const agentNames = { codex: 'Codex', 'claude-code': 'Claude Code' };
            breadcrumb.textContent = this.switchSelectedAgent
                ? `${i.t('switch.title')} — ${agentNames[this.switchSelectedAgent] || ''}`
                : i.t('switch.title');
            return;
        }

        if (this.currentTab === 'monitor') {
            back.classList.add('hidden');
            diff.classList.add('hidden');
            sync.classList.add('hidden');
            scanInvalid.classList.add('hidden');
            const notificationEnabled = this.monitorConfig?.notification_enabled ?? false;
            breadcrumb.innerHTML = `
                <span>${i.t('monitor.title')}</span>
                <div class="flex-1"></div>
                <label class="flex items-center gap-2 text-xs text-gray-400 cursor-pointer mr-2">
                    <span>${i.t('monitor.notification_toggle')}</span>
                    <input type="checkbox" id="monitor-notification-toggle" ${notificationEnabled ? 'checked' : ''} class="cursor-pointer" />
                </label>
            `;
            const toggle = document.getElementById('monitor-notification-toggle');
            if (toggle) {
                toggle.addEventListener('change', async (e) => {
                    try {
                        this.monitorConfig = await Api.setMonitorConfig({ notification_enabled: e.target.checked });
                        if (e.target.checked) {
                            const anyConfigured = Object.values(this.hooksStatus || {}).some(v => v);
                            if (!anyConfigured) {
                                this.showToast(i.t('monitor.hooks_needed_hint'), 'warning');
                            }
                        }
                    } catch (err) {
                        console.error('Failed to update monitor config:', err);
                        e.target.checked = !e.target.checked;
                    }
                });
            }
            return;
        }

        scanInvalid.classList.remove('hidden');
        back.classList.toggle('hidden', this.currentView === 'skills');
        const showAction = this.currentView === 'detail' || this.currentView === 'diff';
        diff.classList.toggle('hidden', !showAction);
        diff.textContent = i.t('action.diff');
        sync.classList.toggle('hidden', this.currentView !== 'detail');
        sync.textContent = i.t('action.sync');

        if (this.currentView === 'skills' && this.selectedPlatformId) {
            const p = this.platforms.find(p => p.id === this.selectedPlatformId);
            breadcrumb.textContent = p ? p.display_name : '';
        } else if (this.selectedSkillName) {
            breadcrumb.textContent = this.selectedFolder
                ? `${this.selectedFolder}/${this.selectedSkillName}`
                : this.selectedSkillName;
        } else {
            breadcrumb.textContent = '';
        }
    }

    renderView() {
        const skillViews = ['skills', 'detail', 'diff', 'search'];
        const allViews = ['skills', 'detail', 'diff', 'search', 'mcp-servers', 'sessions', 'monitor', 'switch'];
        let activeViewId = null;

        if (this.currentTab === 'mcp') {
            for (const v of allViews) {
                document.getElementById(`view-${v}`).classList.toggle('hidden', v !== 'mcp-servers');
            }
            activeViewId = 'view-mcp-servers';
            this.renderMcpServerList();
        } else if (this.currentTab === 'sessions') {
            for (const v of allViews) {
                document.getElementById(`view-${v}`).classList.toggle('hidden', v !== 'sessions');
            }
            activeViewId = 'view-sessions';
            this.renderSessionsView();
        } else if (this.currentTab === 'monitor') {
            for (const v of allViews) {
                document.getElementById(`view-${v}`).classList.toggle('hidden', v !== 'monitor');
            }
            activeViewId = 'view-monitor';
            this.renderMonitorView();
        } else if (this.currentTab === 'switch') {
            for (const v of allViews) {
                document.getElementById(`view-${v}`).classList.toggle('hidden', v !== 'switch');
            }
            activeViewId = 'view-switch';
            this.renderSwitchView();
        } else {
            for (const v of allViews) {
                document.getElementById(`view-${v}`).classList.toggle('hidden', !skillViews.includes(v) || this.currentView !== v);
            }
            activeViewId = `view-${this.currentView}`;
            if (this.currentView === 'skills') this.renderSkillList();
            if (this.currentView === 'detail') this.renderSkillDetail();
            if (this.currentView === 'diff') this.renderDiffView();
            if (this.currentView === 'search') this.renderSearchResults();
        }

        const activeEl = activeViewId ? document.getElementById(activeViewId) : null;
        if (activeEl) {
            activeEl.classList.remove('view-transitioning');
            void activeEl.offsetWidth;
            activeEl.classList.add('view-transitioning');
        }
    }

    renderSessionsView() {
        const el = document.getElementById('view-sessions');
        const i = this.i18n;
        if (this.isSessionsLoading) {
            el.innerHTML = `<div class="loading-pulse text-gray-500">${i.t('session.loading_messages')}</div>`;
            return;
        }
        if (this.sessionsLoadError) {
            el.innerHTML = `<p class="text-red-400">${esc(i.tWith('session.load_failed', { error: this.sessionsLoadError }))}</p>`;
            return;
        }
        if (!this.selectedSessionPlatform) {
            el.innerHTML = this.renderEmptyState(i.t('session.select_platform'), 'search');
            return;
        }
        const terminalSource = (this.sessionTerminals && this.sessionTerminals.length > 0)
            ? this.sessionTerminals
            : [{ id: 'terminal-default', display_name: 'Terminal (Default)', available: true }];
        const terminalOptions = terminalSource.map(item => {
            const selected = item.id === this.selectedSessionTerminal ? 'selected' : '';
            const disabled = item.available ? '' : 'disabled';
            const label = item.available ? item.display_name : `${item.display_name} (${i.t('session.unavailable')})`;
            return `<option value="${esc(item.id)}" ${selected} ${disabled}>${esc(label)}</option>`;
        }).join('');
        const pathOptions = (this.sessionPathOptions && this.sessionPathOptions.length > 0
            ? this.sessionPathOptions
            : ['all', 'unknown']
        ).map((pathValue) => {
            const selected = pathValue === this.selectedSessionPathFilter ? 'selected' : '';
            let label = pathValue;
            if (pathValue === 'all') {
                label = i.t('session.path_filter_all');
            } else if (pathValue === 'unknown') {
                label = i.t('session.path_filter_unknown');
            }
            return `<option value="${esc(pathValue)}" ${selected}>${esc(label)}</option>`;
        }).join('');
        const currentPathLabel = this.selectedSessionPathFilter === 'all'
            ? i.t('session.path_filter_all')
            : (this.selectedSessionPathFilter === 'unknown'
                ? i.t('session.path_filter_unknown')
                : this.selectedSessionPathFilter);

        let html = `<div class="rounded-lg border border-gray-700 bg-gray-900/50 p-3 mb-3">
            <div class="flex items-start justify-between gap-3">
                <div class="text-xs text-gray-400">${i.tWith('session.loaded_summary', { loaded: formatInt(this.sessions.length), total: formatInt(this.sessionTotal || 0) })}</div>
                <div class="flex flex-col items-end gap-2">
                    <div class="flex items-center gap-2">
                        <span class="text-xs text-gray-500">${i.t('session.path_filter_label')}</span>
                        <select id="session-path-filter-select" class="text-xs bg-gray-800 border border-gray-700 rounded px-2 py-1 cursor-pointer w-[30rem] max-w-[65vw]">
                            ${pathOptions}
                        </select>
                    </div>
                    <div class="text-[11px] text-gray-500 max-w-[65vw] break-all text-right">${esc(i.tWith('session.path_filter_current', { path: currentPathLabel }))}</div>
                    <div class="flex items-center gap-2">
                        <span class="text-xs text-gray-500">${i.t('session.resume_terminal')}</span>
                        <select id="session-terminal-select" class="text-xs bg-gray-800 border border-gray-700 rounded px-2 py-1 cursor-pointer">
                            ${terminalOptions}
                        </select>
                    </div>
                </div>
            </div>
        </div>`;

        if (this.sessions.length === 0) {
            const emptyMessage = this.selectedSessionPathFilter && this.selectedSessionPathFilter !== 'all'
                ? i.t('session.path_filter_empty')
                : i.t('session.no_sessions');
            html += `<p class="text-gray-500">${emptyMessage}</p>`;
            el.innerHTML = html;
            const terminalSelect = el.querySelector('#session-terminal-select');
            if (terminalSelect) {
                terminalSelect.addEventListener('change', (e) => {
                    this.selectedSessionTerminal = e.target.value;
                });
            }
            const pathFilterSelect = el.querySelector('#session-path-filter-select');
            if (pathFilterSelect) {
                pathFilterSelect.addEventListener('change', (e) => this.changeSessionPathFilter(e.target.value));
            }
            return;
        }

        html += `<div class="space-y-2">`;
        for (const session of this.sessions) {
            const modelTag = session.model ? `<span class="text-cyan-400">${esc(session.model)}</span>` : '';
            const tokensTag = session.tokens_used != null ? `<span class="text-yellow-500">${i.tWith('session.tokens_value', { count: formatInt(session.tokens_used) })}</span>` : '';
            const messagesTag = session.message_count != null ? `<span class="text-purple-400">${i.tWith('session.messages_value', { count: formatInt(session.message_count) })}</span>` : '';
            const meta = [modelTag, tokensTag, messagesTag].filter(Boolean).join(' · ');
            const isResuming = this.resumingSessionId === session.id;
            const isDeleting = this.deletingSessionId === session.id;
            const isConfirmingDelete = this.confirmingSessionDeleteId === session.id;
            const deleteLabel = isDeleting
                ? i.t('session.deleting')
                : (isConfirmingDelete ? i.t('session.confirm_delete') : i.t('session.delete'));
            const deleteClass = isConfirmingDelete
                ? 'bg-red-700 hover:bg-red-600 text-white border-red-700'
                : 'text-red-500 hover:text-red-400 border-red-700/50 hover:bg-red-900/20';
            const actionDisabled = isDeleting ? 'disabled' : '';

            html += `<div class="rounded-lg border border-gray-700 hover:border-cyan-700 hover:bg-gray-800/50 p-3">
                <div class="flex items-center justify-between gap-3">
                    <h3 class="text-sm font-semibold text-gray-100 truncate">${esc(session.title || i.t('session.untitled'))}</h3>
                    <span class="text-xs text-gray-500 whitespace-nowrap">${esc(this.formatSessionTime(session.updated_at))}</span>
                </div>
                <div class="text-xs text-gray-500 break-all whitespace-normal mt-1">${esc(session.project_path || i.t('session.no_project'))}</div>
                ${meta ? `<div class="text-xs mt-2">${meta}</div>` : ''}
                <div class="mt-2 flex items-center justify-between gap-2">
                    <span class="text-xs text-gray-600">${i.tWith('session.started_at', { time: this.formatSessionTime(session.started_at) })}</span>
                    <div class="flex items-center gap-2">
                        <button class="px-2 py-1 text-xs border border-gray-600 rounded hover:bg-gray-700 cursor-pointer session-open-btn" data-session-id="${esc(session.id)}">${i.t('session.view_messages')}</button>
                        <button class="px-2 py-1 text-xs bg-green-700 hover:bg-green-600 rounded cursor-pointer session-resume-btn ${(isResuming || isDeleting) ? 'opacity-50' : ''}" data-session-id="${esc(session.id)}" ${(isResuming || isDeleting) ? 'disabled' : ''}>${isResuming ? i.t('session.resuming') : i.t('session.resume')}</button>
                        <button class="px-2 py-1 text-xs border rounded cursor-pointer session-delete-btn ${deleteClass} ${isDeleting ? 'opacity-50' : ''}" data-session-id="${esc(session.id)}" ${actionDisabled}>${deleteLabel}</button>
                    </div>
                </div>
            </div>`;
        }
        html += `</div>`;

        html += `<div class="mt-3 flex items-center justify-between">
            <span class="text-xs text-gray-500">${this.sessionHasMore ? i.t('session.more_available') : i.t('session.no_more_sessions')}</span>
            ${this.sessionHasMore ? `<button id="session-load-more-btn" class="px-3 py-1 text-xs bg-cyan-700 hover:bg-cyan-600 rounded cursor-pointer ${this.sessionLoadingMore ? 'opacity-50' : ''}" ${this.sessionLoadingMore ? 'disabled' : ''}>${this.sessionLoadingMore ? i.t('session.loading_more') : i.t('session.load_more')}</button>` : ''}
        </div>`;

        el.innerHTML = html;
        const terminalSelect = el.querySelector('#session-terminal-select');
        if (terminalSelect) {
            terminalSelect.addEventListener('change', (e) => {
                this.selectedSessionTerminal = e.target.value;
            });
        }
        const pathFilterSelect = el.querySelector('#session-path-filter-select');
        if (pathFilterSelect) {
            pathFilterSelect.addEventListener('change', (e) => this.changeSessionPathFilter(e.target.value));
        }
        el.querySelectorAll('.session-open-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const session = this.sessions.find(item => item.id === btn.dataset.sessionId);
                if (session) this.showSessionMessagesModal(session);
            });
        });
        el.querySelectorAll('.session-resume-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const session = this.sessions.find(item => item.id === btn.dataset.sessionId);
                if (session) this.resumeSessionInTerminal(session);
            });
        });
        el.querySelectorAll('.session-delete-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const session = this.sessions.find(item => item.id === btn.dataset.sessionId);
                if (session) this.deleteSessionRecord(session);
            });
        });
        const loadMoreBtn = el.querySelector('#session-load-more-btn');
        if (loadMoreBtn) {
            loadMoreBtn.addEventListener('click', () => this.loadMoreSessions());
        }
    }

    formatSessionTime(timestamp) {
        const n = Number(timestamp);
        if (!Number.isFinite(n) || n <= 0) return '-';
        const ms = n < 1e12 ? n * 1000 : n;
        const locale = this.i18n.locale === 'zh-CN' ? 'zh-CN' : 'en-US';
        return new Date(ms).toLocaleString(locale);
    }

    renderMcpServerList() {
        const el = document.getElementById('view-mcp-servers');
        const i = this.i18n;
        if (!this.selectedMcpPlatform) {
            el.innerHTML = this.renderEmptyState(i.t('mcp.title'), 'search');
            return;
        }
        if (this.mcpServers.length === 0) {
            el.innerHTML = `<div class="flex justify-between items-center mb-4">
                <div class="empty-state" style="padding:2rem 0"><div class="empty-state-icon">${Icons.search.replace(/width="\d+"/, 'width="28"').replace(/height="\d+"/, 'height="28"')}</div><p>${i.t('mcp.no_servers')}</p></div>
                <button class="px-3 py-1 text-xs bg-cyan-700 hover:bg-cyan-600 rounded cursor-pointer mcp-add-btn">+ ${i.t('mcp.add')}</button>
            </div>`;
            el.querySelector('.mcp-add-btn').addEventListener('click', () => this.showMcpAdd());
            return;
        }
        let html = `<div class="flex justify-end mb-4">
            <button class="px-3 py-1 text-xs bg-cyan-700 hover:bg-cyan-600 rounded cursor-pointer mcp-add-btn">+ ${i.t('mcp.add')}</button>
        </div>
        <div class="space-y-1">`;
        for (const s of this.mcpServers) {
            const expanded = this.expandedMcpServer === s.name;
            const detail = this.mcpServerDetails[s.name];
            html += `<div class="rounded ${expanded ? 'bg-gray-800' : 'hover:bg-gray-800/50'}">
                <div class="flex items-center gap-2 px-3 py-2 group">
                    <button class="flex-1 text-left cursor-pointer mcp-server-item flex items-center gap-2" data-name="${esc(s.name)}">
                        <span class="flex-shrink-0 text-gray-500 transition-transform duration-200 ${expanded ? 'rotate-90' : ''}">${Icons.arrowRight}</span>
                        <div class="flex-1">
                            <div class="text-cyan-400 font-medium">${esc(s.name)}</div>
                            <div class="text-gray-500 text-sm">${esc(s.summary)}</div>
                        </div>
                    </button>
                    <button class="text-xs text-cyan-600 hover:text-cyan-400 px-2 py-1 cursor-pointer opacity-0 group-hover:opacity-100 transition-opacity mcp-sync-btn" data-name="${esc(s.name)}">${i.t('mcp.sync')}</button>
                    <button class="text-xs text-red-600 hover:text-red-400 px-2 py-1 cursor-pointer opacity-0 group-hover:opacity-100 transition-opacity mcp-delete-btn" data-name="${esc(s.name)}">${i.t('mcp.delete')}</button>
                </div>
                <div class="mcp-expand-area ${expanded ? '' : 'hidden'}" data-server="${esc(s.name)}">
                    ${detail ? this.renderMcpAccordionContent(s.name, detail) : '<div class="px-3 pb-2 text-gray-500 text-sm">Loading...</div>'}
                </div>
            </div>`;
        }
        html += '</div>';
        el.innerHTML = html;
        el.querySelector('.mcp-add-btn').addEventListener('click', () => this.showMcpAdd());
        el.querySelectorAll('.mcp-server-item').forEach(btn => {
            btn.addEventListener('click', () => this.toggleMcpServer(btn.dataset.name));
        });
        el.querySelectorAll('.mcp-delete-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                this.deleteMcpServer(btn.dataset.name, btn);
            });
        });
        el.querySelectorAll('.mcp-sync-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                this.showMcpSyncModal(btn.dataset.name);
            });
        });
        // Bind blur auto-save on textareas
        el.querySelectorAll('textarea[data-edit-name]').forEach(ta => {
            const name = ta.dataset.editName;
            const detail = this.mcpServerDetails[name];
            const i = this.i18n;
            // Store original wrapped text for revert
            const originalText = ta.value;
            ta.addEventListener('blur', async () => {
                const text = ta.value.trim();
                if (text === originalText.trim()) return;
                // Validate and unwrap
                let saveText;
                if (detail.format === 'toml') {
                    // The backend accepts either a server fragment or a full [mcp_servers.*] block.
                    // If the displayed fragment has local subtables like [env], strip the wrapper so
                    // those subtables stay inside the edited server config.
                    const startsWithMcpWrapper = /^\s*\[mcp_servers\.[^\]]+\]\s*\n?/.test(text);
                    const hasLocalSubtable = /\n\s*\[(?!mcp_servers\.)[^\]]+\]/.test(text);
                    saveText = startsWithMcpWrapper && hasLocalSubtable
                        ? text.replace(/^\s*\[mcp_servers\.[^\]]+\]\s*\n?/, '')
                        : text;
                    if (!saveText.trim()) {
                        this.showToast('Empty config', 'warning');
                        ta.value = originalText;
                        return;
                    }
                } else {
                    try {
                        const parsed = JSON.parse(text);
                        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed) && Object.keys(parsed).length === 1 && parsed[name]) {
                            saveText = JSON.stringify(parsed[name], null, 2);
                        } else {
                            saveText = JSON.stringify(parsed, null, 2);
                        }
                    } catch {
                        this.showToast(i.t('mcp.parse_error').includes('{error}') ? 'Invalid JSON format' : i.tWith('mcp.parse_error', { error: 'Invalid JSON' }), 'error');
                        ta.value = originalText;
                        return;
                    }
                }
                try {
                    await Api.importMcpServer(this.selectedMcpPlatform, name, saveText);
                    // Update cache with new detail
                    const newDetail = await Api.getMcpServer(this.selectedMcpPlatform, name);
                    this.mcpServerDetails[name] = { config_text: newDetail.config_text, format: newDetail.format };
                    this.renderMcpServerList();
                } catch (e) {
                    this.showToast(i.tWith('mcp.parse_error', { error: e.SyncError || e }), 'error');
                    ta.value = originalText;
                }
            });
        });
    }

    renderMcpAccordionContent(name, detail) {
        const i = this.i18n;
        const isToml = detail.format === 'toml';
        // Wrap config for display
        let wrapped;
        let parsedConfig = null;
        if (isToml) {
            wrapped = `[mcp_servers.${name}]\n${detail.config_text}`;
        } else {
            try {
                const configObj = JSON.parse(detail.config_text);
                parsedConfig = configObj;
                const wrappedObj = {};
                wrappedObj[name] = configObj;
                wrapped = JSON.stringify(wrappedObj, null, 2);
            } catch {
                wrapped = detail.config_text;
            }
        }

        // Build structured config summary
        let summaryHtml = '';
        if (parsedConfig) {
            summaryHtml += '<div class="space-y-1.5 mb-3">';
            // Command
            if (parsedConfig.command) {
                summaryHtml += `<div class="flex items-start gap-2 text-sm">
                    <span class="text-gray-500 w-16 flex-shrink-0 text-xs font-medium uppercase tracking-wide">Command</span>
                    <code class="text-cyan-400 font-mono text-xs break-all">${esc(parsedConfig.command)}</code>
                </div>`;
            }
            // Args
            if (parsedConfig.args && Array.isArray(parsedConfig.args) && parsedConfig.args.length > 0) {
                summaryHtml += `<div class="flex items-start gap-2 text-sm">
                    <span class="text-gray-500 w-16 flex-shrink-0 text-xs font-medium uppercase tracking-wide">Args</span>
                    <code class="text-gray-300 font-mono text-xs">${esc(parsedConfig.args.join(' '))}</code>
                </div>`;
            }
            // Env
            if (parsedConfig.env && typeof parsedConfig.env === 'object' && Object.keys(parsedConfig.env).length > 0) {
                summaryHtml += '<div class="flex items-start gap-2 text-sm">';
                summaryHtml += '<span class="text-gray-500 w-16 flex-shrink-0 text-xs font-medium uppercase tracking-wide">Env</span>';
                summaryHtml += '<div class="space-y-0.5">';
                for (const [k, v] of Object.entries(parsedConfig.env)) {
                    summaryHtml += `<div class="text-xs font-mono"><span class="text-purple-400">${esc(k)}</span><span class="text-gray-600">=</span><span class="text-green-400">${esc(String(v))}</span></div>`;
                }
                summaryHtml += '</div></div>';
            }
            summaryHtml += '</div>';
        }

        return `<div class="px-3 pb-3 space-y-2">
            ${summaryHtml}
            <div class="text-xs text-gray-500">${isToml ? 'TOML' : 'JSON'}</div>
            <textarea data-edit-name="${esc(name)}" style="height:16rem" class="w-full bg-gray-900 text-sm text-gray-200 font-mono rounded p-3 border border-gray-600 focus:border-cyan-500 outline-none resize-y">${esc(wrapped)}</textarea>
        </div>`;
    }

    async toggleMcpServer(name) {
        if (this.expandedMcpServer === name) {
            this.expandedMcpServer = null;
            this.renderMcpServerList();
            return;
        }
        this.expandedMcpServer = name;
        // Fetch detail if not cached
        if (!this.mcpServerDetails[name]) {
            try {
                const detail = await Api.getMcpServer(this.selectedMcpPlatform, name);
                this.mcpServerDetails[name] = { config_text: detail.config_text, format: detail.format };
            } catch (e) {
                this.showToast('Error: ' + e, 'error');
                this.expandedMcpServer = null;
                return;
            }
        }
        this.renderMcpServerList();
    }

    renderSkillList() {
        const el = document.getElementById('view-skills');
        const i = this.i18n;
        if (this.skills.length === 0) {
            el.innerHTML = this.renderEmptyState(i.t('ui.no_skills'), 'folder');
            return;
        }

        const platform = this.platforms.find(p => p.id === this.selectedPlatformId);
        const platformName = platform ? platform.display_name : (this.selectedPlatformId || '');

        // KPI computed values
        const totalSkills = this.skills.length;
        const enabledSkills = this.skills.filter(s => s.version || s.description).length;
        const totalBytes = this.skills.reduce((acc, s) => acc + (s.total_size || 0), 0);
        const totalSizeStr = formatBytes(totalBytes);

        // Group by folder (root="" first, then folders in encounter order)
        const groups = new Map();
        groups.set('', []);
        for (const s of this.skills) {
            const key = s.folder || '';
            if (!groups.has(key)) groups.set(key, []);
            groups.get(key).push(s);
        }
        if (groups.get('').length === 0) groups.delete('');

        // Sort within each group
        const sortFn = (a, b) => this.skillSortDir === 'desc'
            ? (b.total_size || 0) - (a.total_size || 0)
            : (a.total_size || 0) - (b.total_size || 0);
        if (this.skillSortBy === 'size') {
            for (const arr of groups.values()) arr.sort(sortFn);
        }

        const sortIcon = this.skillSortDir === 'asc' ? Icons.sortAsc : Icons.sortDesc;
        const sizeSortClass = this.skillSortBy === 'size' ? 'sorted' : '';

        const renderRow = (s) => {
            const tone = avatarToneFromName(s.name);
            const initial = (s.name || '?').charAt(0).toUpperCase();
            const sizeStr = s.total_size < 1024 ? `${s.total_size} B`
                : s.total_size < 1048576 ? `${(s.total_size / 1024).toFixed(1)} KB`
                : `${(s.total_size / 1048576).toFixed(1)} MB`;
            const versionChip = s.version
                ? `<span class="ah-version-chip">v${esc(s.version)}</span>` : '';
            const linkMark = s.is_symlink
                ? `<span class="ah-row__symlink" title="→ ${esc(s.symlink_target || '')}">${Icons.symlink}</span>`
                : '';
            const desc = s.description ? esc(s.description) : '';
            return `<div class="ah-row" data-name="${esc(s.name)}" data-folder="${esc(s.folder || '')}">
                <div class="ah-row__name">
                    <div class="ah-avatar" data-tone="${tone}">${initial}</div>
                    <div class="ah-row__name-text">
                        <span class="ah-row__skill-name">${esc(s.name)}</span>
                        ${versionChip}${linkMark}
                    </div>
                </div>
                <div class="ah-row__desc">${desc}</div>
                <div class="ah-row__size">${sizeStr}</div>
                <div class="ah-row__actions">
                    <button class="ah-kebab ah-kebab-trigger" data-name="${esc(s.name)}" data-folder="${esc(s.folder || '')}" title="更多操作">${Icons.moreVert}</button>
                </div>
            </div>`;
        };

        // Folder symlink detection (any child symlink → mark folder)
        const folderSymlinkInfo = (items) => {
            const s = items.find(x => x.is_symlink);
            return s ? { is_symlink: true, target: s.symlink_target || '' } : { is_symlink: false };
        };

        // Build grouped rows html (root group first, no header)
        let groupsHtml = '';
        for (const [folder, items] of groups) {
            if (folder === '') {
                groupsHtml += `<div class="ah-rows-group">${items.map(renderRow).join('')}</div>`;
            } else {
                const collapsed = this.collapsedFolders.has(folder);
                const linkInfo = folderSymlinkInfo(items);
                const symlinkBadge = linkInfo.is_symlink
                    ? `<span class="ah-symlink-badge" title="→ ${esc(linkInfo.target)}">${Icons.symlink}<span>软链接</span></span>`
                    : '';
                groupsHtml += `<section class="ah-folder-section">
                    <button class="ah-folder-header" data-folder="${esc(folder)}">
                        <span class="ah-folder-arrow">${collapsed ? '▶' : '▼'}</span>
                        <span class="ah-folder-name">${esc(folder)}</span>
                        <span class="ah-folder-count">(${items.length})</span>
                        ${symlinkBadge}
                    </button>
                    <div class="ah-folder-content" data-folder-content="${esc(folder)}" ${collapsed ? 'hidden' : ''}>
                        ${items.map(renderRow).join('')}
                    </div>
                </section>`;
            }
        }

        el.innerHTML = `
            <div class="ah-page-header">
                <h1 class="ah-page-title">${esc(platformName)}</h1>
            </div>
            <div class="ah-kpi-row">
                <article class="ah-kpi">
                    <div class="ah-kpi__chip" data-tone="accent">${Icons.grid22}</div>
                    <div class="ah-kpi__body">
                        <p class="ah-kpi__label">技能总数</p>
                        <p class="ah-kpi__value"><span class="ah-kpi__num">${totalSkills}</span><span class="ah-kpi__unit">个技能</span></p>
                    </div>
                </article>
                <article class="ah-kpi">
                    <div class="ah-kpi__chip" data-tone="success">${Icons.code22}</div>
                    <div class="ah-kpi__body">
                        <p class="ah-kpi__label">启用中</p>
                        <p class="ah-kpi__value"><span class="ah-kpi__num">${enabledSkills}</span><span class="ah-kpi__unit">个技能</span></p>
                    </div>
                </article>
                <article class="ah-kpi">
                    <div class="ah-kpi__chip" data-tone="warning">${Icons.clock22}</div>
                    <div class="ah-kpi__body">
                        <p class="ah-kpi__label">最近更新</p>
                        <p class="ah-kpi__value"><span class="ah-kpi__num" style="font-size:18px">—</span></p>
                    </div>
                </article>
                <article class="ah-kpi">
                    <div class="ah-kpi__chip" data-tone="highlight">${Icons.cube22}</div>
                    <div class="ah-kpi__body">
                        <p class="ah-kpi__label">总大小</p>
                        <p class="ah-kpi__value"><span class="ah-kpi__num" style="font-size:20px">${esc(totalSizeStr)}</span></p>
                    </div>
                </article>
            </div>
            <div class="ah-table-wrap">
                <div class="ah-thead">
                    <div class="ah-th">技能名称</div>
                    <div class="ah-th">描述</div>
                    <div class="ah-th ah-th--size sortable ${sizeSortClass}" id="ah-sort-size">
                        大小 <span class="ah-sort-icon">${sizeSortClass ? sortIcon : Icons.sortBoth}</span>
                    </div>
                    <div class="ah-th"></div>
                </div>
                ${groupsHtml}
            </div>`;

        // Row click → detail
        el.querySelectorAll('.ah-row').forEach(row => {
            row.addEventListener('click', (e) => {
                if (e.target.closest('.ah-kebab-trigger')) return;
                this.selectSkill(row.dataset.name, row.dataset.folder);
            });
        });

        // Sort by size
        const sortSizeBtn = el.querySelector('#ah-sort-size');
        if (sortSizeBtn) {
            sortSizeBtn.addEventListener('click', () => {
                if (this.skillSortBy === 'size') {
                    this.skillSortDir = this.skillSortDir === 'desc' ? 'asc' : 'desc';
                } else {
                    this.skillSortBy = 'size';
                    this.skillSortDir = 'desc';
                }
                this.renderSkillList();
            });
        }

        // Folder toggle
        el.querySelectorAll('.ah-folder-header').forEach(btn => {
            btn.addEventListener('click', () => {
                this.toggleFolder(btn.dataset.folder);
            });
        });

        // Kebab menus
        el.querySelectorAll('.ah-kebab-trigger').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                this._toggleKebab(btn, btn.dataset.name, btn.dataset.folder);
            });
        });

        // Close kebab on outside click
        document.addEventListener('click', this._kebabOutsideHandler = () => {
            document.querySelectorAll('.ah-kebab-menu').forEach(m => m.remove());
            document.removeEventListener('click', this._kebabOutsideHandler);
        }, { once: true });
    }

    _renderPagination(total, page, pageCount) {
        if (pageCount <= 1) return '';
        const prev = page > 0;
        const next = page < pageCount - 1;

        let pageNums = '';
        const addPage = (p) => {
            const active = p === page ? 'is-active' : '';
            pageNums += `<li><button class="ah-page-btn ${active}" data-page="${p}">${p + 1}</button></li>`;
        };
        const addEllipsis = () => { pageNums += `<li><span class="ah-page-ellipsis">…</span></li>`; };

        if (pageCount <= 7) {
            for (let p = 0; p < pageCount; p++) addPage(p);
        } else {
            addPage(0);
            if (page > 2) addEllipsis();
            for (let p = Math.max(1, page - 1); p <= Math.min(pageCount - 2, page + 1); p++) addPage(p);
            if (page < pageCount - 3) addEllipsis();
            addPage(pageCount - 1);
        }

        return `<nav class="ah-pagination">
            <span class="ah-pagination__total">共 ${total} 项</span>
            <ul class="ah-pagination__pages">
                <li><button class="ah-page-btn" data-page="${page - 1}" ${prev ? '' : 'disabled'}>‹</button></li>
                ${pageNums}
                <li><button class="ah-page-btn" data-page="${page + 1}" ${next ? '' : 'disabled'}>›</button></li>
            </ul>
            <div class="ah-pagination__size">
                <select id="ah-page-size">
                    ${[10, 20, 50].map(n => `<option value="${n}" ${n === this.skillPageSize ? 'selected' : ''}>${n} 条/页</option>`).join('')}
                </select>
            </div>
        </nav>`;
    }

    _toggleKebab(triggerBtn, name, folder) {
        document.querySelectorAll('.ah-kebab-menu').forEach(m => m.remove());
        const i = this.i18n;
        const menu = document.createElement('div');
        menu.className = 'ah-kebab-menu';
        menu.innerHTML = `
            <button class="ah-kebab-item" data-action="open">${i.t('action.diff') || '对比'}</button>
            <button class="ah-kebab-item" data-action="sync">${i.t('action.sync') || '同步'}</button>
            <button class="ah-kebab-item danger" data-action="delete">${i.t('skill.delete') || '删除'}</button>`;

        const rect = triggerBtn.getBoundingClientRect();
        menu.style.top = `${rect.bottom + 4}px`;
        menu.style.right = `${window.innerWidth - rect.right}px`;
        document.body.appendChild(menu);

        menu.querySelectorAll('[data-action]').forEach(item => {
            item.addEventListener('click', (e) => {
                e.stopPropagation();
                menu.remove();
                const action = item.dataset.action;
                if (action === 'delete') this.deleteSkill(name, folder, triggerBtn);
                else if (action === 'open') this.selectSkill(name, folder);
                else if (action === 'sync') {
                    this.selectSkill(name, folder).then(() => { /* sync modal */ });
                }
            });
        });
    }

    renderSkillItem(s) {
        // Legacy fallback — not used in skills tab anymore, kept for search results
        const i = this.i18n;
        const version = s.version ? `<span class="text-gray-500 text-xs ml-2">v${esc(s.version)}</span>` : '';
        const desc = s.description ? `<span class="text-gray-500 text-sm ml-2">${esc(truncate(s.description, 60))}</span>` : '';
        return `<div class="flex items-center rounded hover:bg-gray-800 group">
            <button class="flex-1 text-left px-3 py-2 text-gray-200 cursor-pointer skill-item"
                data-name="${esc(s.name)}" data-folder="${esc(s.folder)}">
                <span class="text-cyan-400">${esc(s.name)}</span>${version}${desc}
            </button>
            <button class="text-xs text-red-600 hover:text-red-400 px-2 py-1 cursor-pointer opacity-0 group-hover:opacity-100 transition-opacity skill-delete-btn"
                data-name="${esc(s.name)}" data-folder="${esc(s.folder)}">${i.t('skill.delete')}</button>
        </div>`;
    }

    toggleFolder(folder) {
        if (this.collapsedFolders.has(folder)) {
            this.collapsedFolders.delete(folder);
        } else {
            this.collapsedFolders.add(folder);
        }
        this.renderSkillList();
    }

    async renderSkillDetail() {
        const el = document.getElementById('view-detail');
        const i = this.i18n;
        try {
            const detail = await Api.getSkillDetail(this.selectedPlatformId, this.selectedSkillName, this.selectedFolder);
            const sizeStr = detail.total_size < 1024 ? `${detail.total_size} B`
                : detail.total_size < 1048576 ? `${(detail.total_size / 1024).toFixed(1)} KB`
                : `${(detail.total_size / 1048576).toFixed(1)} MB`;

            const tone = avatarToneFromName(detail.name);

            // Hero
            const heroHtml = `
                <header class="ah-hero">
                    <div class="ah-hero__icon" data-tone="${tone}">${skillIconSvg(detail.name)}</div>
                    <div class="ah-hero__text">
                        <h1 class="ah-hero__title">${esc(detail.name)}</h1>
                        ${detail.description ? `<p class="ah-hero__subtitle">${esc(detail.description)}</p>` : ''}
                    </div>
                </header>`;

            // Metadata row (6 cards: + 类型)
            const shortPath = detail.path.length > 36
                ? '…' + detail.path.slice(detail.path.length - 36)
                : detail.path;
            const typeLabel = detail.is_symlink ? '软链接' : '目录';
            const typeTarget = detail.is_symlink && detail.symlink_target
                ? (detail.symlink_target.length > 28
                    ? '…' + detail.symlink_target.slice(detail.symlink_target.length - 28)
                    : detail.symlink_target)
                : '';
            const metaHtml = `
                <div class="ah-meta-row ah-meta-row--6">
                    <article class="ah-meta">
                        <div class="ah-meta__chip">${Icons.layers18}</div>
                        <p class="ah-meta__label">平台</p>
                        <p class="ah-meta__value">${esc(detail.platform_id)}</p>
                    </article>
                    <article class="ah-meta">
                        <div class="ah-meta__chip">${Icons.folder18}</div>
                        <p class="ah-meta__label">路径</p>
                        <p class="ah-meta__value ah-meta__value--mono" title="${esc(detail.path)}">${esc(shortPath)}</p>
                    </article>
                    <article class="ah-meta">
                        <div class="ah-meta__chip">${Icons.tag18}</div>
                        <p class="ah-meta__label">版本</p>
                        <p class="ah-meta__value ah-meta__value--mono">${detail.version ? esc(detail.version) : '—'}</p>
                    </article>
                    <article class="ah-meta">
                        <div class="ah-meta__chip">${Icons.cube18}</div>
                        <p class="ah-meta__label">大小</p>
                        <p class="ah-meta__value ah-meta__value--mono">${sizeStr}</p>
                    </article>
                    <article class="ah-meta">
                        <div class="ah-meta__chip">${Icons.file18}</div>
                        <p class="ah-meta__label">文件数</p>
                        <p class="ah-meta__value">${detail.files.length} 个文件</p>
                    </article>
                    <article class="ah-meta">
                        <div class="ah-meta__chip" ${detail.is_symlink ? 'data-tone="accent"' : ''}>${Icons.symlink}</div>
                        <p class="ah-meta__label">类型</p>
                        <p class="ah-meta__value${detail.is_symlink ? ' ah-meta__value--mono' : ''}"
                           ${detail.is_symlink ? `title="${esc(detail.symlink_target || '')}"` : ''}>
                            ${esc(typeLabel)}${typeTarget ? ` <span style="color:var(--ink-4)">→ ${esc(typeTarget)}</span>` : ''}
                        </p>
                    </article>
                </div>`;

            // Pick default file: SKILL.md (case-insensitive) → first file
            const defaultFile =
                detail.files.find(f => /(^|\/)SKILL\.md$/i.test(f))
                || detail.files[0]
                || null;

            // File list + always-visible viewer
            const filesHtml = detail.files.length > 0 ? `
                <section class="ah-desc-block" style="margin-bottom:12px">
                    <h2 class="ah-section-title">文件</h2>
                    <div class="ah-files-list">
                        ${detail.files.map(f => `
                            <div class="ah-file file-item" data-file="${esc(f)}" ${f === defaultFile ? 'data-active="true"' : ''}>
                                <span class="ah-file__icon">${Icons.fileSmall}</span>
                                <span class="ah-file__name">${esc(f)}</span>
                                <span class="ah-file__size"></span>
                            </div>`).join('')}
                    </div>
                    <div id="file-viewer">
                        <div class="ah-file-viewer__header">
                            <span id="file-viewer-path"></span>
                        </div>
                        <pre id="file-viewer-content" class="ah-file-viewer"></pre>
                    </div>
                </section>` : '';

            el.innerHTML = heroHtml + metaHtml + filesHtml;

            // Internal: load file + update active state
            const loadFile = async (filePath) => {
                el.querySelectorAll('.file-item').forEach(item => {
                    item.removeAttribute('data-active');
                    if (item.dataset.file === filePath) item.setAttribute('data-active', 'true');
                });
                const pathEl = document.getElementById('file-viewer-path');
                const contentEl = document.getElementById('file-viewer-content');
                if (pathEl) pathEl.textContent = filePath;
                if (contentEl) contentEl.textContent = '';
                try {
                    const content = await Api.readSkillFile(this.selectedPlatformId, this.selectedSkillName, this.selectedFolder, filePath);
                    if (contentEl) contentEl.textContent = content;
                } catch (e) {
                    if (contentEl) contentEl.textContent = `Error: ${e}`;
                }
            };

            // Bind file clicks
            el.querySelectorAll('.file-item').forEach(item => {
                item.addEventListener('click', () => loadFile(item.dataset.file));
            });

            // Auto-load default file
            if (defaultFile) loadFile(defaultFile);
        } catch (e) {
            el.innerHTML = `<div style="padding:20px;color:var(--danger)">Error: ${esc(String(e))}</div>`;
        }
    }

    renderDiffView() {
        const el = document.getElementById('view-diff');
        const i = this.i18n;
        const diff = this.diffResult;
        if (!diff) { el.innerHTML = ''; return; }

        let html = `
            <h2 class="text-xl font-bold text-purple-400 mb-4">${i.t('diff.title')}: ${esc(diff.skill_name)}</h2>
            <div class="mb-4 text-sm">
                <span class="text-cyan-400">${i.t('diff.source_label')}:</span> <span class="text-gray-300">${esc(diff.source_platform)}</span>
                <span class="text-gray-600 mx-2">→</span>
                <span class="text-cyan-400">${i.t('diff.target_label')}:</span> <span class="text-gray-300">${esc(diff.target_platform)}</span>
                <span class="ml-4 text-green-400">${i.tWith('diff.added', { n: diff.stats.added })}</span>
                <span class="mx-1 text-gray-600">/</span>
                <span class="text-red-400">${i.tWith('diff.removed', { n: diff.stats.removed })}</span>
            </div>`;

        for (const fd of diff.file_diffs) {
            html += `<div class="mb-6">
                <div class="font-bold text-purple-400 mb-1">--- ${esc(fd.file_path)} `;
            if (fd.source_only) {
                html += `<span class="text-green-400">${i.tWith('diff.only_in', { platform: diff.source_platform })}</span>`;
            } else if (fd.target_only) {
                html += `<span class="text-red-400">${i.tWith('diff.only_in', { platform: diff.target_platform })}</span>`;
            } else {
                html += `<span class="text-gray-500">+${fd.stats.added} -${fd.stats.removed}</span>`;
            }
            html += `</div>`;
            html += renderSideBySide(toSideBySide(fd.lines));
            html += `</div>`;
        }

        if (diff.file_diffs.length === 0) {
            html += `<p class="text-gray-500">${i.t('diff.no_diff')}</p>`;
        }

        el.innerHTML = html;
    }

    renderSearchResults() {
        const el = document.getElementById('view-search');
        const i = this.i18n;
        if (this.searchLoading) {
            el.innerHTML = `<div class="flex items-center justify-center py-16 text-gray-400 text-sm gap-2">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="animate-spin"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
                <span>${i.t('ui.searching')}</span>
            </div>`;
            return;
        }
        if (this.searchResults.length === 0) {
            el.innerHTML = this.renderEmptyState(i.t('ui.no_results'), 'search');
            return;
        }
        el.innerHTML = `<h2 class="text-lg font-bold text-gray-300 mb-3">${i.t('ui.search_results')}</h2>
            <div class="space-y-0.5">${this.searchResults.map(r => {
                const folderTag = r.folder ? `<span class="text-yellow-600 text-xs ml-1">${esc(r.folder)}/</span>` : '';
                return `<button class="w-full text-left px-3 py-2 rounded hover:bg-gray-800 cursor-pointer search-result"
                    data-platform="${esc(r.platform_id)}" data-skill="${esc(r.skill_name)}" data-folder="${esc(r.folder)}">
                    <span class="text-cyan-400">${esc(r.skill_name)}</span>${folderTag}
                    <span class="text-gray-500 text-xs ml-2">${esc(r.platform_name)}</span>
                    <span class="text-gray-600 text-sm ml-2">${esc(truncate(r.description, 50))}</span>
                </button>`;
            }).join('')}</div>`;
        el.querySelectorAll('.search-result').forEach(btn => {
            btn.addEventListener('click', () => {
                this.selectedPlatformId = btn.dataset.platform;
                this.selectedSkillName = btn.dataset.skill;
                this.selectedFolder = btn.dataset.folder || '';
                this.currentView = 'detail';
                this.loadSkills().then(() => this.render());
            });
        });
    }

    // --- Monitor Methods ---

    async refreshMonitor() {
        try {
            // Force a fresh scan up front so the UI doesn't sit on a stale cache
            // between 5s polling ticks. ensure_scanned in the backend only runs
            // on the very first call.
            await Api.forcePollMonitor();
            const [sessions, config] = await Promise.all([
                Api.getActiveSessions(),
                Api.getMonitorConfig(),
            ]);
            this.monitorSessions = sessions;
            this.monitorConfig = config;
        } catch (err) {
            console.error('Failed to load monitor data:', err);
            this.monitorSessions = [];
        }
        // Fetch hooks status separately so it doesn't block main data
        try {
            this.hooksStatus = await Api.getHooksStatus();
        } catch (err) {
            console.warn('Failed to load hooks status:', err);
            this.hooksStatus = {};
        }
    }

    startMonitorListener() {
        if (this.monitorUnlisten) return;
        const { listen } = window.__TAURI_INTERNALS__;
        if (!listen) return;
        listen('monitor:state-changed', (event) => {
            const { change, session } = event.payload || {};
            if (!session) return;
            const idx = this.monitorSessions.findIndex(s => s.session_id === session.session_id);
            if (change === 'added') {
                if (idx === -1) {
                    this.monitorSessions.push(session);
                } else {
                    this.monitorSessions[idx] = session;
                }
            } else if (change === 'updated') {
                if (idx !== -1) {
                    this.monitorSessions[idx] = session;
                } else {
                    this.monitorSessions.push(session);
                }
            } else if (change === 'removed') {
                if (idx !== -1) {
                    this.monitorSessions[idx] = session;
                }
                // Remove ended sessions after 30s
                setTimeout(() => {
                    this.monitorSessions = this.monitorSessions.filter(
                        s => s.session_id !== session.session_id || s.status !== 'ended'
                    );
                    this.render();
                }, 30000);
            }
            this.render();
        }).then(fn => { this.monitorUnlisten = fn; });
    }

    stopMonitorListener() {
        if (this.monitorUnlisten) {
            this.monitorUnlisten();
            this.monitorUnlisten = null;
        }
    }

    monitorSortValue(session) {
        const value = session?.last_reply_at || session?.last_activity || session?.started_at;
        if (!value) return 0;
        if (typeof value === 'number') return value < 1e12 ? value * 1000 : value;
        const parsed = Date.parse(value);
        return Number.isFinite(parsed) ? parsed : 0;
    }

    formatMonitorTime(value) {
        const ms = this.monitorSortValue({ last_reply_at: value });
        if (!ms) return '-';
        const locale = this.i18n.locale === 'zh-CN' ? 'zh-CN' : 'en-US';
        return new Date(ms).toLocaleString(locale);
    }

    renderMonitorView() {
        const el = document.getElementById('view-monitor');
        const i = this.i18n;
        const activeSessions = this.monitorSessions.filter(s => s.status !== 'ended');

        if (activeSessions.length === 0) {
            el.innerHTML = `
                <div class="flex flex-col items-center justify-center h-full text-gray-500 gap-4">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-gray-600">
                        <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>
                    </svg>
                    <p class="text-sm">${i.t('monitor.empty')}</p>
                    <p class="text-xs text-gray-600">${i.t('monitor.empty_hint')}</p>
                </div>`;
            return;
        }

        if (!this.selectedMonitorAgent) {
            el.innerHTML = `<div class="flex flex-col items-center justify-center h-full text-gray-500 gap-3">
                <p class="text-sm">${i.t('monitor.no_selection')}</p>
            </div>`;
            return;
        }

        const sessions = this.selectedMonitorAgent === 'all'
            ? activeSessions
            : activeSessions.filter(s => s.agent_type === this.selectedMonitorAgent);
        sessions.sort((a, b) => this.monitorSortValue(b) - this.monitorSortValue(a));
        if (sessions.length === 0) {
            el.innerHTML = `<p class="text-gray-500 text-sm">${i.t('monitor.empty')}</p>`;
            return;
        }

        // Data limited warning
        let html = '';
        const limitedSession = sessions.find(s => s.data_limited);
        if (limitedSession) {
            const reasonKey = limitedSession.data_limited_reason || '';
            const reasonText = i.t(reasonKey);
            html += `<div class="mb-3 px-3 py-2 rounded text-xs text-yellow-400 bg-yellow-900/20 border border-yellow-800/30">⚠️ ${esc(reasonText)}</div>`;
        }

        html += `<div class="space-y-2">`;
        for (const s of sessions) {
            html += this.renderMonitorCard(s);
        }
        html += `</div>`;

        el.innerHTML = html;
    }

    renderMonitorCard(session) {
        const i = this.i18n;
        const statusMap = {
            active: { color: 'bg-green-500', pulse: true, text: i.t('monitor.status.active') },
            idle: { color: 'bg-gray-400', pulse: false, text: i.t('monitor.status.idle') },
            completed: { color: 'bg-blue-500', pulse: false, text: i.t('monitor.status.completed') },
            ended: { color: 'bg-red-500', pulse: false, text: i.t('monitor.status.ended') },
        };
        const wsMap = {
            working: { color: 'bg-green-500', pulse: true, text: i.t('monitor.working_state.working') },
            idle: { color: 'bg-gray-400', pulse: false, text: i.t('monitor.working_state.idle') },
            finished: { color: 'bg-blue-500', pulse: false, text: i.t('monitor.working_state.finished') },
        };
        // Adapters with stream data expose real working_state; otherwise fall back to status.
        const ws = session.working_state || 'idle';
        const st = (!session.data_limited && wsMap[ws]) || statusMap[session.status] || statusMap.idle;
        const sourceStyle = session.source_tag === 'CLI'
            ? 'bg-gray-700 text-gray-300'
            : 'bg-gray-600 text-gray-200';

        let duration = '';
        if (session.started_at) {
            const started = new Date(session.started_at);
            const now = Date.now();
            const secs = Math.floor((now - started.getTime()) / 1000);
            if (secs < 60) duration = `${secs}s`;
            else if (secs < 3600) duration = `${Math.floor(secs / 60)}m ${secs % 60}s`;
            else duration = `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
        }

        const cwdShort = session.cwd
            ? session.cwd.split('/').slice(-2).join('/')
            : '';
        const pidStr = session.pid ? i.tWith('monitor.pid', { pid: session.pid }) : '';
        const preview = session.last_message_preview || '';
        const replyTime = session.last_reply_at ? this.formatMonitorTime(session.last_reply_at) : '';
        const previewId = `preview-${session.session_id.replace(/[^a-zA-Z0-9-]/g, '_')}`;
        // Q-prompt-A-reply: headline = last user prompt. Falls back to project title
        // when no local conversation data is available.
        const userPrompt = (session.last_user_prompt || '').trim();
        const headline = userPrompt || session.title || session.session_id;
        const projectTag = userPrompt ? (session.title || '') : '';

        // Prefer the agent's last reply; fall back to the current working state.
        let subtitle = preview;
        if (!subtitle && !session.data_limited) {
            subtitle = session.working_state === 'working'
                ? i.t('monitor.working_state.working')
                : i.t('monitor.working_state.idle');
        }

        return `<div class="bg-gray-800 rounded-lg border border-gray-700 p-3">
            <div class="flex items-start justify-between">
                <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2 mb-1">
                        <span class="${st.color} ${st.pulse ? 'animate-pulse' : ''} w-2 h-2 rounded-full inline-block"></span>
                        <span class="text-xs text-gray-400">${esc(st.text)}</span>
                        <span class="text-xs px-1.5 py-0.5 rounded ${sourceStyle}">${esc(session.source_tag)}</span>
                        ${pidStr ? `<span class="text-xs text-gray-600 font-mono">${esc(pidStr)}</span>` : ''}
                    </div>
                    <div class="text-sm text-gray-200 truncate" title="${esc(headline)}">${esc(headline)}</div>
                    ${projectTag ? `<div class="text-xs text-gray-500 truncate mt-0.5">${esc(projectTag)}</div>` : ''}
                    <div class="flex items-center gap-3 mt-1 text-xs text-gray-500">
                        <span>${esc(session.model)}</span>
                        ${cwdShort ? `<span class="truncate" title="${esc(session.cwd)}">${esc(cwdShort)}</span>` : ''}
                        ${duration ? `<span>${esc(duration)}</span>` : ''}
                        ${replyTime ? `<span>${i.tWith('monitor.reply_time', { time: replyTime })}</span>` : ''}
                    </div>
                    ${subtitle ? `<div id="${previewId}" class="text-xs text-gray-500 mt-2 cursor-pointer hover:text-gray-300 transition-colors" style="display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;white-space:pre-wrap;word-break:break-word;" onclick="app.showMonitorPreview('${previewId}','${esc(session.session_id)}')">${esc(subtitle)}</div>` : ''}
                </div>
            </div>
        </div>`;
    }

    showMonitorPreview(previewId, sessionId) {
        const el = document.getElementById(previewId);
        if (!el) return;
        // Toggle expanded state
        if (el.dataset.expanded === 'true') {
            el.style.display = '-webkit-box';
            el.style.webkitLineClamp = '2';
            el.dataset.expanded = 'false';
            return;
        }
        el.style.display = 'block';
        el.style.webkitLineClamp = '';
        el.dataset.expanded = 'true';
    }

    // --- Switch (Codex account switcher) ---

    async selectSwitchAgent(agentType) {
        this.switchSelectedAgent = agentType;
        this.switchAddFormOpen = false;
        this.switchClearConfirmOpen = false;
        this.switchEditingId = null;
        this.switchEditingContentId = null;
        this.switchContentCache = {};
        this.switchProfiles = [];
        this.render();
        await this.loadSwitchProfiles();
    }

    async loadSwitchProfiles() {
        if (!this.switchSelectedAgent) return;
        try {
            const resp = await Api.listSwitchProfiles(this.switchSelectedAgent);
            this.switchProfiles = resp.profiles || [];
            this.switchCurrentKey = resp.current_key || null;
        } catch (e) {
            this.switchProfiles = [];
            this.switchCurrentKey = null;
        }
        this.render();
    }

    renderSwitchView() {
        const el = document.getElementById('view-switch');
        const i = this.i18n;
        if (!el) return;

        if (!this.switchSelectedAgent) {
            el.innerHTML = this.renderEmptyState(i.t('switch.select_agent'), 'search');
            return;
        }

        let html = `<div class="max-w-2xl mx-auto">`;

        // Toolbar
        const currentKeyHtml = this.switchCurrentKey
            ? `<span class="text-xs text-gray-500 font-mono truncate max-w-[200px] inline-block align-middle" title="${esc(this.switchCurrentKey)}">${esc(i.t('switch.current_key'))}: ${esc(this.switchCurrentKey)}</span>`
            : '';
        html += `<div class="flex gap-2 mb-4 flex-wrap items-center">
            <button id="sw-btn-save" class="px-3 py-1.5 text-sm rounded bg-cyan-900/40 text-cyan-300 hover:bg-cyan-900/60 cursor-pointer border border-cyan-800/50">
                ${esc(i.t('switch.save_current'))}
            </button>
            <button id="sw-btn-add" class="px-3 py-1.5 text-sm rounded bg-gray-700 text-gray-300 hover:bg-gray-600 cursor-pointer border border-gray-600">
                ${esc(i.t('switch.add_account'))}
            </button>
            <div class="flex-1"></div>
            ${currentKeyHtml}
            <button id="sw-btn-clear" class="px-3 py-1.5 text-sm rounded bg-red-900/30 text-red-400 hover:bg-red-900/50 cursor-pointer border border-red-800/40">
                ${esc(i.t('switch.clear_active'))}
            </button>
        </div>`;

        // Add-form (inline)
        if (this.switchAddFormOpen) {
            html += `<div class="mb-4 p-3 rounded-lg border border-gray-600 bg-gray-800/50">
                <div class="mb-2 text-sm text-gray-300 font-medium">${esc(i.t('switch.add_account'))}</div>
                <textarea id="sw-add-content" rows="6" class="w-full text-xs font-mono rounded p-2 bg-gray-900 border border-gray-600 text-gray-300 outline-none resize-none mb-2" placeholder="{ &quot;auth_mode&quot;: ... }"></textarea>
                <input id="sw-add-note" type="text" class="w-full text-sm rounded px-2 py-1 bg-gray-900 border border-gray-600 text-gray-300 outline-none mb-2" placeholder="${esc(i.t('switch.note_placeholder'))}" />
                <div id="sw-add-error" class="hidden text-xs text-red-400 mb-2"></div>
                <div class="flex gap-2">
                    <button id="sw-add-confirm" class="px-3 py-1.5 text-sm rounded bg-green-900/40 text-green-300 hover:bg-green-900/60 cursor-pointer border border-green-800/50">${esc(i.t('switch.confirm_add'))}</button>
                    <button id="sw-add-cancel" class="px-3 py-1.5 text-sm rounded bg-gray-700 text-gray-400 hover:bg-gray-600 cursor-pointer border border-gray-600">${esc(i.t('action.cancel'))}</button>
                </div>
            </div>`;
        }

        // Clear-confirm panel (two-step)
        if (this.switchClearConfirmOpen) {
            html += `<div class="mb-4 p-3 rounded-lg border border-red-800/60 bg-red-900/20">
                <div class="text-sm text-red-300 font-medium mb-1">${esc(i.t('switch.clear_warning_title'))}</div>
                <div class="text-xs text-red-400 mb-3">${esc(i.t('switch.clear_warning_body'))}</div>
                <div class="flex gap-2">
                    <button id="sw-clear-confirm" class="px-3 py-1.5 text-sm rounded bg-red-700 text-white hover:bg-red-600 cursor-pointer">${esc(i.t('switch.confirm_clear'))}</button>
                    <button id="sw-clear-cancel" class="px-3 py-1.5 text-sm rounded bg-gray-700 text-gray-400 hover:bg-gray-600 cursor-pointer border border-gray-600">${esc(i.t('action.cancel'))}</button>
                </div>
            </div>`;
        }

        // Profile list
        if (this.switchProfiles.length === 0) {
            html += `<div class="text-center text-gray-500 text-sm py-12">${esc(i.t('switch.empty'))}</div>`;
        } else {
            html += `<div class="space-y-2">`;
            this.switchProfiles.forEach((profile, idx) => {
                const label = profile.note || i.tWith('switch.account_fallback', { n: idx + 1 });
                const date = profile.saved_at ? profile.saved_at.replace('T', ' ').replace('Z', ' UTC').substring(0, 19) + ' UTC' : '';
                const isEditing = this.switchEditingId === profile.id;
                const isDeleting = this.switchDeleteConfirmId === profile.id;
                const isSwitching = this.switchAccountConfirmId === profile.id;

                html += `<div class="p-3 rounded-lg border ${profile.is_active ? 'border-cyan-700/60 bg-cyan-900/10' : 'border-gray-700 bg-gray-800/40'}">
                    <div class="flex items-start justify-between gap-2">
                        <div class="flex-1 min-w-0">
                            <div class="flex items-center gap-2 mb-0.5">
                                <span class="text-sm font-medium text-gray-200">${esc(label)}</span>
                                ${profile.is_active ? `<span class="text-xs px-1.5 py-0.5 rounded bg-cyan-900/50 text-cyan-400 border border-cyan-700/50">${esc(i.t('switch.active_badge'))}</span>` : ''}
                            </div>
                            <div class="text-xs text-gray-500">${esc(date)}${profile.key ? ` · <span class="font-mono">${esc(profile.key)}</span>` : ''}</div>
                        </div>
                        <div class="flex gap-1.5 flex-shrink-0 items-center">`;

                if (isDeleting) {
                    html += `<span class="text-xs text-red-400">${esc(i.t('switch.confirm_delete'))}</span>
                             <button class="sw-btn-delete-confirm text-xs px-2 py-1 rounded bg-red-900/30 text-red-400 hover:bg-red-900/50 cursor-pointer border border-red-800/40" data-id="${esc(profile.id)}">${esc(i.t('action.confirm'))}</button>
                             <button class="sw-btn-delete-cancel text-xs px-2 py-1 rounded bg-gray-700 text-gray-400 hover:bg-gray-600 cursor-pointer border border-gray-600" data-id="${esc(profile.id)}">${esc(i.t('action.cancel'))}</button>`;
                } else if (isSwitching) {
                    html += `<span class="text-xs text-cyan-400">${esc(i.t('switch.confirm_switch'))}</span>
                             <button class="sw-btn-switch-confirm text-xs px-2 py-1 rounded bg-gray-700 text-gray-300 hover:bg-gray-600 cursor-pointer border border-gray-600" data-id="${esc(profile.id)}">${esc(i.t('action.confirm'))}</button>
                             <button class="sw-btn-switch-cancel text-xs px-2 py-1 rounded bg-gray-700 text-gray-400 hover:bg-gray-600 cursor-pointer border border-gray-600" data-id="${esc(profile.id)}">${esc(i.t('action.cancel'))}</button>`;
                } else {
                    html += `${!profile.is_active ? `<button class="sw-btn-switch text-xs px-2 py-1 rounded bg-gray-700 text-gray-300 hover:bg-gray-600 cursor-pointer border border-gray-600" data-id="${esc(profile.id)}">${esc(i.t('switch.switch_btn'))}</button>` : ''}
                            <button class="sw-btn-edit-content text-xs px-2 py-1 rounded bg-gray-700 text-gray-400 hover:bg-gray-600 cursor-pointer border border-gray-600" data-id="${esc(profile.id)}">${esc(i.t('switch.edit_content_btn'))}</button>
                            <button class="sw-btn-note text-xs px-2 py-1 rounded bg-gray-700 text-gray-400 hover:bg-gray-600 cursor-pointer border border-gray-600" data-id="${esc(profile.id)}">${esc(i.t('switch.note_btn'))}</button>
                            <button class="sw-btn-delete text-xs px-2 py-1 rounded bg-red-900/30 text-red-400 hover:bg-red-900/50 cursor-pointer border border-red-800/40" data-id="${esc(profile.id)}">${esc(i.t('switch.delete_btn'))}</button>`;
                }

                html += `</div>
                    </div>`;

                if (isEditing) {
                    html += `<div class="mt-2 flex gap-2 items-center sw-edit-panel">
                        <input id="sw-note-input-${esc(profile.id)}" type="text" value="${esc(profile.note)}" class="flex-1 text-sm rounded px-2 py-1 bg-gray-900 border border-gray-600 text-gray-300 outline-none" placeholder="${esc(i.t('switch.note_placeholder'))}" />
                        <button class="sw-note-save text-xs px-2 py-1 rounded bg-green-900/40 text-green-300 hover:bg-green-900/60 cursor-pointer border border-green-800/50" data-id="${esc(profile.id)}">${esc(i.t('switch.save_note'))}</button>
                        <button class="sw-note-cancel text-xs px-2 py-1 rounded bg-gray-700 text-gray-400 hover:bg-gray-600 cursor-pointer border border-gray-600" data-id="${esc(profile.id)}">${esc(i.t('action.cancel'))}</button>
                    </div>`;
                }

                const isEditingContent = this.switchEditingContentId === profile.id;
                if (isEditingContent) {
                    const cached = this.switchContentCache[profile.id] || '';
                    html += `<div class="mt-2 sw-edit-panel">
                        <textarea id="sw-content-input-${esc(profile.id)}" rows="10" class="w-full text-xs font-mono rounded p-2 bg-gray-900 border border-gray-600 text-gray-300 outline-none resize-y">${esc(cached)}</textarea>
                        <div id="sw-content-error-${esc(profile.id)}" class="hidden text-xs text-red-400 mt-1"></div>
                        <div class="flex gap-2 mt-2">
                            <button class="sw-content-save text-xs px-2 py-1 rounded bg-green-900/40 text-green-300 hover:bg-green-900/60 cursor-pointer border border-green-800/50" data-id="${esc(profile.id)}">${esc(i.t('switch.save_content'))}</button>
                            <button class="sw-content-cancel text-xs px-2 py-1 rounded bg-gray-700 text-gray-400 hover:bg-gray-600 cursor-pointer border border-gray-600" data-id="${esc(profile.id)}">${esc(i.t('action.cancel'))}</button>
                        </div>
                    </div>`;
                }

                html += `</div>`;
            });
            html += `</div>`;
        }

        html += `</div>`;
        el.innerHTML = html;

        // Bind toolbar buttons
        document.getElementById('sw-btn-save')?.addEventListener('click', () => this.handleSwitchSaveCurrent());
        document.getElementById('sw-btn-add')?.addEventListener('click', () => {
            this.switchAddFormOpen = !this.switchAddFormOpen;
            this.render();
        });
        document.getElementById('sw-btn-clear')?.addEventListener('click', () => {
            this.switchClearConfirmOpen = true;
            this.render();
        });

        // Add form
        document.getElementById('sw-add-confirm')?.addEventListener('click', () => this.handleSwitchAdd());
        document.getElementById('sw-add-cancel')?.addEventListener('click', () => {
            this.switchAddFormOpen = false;
            this.render();
        });

        // Clear confirm
        document.getElementById('sw-clear-confirm')?.addEventListener('click', () => this.handleSwitchClear());
        document.getElementById('sw-clear-cancel')?.addEventListener('click', () => {
            this.switchClearConfirmOpen = false;
            this.render();
        });

        // Row action buttons
        el.querySelectorAll('.sw-btn-switch').forEach(btn => {
            btn.addEventListener('click', () => this.handleSwitchAccount(btn.dataset.id));
        });
        el.querySelectorAll('.sw-btn-switch-confirm').forEach(btn => {
            btn.addEventListener('click', () => this.handleSwitchAccount(btn.dataset.id));
        });
        el.querySelectorAll('.sw-btn-switch-cancel').forEach(btn => {
            btn.addEventListener('click', () => {
                this.switchAccountConfirmId = null;
                this.render();
            });
        });
        el.querySelectorAll('.sw-btn-edit-content').forEach(btn => {
            btn.addEventListener('click', () => this.handleOpenContentEditor(btn.dataset.id));
        });
        el.querySelectorAll('.sw-content-save').forEach(btn => {
            btn.addEventListener('click', () => this.handleSwitchSaveContent(btn.dataset.id));
        });
        el.querySelectorAll('.sw-content-cancel').forEach(btn => {
            btn.addEventListener('click', () => {
                this.switchEditingContentId = null;
                this.render();
            });
        });
        el.querySelectorAll('.sw-btn-note').forEach(btn => {
            btn.addEventListener('click', () => {
                this.switchEditingId = this.switchEditingId === btn.dataset.id ? null : btn.dataset.id;
                this.render();
            });
        });
        el.querySelectorAll('.sw-btn-delete').forEach(btn => {
            btn.addEventListener('click', () => this.handleSwitchDelete(btn.dataset.id));
        });
        el.querySelectorAll('.sw-btn-delete-confirm').forEach(btn => {
            btn.addEventListener('click', () => this.handleSwitchDelete(btn.dataset.id));
        });
        el.querySelectorAll('.sw-btn-delete-cancel').forEach(btn => {
            btn.addEventListener('click', () => {
                this.switchDeleteConfirmId = null;
                this.render();
            });
        });
        el.querySelectorAll('.sw-note-save').forEach(btn => {
            btn.addEventListener('click', () => this.handleSwitchSaveNote(btn.dataset.id));
        });
        el.querySelectorAll('.sw-note-cancel').forEach(btn => {
            btn.addEventListener('click', () => {
                this.switchEditingId = null;
                this.render();
            });
        });

        // Click-outside to dismiss edit panels
        if (this._switchOutsideClickHandler) {
            document.removeEventListener('mousedown', this._switchOutsideClickHandler);
        }
        if (this.switchEditingId || this.switchEditingContentId) {
            const self = this;
            this._switchOutsideClickHandler = (e) => {
                if (e.target.closest('.sw-edit-panel')) return;
                if (e.target.closest('button')) return;
                self.switchEditingId = null;
                self.switchEditingContentId = null;
                document.removeEventListener('mousedown', self._switchOutsideClickHandler);
                self._switchOutsideClickHandler = null;
                self.render();
            };
            setTimeout(() => {
                document.addEventListener('mousedown', this._switchOutsideClickHandler);
            }, 10);
        }
    }

    async handleSwitchSaveCurrent() {
        const i = this.i18n;
        const at = this.switchSelectedAgent;
        try {
            await Api.saveCurrentAuthProfile(at, '');
            this.showToast(i.t('switch.saved_toast'), 'success');
            await this.loadSwitchProfiles();
        } catch (e) {
            if (e === 'no_active_auth' || e?.message === 'no_active_auth') {
                this.showToast(i.t('switch.no_active_auth_error'), 'error');
            } else if (e === 'duplicate_key' || e?.message === 'duplicate_key') {
                this.showToast(i.t('switch.duplicate_key_error'), 'error');
            } else {
                this.showToast(String(e?.message || e), 'error');
            }
        }
    }

    async handleSwitchAdd() {
        const i = this.i18n;
        const at = this.switchSelectedAgent;
        const content = document.getElementById('sw-add-content')?.value?.trim() || '';
        const note = document.getElementById('sw-add-note')?.value?.trim() || '';
        const errEl = document.getElementById('sw-add-error');
        try {
            await Api.addAuthProfile(at, content, note);
            this.switchAddFormOpen = false;
            this.showToast(i.t('switch.added_toast'), 'success');
            await this.loadSwitchProfiles();
        } catch (e) {
            if (e === 'invalid_json' || e?.message === 'invalid_json') {
                if (errEl) { errEl.textContent = i.t('switch.invalid_json_error'); errEl.classList.remove('hidden'); }
            } else if (e === 'duplicate_key' || e?.message === 'duplicate_key') {
                if (errEl) { errEl.textContent = i.t('switch.duplicate_key_error'); errEl.classList.remove('hidden'); }
            } else {
                this.showToast(String(e?.message || e), 'error');
            }
        }
    }

    async handleSwitchAccount(id) {
        const i = this.i18n;
        const at = this.switchSelectedAgent;
        if (this.switchAccountConfirmId !== id) {
            this.switchAccountConfirmId = id;
            this.switchDeleteConfirmId = null;
            this.render();
            return;
        }
        try {
            await Api.switchAuthProfile(at, id);
            this.switchAccountConfirmId = null;
            this.showToast(i.t('switch.switched_toast'), 'success');
            await this.loadSwitchProfiles();
        } catch (e) {
            this.showToast(String(e?.message || e), 'error');
        }
    }

    async handleSwitchSaveNote(id) {
        const at = this.switchSelectedAgent;
        const note = document.getElementById(`sw-note-input-${id}`)?.value?.trim() || '';
        try {
            await Api.updateAuthProfileNote(at, id, note);
            this.switchEditingId = null;
            await this.loadSwitchProfiles();
        } catch (e) {
            this.showToast(String(e?.message || e), 'error');
        }
    }

    async handleSwitchDelete(id) {
        const i = this.i18n;
        const at = this.switchSelectedAgent;
        if (this.switchDeleteConfirmId !== id) {
            this.switchDeleteConfirmId = id;
            this.switchAccountConfirmId = null;
            this.render();
            return;
        }
        try {
            await Api.deleteAuthProfile(at, id);
            this.switchDeleteConfirmId = null;
            await this.loadSwitchProfiles();
        } catch (e) {
            this.showToast(String(e?.message || e), 'error');
        }
    }

    async handleSwitchClear() {
        const i = this.i18n;
        const at = this.switchSelectedAgent;
        try {
            await Api.clearActiveAuth(at);
            this.switchClearConfirmOpen = false;
            this.showToast(i.t('switch.cleared_toast'), 'success');
            await this.loadSwitchProfiles();
        } catch (e) {
            this.switchClearConfirmOpen = false;
            if (e === 'no_active_auth' || e?.message === 'no_active_auth') {
                this.showToast(i.t('switch.no_active_auth_error'), 'error');
            } else {
                this.showToast(String(e?.message || e), 'error');
            }
            this.render();
        }
    }

    async handleOpenContentEditor(id) {
        const at = this.switchSelectedAgent;
        this.switchEditingContentId = id;
        this.switchEditingId = null;
        if (!this.switchContentCache[id]) {
            try {
                this.switchContentCache[id] = await Api.getAuthProfileContent(at, id);
            } catch (e) {
                this.showToast(String(e?.message || e), 'error');
                this.switchEditingContentId = null;
            }
        }
        this.render();
    }

    async handleSwitchSaveContent(id) {
        const i = this.i18n;
        const at = this.switchSelectedAgent;
        const textarea = document.getElementById(`sw-content-input-${id}`);
        const errEl = document.getElementById(`sw-content-error-${id}`);
        const content = textarea?.value || '';
        try {
            await Api.updateAuthProfileContent(at, id, content);
            this.switchContentCache[id] = content;
            this.switchEditingContentId = null;
            this.showToast(i.t('switch.content_saved_toast'), 'success');
            await this.loadSwitchProfiles();
        } catch (e) {
            if (e === 'invalid_json' || e?.message === 'invalid_json') {
                if (errEl) { errEl.textContent = i.t('switch.invalid_json_error'); errEl.classList.remove('hidden'); }
            } else {
                this.showToast(String(e?.message || e), 'error');
            }
        }
    }
}

function esc(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

function truncate(str, len) {
    return str && str.length > len ? str.substring(0, len) + '...' : str;
}

// Convert unified diff lines to side-by-side pairs with line numbers
// Output: [{ left: {text, type, num}, right: {text, type, num}, isChanged }]
function toSideBySide(lines) {
    const pairs = [];
    const normalized = lines.map(l => {
        if (l.tag !== undefined) return l;
        if (l.Context !== undefined) return { tag: 'context', content: l.Context };
        if (l.Added !== undefined) return { tag: 'added', content: l.Added };
        if (l.Removed !== undefined) return { tag: 'removed', content: l.Removed };
        if (l.FileHeader !== undefined) return { tag: 'context', content: l.FileHeader };
        return { tag: 'context', content: '' };
    });

    let leftNum = 1, rightNum = 1;
    let i = 0;
    while (i < normalized.length) {
        const line = normalized[i];
        if (line.tag === 'context') {
            pairs.push({
                left: { text: line.content, type: 'context', num: leftNum++ },
                right: { text: line.content, type: 'context', num: rightNum++ },
                isChanged: false
            });
            i++;
        } else if (line.tag === 'removed') {
            const removed = [];
            while (i < normalized.length && normalized[i].tag === 'removed') {
                removed.push(normalized[i].content);
                i++;
            }
            const added = [];
            while (i < normalized.length && normalized[i].tag === 'added') {
                added.push(normalized[i].content);
                i++;
            }
            const max = Math.max(removed.length, added.length);
            for (let j = 0; j < max; j++) {
                const l = j < removed.length ? { text: removed[j], type: 'removed', num: leftNum++ } : { text: '', type: 'empty', num: null };
                const r = j < added.length ? { text: added[j], type: 'added', num: rightNum++ } : { text: '', type: 'empty', num: null };
                pairs.push({ left: l, right: r, isChanged: true });
            }
        } else if (line.tag === 'added') {
            pairs.push({
                left: { text: '', type: 'empty', num: null },
                right: { text: line.content, type: 'added', num: rightNum++ },
                isChanged: true
            });
            i++;
        } else {
            i++;
        }
    }
    return pairs;
}

function renderSideBySide(pairs) {
    const CTX = 3; // context lines before/after changes

    // Find which indices to show
    const show = new Set();
    for (let i = 0; i < pairs.length; i++) {
        if (pairs[i].isChanged) {
            for (let j = Math.max(0, i - CTX); j <= Math.min(pairs.length - 1, i + CTX); j++) {
                show.add(j);
            }
        }
    }
    // If no changes, show all
    if (show.size === 0) {
        for (let i = 0; i < pairs.length; i++) show.add(i);
    }

    const lineNumStyle = 'color:#475569;min-width:2.5rem;text-align:right;padding-right:0.5rem;user-select:none;flex-shrink:0';
    let html = `<div style="display:grid;grid-template-columns:1fr 1fr;font-size:0.8125rem;line-height:1.6;font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;background:#0a0a0f;border-radius:0.5rem;overflow-x:auto;border:1px solid #1e293b">`;
    html += `<div style="background:#1e293b;color:#94a3b8;font-size:0.7rem;font-weight:600;text-transform:uppercase;letter-spacing:0.05em;text-align:center;padding:0.4rem 0;border-right:1px solid #334155">Before</div>`;
    html += `<div style="background:#1e293b;color:#94a3b8;font-size:0.7rem;font-weight:600;text-transform:uppercase;letter-spacing:0.05em;text-align:center;padding:0.4rem 0">After</div>`;

    let lastShown = -1;
    for (let i = 0; i < pairs.length; i++) {
        if (!show.has(i)) {
            if (lastShown === i - 1 && i + 1 < pairs.length && show.has(i + 1)) {
                // Show ellipsis
                html += `<div style="color:#475569;text-align:center;padding:0.2rem 0;border-right:1px solid #1e293b;font-size:0.75rem">···</div>`;
                html += `<div style="color:#475569;text-align:center;padding:0.2rem 0;font-size:0.75rem">···</div>`;
            }
            continue;
        }
        lastShown = i;
        const p = pairs[i];
        const lBg = p.left.type === 'removed' ? 'background:rgba(252,165,165,0.12);color:#fca5a5' : p.left.type === 'empty' ? 'background:rgba(26,26,46,0.6)' : 'color:#94a3b8';
        const rBg = p.right.type === 'added' ? 'background:rgba(110,231,183,0.12);color:#6ee7b7' : p.right.type === 'empty' ? 'background:rgba(26,26,46,0.6)' : 'color:#94a3b8';
        const leftPre = p.left.type === 'removed' ? '<span style="color:#fca5a5;font-weight:600">-</span> ' : p.left.type === 'empty' ? ' ' : ' ';
        const rightPre = p.right.type === 'added' ? '<span style="color:#6ee7b7;font-weight:600">+</span> ' : p.right.type === 'empty' ? ' ' : ' ';
        const leftNum = p.left.num != null ? `<span style="${lineNumStyle}">${p.left.num}</span>` : `<span style="${lineNumStyle}"></span>`;
        const rightNum = p.right.num != null ? `<span style="${lineNumStyle}">${p.right.num}</span>` : `<span style="${lineNumStyle}"></span>`;
        html += `<div style="${lBg};padding:0 0.6rem;white-space:pre;overflow:hidden;border-right:1px solid #1e293b;min-height:1.6em;display:flex;align-items:baseline">${leftNum}${leftPre}${esc(p.left.text)}</div>`;
        html += `<div style="${rBg};padding:0 0.6rem;white-space:pre;overflow:hidden;min-height:1.6em;display:flex;align-items:baseline">${rightNum}${rightPre}${esc(p.right.text)}</div>`;
    }
    html += `</div>`;
    return html;
}

const app = new App();
app.init();
