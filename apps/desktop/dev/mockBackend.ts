// In-memory imitation of the Tauri commands in src-tauri/src/commands.rs.
// Filters and sort orders follow crates/biblion-core/src/db.rs.
import type { Annotation, AnnotationQuery, Book, BookQuery, BookSummary, Format, NewAnnotation, Shelf } from '../src/lib/types'

const FORMATS: Format[] = ['epub', 'epub', 'epub', 'pdf', 'mobi', 'azw3', 'cbz', 'cbr']
const WORDS = ['Abyss', 'Garden', 'Storm', 'River', 'Night', 'Island', 'Wolf', 'Silence', 'Border', 'Mirror', 'Labyrinth', 'Salt']
const AUTHORS = ['Ana Paz', 'Bruno Rulfo', 'Clara Neruda', 'David Le Guin', 'Elena Borges', 'Félix Mistral', 'Gema Cortázar']
const NOW = 1_800_000_000

interface Row extends BookSummary {
  added_at: number
  read_at: number | null
  finished: boolean
  tags: string[]
  language: string
}

const SUBJECTS = ['Science Fiction', 'History', 'Poetry', 'Fantasy', 'Mystery', 'Philosophy']
const LANGUAGES = ['en', 'en', 'es', 'fr', 'de']

const rows: Row[] = Array.from({ length: 3000 }, (_, i) => {
  const reading = i % 37 === 0
  const done = i % 53 === 0
  return {
    id: i + 1,
    title: `${WORDS[(i * 7) % WORDS.length]} of the ${WORDS[(i * 3) % WORDS.length].toLowerCase()} ${i + 1}`,
    authors: [AUTHORS[i % AUTHORS.length]],
    series: i % 11 === 0 ? `Saga ${WORDS[i % WORDS.length]}` : null,
    series_index: i % 11 === 0 ? (i % 5) + 1 : null,
    format: FORMATS[i % FORMATS.length],
    has_cover: false,
    cover_key: null,
    progress: done ? 1 : reading ? ((i * 13) % 90) / 100 + 0.05 : null,
    favorite: i % 29 === 0,
    added_at: NOW - i * 3600,
    read_at: reading || done ? NOW - ((i * 977) % 100000) : null,
    finished: done,
    tags: [SUBJECTS[i % SUBJECTS.length], ...(i % 4 === 0 ? [SUBJECTS[(i + 1) % SUBJECTS.length]] : [])],
    language: LANGUAGES[i % LANGUAGES.length],
  }
})
const shelves: Shelf[] = [{ id: 1, name: 'To read', book_count: 0 }]

/** Format of a mock book, so the harness can serve the right sample file. */
export const formatOf = (id: number): Format | undefined => rows[id - 1]?.format
const shelfBooks = new Map<number, Set<number>>([[1, new Set()]])

const cmp = (a: string | number | null, b: string | number | null) =>
  a === b ? 0 : a === null ? 1 : b === null ? -1 : a < b ? -1 : 1

function query(q: BookQuery) {
  let list = rows.slice()
  const text = q.text?.trim().toLowerCase()
  if (text) list = list.filter((r) => `${r.title} ${r.authors.join(' ')} ${r.series ?? ''}`.toLowerCase().includes(text))
  if (q.author_id) list = list.filter((r) => AUTHORS.indexOf(r.authors[0]) + 1 === q.author_id)
  if (q.series) list = list.filter((r) => r.series?.toLowerCase() === q.series!.toLowerCase())
  if (q.format) list = list.filter((r) => r.format === q.format)
  if (q.tag) list = list.filter((r) => r.tags.some((t) => t.toLowerCase() === q.tag!.toLowerCase()))
  if (q.language) list = list.filter((r) => r.language === q.language!.toLowerCase())
  if (q.shelf_id) list = list.filter((r) => shelfBooks.get(q.shelf_id!)?.has(r.id))
  if (q.favorites_only) list = list.filter((r) => r.favorite)
  if (q.reading_only) list = list.filter((r) => r.read_at !== null && !r.finished)
  if (q.finished_only) list = list.filter((r) => r.finished)
  const by: Record<string, (a: Row, b: Row) => number> = {
    title: (a, b) => cmp(a.title.toLowerCase(), b.title.toLowerCase()),
    author: (a, b) => cmp(a.authors[0], b.authors[0]) || cmp(a.title, b.title),
    series: (a, b) => cmp(a.series, b.series) || cmp(a.series_index, b.series_index) || cmp(a.title, b.title),
    added_desc: (a, b) => b.added_at - a.added_at || b.id - a.id,
    recently_read: (a, b) => cmp(a.read_at === null ? 1 : 0, b.read_at === null ? 1 : 0) || (b.read_at ?? 0) - (a.read_at ?? 0),
    relevance: (a, b) => cmp(a.title, b.title),
  }
  list.sort(by[q.sort ?? 'title'])
  const items = list
    .slice(q.offset, q.offset + q.limit)
    .map(({ added_at: _a, read_at: _r, finished: _f, tags: _t, language: _l, ...s }) => s)
  return { items, total: list.length, offset: q.offset }
}

function book(id: number): Book {
  const r = rows[id - 1]
  if (!r) throw { code: 'not_found', message: `not found: book ${id}` }
  return {
    id: r.id,
    path: `/books/${r.authors[0]}/${r.title}.${r.format}`,
    format: r.format,
    size: 1_234_567,
    fingerprint: `fp${id}`,
    metadata: {
      title: r.title,
      authors: r.authors,
      series: r.series,
      series_index: r.series_index,
      publisher: 'Mock Press',
      language: r.language,
      isbn: null,
      published: '2019-04-12',
      description: 'A mock book used to test the interface.',
      tags: r.tags,
      page_count: null,
      subtitle: id % 2 ? 'A Novel' : null,
      contributors: [{ name: 'Tess Translator', role: 'trl' }],
      identifiers: [
        { scheme: 'isbn', value: '978-84-01-35230-9' },
        { scheme: 'uuid', value: '7c9e6679-7425-40de-944b-e07fc1f90ae7' },
      ],
      rights: 'All rights reserved',
      source: null,
      modified: '2020-01-15T10:00:00Z',
      format_version: 'EPUB 3.0',
    },
    has_cover: false,
    cover_key: null,
    added_at: r.added_at,
    updated_at: r.added_at,
    missing: false,
    favorite: r.favorite,
    rating: null,
    progress: r.progress === null ? null : { locator: '', percent: r.progress, updated_at: r.read_at ?? NOW, finished: r.finished },
  }
}

const annotations: Annotation[] = []
let nextAnnotation = 1
let clock = NOW

let dictionaries = [{ id: 'en/test.ifo', name: 'Test English Dictionary', word_count: 2 }]
const DEFINITIONS: Record<string, string> = {
  keeper: 'A person who looks after something, such as a lighthouse.',
  logbook: 'A book in which a record of events is kept.',
}

let readerSettings: unknown = null
let uiSettings: unknown = null
let ambienceSettings: unknown = null
let watchFolders = true
declare global {
  interface Window {
    __calls: { cmd: string; args: Record<string, unknown> }[]
  }
}
window.__calls = []

export function handle(cmd: string, args: Record<string, unknown>): unknown {
  window.__calls.push({ cmd, args })
  const count = (f: (r: Row) => boolean) => rows.filter(f).length
  switch (cmd) {
    case 'get_roots':
      return ['/books']
    case 'add_root':
    case 'remove_root':
      return ['/books']
    case 'is_scanning':
      return false
    case 'scan':
      return { found: rows.length, unchanged: rows.length, added: 0, updated: 0, missing: 0, errors: 0, covers: 0, elapsed_ms: 900, error_samples: [] }
    case 'cancel_scan':
      return null
    case 'list_books':
      return query(args.query as BookQuery)
    case 'get_book':
      return book(args.id as number)
    case 'stats':
      return {
        books: rows.length,
        missing: 0,
        authors: AUTHORS.length,
        series: new Set(rows.map((r) => r.series).filter(Boolean)).size,
        with_cover: 0,
        total_bytes: rows.length * 1_234_567,
        by_format: (['epub', 'pdf', 'mobi', 'azw3', 'cbz', 'cbr'] as Format[]).map((f) => [f, count((r) => r.format === f)]),
      }
    case 'list_authors': {
      const prefix = ((args.prefix as string | null) ?? '').toLowerCase()
      const items = AUTHORS.map((name, i) => ({ id: i + 1, name, book_count: count((r) => r.authors[0] === name) })).filter((a) =>
        a.name.toLowerCase().startsWith(prefix),
      )
      return { items: items.slice(args.offset as number, (args.offset as number) + (args.limit as number)), total: items.length, offset: args.offset }
    }
    case 'list_series': {
      const names = [...new Set(rows.map((r) => r.series).filter((s): s is string => !!s))].sort()
      const items = names.map((n) => [n, count((r) => r.series === n)])
      return { items: items.slice(args.offset as number, (args.offset as number) + (args.limit as number)), total: items.length, offset: args.offset }
    }
    case 'set_favorite':
      rows[(args.id as number) - 1].favorite = args.favorite as boolean
      return null
    case 'list_shelves':
      return shelves.map((s) => ({ ...s, book_count: shelfBooks.get(s.id)?.size ?? 0 }))
    case 'create_shelf': {
      const id = shelves.length + 1
      shelves.push({ id, name: args.name as string, book_count: 0 })
      shelfBooks.set(id, new Set())
      return id
    }
    case 'set_in_shelf': {
      const set = shelfBooks.get(args.shelf as number)!
      if (args.member) set.add(args.book as number)
      else set.delete(args.book as number)
      return null
    }
    case 'remove_from_shelf': {
      const set = shelfBooks.get(args.shelf as number)
      if (!set) throw { code: 'not_found', message: 'not found' }
      let removed = 0
      for (const id of args.books as number[]) removed += set.delete(id) ? 1 : 0
      return removed
    }
    case 'rename_shelf': {
      const s = shelves.find((x) => x.id === args.id)
      if (!s) throw { code: 'not_found', message: 'not found' }
      if (shelves.some((x) => x !== s && x.name.toLowerCase() === (args.name as string).toLowerCase()))
        throw { code: 'already_exists', message: 'A collection with that name already exists' }
      s.name = args.name as string
      return null
    }
    case 'delete_shelf': {
      const i = shelves.findIndex((x) => x.id === args.id)
      if (i < 0) throw { code: 'not_found', message: 'not found' }
      shelves.splice(i, 1)
      shelfBooks.delete(args.id as number)
      return null
    }
    case 'list_tags': {
      const prefix = ((args.prefix as string | null) ?? '').toLowerCase()
      const items = SUBJECTS.filter((s) => s.toLowerCase().startsWith(prefix))
        .sort()
        .map((name, i) => ({ id: i + 1, name, book_count: count((r) => r.tags.includes(name)) }))
      return { items: items.slice(args.offset as number, (args.offset as number) + (args.limit as number)), total: items.length, offset: args.offset }
    }
    case 'list_languages': {
      const counts = new Map<string, number>()
      for (const r of rows) counts.set(r.language, (counts.get(r.language) ?? 0) + 1)
      return [...counts].sort((a, b) => b[1] - a[1])
    }
    case 'book_shelves':
      return [...shelfBooks].filter(([, s]) => s.has(args.id as number)).map(([id]) => id)
    case 'get_reader_settings':
      return readerSettings
    case 'set_reader_settings':
      readerSettings = args.value
      return null
    case 'get_ui_settings':
      return uiSettings
    case 'set_ui_settings':
      uiSettings = args.value
      return null
    case 'set_progress':
      return null
    case 'get_watch_folders':
      return watchFolders
    case 'set_watch_folders':
      watchFolders = args.on as boolean
      return null
    case 'get_ambience_settings':
      return ambienceSettings
    case 'set_ambience_settings':
      ambienceSettings = args.value
      return null
    case 'add_annotation': {
      const a = args.annotation as NewAnnotation
      const note = a.note?.trim() || null
      annotations.push({ ...a, note, id: nextAnnotation, created_at: ++clock, updated_at: clock })
      return nextAnnotation++
    }
    case 'update_annotation': {
      const a = annotations.find((x) => x.id === args.id)
      if (!a) throw { code: 'not_found', message: 'not found' }
      if (args.note !== null && args.note !== undefined) a.note = (args.note as string).trim() || null
      if (args.color) a.color = args.color as string
      a.updated_at = ++clock
      return null
    }
    case 'delete_annotation': {
      const i = annotations.findIndex((x) => x.id === args.id)
      if (i < 0) throw { code: 'not_found', message: 'not found' }
      annotations.splice(i, 1)
      return null
    }
    case 'book_annotations':
      return annotations.filter((a) => a.book_id === args.id).sort((a, b) => a.percent - b.percent || a.id - b.id)
    case 'list_annotations': {
      const q = args.query as AnnotationQuery
      const text = q.text?.trim().toLowerCase()
      const list = annotations
        .filter((a) => (q.book_id ? a.book_id === q.book_id : true))
        .filter((a) => (q.kind ? a.kind === q.kind : true))
        .filter((a) => (q.color ? a.color === q.color : true))
        .filter((a) => (text ? `${a.text} ${a.note ?? ''}`.toLowerCase().includes(text) : true))
        .sort((a, b) => b.created_at - a.created_at)
      const items = list.slice(q.offset, q.offset + q.limit).map((a) => {
        const r = rows[a.book_id - 1]
        return { annotation: a, book_title: r.title, book_authors: r.authors, cover_key: null, format: r.format }
      })
      return { items, total: list.length, offset: q.offset }
    }
    case 'set_cover': {
      const row = rows[(args.id as number) - 1]
      row.has_cover = true
      return `cover-${row.id}`
    }
    case 'books_without_cover':
      // Only when a test asks for it: otherwise every test would render 375 covers.
      if (!(window as unknown as { __mockCoverQueue?: boolean }).__mockCoverQueue) return []
      return rows.filter((r) => r.format === args.format && !r.has_cover).slice(0, args.limit as number).map((r) => r.id)
    case 'comic_pages':
      return Array.from({ length: 7 }, (_, i) => `page-${String(i + 1).padStart(2, '0')}.jpg`)
    case 'list_dictionaries':
      return dictionaries
    case 'define': {
      const word = (args.word as string).toLowerCase().replace(/s$/, '')
      const body = dictionaries.length ? DEFINITIONS[word] : undefined
      return body ? [{ dictionary: dictionaries[0].name, word, body, markup: false }] : []
    }
    case 'install_dictionary':
      dictionaries = [...dictionaries, { id: 'new/new.ifo', name: 'Installed Dictionary', word_count: 1000 }]
      return dictionaries
    case 'remove_dictionary':
      dictionaries = dictionaries.filter((d) => d.id !== args.id)
      return dictionaries
    case 'annotation_counts':
      return [annotations.filter((a) => a.kind === 'highlight').length, annotations.filter((a) => a.kind === 'bookmark').length]
    case 'analyze_book':
      return { words: 98_765, characters: 512_000, chapters: 31, images: 4, reading_minutes: 396 }
    case 'data_dir':
      return '/home/reader/.local/share/io.github.3vynia.biblion'
    default:
      console.warn('mock: unhandled command', cmd, args)
      return null
  }
}
