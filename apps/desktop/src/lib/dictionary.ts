// Word lookups: offline StarDict dictionaries first, then (optionally)
// Wiktionary. Definitions are always turned into plain text before display.

import { api } from './api'
import type { Definition } from './types'

export interface Sense {
  partOfSpeech: string
  definitions: string[]
}

export interface LookupResult {
  word: string
  source: 'offline' | 'wiktionary' | 'none'
  /** Offline definitions, one per dictionary entry. */
  offline: Definition[]
  /** Wiktionary senses grouped by language name. */
  online: { language: string; senses: Sense[] }[]
  /** Set when the online lookup failed (offline, blocked…). */
  error?: string
}

const WIKTIONARY = 'https://en.wiktionary.org/api/rest_v1/page/definition/'

/** The word to look up from a selection: one to three words, trimmed. */
export function wordFromSelection(text: string): string | null {
  const t = text.replace(/\s+/g, ' ').trim().replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, '')
  if (!t || t.length > 60) return null
  return t.split(' ').length <= 3 ? t : null
}

/** Plain text from dictionary markup, keeping line breaks. Never rendered as HTML. */
export function markupToText(markup: string): string {
  const doc = new DOMParser().parseFromString(
    markup.replace(/<\s*(br|\/p|\/div|\/li|\/dd|\/dt|\/h\d)\s*\/?>/gi, '\n$&'),
    'text/html',
  )
  return (doc.body.textContent ?? '').replace(/[ \t]+\n/g, '\n').replace(/\n{3,}/g, '\n\n').trim()
}

interface WiktionaryEntry {
  partOfSpeech: string
  language: string
  definitions: { definition: string }[]
}

async function wiktionary(word: string, bookLanguage: string | null): Promise<LookupResult['online']> {
  for (const candidate of [word, word.toLowerCase()]) {
    const res = await fetch(WIKTIONARY + encodeURIComponent(candidate.replace(/ /g, '_')), {
      headers: { 'Api-User-Agent': 'Biblion/0.1 (desktop e-book reader)' },
    })
    if (res.status === 404) continue
    if (!res.ok) throw new Error(`Wiktionary answered ${res.status}`)
    const data = (await res.json()) as Record<string, WiktionaryEntry[]>
    const preferred = bookLanguage?.toLowerCase().split(/[-_]/)[0]
    const codes = Object.keys(data).sort((a, b) => (b === preferred ? 1 : 0) - (a === preferred ? 1 : 0))
    return codes.map((code) => ({
      language: data[code][0]?.language ?? code,
      senses: data[code]
        .map((e) => ({
          partOfSpeech: e.partOfSpeech,
          definitions: e.definitions.map((d) => markupToText(d.definition)).filter(Boolean).slice(0, 4),
        }))
        .filter((s) => s.definitions.length),
    })).filter((l) => l.senses.length)
  }
  return []
}

export async function lookup(word: string, opts: { online: boolean; bookLanguage: string | null }): Promise<LookupResult> {
  const offline = await api.define(word).catch(() => [] as Definition[])
  if (offline.length) return { word, source: 'offline', offline, online: [] }
  if (!opts.online) return { word, source: 'none', offline: [], online: [] }
  try {
    const online = await wiktionary(word, opts.bookLanguage)
    return { word, source: online.length ? 'wiktionary' : 'none', offline: [], online }
  } catch (e) {
    return { word, source: 'none', offline: [], online: [], error: e instanceof Error ? e.message : String(e) }
  }
}
