import type { Directive } from 'vue'

/**
 * v-tooltip — styled replacement for the native `title` attribute.
 *
 * Usage: `v-tooltip="text"`. Nothing shows when the value is empty.
 * A single tooltip element is shared app-wide (appended to <body>); its
 * colors invert with the theme via --ink / --canvas (styles live in
 * theme.css under § Tooltip).
 */

let tipEl: HTMLDivElement | null = null

function ensureTip(): HTMLDivElement {
  if (!tipEl) {
    tipEl = document.createElement('div')
    tipEl.className = 'ah-tooltip'
    document.body.appendChild(tipEl)
    // Any scroll/resize repositions nothing — just dismiss.
    window.addEventListener('scroll', hide, true)
    window.addEventListener('resize', hide)
  }
  return tipEl
}

function hide() {
  tipEl?.classList.remove('is-visible')
}

function show(el: HTMLElement, text: string) {
  if (!text) return
  const tip = ensureTip()
  tip.textContent = text
  tip.classList.add('is-visible')

  // Measure after the text is in, then clamp into the viewport. Default
  // placement is below the anchor; flip above when there is no room.
  const rect = el.getBoundingClientRect()
  const tipRect = tip.getBoundingClientRect()
  const margin = 8
  const left = Math.max(
    margin,
    Math.min(rect.left, window.innerWidth - tipRect.width - margin),
  )
  let top = rect.bottom + 6
  if (top + tipRect.height > window.innerHeight - margin) {
    top = Math.max(margin, rect.top - tipRect.height - 6)
  }
  tip.style.left = `${left}px`
  tip.style.top = `${top}px`
}

type TooltipElement = HTMLElement & { __ahTooltip__?: { onEnter: () => void; onLeave: () => void } }

export const vTooltip: Directive<TooltipElement, string | undefined> = {
  mounted(el, binding) {
    el.dataset.ahTooltip = binding.value ?? ''
    const handlers = {
      onEnter: () => show(el, el.dataset.ahTooltip || ''),
      onLeave: hide,
    }
    el.__ahTooltip__ = handlers
    el.addEventListener('mouseenter', handlers.onEnter)
    el.addEventListener('mouseleave', handlers.onLeave)
  },
  updated(el, binding) {
    el.dataset.ahTooltip = binding.value ?? ''
  },
  unmounted(el) {
    const handlers = el.__ahTooltip__
    if (handlers) {
      el.removeEventListener('mouseenter', handlers.onEnter)
      el.removeEventListener('mouseleave', handlers.onLeave)
      delete el.__ahTooltip__
    }
    hide()
  },
}
