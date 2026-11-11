// Mirrors the serde types in crates/biblion-core/src/model.rs and scanner.rs.

export type BookId = number

export type Format = 'epub' | 'pdf' | 'cbz' | 'cbr' | 'mobi' | 'azw3'

export type SortBy = 'title' | 'author' | 'series' | 'added_desc' | 'recently_read' | 'relevance'

export interface BookQuery {
  text?: string | null
  author_id?: number | null
  series?: string | null
  tag?: string | null
  shelf_id?: number | null
  format?: Format | null
  favorites_only?: boolean
  reading_only?: boolean
  finished_only?: boolean
  language?: string | null
  include_missing?: boolean
  sort?: SortBy
  offset: number
  limit: number
}

export interface Page<T> {
  items: T[]
  total: number
  offset: number
}

export interface BookSummary {
  id: BookId
  title: string
  authors: string[]
  series: string | null
  series_index: number | null
  format: Format
  has_cover: boolean
  cover_key: string | null
  progress: number | null
  favorite: boolean
}

export interface Metadata {
  title: string | null
  authors: string[]
  series: string | null
  series_index: number | null
  publisher: string | null
  language: string | null
  isbn: string | null
  published: string | null
  description: string | null
  tags: string[]
  page_count: number | null
  subtitle?: string | null
  contributors?: { name: string; role: string }[]
  identifiers?: { scheme: string; value: string }[]
  rights?: string | null
  source?: string | null
  modified?: string | null
  format_version?: string | null
}

export interface BookStats {
  words: number
  characters: number
  chapters: number
  images: number
  reading_minutes: number
}

export interface ReadingProgress {
  locator: string
  percent: number
  updated_at: number
  finished: boolean
}

export interface Book {
  id: BookId
  path: string
  format: Format
  size: number
  fingerprint: string
  metadata: Metadata
  has_cover: boolean
  cover_key: string | null
  added_at: number
  updated_at: number
  missing: boolean
  favorite: boolean
  rating: number | null
  progress: ReadingProgress | null
}

export interface AuthorEntry {
  id: number
  name: string
  book_count: number
}

export interface TagEntry {
  id: number
  name: string
  book_count: number
}

export interface Shelf {
  id: number
  name: string
  book_count: number
}

export interface LibraryStats {
  books: number
  missing: number
  authors: number
  series: number
  with_cover: number
  total_bytes: number
  by_format: [Format, number][]
}

export interface ScanReport {
  found: number
  unchanged: number
  added: number
  updated: number
  missing: number
  errors: number
  covers: number
  elapsed_ms: number
  error_samples: [string, string][]
}

export type ScanEvent =
  | { kind: 'walking'; found: number }
  | { kind: 'processing'; done: number; total: number }
  | { kind: 'finished'; report: ScanReport }

/** Error returned by every command (see src-tauri/src/commands.rs). */
export interface CmdError {
  code: string
  message: string
}
