import { I18n } from './i18n.js';
import * as Api from './api.js';

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
};

// Updater / Process plugins via global Tauri API (no bundler)
const tauriInvoke = window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
async function checkUpdate() {
    try {
        return await tauriInvoke('plugin:updater|check');
    } catch { return null; }
}
function relaunch() {
    return tauriInvoke('plugin:process|restart');
}

class App {
    constructor() {
        this.platforms = [];
        this.skills = [];
        this.selectedPlatformId = null;
        this.selectedSkillName = null;
        this.selectedFolder = '';
        this.currentView = 'skills';
        this.currentTab = 'skills'; // 'skills' | 'mcp'
        this.diffResult = null;
        this.searchResults = [];
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
        // Trash state
        this.trashCount = 0;
        // Update state
        this.updateInfo = null; // { version, body, date } when update available
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

    async refreshPlatforms() {
        this.platforms = await Api.listPlatforms();
        if (this.platforms.length > 0 && !this.selectedPlatformId) {
            this.selectedPlatformId = this.platforms[0].id;
        }
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
            await this.refreshPlatforms();
            this.currentView = 'skills';
            this.selectedSkillName = null;
            this.selectedFolder = '';
            this.render();
        } catch (e) {
            alert(this.i18n.tWith('sync.failed', { error: e.SyncError || e }));
        }
    }

    async doFolderSync(targetPlatformId, folder) {
        try {
            const result = await Api.syncFolder(this.selectedPlatformId, targetPlatformId, folder);
            this.closeModal();
            await this.refreshPlatforms();
            this.currentView = 'skills';
            this.render();
            const i = this.i18n;
            alert(i.tWith('sync.done') + ` (${result.synced}/${result.total})`);
        } catch (e) {
            alert(this.i18n.tWith('sync.failed', { error: e.SyncError || e }));
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
                await this.refreshPlatforms();
                await this.refreshTrashCount();
                this.render();
            } catch (e) {
                alert(i.tWith('skill.delete_failed', { error: e.SyncError || e }));
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
            this.render();
            return;
        }
        this.searchResults = await Api.searchSkills(query);
        this.currentView = 'search';
        this.render();
    }

    // --- Update ---
    async checkForUpdate() {
        try {
            const update = await checkUpdate();
            if (update) {
                this.updateInfo = { version: update.version, body: update.body, date: update.date, currentVersion: update.currentVersion };
                this.renderUpdateBadge();
            }
        } catch {}
    }

    renderVersion() {
        const el = document.getElementById('version-label');
        if (el) el.textContent = `v${this.appVersion}`;
    }

    renderUpdateBadge() {
        if (!this.updateInfo) return;
        const el = document.getElementById('update-badge');
        const i = this.i18n;
        el.className = 'p-2 border-t border-gray-700 cursor-pointer hover:bg-gray-700/50';
        el.innerHTML = `<div class="flex items-center gap-1.5 px-1">
            <span class="text-green-400 flex-shrink-0">${Icons.dot}</span>
            <span class="text-xs text-gray-400">${i.t('update.badge')}</span>
            <span class="text-xs text-gray-500">v${esc(this.updateInfo.version)}</span>
        </div>`;
        el.onclick = () => this.showUpdateModal();
    }

    showUpdateModal() {
        const i = this.i18n;
        const info = this.updateInfo;
        const transition = i.tWith('update.transition', { current: info.currentVersion, latest: info.version });
        const bodyHtml = info.body ? `<div class="text-sm text-gray-400 mb-4 max-h-32 overflow-y-auto whitespace-pre-wrap">${esc(info.body)}</div>` : '';
        const html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-3">${i.t('update.title')}</h2>
            <p class="text-sm text-green-400 font-mono mb-3">${transition}</p>
            ${bodyHtml}
            <div id="update-status" class="text-sm text-gray-500 mb-3"></div>
            <div class="flex gap-3 justify-end">
                <button class="px-4 py-2 bg-green-700 hover:bg-green-600 rounded text-sm cursor-pointer update-confirm-btn">${i.t('update.confirm')}</button>
                <button class="px-4 py-2 text-gray-400 hover:text-white text-sm cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
            </div>
        </div>`;
        this.openModal(html);
        this.modalEl().querySelector('.update-confirm-btn').addEventListener('click', () => this.doUpdate());
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    async doUpdate() {
        const i = this.i18n;
        const statusEl = document.getElementById('update-status');
        const confirmBtn = this.modalEl().querySelector('.update-confirm-btn');
        const cancelBtn = this.modalEl().querySelector('.modal-cancel');
        try {
            confirmBtn.disabled = true;
            confirmBtn.classList.add('opacity-50');
            cancelBtn.classList.add('hidden');
            statusEl.textContent = i.t('update.downloading');
            await tauriInvoke('plugin:updater|download');
            statusEl.textContent = i.t('update.installing');
            await tauriInvoke('plugin:updater|install');
            await relaunch();
        } catch (e) {
            statusEl.textContent = i.tWith('update.error', { error: e.message || e });
            confirmBtn.disabled = false;
            confirmBtn.classList.remove('opacity-50');
            cancelBtn.classList.remove('hidden');
        }
    }

    async switchLang() {
        await this.i18n.switchLocale();
        await Api.setLocale(this.i18n.locale);
        this.render();
    }

    // --- Events ---
    bindEvents() {
        document.getElementById('btn-refresh').addEventListener('click', () => {
            if (this.currentTab === 'mcp') { this.refreshMcpPlatforms().then(() => this.render()); }
            else { this.refreshPlatforms().then(() => this.render()); }
        });
        document.getElementById('btn-lang').addEventListener('click', () => this.switchLang());
        document.getElementById('btn-back').addEventListener('click', () => this.backToList());
        document.getElementById('btn-diff').addEventListener('click', () => this.showDiffModal());
        document.getElementById('btn-sync').addEventListener('click', () => this.showSyncModal());

        let debounce;
        document.getElementById('search-input').addEventListener('input', (e) => {
            clearTimeout(debounce);
            debounce = setTimeout(() => this.doSearch(e.target.value), 300);
        });

        document.getElementById('modal-overlay').addEventListener('click', (e) => {
            if (e.target.id === 'modal-overlay') this.closeModal();
        });

        // Tab switching
        document.getElementById('tab-skills').addEventListener('click', () => this.switchTab('skills'));
        document.getElementById('tab-mcp').addEventListener('click', () => this.switchTab('mcp'));
    }

    switchTab(tab) {
        this.currentTab = tab;
        this.render();
    }

    // --- MCP ---
    async refreshMcpPlatforms() {
        this.mcpPlatforms = await Api.listMcpPlatforms();
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
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-3">${i.t('mcp.add')}</h2>
            <div class="mb-2">
                <label class="text-xs text-gray-400">${i.t('mcp.server_name')}</label>
                <input id="mcp-add-name" class="w-full bg-gray-900 text-sm text-gray-200 rounded px-3 py-1.5 border border-gray-600 focus:border-cyan-500 outline-none" />
            </div>
            <div class="mb-2">
                <label class="text-xs text-gray-400">${i.t('mcp.config')} (${i.t('mcp.format_json')} / ${i.t('mcp.format_toml')})</label>
                <textarea id="mcp-add-area" class="w-full h-48 bg-gray-900 text-sm text-gray-200 font-mono rounded p-3 border border-gray-600 focus:border-cyan-500 outline-none resize-y" placeholder="${i.t('mcp.import_hint')}"></textarea>
            </div>
            <div class="flex gap-3 justify-end mt-3">
                <button class="px-4 py-2 bg-cyan-700 hover:bg-cyan-600 rounded text-sm cursor-pointer mcp-add-save">${i.t('mcp.save')}</button>
                <button class="px-4 py-2 text-gray-400 hover:text-white text-sm cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
            </div></div>`;
        this.openModal(html);
        this.modalEl().querySelector('.mcp-add-save').addEventListener('click', async () => {
            const name = document.getElementById('mcp-add-name').value.trim();
            const text = document.getElementById('mcp-add-area').value.trim();
            if (!name) { alert('Server name required'); return; }
            try {
                await Api.importMcpServer(this.selectedMcpPlatform, name, text);
                this.closeModal();
                await this.selectMcpPlatform(this.selectedMcpPlatform);
            } catch (e) {
                alert(i.tWith('mcp.parse_error', { error: e.SyncError || e }));
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
                alert(i.tWith('mcp.delete_failed', { error: e.SyncError || e }));
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
            alert(i.t('trash.empty'));
            return;
        }
        const now = Math.floor(Date.now() / 1000);
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-4">${i.t('trash.title')} (${items.length})</h2>
            <div class="space-y-1 mb-4 max-h-[60vh] overflow-y-auto">`;
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
                <button class="text-xs text-green-600 hover:text-green-400 px-2 py-1 cursor-pointer hidden group-hover:inline trash-restore-btn">${i.t('trash.restore')}</button>
                <button class="text-xs text-red-600 hover:text-red-400 px-2 py-1 cursor-pointer hidden group-hover:inline trash-delete-btn">${i.t('trash.delete_forever')}</button>
            </div>`;
        }
        html += `</div>
            <div class="flex gap-3 justify-between">
                <button class="px-4 py-2 bg-red-800 hover:bg-red-700 rounded text-sm cursor-pointer trash-empty-btn">${i.t('trash.empty_trash')}</button>
                <button class="px-4 py-2 text-gray-400 hover:text-white cursor-pointer modal-cancel">${i.t('action.cancel')}</button>
            </div></div>`;
        this.openModal(html);

        // Restore buttons
        this.modalEl().querySelectorAll('.trash-restore-btn').forEach(btn => {
            btn.addEventListener('click', async (e) => {
                e.stopPropagation();
                const row = btn.closest('[data-trash-id]');
                await this.restoreTrashItem(row.dataset.trashId);
            });
        });

        // Permanent delete buttons
        this.modalEl().querySelectorAll('.trash-delete-btn').forEach(btn => {
            btn.addEventListener('click', async (e) => {
                e.stopPropagation();
                if (!confirm(i.t('trash.confirm_delete_forever'))) return;
                const row = btn.closest('[data-trash-id]');
                await this.permanentlyDeleteTrashItem(row.dataset.trashId);
            });
        });

        // Empty trash button
        this.modalEl().querySelector('.trash-empty-btn').addEventListener('click', async () => {
            if (!confirm(i.t('trash.confirm_empty'))) return;
            await this.emptyTrash();
        });

        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    async restoreTrashItem(id) {
        const i = this.i18n;
        try {
            await Api.restoreTrashItem(id);
            this.closeModal();
            await this.refreshPlatforms();
            await this.refreshMcpPlatforms();
            await this.refreshTrashCount();
            // If MCP tab and was viewing a platform, re-select it
            if (this.currentTab === 'mcp' && this.selectedMcpPlatform) {
                await this.selectMcpPlatform(this.selectedMcpPlatform);
            }
            this.render();
        } catch (e) {
            alert(i.tWith('trash.restore_failed', { error: e.SyncError || e }));
        }
    }

    async permanentlyDeleteTrashItem(id) {
        try {
            await Api.permanentlyDeleteTrashItem(id);
            this.closeModal();
            await this.refreshTrashCount();
            if (this.trashCount > 0) this.showTrashModal();
        } catch (e) {
            alert('Error: ' + (e.SyncError || e));
        }
    }

    async emptyTrash() {
        try {
            await Api.emptyTrash();
            this.closeModal();
            await this.refreshTrashCount();
        } catch (e) {
            alert('Error: ' + (e.SyncError || e));
        }
    }

    async showMcpSyncModal(serverName) {
        const targets = await Api.getMcpSyncTargets(this.selectedMcpPlatform, serverName);
        if (targets.length === 0) {
            alert(this.i18n.t('error.no_target'));
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
                alert(i.tWith('mcp.sync_failed', { error: e.SyncError || e }));
            }
        });
        this.modalEl().querySelector('.modal-cancel').addEventListener('click', () => this.closeModal());
    }

    // --- Modals ---
    async showDiffModal() {
        const candidates = await Api.getDiffCandidates(this.selectedPlatformId, this.selectedSkillName, this.selectedFolder);
        if (candidates.length === 0) {
            alert(this.i18n.t('diff.no_other'));
            return;
        }
        const i = this.i18n;
        let html = `<div class="p-5">
            <h2 class="text-lg font-bold text-yellow-400 mb-4">${i.t('diff.select_platform')}</h2>
            <div class="space-y-1">`;
        for (const c of candidates) {
            html += `<button class="w-full text-left px-3 py-2 rounded hover:bg-gray-700 text-gray-200 cursor-pointer diff-target" data-id="${c.id}">
                ${c.display_name} <span class="text-gray-500">(${c.skill_count} skills)</span></button>`;
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
            alert(this.i18n.t('error.no_target'));
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
            alert(this.i18n.t('error.no_target'));
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
    }

    renderTabBar() {
        const skillsTab = document.getElementById('tab-skills');
        const mcpTab = document.getElementById('tab-mcp');
        skillsTab.className = `flex-1 py-2 text-sm text-center cursor-pointer border-b-2 ${this.currentTab === 'skills' ? 'text-gray-300 border-cyan-500' : 'text-gray-500 hover:text-white border-transparent'}`;
        mcpTab.className = `flex-1 py-2 text-sm text-center cursor-pointer border-b-2 ${this.currentTab === 'mcp' ? 'text-gray-300 border-cyan-500' : 'text-gray-500 hover:text-white border-transparent'}`;
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
                    <span class="text-xs text-gray-500">${i.tWith('platform.skills_count', { count: p.skill_count })}</span>
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

        if (this.currentTab === 'mcp') {
            back.classList.add('hidden');
            diff.classList.add('hidden');
            sync.classList.add('hidden');
            breadcrumb.textContent = this.selectedMcpPlatform
                ? (this.mcpPlatforms.find(p => p.id === this.selectedMcpPlatform)?.display_name || '')
                : i.t('mcp.title');
            return;
        }

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
        const allViews = ['skills', 'detail', 'diff', 'search', 'mcp-servers'];

        if (this.currentTab === 'mcp') {
            for (const v of allViews) {
                document.getElementById(`view-${v}`).classList.toggle('hidden', v !== 'mcp-servers');
            }
            this.renderMcpServerList();
            return;
        }

        for (const v of allViews) {
            document.getElementById(`view-${v}`).classList.toggle('hidden', !skillViews.includes(v) || this.currentView !== v);
        }
        if (this.currentView === 'skills') this.renderSkillList();
        if (this.currentView === 'detail') this.renderSkillDetail();
        if (this.currentView === 'diff') this.renderDiffView();
        if (this.currentView === 'search') this.renderSearchResults();
    }

    renderMcpServerList() {
        const el = document.getElementById('view-mcp-servers');
        const i = this.i18n;
        if (!this.selectedMcpPlatform) {
            el.innerHTML = `<p class="text-gray-500">${i.t('mcp.title')}</p>`;
            return;
        }
        if (this.mcpServers.length === 0) {
            el.innerHTML = `<div class="flex justify-between items-center mb-4">
                <p class="text-gray-500">${i.t('mcp.no_servers')}</p>
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
                    <button class="text-xs text-cyan-600 hover:text-cyan-400 px-2 py-1 cursor-pointer hidden group-hover:inline mcp-sync-btn" data-name="${esc(s.name)}">${i.t('mcp.sync')}</button>
                    <button class="text-xs text-red-600 hover:text-red-400 px-2 py-1 cursor-pointer hidden group-hover:inline mcp-delete-btn" data-name="${esc(s.name)}">${i.t('mcp.delete')}</button>
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
                    // Strip [mcp_servers.xxx] header if present
                    saveText = text.replace(/^\[mcp_servers\.[^\]]+\]\s*\n?/, '');
                    if (!saveText.trim()) {
                        alert('Empty config');
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
                        alert(i.t('mcp.parse_error').includes('{error}') ? 'Invalid JSON format' : i.tWith('mcp.parse_error', { error: 'Invalid JSON' }));
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
                    alert(i.tWith('mcp.parse_error', { error: e.SyncError || e }));
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
        if (isToml) {
            wrapped = `[mcp_servers.${name}]\n${detail.config_text}`;
        } else {
            try {
                const configObj = JSON.parse(detail.config_text);
                const wrappedObj = {};
                wrappedObj[name] = configObj;
                wrapped = JSON.stringify(wrappedObj, null, 2);
            } catch {
                wrapped = detail.config_text;
            }
        }
        return `<div class="px-3 pb-3 space-y-2">
            <div class="text-xs text-gray-500">${isToml ? 'TOML' : 'JSON'}</div>
            <textarea data-edit-name="${esc(name)}" style="height:20rem" class="w-full bg-gray-900 text-sm text-gray-200 font-mono rounded p-3 border border-gray-600 focus:border-cyan-500 outline-none resize-y">${esc(wrapped)}</textarea>
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
                alert('Error: ' + e);
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
            el.innerHTML = `<p class="text-gray-500">${i.t('ui.no_skills')}</p>`;
            return;
        }

        // Group skills by folder
        const groups = new Map();
        for (const s of this.skills) {
            const folder = s.folder || '';
            if (!groups.has(folder)) groups.set(folder, []);
            groups.get(folder).push(s);
        }

        let html = '<div class="space-y-0.5">';

        // Root-level skills first
        if (groups.has('')) {
            for (const s of groups.get('')) {
                html += this.renderSkillItem(s);
            }
            groups.delete('');
        }

        // Folder groups
        for (const [folder, folderSkills] of groups) {
            const count = folderSkills.length;
            const collapsed = this.collapsedFolders.has(folder);
            html += `<div class="mt-2">
                <div class="flex items-center">
                    <button class="flex-1 text-left px-2 py-1.5 text-sm text-gray-400 hover:text-gray-200 cursor-pointer folder-header flex items-center gap-1"
                        data-folder="${esc(folder)}">
                        <span class="flex-shrink-0 text-gray-500 transition-transform duration-200 ${collapsed ? '' : 'rotate-90'}">${Icons.arrowRight}</span>
                        <span class="text-yellow-500">${esc(folder)}</span>
                        <span class="text-gray-600 text-xs ml-1">(${count})</span>
                    </button>
                    <button class="flex items-center gap-1 text-xs text-gray-500 hover:text-cyan-400 px-2 py-1 cursor-pointer folder-sync-btn transition-colors"
                        data-folder="${esc(folder)}" data-count="${count}" title="Sync all in folder">${Icons.sync}</button>
                </div>
                <div class="pl-3 ${collapsed ? 'hidden' : ''}" data-folder-content="${esc(folder)}">`;
            for (const s of folderSkills) {
                html += this.renderSkillItem(s);
            }
            html += `</div></div>`;
        }

        html += '</div>';
        el.innerHTML = html;

        // Bind skill item clicks
        el.querySelectorAll('.skill-item').forEach(btn => {
            btn.addEventListener('click', () => this.selectSkill(btn.dataset.name, btn.dataset.folder));
        });

        // Bind skill delete buttons
        el.querySelectorAll('.skill-delete-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                this.deleteSkill(btn.dataset.name, btn.dataset.folder, btn);
            });
        });

        // Bind folder toggle
        el.querySelectorAll('.folder-header').forEach(btn => {
            btn.addEventListener('click', () => this.toggleFolder(btn.dataset.folder));
        });

        // Bind folder sync
        el.querySelectorAll('.folder-sync-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                this.showFolderSyncModal(btn.dataset.folder, parseInt(btn.dataset.count));
            });
        });
    }

    renderSkillItem(s) {
        const i = this.i18n;
        const version = s.version ? `<span class="text-gray-500 text-xs ml-2">v${esc(s.version)}</span>` : '';
        const symlink = s.is_symlink ? `<span class="text-cyan-500 text-xs ml-1 inline-flex align-middle">${Icons.symlink}</span>` : '';
        const desc = s.description ? `<span class="text-gray-500 text-sm ml-2">${esc(truncate(s.description, 60))}</span>` : '';
        const size = s.total_size > 1024 ? `<span class="text-gray-600 text-xs ml-2">${(s.total_size / 1024).toFixed(0)}KB</span>` : '';
        return `<div class="flex items-center rounded hover:bg-gray-800 group">
            <button class="flex-1 text-left px-3 py-2 text-gray-200 cursor-pointer skill-item"
                data-name="${esc(s.name)}" data-folder="${esc(s.folder)}">
                <span class="text-cyan-400">${esc(s.name)}</span>${version}${desc}${symlink}${size}
            </button>
            <button class="text-xs text-red-600 hover:text-red-400 px-2 py-1 cursor-pointer hidden group-hover:inline skill-delete-btn"
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
            const version = detail.version ? `<div class="mb-1"><span class="text-yellow-400">${i.t('skill.version')}:</span> ${esc(detail.version)}</div>` : '';
            const sizeStr = detail.total_size < 1024 ? `${detail.total_size} B`
                : detail.total_size < 1048576 ? `${(detail.total_size / 1024).toFixed(1)} KB`
                : `${(detail.total_size / 1048576).toFixed(1)} MB`;
            const symlink = detail.is_symlink
                ? `<div class="text-cyan-400 flex items-center gap-1">${Icons.symlink} ${i.tWith('skill.symlink_to', { target: detail.symlink_target || '?' })}</div>` : '';
            const filesList = detail.files.map(f =>
                `<div class="text-cyan-400 hover:text-cyan-300 text-sm pl-4 cursor-pointer file-item" data-file="${esc(f)}">${esc(f)}</div>`
            ).join('');

            el.innerHTML = `
                <h2 class="text-2xl font-bold text-green-400 mb-4">${esc(detail.name)}</h2>
                <div class="space-y-2 mb-6">
                    <div><span class="text-yellow-400">${i.t('skill.platform')}:</span> ${esc(detail.platform_id)}</div>
                    <div><span class="text-yellow-400">Path:</span> <span class="text-gray-400 text-sm" title="${esc(detail.path)}">${esc(detail.path)}</span></div>
                    ${version}
                    <div><span class="text-yellow-400">${i.t('skill.size')}:</span> ${sizeStr}</div>
                    ${symlink}
                    <div><span class="text-yellow-400">${i.t('skill.files')}:</span> ${i.tWith('skill.total_files', { count: detail.files.length })}</div>
                    ${filesList}
                </div>
                ${detail.description ? `<div class="mb-4"><span class="text-yellow-400">${i.t('skill.description')}:</span><div class="mt-1 text-gray-300">${esc(detail.description)}</div></div>` : ''}
                <div id="file-viewer" class="hidden mb-4">
                    <div class="flex items-center justify-between mb-2">
                        <span id="file-viewer-path" class="text-sm text-purple-400 font-bold"></span>
                        <button id="file-viewer-close" class="text-xs text-gray-400 hover:text-white cursor-pointer">Close</button>
                    </div>
                    <pre id="file-viewer-content" class="text-sm text-gray-300 whitespace-pre-wrap font-mono bg-gray-950 rounded-lg p-4 overflow-x-auto max-h-[40vh]"></pre>
                </div>
                <div class="border-t border-gray-700 pt-4">
                    <pre class="text-sm text-gray-300 whitespace-pre-wrap font-mono bg-gray-950 rounded-lg p-4 overflow-x-auto">${esc(detail.body)}</pre>
                </div>`;

            // Bind file click events
            el.querySelectorAll('.file-item').forEach(item => {
                item.addEventListener('click', async () => {
                    const filePath = item.dataset.file;
                    try {
                        const content = await Api.readSkillFile(this.selectedPlatformId, this.selectedSkillName, this.selectedFolder, filePath);
                        document.getElementById('file-viewer-path').textContent = filePath;
                        document.getElementById('file-viewer-content').textContent = content;
                        document.getElementById('file-viewer').classList.remove('hidden');
                    } catch (e) {
                        document.getElementById('file-viewer-path').textContent = filePath;
                        document.getElementById('file-viewer-content').textContent = `Error: ${e}`;
                        document.getElementById('file-viewer').classList.remove('hidden');
                    }
                });
            });
            const closeBtn = document.getElementById('file-viewer-close');
            if (closeBtn) closeBtn.addEventListener('click', () => {
                document.getElementById('file-viewer').classList.add('hidden');
            });
        } catch (e) {
            el.innerHTML = `<p class="text-red-400">Error: ${e}</p>`;
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
        if (this.searchResults.length === 0) {
            el.innerHTML = `<p class="text-gray-500">${i.t('ui.no_results')}</p>`;
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

// Convert unified diff lines to side-by-side pairs
// Input: [{ tag: 'context'|'removed'|'added', content }] or [{ Context, Added, Removed, FileHeader }]
// Output: [{ left: {text, type}, right: {text, type} }]
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

    let i = 0;
    while (i < normalized.length) {
        const line = normalized[i];
        if (line.tag === 'context') {
            pairs.push({ left: { text: line.content, type: 'context' }, right: { text: line.content, type: 'context' } });
            i++;
        } else if (line.tag === 'removed') {
            // Collect consecutive removed
            const removed = [];
            while (i < normalized.length && normalized[i].tag === 'removed') {
                removed.push(normalized[i].content);
                i++;
            }
            // Collect consecutive added
            const added = [];
            while (i < normalized.length && normalized[i].tag === 'added') {
                added.push(normalized[i].content);
                i++;
            }
            // Pair them up
            const max = Math.max(removed.length, added.length);
            for (let j = 0; j < max; j++) {
                pairs.push({
                    left: j < removed.length ? { text: removed[j], type: 'removed' } : { text: '', type: 'empty' },
                    right: j < added.length ? { text: added[j], type: 'added' } : { text: '', type: 'empty' },
                });
            }
        } else if (line.tag === 'added') {
            pairs.push({ left: { text: '', type: 'empty' }, right: { text: line.content, type: 'added' } });
            i++;
        } else {
            i++;
        }
    }
    return pairs;
}

function renderSideBySide(pairs) {
    let html = `<div style="display:grid;grid-template-columns:1fr 1fr;font-size:0.8125rem;line-height:1.6;font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;background:#0a0a0f;border-radius:0.5rem;overflow:hidden;border:1px solid #1e293b">`;
    html += `<div style="background:#1e293b;color:#94a3b8;font-size:0.7rem;font-weight:600;text-transform:uppercase;letter-spacing:0.05em;text-align:center;padding:0.4rem 0;border-right:1px solid #334155">Before</div>`;
    html += `<div style="background:#1e293b;color:#94a3b8;font-size:0.7rem;font-weight:600;text-transform:uppercase;letter-spacing:0.05em;text-align:center;padding:0.4rem 0">After</div>`;
    for (const p of pairs) {
        const lBg = p.left.type === 'removed' ? 'background:rgba(153,27,27,0.25);color:#fca5a5' : p.left.type === 'empty' ? 'background:rgba(10,10,15,0.6)' : 'color:#64748b';
        const rBg = p.right.type === 'added' ? 'background:rgba(22,101,52,0.25);color:#86efac' : p.right.type === 'empty' ? 'background:rgba(10,10,15,0.6)' : 'color:#64748b';
        const leftPre = p.left.type === 'removed' ? '<span style="color:#ef4444;font-weight:600">-</span> ' : '<span style="color:#334155"> </span> ';
        const rightPre = p.right.type === 'added' ? '<span style="color:#22c55e;font-weight:600">+</span> ' : '<span style="color:#334155"> </span> ';
        html += `<div style="${lBg};padding:0 0.75rem;white-space:pre;overflow-x:auto;border-right:1px solid #1e293b;min-height:1.6em">${leftPre}${esc(p.left.text)}</div>`;
        html += `<div style="${rBg};padding:0 0.75rem;white-space:pre;overflow-x:auto;min-height:1.6em">${rightPre}${esc(p.right.text)}</div>`;
    }
    html += `</div>`;
    return html;
}

const app = new App();
app.init();
