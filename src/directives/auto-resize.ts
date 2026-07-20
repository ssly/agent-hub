import type { Directive } from 'vue'

/**
 * v-auto-resize — let a <textarea> grow with its content.
 *
 * Height tracks scrollHeight, capped at 52vh; beyond that the textarea
 * scrolls internally. Recalculated on mount and on every component update
 * (v-model changes trigger re-render, which fires `updated`).
 */
function resize(el: HTMLTextAreaElement) {
  const max = Math.round(window.innerHeight * 0.52)
  el.style.height = 'auto'
  const next = Math.min(el.scrollHeight, max)
  el.style.height = `${next}px`
  el.style.overflowY = el.scrollHeight > max ? 'auto' : 'hidden'
}

export const vAutoResize: Directive<HTMLTextAreaElement> = {
  mounted(el) {
    // Wait one frame so web-fonts/layout settle before measuring
    requestAnimationFrame(() => resize(el))
  },
  updated(el) {
    resize(el)
  },
}
