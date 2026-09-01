<script setup lang="ts">
// Fully custom dropdown (NOT a native <select>).
// A native select's open <option> list is rendered by the OS and can't be styled,
// so we render the trigger and the floating list ourselves. Keeps keyboard nav,
// outside-click close, disabled options, and a11y roles.
import { ref, computed, nextTick, watch, onMounted, onUnmounted } from 'vue'
import { Search, X } from 'lucide-vue-next'

interface SelectOption {
  value: string
  label: string
  disabled?: boolean
}

const props = withDefaults(defineProps<{
  modelValue: string
  options: SelectOption[]
  disabled?: boolean
  placeholder?: string
  searchable?: boolean
  searchPlaceholder?: string
  searchEmpty?: string
  searchClearLabel?: string
}>(), {
  disabled: false,
  placeholder: '',
  searchable: false,
  searchPlaceholder: '',
  searchEmpty: '',
  searchClearLabel: '',
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const open = ref(false)
const rootRef = ref<HTMLElement | null>(null)
const panelRef = ref<HTMLElement | null>(null)
const optionsRef = ref<HTMLElement | null>(null)
const searchInput = ref<HTMLInputElement | null>(null)
const activeIndex = ref(-1)
const query = ref('')
const listPos = ref<Record<string, string>>({})

const selected = computed(() => props.options.find(o => o.value === props.modelValue))
const displayLabel = computed(() => selected.value?.label ?? props.placeholder)

const visibleOptions = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return props.options
  return props.options.filter(o =>
    o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q),
  )
})

function isPathLabel(label: string) {
  return label.startsWith('/') || /^[A-Za-z]:[\\/]/.test(label)
}

function placeList() {
  const el = rootRef.value
  if (!el) return
  const rect = el.getBoundingClientRect()
  const pad = 8
  const preferred = Math.min(420, window.innerWidth - pad * 2)
  const width = Math.max(rect.width, preferred)
  let left = rect.right - width
  if (left < pad) left = pad
  if (left + width > window.innerWidth - pad) {
    left = Math.max(pad, window.innerWidth - pad - width)
  }
  const finalWidth = Math.min(width, window.innerWidth - pad - left)
  const spaceBelow = window.innerHeight - rect.bottom - pad
  const spaceAbove = rect.top - pad
  const openUp = spaceBelow < 140 && spaceAbove > spaceBelow
  const maxHeight = Math.min(320, Math.max(160, (openUp ? spaceAbove : spaceBelow) - 4))
  listPos.value = {
    top: openUp ? 'auto' : `${Math.round(rect.bottom + 4)}px`,
    bottom: openUp ? `${Math.round(window.innerHeight - rect.top + 4)}px` : 'auto',
    left: `${Math.round(left)}px`,
    width: `${Math.round(finalWidth)}px`,
    maxHeight: `${Math.round(maxHeight)}px`,
  }
}

function openMenu() {
  if (props.disabled || open.value) return
  query.value = ''
  open.value = true
  const idx = visibleOptions.value.findIndex(o => o.value === props.modelValue)
  activeIndex.value = idx >= 0 ? idx : firstEnabled()
  placeList()
  nextTick(() => {
    searchInput.value?.focus()
    scrollActiveIntoView()
  })
}
function close() {
  open.value = false
  query.value = ''
}
function toggle() { open.value ? close() : openMenu() }

function choose(opt: SelectOption) {
  if (opt.disabled) return
  emit('update:modelValue', opt.value)
  close()
}

function clearQuery() {
  query.value = ''
  nextTick(() => searchInput.value?.focus())
}

function firstEnabled() {
  const i = visibleOptions.value.findIndex(o => !o.disabled)
  return i >= 0 ? i : 0
}
function stepEnabled(from: number, dir: 1 | -1) {
  const opts = visibleOptions.value
  if (opts.length === 0) return -1
  let i = from
  for (let n = 0; n < opts.length; n++) {
    i = (i + dir + opts.length) % opts.length
    if (!opts[i].disabled) return i
  }
  return from
}

function onTriggerKeydown(e: KeyboardEvent) {
  if (props.disabled) return
  if (!open.value) {
    if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      openMenu()
    }
  }
}

function onSearchKeydown(e: KeyboardEvent) {
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      activeIndex.value = stepEnabled(activeIndex.value, 1)
      nextTick(scrollActiveIntoView)
      break
    case 'ArrowUp':
      e.preventDefault()
      activeIndex.value = stepEnabled(activeIndex.value, -1)
      nextTick(scrollActiveIntoView)
      break
    case 'Enter':
      e.preventDefault()
      if (activeIndex.value >= 0 && visibleOptions.value[activeIndex.value]) {
        choose(visibleOptions.value[activeIndex.value])
      }
      break
    case 'Escape':
      e.preventDefault()
      if (query.value) clearQuery()
      else close()
      break
    case 'Tab':
      close()
      break
  }
}

function scrollActiveIntoView() {
  const list = optionsRef.value
  if (!list || activeIndex.value < 0) return
  const el = list.children[activeIndex.value] as HTMLElement | undefined
  el?.scrollIntoView({ block: 'nearest' })
}

function onDocumentMousedown(e: MouseEvent) {
  const target = e.target as Node
  if (rootRef.value?.contains(target) || panelRef.value?.contains(target)) return
  close()
}

function onReposition() {
  if (open.value) placeList()
}

watch(query, () => {
  const selectedIdx = visibleOptions.value.findIndex(o => o.value === props.modelValue)
  activeIndex.value = selectedIdx >= 0 ? selectedIdx : firstEnabled()
})

onMounted(() => {
  document.addEventListener('mousedown', onDocumentMousedown)
  window.addEventListener('resize', onReposition)
  window.addEventListener('scroll', onReposition, true)
})
onUnmounted(() => {
  document.removeEventListener('mousedown', onDocumentMousedown)
  window.removeEventListener('resize', onReposition)
  window.removeEventListener('scroll', onReposition, true)
})
</script>

<template>
  <div ref="rootRef" class="app-select" :class="{ 'is-open': open, 'is-disabled': disabled }">
    <button
      type="button"
      class="app-select__trigger"
      :disabled="disabled"
      :aria-expanded="open"
      aria-haspopup="listbox"
      @click="toggle"
      @keydown="onTriggerKeydown"
    >
      <span v-if="$slots.prefix" class="app-select__prefix"><slot name="prefix" /></span>
      <span class="app-select__value" :class="{ 'is-placeholder': !selected }" :title="displayLabel">{{ displayLabel }}</span>
      <svg
        class="app-select__chevron"
        width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
        stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
    </button>

    <Teleport to="body">
      <transition name="app-select-pop">
        <div
          v-if="open"
          ref="panelRef"
          class="app-select__list"
          :style="listPos"
        >
          <div v-if="searchable" class="app-select__search">
            <Search :size="13" :stroke-width="2" class="app-select__search-icon" />
            <input
              ref="searchInput"
              v-model="query"
              type="text"
              class="app-select__search-input"
              :placeholder="searchPlaceholder"
              autocomplete="off"
              spellcheck="false"
              @keydown="onSearchKeydown"
            />
            <button
              type="button"
              class="app-select__search-clear"
              :disabled="!query"
              :aria-label="searchClearLabel || searchPlaceholder"
              @mousedown.prevent
              @click="clearQuery"
            >
              <X :size="13" :stroke-width="2.25" />
            </button>
          </div>
          <ul v-if="visibleOptions.length > 0" ref="optionsRef" class="app-select__options" role="listbox">
            <li
              v-for="(opt, idx) in visibleOptions"
              :key="opt.value"
              role="option"
              :aria-selected="opt.value === modelValue"
              :class="['app-select__option', {
                'is-active': idx === activeIndex,
                'is-selected': opt.value === modelValue,
                'is-disabled': opt.disabled,
              }]"
              @click="choose(opt)"
              @mouseenter="activeIndex = idx"
            >
              <span
                class="app-select__option-label"
                :class="{ 'is-path': isPathLabel(opt.label) }"
                :title="opt.label"
              >{{ opt.label }}</span>
              <svg
                v-if="opt.value === modelValue"
                class="app-select__check"
                width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </li>
          </ul>
          <div v-else class="app-select__empty">{{ searchEmpty }}</div>
        </div>
      </transition>
    </Teleport>
  </div>
</template>

<style scoped>
.app-select {
  position: relative;
  width: 100%;
  min-width: 0;
}
.app-select__trigger {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 8px 0 8px;
  background: var(--surface);
  border: 1px solid var(--hairline);
  border-radius: var(--radius-sm);
  color: var(--ink-2);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  outline: none;
  box-shadow: var(--shadow-mist);
  transition: border-color var(--dur-fast) var(--ease-soft), background var(--dur-fast) var(--ease-soft), color var(--dur-fast) var(--ease-soft);
}
.app-select__trigger:hover:not(:disabled) {
  border-color: var(--border-strong);
  background: var(--hover);
  color: var(--ink);
}
.app-select__trigger:focus-visible,
.app-select.is-open .app-select__trigger {
  border-color: var(--accent);
  color: var(--ink);
}
.app-select__prefix {
  display: inline-flex;
  align-items: center;
  color: var(--ink-3);
  flex: none;
}
.app-select__trigger:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.app-select__value {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}
.app-select__value.is-placeholder {
  color: var(--ink-4);
}
.app-select__chevron {
  flex: none;
  margin-left: auto;
  color: var(--ink-4);
  transition: transform var(--dur-fast) var(--ease-soft), color var(--dur-fast) var(--ease-soft);
}
.app-select.is-open .app-select__chevron {
  transform: rotate(180deg);
  color: var(--accent);
}

.app-select__list {
  position: fixed;
  z-index: 400;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin: 0;
  list-style: none;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-soft);
}
.app-select__search {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: none;
  padding: 6px 8px;
  border-bottom: 1px solid var(--hairline);
}
.app-select__search-icon {
  flex: none;
  color: var(--ink-4);
}
.app-select__search-input {
  flex: 1;
  min-width: 0;
  height: 22px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--ink);
  font-size: 12px;
  outline: none;
}
.app-select__search-input::placeholder {
  color: var(--ink-4);
}
.app-select__search-clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--ink-3);
  cursor: pointer;
}
.app-select__search-clear:hover:not(:disabled) {
  background: var(--hover);
  color: var(--ink);
}
.app-select__search-clear:disabled {
  opacity: 0.35;
  cursor: default;
}
.app-select__options {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  margin: 0;
  padding: 4px;
  list-style: none;
}
.app-select__empty {
  padding: 18px 12px;
  text-align: center;
  font-size: 12px;
  color: var(--ink-4);
}
.app-select__option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  font-size: 12px;
  color: var(--ink);
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-soft);
}
.app-select__option.is-active:not(.is-disabled) {
  background: var(--hover);
}
.app-select__option.is-selected {
  color: var(--accent);
  font-weight: 500;
}
.app-select__option.is-disabled {
  color: var(--ink-4);
  cursor: not-allowed;
  opacity: 0.6;
}
.app-select__option-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}
.app-select__option-label.is-path {
  font-family: var(--font-mono);
  font-size: 11.5px;
  direction: rtl;
  text-align: left;
}
.app-select__check {
  flex-shrink: 0;
  color: var(--accent);
}

.app-select-pop-enter-active,
.app-select-pop-leave-active {
  transition: opacity var(--dur-fast) var(--ease-soft), transform var(--dur-fast) var(--ease-soft);
}
.app-select-pop-enter-from,
.app-select-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
