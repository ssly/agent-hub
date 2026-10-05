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

export interface SearchSnippetMatch {
  location: 'content' | 'thinking' | 'system'
  locationLabelKey: 'session.thinking' | 'session.system_reminder' | ''
  snippet: string
  matchText: string
  index: number
}

/**
 * Extracts compact context snippet(s) around matching query occurrences.
 * Limits snippet length to around 160 characters to avoid huge text blocks.
 */
export function extractSearchSnippets(
  message: { content?: string; thinking?: string | null; system?: string | null },
  query: string,
  maxSnippetsPerMessage = 4,
): SearchSnippetMatch[] {
  if (!query || !query.trim()) return []
  const q = query.trim().toLowerCase()
  const results: SearchSnippetMatch[] = []

  const sources: Array<{
    text: string
    location: 'content' | 'thinking' | 'system'
    labelKey: 'session.thinking' | 'session.system_reminder' | ''
  }> = [
    { text: compactSessionPreview(message.content), location: 'content', labelKey: '' },
    { text: message.thinking ? compactSessionPreview(message.thinking) : '', location: 'thinking', labelKey: 'session.thinking' },
    { text: message.system ? compactSessionPreview(message.system) : '', location: 'system', labelKey: 'session.system_reminder' },
  ]

  for (const src of sources) {
    if (!src.text) continue
    const lower = src.text.toLowerCase()
    let lastIndex = 0
    let matchIdx = lower.indexOf(q, lastIndex)

    while (matchIdx !== -1 && results.length < maxSnippetsPerMessage) {
      const beforeChars = 60
      const afterChars = 80
      let start = Math.max(0, matchIdx - beforeChars)
      let end = Math.min(src.text.length, matchIdx + q.length + afterChars)

      if (start > 0) {
        const spaceIdx = src.text.indexOf(' ', start)
        if (spaceIdx !== -1 && spaceIdx < start + 15 && spaceIdx < matchIdx) {
          start = spaceIdx + 1
        }
      }
      if (end < src.text.length) {
        const spaceIdx = src.text.lastIndexOf(' ', end)
        if (spaceIdx !== -1 && spaceIdx > end - 15 && spaceIdx > matchIdx + q.length) {
          end = spaceIdx
        }
      }

      const snippetStr =
        (start > 0 ? '... ' : '') +
        src.text.slice(start, end).replace(/\s+/g, ' ').trim() +
        (end < src.text.length ? ' ...' : '')

      const rawMatch = src.text.slice(matchIdx, matchIdx + q.length)

      results.push({
        location: src.location,
        locationLabelKey: src.labelKey,
        snippet: snippetStr,
        matchText: rawMatch,
        index: matchIdx,
      })

      lastIndex = Math.max(matchIdx + q.length, end - 20)
      matchIdx = lower.indexOf(q, lastIndex)
    }
  }

  // Fallback: If nothing matched, provide the start of the content
  if (results.length === 0 && message.content) {
    const preview = compactSessionPreview(message.content).replace(/\s+/g, ' ').trim()
    results.push({
      location: 'content',
      locationLabelKey: '',
      snippet: preview.slice(0, 140) + (preview.length > 140 ? ' ...' : ''),
      matchText: '',
      index: 0,
    })
  }

  return results
}

