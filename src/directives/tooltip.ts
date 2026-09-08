import type { Directive } from 'vue'

/**
 * v-tooltip — styled replacement for the native `title` attribute.
 *
 * Usage:
 *   v-tooltip="text"
 *   v-tooltip:bottom / :top / :left / :right
 *   v-tooltip.clamp="text"          — clamp body to 3 lines + ellipsis
 *   v-tooltip:top.clamp="text"
 *   v-tooltip="{ text, clamp, placement }"
 *
 * Dynamic args work (`v-tooltip:[side]="text"`). Empty value shows nothing.
 * A single tooltip element is shared app-wide (appended to <body>); colors
 * invert with the theme via --ink / --canvas (styles in theme.css § Tooltip).
 */

export const DEFAULT_TOOLTIP_MAX_LINES = 12

export type TooltipPlacement = 'bottom' | 'left' | 'right' | 'top'
export type TooltipValue =
  | string
  | {
      text?: string
      clamp?: boolean | number
      maxLines?: number
      placement?: TooltipPlacement
    }
  | undefined

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

function usedLineHeight(style: CSSStyleDeclaration): number {
  const raw = parseFloat(style.lineHeight)
  if (Number.isFinite(raw) && raw >= 8) return raw
  const font = parseFloat(style.fontSize)
  return (Number.isFinite(font) && font > 0 ? font : 12) * 1.45
}

function clampToLines(tip: HTMLElement, text: string, maxLines: number) {
  if (maxLines <= 0) {
    tip.textContent = text
    return
  }
  const style = getComputedStyle(tip)
  const lineHeight = usedLineHeight(style)
  const paddingTop = parseFloat(style.paddingTop) || 0
  const paddingBottom = parseFloat(style.paddingBottom) || 0
  const limit = lineHeight * maxLines + paddingTop + paddingBottom

  const normalised = text.replace(/\r\n/g, '\n').replace(/\n{3,}/g, '\n\n').trim()
  tip.textContent = normalised
  if (tip.scrollHeight <= limit + 1) return

  let lo = 0
  let hi = normalised.length
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    let cut = mid
    if (cut > 0 && cut < normalised.length) {
      const code = normalised.charCodeAt(cut - 1)
      if (code >= 0xd800 && code <= 0xdbff) cut--
    }
    tip.textContent = `${normalised.slice(0, cut).trimEnd()}…`
    if (tip.scrollHeight <= limit + 1) {
      lo = mid
    } else {
      hi = mid - 1
    }
  }

  let finalCut = lo
  if (finalCut > 0 && finalCut < normalised.length) {
    const code = normalised.charCodeAt(finalCut - 1)
    if (code >= 0xd800 && code <= 0xdbff) finalCut--
  }
  tip.textContent = finalCut > 0 ? `${normalised.slice(0, finalCut).trimEnd()}…` : '…'
}

function capWidth(margin: number): number {
  const root = parseFloat(getComputedStyle(document.documentElement).fontSize)
  const rem = (Number.isFinite(root) && root > 0 ? root : 16) * 26
  return Math.min(rem, window.innerWidth - margin * 2)
}

function show(
  el: HTMLElement,
  text: string,
  placement: TooltipPlacement,
  maxLines: number,
) {
  if (!text) return
  const tip = ensureTip()
  const isClamped = maxLines > 0
  tip.classList.toggle('is-clamped', isClamped)
  if (isClamped) {
    tip.style.setProperty('--tooltip-max-lines', String(maxLines))
  } else {
    tip.style.removeProperty('--tooltip-max-lines')
  }
  tip.classList.add('is-visible')

  const margin = 16

  tip.style.left = '0px'
  tip.style.top = '0px'
  tip.style.maxWidth = `${capWidth(margin)}px`
  if (isClamped) {
    clampToLines(tip, text, maxLines)
  } else {
    tip.textContent = text
  }

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
    if (isClamped) {
      clampToLines(tip, text, maxLines)
    }
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

type TooltipElement = HTMLElement & {
  __ahTooltip__?: { onEnter: () => void; onLeave: () => void }
  __ahTooltipConfig__?: { text: string; placement: TooltipPlacement; maxLines: number }
}

function parsePlacement(arg: string | undefined): TooltipPlacement {
  if (arg === 'left' || arg === 'right' || arg === 'top' || arg === 'bottom') return arg
  return 'bottom'
}

function resolveBinding(
  value: TooltipValue,
  arg: string | undefined,
  clampModifier: boolean,
): { text: string; placement: TooltipPlacement; maxLines: number } {
  if (value && typeof value === 'object') {
    let maxLines = DEFAULT_TOOLTIP_MAX_LINES
    if (typeof value.maxLines === 'number') {
      maxLines = Math.max(0, value.maxLines)
    } else if (typeof value.clamp === 'number') {
      maxLines = Math.max(0, value.clamp)
    } else if (value.clamp === false) {
      maxLines = 0
    } else if (value.clamp === true || clampModifier) {
      maxLines = DEFAULT_TOOLTIP_MAX_LINES
    }
    return {
      text: value.text ?? '',
      placement: value.placement ?? parsePlacement(arg),
      maxLines,
    }
  }
  return {
    text: value ?? '',
    placement: parsePlacement(arg),
    maxLines: DEFAULT_TOOLTIP_MAX_LINES,
  }
}

export const vTooltip: Directive<TooltipElement, TooltipValue> = {
  mounted(el, binding) {
    el.__ahTooltipConfig__ = resolveBinding(binding.value, binding.arg, !!binding.modifiers.clamp)
    const handlers = {
      onEnter: () => {
        const cfg = el.__ahTooltipConfig__
        if (!cfg) return
        show(el, cfg.text, cfg.placement, cfg.maxLines)
      },
      onLeave: hide,
    }
    el.__ahTooltip__ = handlers
    el.addEventListener('mouseenter', handlers.onEnter)
    el.addEventListener('mouseleave', handlers.onLeave)
  },
  updated(el, binding) {
    el.__ahTooltipConfig__ = resolveBinding(binding.value, binding.arg, !!binding.modifiers.clamp)
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
