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

export type AnnotationKind = 'highlight' | 'bookmark'

export interface Annotation {
  id: number
  book_id: BookId
  kind: AnnotationKind
  /** EPUB CFI (range for highlights, point for bookmarks). */
  locator: string
  text: string
  note: string | null
  color: string | null
  chapter: string | null
  percent: number
  created_at: number
  updated_at: number
}

export type NewAnnotation = Omit<Annotation, 'id' | 'created_at' | 'updated_at'>

export interface AnnotationQuery {
  book_id?: BookId | null
  kind?: AnnotationKind | null
  color?: string | null
  text?: string | null
  offset: number
  limit: number
}

export interface AnnotationEntry {
  annotation: Annotation
  book_title: string
  book_authors: string[]
  cover_key: string | null
  format: Format
}

export interface DictionaryInfo {
  /** Path of the .ifo inside the dictionaries folder. */
  id: string
  name: string
  word_count: number
}

export interface Definition {
  dictionary: string
  word: string
  body: string
  /** Body is HTML-like markup; it is always shown as plain text. */
  markup: boolean
}

// ── Library health (crates/biblion-core/src/health.rs) ──

export type DuplicateKind = 'identical' | 'same_book'
export type ProblemKind = 'unreadable' | 'missing'

export interface HealthBook {
  id: BookId
  title: string
  authors: string[]
  format: Format
  cover_key: string | null
  path: string
  size: number
  added_at: number
  progress: number | null
  annotations: number
  favorite: boolean
  error: string | null
  missing: boolean
}

export interface DuplicateGroup {
  kind: DuplicateKind
  books: HealthBook[]
  /** Bytes freed by keeping one file. */
  wasted: number
}

export interface HealthCounts {
  unreadable: number
  missing: number
}

export interface TrashReport {
  removed: number
  /** [path, reason] */
  failed: [string, string][]
}

// ── Reading statistics (crates/biblion-core/src/stats.rs) ──

export interface DayReading {
  /** Local date, YYYY-MM-DD. */
  day: string
  seconds: number
  pages: number
}

export interface ReadingStats {
  days: DayReading[]
  finished: { book: BookSummary; finished_at: number }[]
  top_books: { book: BookSummary; seconds: number }[]
  top_authors: { name: string; seconds: number }[]
  books_read: number
}

// ── Backups (src-tauri/src/backup.rs, crates/biblion-core/src/backup.rs) ──

export interface BackupSummary {
  path: string
  created_at: number
  size: number
  books: number
  progress: number
  annotations: number
  shelves: number
  reading_days: number
}

export interface RestoreReport {
  books: number
  matched: number
  progress: number
  annotations: number
  shelves_created: number
  shelf_books: number
  marks: number
  reading_days: number
  missing_samples: string[]
}

export interface BackupPreview {
  created_at: number
  app_version: string
  has_settings: boolean
  report: RestoreReport
}

export interface AutoBackup {
  path: string
  created_at: number
  size: number
}
