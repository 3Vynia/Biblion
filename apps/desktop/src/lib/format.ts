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

/** MARC relator codes used by EPUB, as readable roles. */
const ROLES: Record<string, [string, string]> = {
  trl: ['Translator', 'Translated by'],
  ill: ['Illustrator', 'Illustrated by'],
  edt: ['Editor', 'Edited by'],
  nrt: ['Narrator', 'Narrated by'],
  aui: ['Introduction', 'Introduction by'],
  aft: ['Afterword', 'Afterword by'],
  ann: ['Annotator', 'Annotated by'],
  art: ['Artist', 'Art by'],
  cov: ['Cover designer', 'Cover by'],
  pht: ['Photographer', 'Photographs by'],
  com: ['Compiler', 'Compiled by'],
  bkp: ['Producer', 'Produced by'],
  pbl: ['Publisher', 'Published by'],
  ctb: ['Contributor', 'With contributions by'],
}

export function roleLabel(code: string): string {
  return ROLES[code.toLowerCase()]?.[1] ?? (code ? `${code.charAt(0).toUpperCase()}${code.slice(1)}` : 'Contributor')
}

/** "es" → "Spanish", "pt-BR" → "Portuguese (Brazil)"; unknown codes are returned as-is. */
export function languageName(code: string | null | undefined): string {
  if (!code) return ''
  try {
    return new Intl.DisplayNames(['en'], { type: 'language' }).of(code.trim()) ?? code
  } catch {
    return code
  }
}

/** "2007-03-27" → "March 27, 2007"; "2007" stays "2007"; free text is kept. */
export function formatPublished(value: string | null | undefined): string {
  if (!value) return ''
  const v = value.trim()
  if (/^\d{4}$/.test(v)) return v
  const m = /^(\d{4})-(\d{2})(?:-(\d{2}))?/.exec(v)
  if (!m) return v
  const [, y, mo, d] = m
  const date = new Date(Date.UTC(Number(y), Number(mo) - 1, d ? Number(d) : 1))
  if (Number.isNaN(date.getTime())) return v
  return date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', ...(d ? { day: 'numeric' } : {}), timeZone: 'UTC' })
}

const ID_LABELS: Record<string, string> = {
  isbn: 'ISBN',
  uuid: 'UUID',
  asin: 'ASIN',
  doi: 'DOI',
  calibre: 'Calibre',
  google: 'Google Books',
}

export const identifierLabel = (scheme: string) => ID_LABELS[scheme] ?? scheme.toUpperCase()
