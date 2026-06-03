import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  const kb = bytes / 1024
  if (kb < 1024) return `${kb.toFixed(1)} KB`
  const mb = kb / 1024
  if (mb < 1024) return `${mb.toFixed(1)} MB`
  const gb = mb / 1024
  return `${gb.toFixed(2)} GB`
}

export function formatInt(value: number | null | undefined): string {
  const n = Number(value)
  if (!Number.isFinite(n)) return '--'
  return Math.round(n).toLocaleString()
}

export function truncate(str: string, len: number): string {
  return str && str.length > len ? str.substring(0, len) + '...' : str
}

export function avatarToneFromName(name: string): number {
  const s = (name || '').toLowerCase()
  let h = 2166136261 >>> 0
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i)
    h = Math.imul(h, 16777619) >>> 0
  }
  return (h % 8) + 1
}

export function formatSessionTime(timestamp: number | string, locale: string = 'zh-CN'): string {
  const n = Number(timestamp)
  if (!Number.isFinite(n) || n <= 0) return '-'
  const ms = n < 1e12 ? n * 1000 : n
  return new Date(ms).toLocaleString(locale === 'zh-CN' ? 'zh-CN' : 'en-US')
}

export function getErrorMessage(error: any): string {
  if (error == null) return 'Unknown error'
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message || String(error)
  if (typeof error === 'object') {
    for (const key of Object.keys(error)) {
      const value = error[key]
      if (typeof value === 'string' && value.length > 0) return value
    }
    if (typeof error.message === 'string') return error.message
    if (typeof error.error === 'string') return error.error
    try { return JSON.stringify(error) } catch { return String(error) }
  }
  return String(error)
}
