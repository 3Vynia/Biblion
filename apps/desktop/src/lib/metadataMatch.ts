// Books found online, from every source mixed into one list, and which
// changes picking one would make. Nothing is applied without the user
// choosing it.

import type { MdCandidate, MdDetails, Metadata, SourceResults } from './types'

export type FieldId = 'title' | 'subtitle' | 'authors' | 'published' | 'description' | 'tags'

export interface Change {
  field: FieldId
  label: string
  current: string
  next: string
  /** Tags only: the ones the book does not have yet. */
  added?: string[]
  /** Authors only: the names, as a list. */
  names?: string[]
  /** Pre-ticked: fills something empty, never replaces what is there. */
  suggested: boolean
}

const same = (a: string, b: string) => norm(a) === norm(b)
const norm = (s: string) =>
  s
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim()

const yearOf = (published: string | null) => published?.match(/\d{4}/)?.[0] ?? ''

/** What applying this Open Library book could change, field by field. */
export function proposedChanges(m: Metadata, c: MdCandidate, work: MdDetails | null): Change[] {
  const changes: Change[] = []
  const title = m.title ?? ''
  if (c.title && !same(title, c.title)) changes.push({ field: 'title', label: 'Title', current: title, next: c.title, suggested: !title })

  const subtitle = m.subtitle ?? ''
  if (c.subtitle && !same(subtitle, c.subtitle))
    changes.push({ field: 'subtitle', label: 'Subtitle', current: subtitle, next: c.subtitle, suggested: !subtitle })

  const authors = m.authors.join(', ')
  if (c.authors.length && !same(authors, c.authors.join(', ')))
    changes.push({ field: 'authors', label: c.authors.length > 1 ? 'Authors' : 'Author', current: authors, next: c.authors.join(', '), names: c.authors, suggested: !authors })

  const year = yearOf(m.published)
  if (c.year && String(c.year) !== year)
    changes.push({ field: 'published', label: 'First published', current: m.published ?? '', next: String(c.year), suggested: !year })

  const description = c.description ?? work?.description ?? ''
  if (description && !same(m.description ?? '', description))
    changes.push({ field: 'description', label: 'Description', current: m.description ?? '', next: description, suggested: !m.description?.trim() })

  const have = new Set(m.tags.map(norm))
  // What it is ("Manhwa") is a subject too: it is how such books are browsed.
  const offered = [...(c.kind ? [c.kind] : []), ...(work?.subjects.length ? work.subjects : c.subjects)]
  const added = offered.filter((t, i) => !have.has(norm(t)) && offered.findIndex((o) => norm(o) === norm(t)) === i).slice(0, 8)
  if (added.length)
    changes.push({ field: 'tags', label: 'Subjects', current: m.tags.join(', '), next: [...m.tags, ...added].join(', '), added, suggested: m.tags.length === 0 })

  return changes
}

/** The metadata with the chosen changes applied. */
export function applyChanges(m: Metadata, changes: Change[], chosen: Set<FieldId>): Metadata {
  const out: Metadata = { ...m, authors: [...m.authors], tags: [...m.tags] }
  for (const ch of changes) {
    if (!chosen.has(ch.field)) continue
    if (ch.field === 'title') out.title = ch.next
    else if (ch.field === 'subtitle') out.subtitle = ch.next
    else if (ch.field === 'authors') out.authors = [...(ch.names ?? [])]
    else if (ch.field === 'published') out.published = ch.next
    else if (ch.field === 'description') out.description = ch.next
    else if (ch.field === 'tags') out.tags = [...m.tags, ...(ch.added ?? [])]
  }
  return out
}

const words = (s: string) => new Set(norm(s).split(' ').filter((w) => w.length > 1))

/** How alike two titles are, 0 to 1: shared words over all words. */
export function titleLikeness(a: string, b: string): number {
  const x = words(a)
  const y = words(b)
  if (!x.size || !y.size) return 0
  let shared = 0
  for (const w of x) if (y.has(w)) shared++
  return shared / (x.size + y.size - shared)
}

/**
 * Every source's matches in one list, best first: the closest title (in any
 * of its names), then a shared author, then a cover. Ties keep each source's
 * own order, so sources interleave instead of one burying the other.
 */
export function rankCandidates(results: SourceResults[], title: string, author: string): MdCandidate[] {
  const authorWords = words(author)
  const score = (c: MdCandidate, i: number) => {
    const t = Math.max(titleLikeness(title, c.title), ...c.alt_titles.map((a) => titleLikeness(title, a)))
    const byAuthor = authorWords.size > 0 && c.authors.some((a) => [...words(a)].some((w) => authorWords.has(w)))
    return t * 10 + (byAuthor ? 3 : 0) + (c.cover ? 0.5 : 0) - i * 0.05
  }
  return results
    .flatMap((r) => r.candidates.map((c, i) => ({ c, s: score(c, i) })))
    .sort((a, b) => b.s - a.s)
    .map((x) => x.c)
}
