/** True for Codex's hidden heartbeat / automation wrapper, not ordinary user text. */
export function isInternalSessionPrompt(value: string | null | undefined): boolean {
  if (!value) return false
  const text = value.trimStart()
  if (/^<heartbeat(?:\s|>|\/)/i.test(text)) return true
  if (/^<automation_id(?:=|\s|>)/i.test(text) && /<current_time_iso(?:\s|>|\/)/i.test(text)) return true
  if (text.startsWith('# Overview') && text.includes('hyperpersonalized suggestions')) return true
  if (text.includes('Codex ambient suggestions')) return true
  return text.startsWith('## Memory Writing Agent')
}

/**
 * Text shown in compact cards, titles, tooltips, and search-result excerpts.
 * Full transcripts use the original message data; this only cleans previews.
 */
export function compactSessionPreview(value: string | null | undefined): string {
  if (!value || isInternalSessionPrompt(value)) return ''
  return value
    .replace(/<img\b[^>]*\/?>/gi, '[图片]')
    .replace(/<image\b[^>]*>[\s\S]*?<\/image>/gi, '[图片]')
    .replace(/!\[[^\]]*\]\((?:<[^>]*>|[^)]*)\)/g, '[图片]')
    .replace(/data:image\/[\w.+-]+;base64,[a-z0-9+/=]+/gi, '[图片]')
    .trim()
}
