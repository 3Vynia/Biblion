import { describe, expect, it } from 'vitest'
import { applyChanges, proposedChanges, type FieldId } from './metadataMatch'
import type { Metadata, OlCandidate } from './types'

const meta = (m: Partial<Metadata> = {}): Metadata => ({
  title: 'The hobbit',
  authors: [],
  series: null,
  series_index: null,
  publisher: 'Allen & Unwin',
  language: 'en',
  isbn: null,
  published: null,
  description: null,
  tags: ['Fantasy'],
  page_count: 310,
  ...m,
})

const hobbit: OlCandidate = {
  key: '/works/OL27482W',
  title: 'The Hobbit',
  subtitle: 'or There and Back Again',
  authors: ['Tolkien, J.R.R.'],
  year: 1937,
  publishers: [],
  isbn: null,
  cover_id: 1,
  languages: ['en'],
  pages: 310,
  editions: 482,
  subjects: ['fantasy', 'Dragons', 'Hobbits'],
}

describe('proposed changes', () => {
  it('suggests filling what is empty and leaves the rest unticked', () => {
    const changes = proposedChanges(meta(), hobbit, { description: 'A tale.', subjects: ['Fantasy', 'Dragons', 'Dwarves'] })
    const by = Object.fromEntries(changes.map((c) => [c.field, c]))
    // Only the case differs: not a change.
    expect(by.title).toBeUndefined()
    expect(by.subtitle.suggested).toBe(true)
    expect(by.authors).toMatchObject({ next: 'Tolkien, J.R.R.', suggested: true })
    expect(by.published).toMatchObject({ next: '1937', suggested: true })
    expect(by.description).toMatchObject({ next: 'A tale.', suggested: true })
    // Work subjects win over search ones; the book's own are not repeated.
    expect(by.tags).toMatchObject({ added: ['Dragons', 'Dwarves'], suggested: false })
  })

  it('never pre-ticks replacing what the book already has', () => {
    const m = meta({ title: 'Hobbit (illustrated)', authors: ['J. R. R. Tolkien'], published: '1966-01-01', description: 'Mine.' })
    const changes = proposedChanges(m, hobbit, { description: 'Theirs.', subjects: [] })
    for (const c of changes.filter((c) => c.field !== 'subtitle')) expect(c.suggested, c.field).toBe(false)
    expect(proposedChanges(meta({ published: '1937-09-21' }), hobbit, null).some((c) => c.field === 'published')).toBe(false)
  })

  it('applies only the chosen fields, keeping author names whole', () => {
    const m = meta()
    const changes = proposedChanges(m, hobbit, { description: 'A tale.', subjects: ['Dragons'] })
    const out = applyChanges(m, changes, new Set<FieldId>(['authors', 'tags']))
    expect(out.authors).toEqual(['Tolkien, J.R.R.'])
    expect(out.tags).toEqual(['Fantasy', 'Dragons'])
    expect(out.description).toBeNull()
    expect(out.publisher).toBe('Allen & Unwin')
    expect(m.tags).toEqual(['Fantasy'])
  })
})
