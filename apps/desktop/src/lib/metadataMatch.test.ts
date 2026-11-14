import { describe, expect, it } from 'vitest'
import { applyChanges, proposedChanges, rankCandidates, titleLikeness, type FieldId } from './metadataMatch'
import type { MdCandidate, Metadata, SourceResults } from './types'

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

const hobbit: MdCandidate = {
  source: 'openlibrary',
  key: '/works/OL27482W',
  kind: null,
  alt_titles: [],
  volumes: null,
  description: null,
  url: null,
  title: 'The Hobbit',
  subtitle: 'or There and Back Again',
  authors: ['Tolkien, J.R.R.'],
  year: 1937,
  publishers: [],
  isbn: null,
  cover: 'ol:1',
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

describe('results from every source', () => {
  const c = (source: string, title: string, extra: Partial<MdCandidate> = {}): MdCandidate => ({ ...hobbit, source, key: title, title, cover: null, authors: [], ...extra })
  const results = (r: Record<string, MdCandidate[]>): SourceResults[] =>
    Object.entries(r).map(([source, candidates]) => ({ source, label: source, candidates, error: null }))

  it('puts the closest title first, whatever its source', () => {
    const list = rankCandidates(
      results({
        openlibrary: [c('openlibrary', 'Solo Leveling Companion'), c('openlibrary', 'Leveling Up Your Life')],
        anilist: [c('anilist', 'Na Honjaman Level Up', { alt_titles: ['Solo Leveling'], kind: 'Manhwa' })],
      }),
      'Solo Leveling',
      '',
    )
    expect(list.map((x) => x.source)).toEqual(['anilist', 'openlibrary', 'openlibrary'])
  })

  it('prefers a shared author, then a cover', () => {
    const list = rankCandidates(
      results({ a: [c('a', 'Crossed'), c('a', 'Crossed', { cover: 'x' }), c('a', 'Crossed', { authors: ['Garth Ennis'] })] }),
      'Crossed',
      'Ennis',
    )
    expect(list.map((x) => [x.authors.length, x.cover])).toEqual([
      [1, null],
      [0, 'x'],
      [0, null],
    ])
  })

  it('compares titles by their words', () => {
    expect(titleLikeness('The Hobbit', 'the hobbit!')).toBe(1)
    expect(titleLikeness('Crossed', 'Badlands')).toBe(0)
    expect(titleLikeness('', 'x')).toBe(0)
  })

  it('offers the kind of book as a subject', () => {
    const manhwa = { ...hobbit, kind: 'Manhwa', subjects: ['Action', 'manhwa'] }
    const tags = proposedChanges(meta({ tags: [] }), manhwa, null).find((x) => x.field === 'tags')
    expect(tags?.added).toEqual(['Manhwa', 'Action'])
  })
})
