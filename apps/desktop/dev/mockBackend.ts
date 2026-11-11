// In-memory imitation of the Tauri commands in src-tauri/src/commands.rs.
// Filters and sort orders follow crates/biblion-core/src/db.rs.
import type { Book, BookQuery, BookSummary, Format, Shelf } from '../src/lib/types'

const FORMATS: Format[] = ['epub', 'epub', 'epub', 'pdf', 'mobi', 'azw3', 'cbz', 'cbr']
const WORDS = ['Abyss', 'Garden', 'Storm', 'River', 'Night', 'Island', 'Wolf', 'Silence', 'Border', 'Mirror', 'Labyrinth', 'Salt']
const AUTHORS = ['Ana Paz', 'Bruno Rulfo', 'Clara Neruda', 'David Le Guin', 'Elena Borges', 'Félix Mistral', 'Gema Cortázar']
const NOW = 1_800_000_000

interface Row extends BookSummary {
  added_at: number
  read_at: number | null
  finished: boolean
}

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
  }
})
const shelves: Shelf[] = [{ id: 1, name: 'To read', book_count: 0 }]
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
  const items = list.slice(q.offset, q.offset + q.limit).map(({ added_at: _a, read_at: _r, finished: _f, ...s }) => s)
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
      language: 'en',
      isbn: null,
      published: '2019-04-12',
      description: 'A mock book used to test the interface.',
      tags: ['Fiction'],
      page_count: null,
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

let readerSettings: unknown = null
let uiSettings: unknown = null
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
    default:
      console.warn('mock: unhandled command', cmd, args)
      return null
  }
}
