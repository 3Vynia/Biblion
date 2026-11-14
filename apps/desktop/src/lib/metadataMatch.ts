// Which changes a book found on Open Library would make. Nothing is applied
// without the user choosing it.

import type { Metadata, OlCandidate, OlWork } from './types'

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
export function proposedChanges(m: Metadata, c: OlCandidate, work: OlWork | null): Change[] {
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

  const description = work?.description ?? ''
  if (description && !same(m.description ?? '', description))
    changes.push({ field: 'description', label: 'Description', current: m.description ?? '', next: description, suggested: !m.description?.trim() })

  const have = new Set(m.tags.map(norm))
  const added = [...(work?.subjects.length ? work.subjects : c.subjects)].filter((t) => !have.has(norm(t))).slice(0, 8)
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
