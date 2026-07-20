import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createI18n } from 'vue-i18n'
import App from './App.vue'
import CodexTrayView from './components/tray/CodexTrayView.vue'
import { vAutoResize } from './directives/auto-resize'
import en from './locales/en.json'
import zhCN from './locales/zh-CN.json'
import './assets/theme.css'
import './assets/main.css'

const savedLocale = localStorage.getItem('ah-locale') || 'zh-CN'

const i18n = createI18n({
  legacy: false,
  locale: savedLocale,
  fallbackLocale: 'en',
  messages: { en, 'zh-CN': zhCN },
})

const isTrayView = new URLSearchParams(window.location.search).get('view') === 'codex-usage'
// The tray window needs a transparent html/body (its scoped styles reset them
// via :global); tag the root element so that reset only applies here and never
// leaks into the main window's bundle.
if (isTrayView) {
  document.documentElement.setAttribute('data-view', 'codex-usage')
}

const rootComponent = isTrayView ? CodexTrayView : App
const app = createApp(rootComponent)
app.use(createPinia())
app.use(i18n)
app.directive('auto-resize', vAutoResize)
app.mount('#app')
