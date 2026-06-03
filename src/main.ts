import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createI18n } from 'vue-i18n'
import App from './App.vue'
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

const app = createApp(App)
app.use(createPinia())
app.use(i18n)
app.mount('#app')
