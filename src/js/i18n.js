export class I18n {
    constructor() {
        this.locale = 'en';
        this.translations = {};
    }

    async load() {
        try {
            const resp = await fetch(`locales/${this.locale}.json`);
            this.translations = await resp.json();
        } catch {
            this.translations = {};
        }
    }

    t(key) {
        const parts = key.split('.');
        let obj = this.translations;
        for (const part of parts) obj = obj?.[part];
        return obj || key;
    }

    tWith(key, args) {
        let template = this.t(key);
        for (const [k, v] of Object.entries(args)) {
            template = template.replace(`{${k}}`, v);
        }
        return template;
    }

    async switchLocale() {
        this.locale = this.locale === 'en' ? 'zh-CN' : 'en';
        await this.load();
    }
}
