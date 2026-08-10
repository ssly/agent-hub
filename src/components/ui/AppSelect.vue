<script setup lang="ts">
// Fully custom dropdown (NOT a native <select>).
// A native select's open <option> list is rendered by the OS and can't be styled,
// so we render the trigger and the floating list ourselves. Keeps keyboard nav,
// outside-click close, disabled options, and a11y roles.
import { ref, computed, nextTick, onMounted, onUnmounted } from 'vue'

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
}>(), {
  disabled: false,
  placeholder: '',
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const open = ref(false)
const rootRef = ref<HTMLElement | null>(null)
const listRef = ref<HTMLElement | null>(null)
const activeIndex = ref(-1)

const selected = computed(() => props.options.find(o => o.value === props.modelValue))
const displayLabel = computed(() => selected.value?.label ?? props.placeholder)

function openMenu() {
  if (props.disabled || open.value) return
  open.value = true
  const idx = props.options.findIndex(o => o.value === props.modelValue)
  activeIndex.value = idx >= 0 ? idx : firstEnabled()
  nextTick(scrollActiveIntoView)
}
function close() { open.value = false }
function toggle() { open.value ? close() : openMenu() }

function choose(opt: SelectOption) {
  if (opt.disabled) return
  emit('update:modelValue', opt.value)
  close()
}

function firstEnabled() {
  const i = props.options.findIndex(o => !o.disabled)
  return i >= 0 ? i : 0
}
function stepEnabled(from: number, dir: 1 | -1) {
  const opts = props.options
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
    return
  }
  switch (e.key) {
    case 'ArrowDown': e.preventDefault(); activeIndex.value = stepEnabled(activeIndex.value, 1); nextTick(scrollActiveIntoView); break
    case 'ArrowUp': e.preventDefault(); activeIndex.value = stepEnabled(activeIndex.value, -1); nextTick(scrollActiveIntoView); break
    case 'Enter': e.preventDefault(); if (activeIndex.value >= 0) choose(props.options[activeIndex.value]); break
    case 'Escape': e.preventDefault(); close(); break
    case 'Tab': close(); break
  }
}

function scrollActiveIntoView() {
  const list = listRef.value
  if (!list) return
  const el = list.children[activeIndex.value] as HTMLElement | undefined
  el?.scrollIntoView({ block: 'nearest' })
}

function onDocumentMousedown(e: MouseEvent) {
  if (rootRef.value && !rootRef.value.contains(e.target as Node)) close()
}

onMounted(() => document.addEventListener('mousedown', onDocumentMousedown))
onUnmounted(() => document.removeEventListener('mousedown', onDocumentMousedown))
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
      <span class="app-select__value" :class="{ 'is-placeholder': !selected }">{{ displayLabel }}</span>
      <svg
        class="app-select__chevron"
        width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
        stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
    </button>

    <transition name="app-select-pop">
      <ul v-if="open" ref="listRef" class="app-select__list" role="listbox">
        <li
          v-for="(opt, idx) in options"
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
          <span class="app-select__option-label">{{ opt.label }}</span>
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
    </transition>
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
  justify-content: space-between;
  gap: 8px;
  height: 28px;
  padding: 0 8px 0 10px;
  background: var(--sunken);
  border: 1px solid var(--border);
  border-radius: var(--radius-xs);
  color: var(--ink);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  outline: none;
  transition: border-color var(--dur-fast) var(--ease-soft), background var(--dur-fast) var(--ease-soft);
}
.app-select__trigger:hover:not(:disabled) {
  border-color: var(--border-strong);
  background: var(--hover);
}
.app-select__trigger:focus-visible,
.app-select.is-open .app-select__trigger {
  border-color: var(--accent);
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
  flex-shrink: 0;
  color: var(--ink-3);
  transition: transform var(--dur-fast) var(--ease-soft), color var(--dur-fast) var(--ease-soft);
}
.app-select.is-open .app-select__chevron {
  transform: rotate(180deg);
  color: var(--accent);
}

/* Floating panel — fully custom, replaces the native <option> list */
.app-select__list {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  min-width: 100%;
  max-width: min(24rem, 80vw);
  max-height: 240px;
  overflow-y: auto;
  /* Don't chain wheel scrolls into the page once the list hits its end. */
  overscroll-behavior: contain;
  margin: 0;
  padding: 4px;
  list-style: none;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-soft);
  z-index: 50;
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
