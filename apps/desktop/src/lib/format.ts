// Small display helpers.

import type { Format } from './types'

export const FORMAT_LABEL: Record<Format, string> = {
  epub: 'EPUB',
  pdf: 'PDF',
  mobi: 'MOBI',
  azw3: 'AZW3',
  cbz: 'CBZ',
  cbr: 'CBR',
}

/** Formats the built-in reader can open in this version. */
export const READABLE: ReadonlySet<Format> = new Set(['epub', 'mobi', 'azw3'])

export function formatBytes(n: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let i = 0
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024
    i++
  }
  return `${n.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

export function formatCount(n: number): string {
  return n.toLocaleString('en-US')
}

export function formatDuration(ms: number): string {
  const s = Math.round(ms / 1000)
  if (s < 60) return `${s} s`
  return `${Math.floor(s / 60)} min ${s % 60} s`
}

export function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' })
}

/** Stable colour for books without a cover, derived from the title. */
export function placeholderHue(text: string): number {
  let h = 0
  for (let i = 0; i < text.length; i++) h = (h * 31 + text.charCodeAt(i)) | 0
  return Math.abs(h) % 360
}

/** Plain text from an OPF description, which may contain HTML. Never rendered as HTML. */
export function plainText(html: string): string {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  return (doc.body.textContent ?? '').replace(/\s+\n/g, '\n').trim()
}
