import { I18n } from './i18n.js';
import * as Api from './api.js';

class App {
    constructor() {
        this.platforms = [];
        this.skills = [];
        this.selectedPlatformId = null;
        this.selectedSkillName = null;
        this.selectedFolder = '';
        this.currentView = 'skills';
        this.diffResult = null;
        this.searchResults = [];
        this.fileViewing = null;
        this.i18n = new I18n();
        this.collapsedFolders = new Set();
        this.pendingSyncTarget = null;
    }

    async init() {
        this.i18n.locale = await Api.getLocale();
        await this.i18n.load();
        await this.refreshPlatforms();
        this.bindEvents();
        this.render();
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

    async switchLang() {
        await this.i18n.switchLocale();
        await Api.setLocale(this.i18n.locale);
        this.render();
    }

    // --- Events ---
    bindEvents() {
        document.getElementById('btn-refresh').addEventListener('click', () => this.refreshPlatforms().then(() => this.render()));
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
            <p class="text-yellow-400 text-sm mb-3">⚠ ${i.t('sync.conflict_warning')}</p>
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
        this.renderSidebar();
        this.renderToolbar();
        this.renderView();
        document.getElementById('btn-lang').textContent = this.i18n.locale === 'en' ? 'EN' : '中文';
    }

    renderSidebar() {
        const i = this.i18n;
        const el = document.getElementById('platform-list');
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
        const views = ['skills', 'detail', 'diff', 'search'];
        for (const v of views) {
            document.getElementById(`view-${v}`).classList.toggle('hidden', this.currentView !== v);
        }
        if (this.currentView === 'skills') this.renderSkillList();
        if (this.currentView === 'detail') this.renderSkillDetail();
        if (this.currentView === 'diff') this.renderDiffView();
        if (this.currentView === 'search') this.renderSearchResults();
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
                        <span class="text-xs transition ${collapsed ? '' : 'rotate-90'} inline-block">&#9654;</span>
                        <span class="text-yellow-500">${esc(folder)}</span>
                        <span class="text-gray-600 text-xs ml-1">(${count})</span>
                    </button>
                    <button class="text-xs text-cyan-600 hover:text-cyan-400 px-2 py-1 cursor-pointer folder-sync-btn"
                        data-folder="${esc(folder)}" data-count="${count}" title="Sync all in folder">⤴</button>
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
        const version = s.version ? `<span class="text-gray-500 text-xs ml-2">v${esc(s.version)}</span>` : '';
        const symlink = s.is_symlink ? `<span class="text-cyan-600 text-xs ml-1">🔗</span>` : '';
        const desc = s.description ? `<span class="text-gray-500 text-sm ml-2">${esc(truncate(s.description, 60))}</span>` : '';
        const size = s.total_size > 1024 ? `<span class="text-gray-600 text-xs ml-2">${(s.total_size / 1024).toFixed(0)}KB</span>` : '';
        return `<button class="w-full text-left px-3 py-2 rounded hover:bg-gray-800 text-gray-200 cursor-pointer flex items-center skill-item"
            data-name="${esc(s.name)}" data-folder="${esc(s.folder)}">
            <span class="text-cyan-400">${esc(s.name)}</span>${version}${desc}${symlink}${size}
        </button>`;
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
                ? `<div class="text-cyan-400">🔗 ${i.tWith('skill.symlink_to', { target: detail.symlink_target || '?' })}</div>` : '';
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
            html += `</div><pre class="text-sm font-mono bg-gray-950 rounded-lg p-3 overflow-x-auto">`;

            for (const line of fd.lines) {
                if (line.Context !== undefined) {
                    html += `<div class="text-gray-600">${esc(line.Context)}</div>`;
                } else if (line.Added !== undefined) {
                    html += `<div class="text-green-400 bg-green-950/30">${esc(line.Added)}</div>`;
                } else if (line.Removed !== undefined) {
                    html += `<div class="text-red-400 bg-red-950/30">${esc(line.Removed)}</div>`;
                } else if (line.FileHeader !== undefined) {
                    html += `<div class="text-yellow-400">${esc(line.FileHeader)}</div>`;
                }
            }
            html += `</pre></div>`;
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

const app = new App();
app.init();
