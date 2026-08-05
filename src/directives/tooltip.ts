import type { Directive } from 'vue'

/**
 * v-tooltip — styled replacement for the native `title` attribute.
 *
 * Usage: `v-tooltip="text"` / `v-tooltip:bottom` (below the anchor, flipping
 * above at the bottom edge), `v-tooltip:top="text"` (above the anchor, flipping
 * below at the top edge), or `v-tooltip:left` / `v-tooltip:right` (beside the
 * anchor on the preferred side, flipping at the screen edge). Dynamic args
 * work (`v-tooltip:[side]="text"`). Nothing shows when the value is empty.
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

function show(el: HTMLElement, text: string, placement: 'bottom' | 'left' | 'right' | 'top') {
  if (!text) return
  const tip = ensureTip()
  tip.textContent = text
  tip.classList.add('is-visible')

  // Reset geometry before measuring: shrink-to-fit would otherwise size the
  // box against the previous show's leftover position, producing narrow,
  // super-tall tooltips.
  tip.style.left = '0px'
  tip.style.top = '0px'
  tip.style.maxWidth = `${window.innerWidth - 16}px`

  const rect = el.getBoundingClientRect()
  const tipRect = tip.getBoundingClientRect()
  const margin = 8

  // Beside the anchor, vertically centered, on the preferred side; flip when
  // that side is off-screen. When neither side fits at natural width (typical
  // in the narrow tray popup), shrink the box into the wider side; only when
  // even that is too cramped fall through to the bottom placement.
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

  // Above the anchor, horizontally centered; flip below at the top edge.
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
type TooltipElement = HTMLElement & { __ahTooltip__?: { onEnter: () => void; onLeave: () => void } }

function parsePlacement(arg: string | undefined): TooltipPlacement {
  if (arg === 'left' || arg === 'right' || arg === 'top' || arg === 'bottom') return arg
  return 'bottom'
}

export const vTooltip: Directive<TooltipElement, string | undefined> = {
  mounted(el, binding) {
    el.dataset.ahTooltip = binding.value ?? ''
    el.dataset.ahTooltipPlacement = parsePlacement(binding.arg)
    const handlers = {
      // Read placement on enter so dynamic args (e.g. monitor row index) stay fresh.
      onEnter: () => show(
        el,
        el.dataset.ahTooltip || '',
        parsePlacement(el.dataset.ahTooltipPlacement),
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
