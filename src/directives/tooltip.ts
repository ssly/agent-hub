import type { Directive } from 'vue'

/**
 * v-tooltip — styled replacement for the native `title` attribute.
 *
 * Usage:
 *   v-tooltip="text"
 *   v-tooltip:bottom / :top / :left / :right
 *   v-tooltip.clamp="text"          — clamp body to 3 lines + ellipsis
 *   v-tooltip:top.clamp="text"
 *
 * Dynamic args work (`v-tooltip:[side]="text"`). Empty value shows nothing.
 * A single tooltip element is shared app-wide (appended to <body>); colors
 * invert with the theme via --ink / --canvas (styles in theme.css § Tooltip).
 */

let tipEl: HTMLDivElement | null = null

function ensureTip(): HTMLDivElement {
  if (!tipEl) {
    tipEl = document.createElement('div')
    tipEl.className = 'ah-tooltip'
    document.body.appendChild(tipEl)
    window.addEventListener('scroll', hide, true)
    window.addEventListener('resize', hide)
  }
  return tipEl
}

function hide() {
  tipEl?.classList.remove('is-visible')
}

function clampToThreeLines(tip: HTMLElement, text: string) {
  const style = getComputedStyle(tip)
  const limit =
    parseFloat(style.lineHeight) * 3 +
    parseFloat(style.paddingTop) +
    parseFloat(style.paddingBottom)
  if (tip.scrollHeight <= limit + 1) return
  // Binary search the longest prefix that, with an ellipsis appended, still
  // fits three lines.
  let lo = 0
  let hi = text.length
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    tip.textContent = `${text.slice(0, mid)}…`
    if (tip.scrollHeight <= limit + 1) lo = mid
    else hi = mid - 1
  }
}

function show(
  el: HTMLElement,
  text: string,
  placement: 'bottom' | 'left' | 'right' | 'top',
  clamp: boolean,
) {
  if (!text) return
  const tip = ensureTip()
  // Clamped tips wrap as normal text (white-space: normal collapses the
  // newlines); the 3-line cut itself is JS because CSS line-clamp paints a
  // ghost 4th line in WebKit/Blink.
  const content = clamp ? text.replace(/\s*\n+\s*/g, ' ').trim() : text
  tip.textContent = content
  tip.classList.toggle('is-clamped', clamp)
  tip.classList.add('is-visible')

  const margin = 16

  // Reset geometry before measuring: shrink-to-fit would otherwise size the
  // box against the previous show's leftover position. Keep a real margin
  // from the window edges — in the 400px usage tray an edge-to-edge bubble
  // reads as broken.
  tip.style.left = '0px'
  tip.style.top = '0px'
  tip.style.maxWidth = `${window.innerWidth - margin * 2}px`
  if (clamp) clampToThreeLines(tip, content)

  const rect = el.getBoundingClientRect()
  const tipRect = tip.getBoundingClientRect()

  const placeBeside = (prefer: 'left' | 'right'): boolean => {
    const placeAt = (left: number, height: number) => {
      let top = rect.top + rect.height / 2 - height / 2
      top = Math.max(margin, Math.min(top, window.innerHeight - height - margin))
      tip.style.left = `${left}px`
      tip.style.top = `${top}px`
    }
    const leftSpace = rect.left - 6 - margin
    const rightSpace = window.innerWidth - margin - rect.right - 6
    const first = prefer === 'left' ? 'left' : 'right'
    const second = prefer === 'left' ? 'right' : 'left'
    for (const side of [first, second]) {
      if (side === 'left' && leftSpace >= tipRect.width) {
        placeAt(rect.left - tipRect.width - 6, tipRect.height)
        return true
      }
      if (side === 'right' && rightSpace >= tipRect.width) {
        placeAt(rect.right + 6, tipRect.height)
        return true
      }
    }
    const space = Math.max(leftSpace, rightSpace)
    if (space < 96) return false
    tip.style.maxWidth = `${space}px`
    const shrunk = tip.getBoundingClientRect()
    placeAt(leftSpace >= rightSpace ? rect.left - shrunk.width - 6 : rect.right + 6, shrunk.height)
    return true
  }

  if ((placement === 'left' || placement === 'right') && placeBeside(placement)) {
    return
  }

  if (placement === 'top') {
    const left = Math.max(
      margin,
      Math.min(rect.left + rect.width / 2 - tipRect.width / 2, window.innerWidth - tipRect.width - margin),
    )
    let top = rect.top - tipRect.height - 6
    if (top < margin) {
      top = rect.bottom + 6
    }
    tip.style.left = `${left}px`
    tip.style.top = `${top}px`
    return
  }

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

type TooltipPlacement = 'bottom' | 'left' | 'right' | 'top'
type TooltipElement = HTMLElement & {
  __ahTooltip__?: { onEnter: () => void; onLeave: () => void }
}

function parsePlacement(arg: string | undefined): TooltipPlacement {
  if (arg === 'left' || arg === 'right' || arg === 'top' || arg === 'bottom') return arg
  return 'bottom'
}

export const vTooltip: Directive<TooltipElement, string | undefined> = {
  mounted(el, binding) {
    el.dataset.ahTooltip = binding.value ?? ''
    el.dataset.ahTooltipPlacement = parsePlacement(binding.arg)
    el.dataset.ahTooltipClamp = binding.modifiers.clamp ? '1' : ''
    const handlers = {
      onEnter: () => show(
        el,
        el.dataset.ahTooltip || '',
        parsePlacement(el.dataset.ahTooltipPlacement),
        el.dataset.ahTooltipClamp === '1',
      ),
      onLeave: hide,
    }
    el.__ahTooltip__ = handlers
    el.addEventListener('mouseenter', handlers.onEnter)
    el.addEventListener('mouseleave', handlers.onLeave)
  },
  updated(el, binding) {
    el.dataset.ahTooltip = binding.value ?? ''
    el.dataset.ahTooltipPlacement = parsePlacement(binding.arg)
    el.dataset.ahTooltipClamp = binding.modifiers.clamp ? '1' : ''
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
