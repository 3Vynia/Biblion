// Typed wrappers around the Tauri commands in src-tauri/src/commands.rs.

import { Channel, convertFileSrc, invoke } from '@tauri-apps/api/core'
import type {
  Annotation,
  AnnotationEntry,
  AnnotationQuery,
  AuthorEntry,
  Book,
  BookId,
  BookStats,
  BookQuery,
  BookSummary,
  CmdError,
  Definition,
  DictionaryInfo,
  LibraryStats,
  NewAnnotation,
  Page,
  ScanEvent,
  ScanReport,
  Shelf,
  TagEntry,
} from './types'

export const api = {
  getRoots: () => invoke<string[]>('get_roots'),
  addRoot: (path: string) => invoke<string[]>('add_root', { path }),
  removeRoot: (path: string) => invoke<string[]>('remove_root', { path }),

  scan(force: boolean, onEvent: (e: ScanEvent) => void): Promise<ScanReport> {
    const channel = new Channel<ScanEvent>()
    channel.onmessage = onEvent
    return invoke<ScanReport>('scan', { force, onEvent: channel })
  },
  cancelScan: () => invoke<void>('cancel_scan'),
  isScanning: () => invoke<boolean>('is_scanning'),

  listBooks: (query: BookQuery) => invoke<Page<BookSummary>>('list_books', { query }),
  getBook: (id: BookId) => invoke<Book>('get_book', { id }),
  stats: () => invoke<LibraryStats>('stats'),
  listAuthors: (prefix: string | null, offset: number, limit: number) =>
    invoke<Page<AuthorEntry>>('list_authors', { prefix, offset, limit }),
  listSeries: (offset: number, limit: number) => invoke<Page<[string, number]>>('list_series', { offset, limit }),

  setFavorite: (id: BookId, favorite: boolean) => invoke<void>('set_favorite', { id, favorite }),
  listShelves: () => invoke<Shelf[]>('list_shelves'),
  createShelf: (name: string) => invoke<number>('create_shelf', { name }),
  renameShelf: (id: number, name: string) => invoke<void>('rename_shelf', { id, name }),
  deleteShelf: (id: number) => invoke<void>('delete_shelf', { id }),
  listTags: (prefix: string | null, offset: number, limit: number) =>
    invoke<Page<TagEntry>>('list_tags', { prefix, offset, limit }),
  listLanguages: () => invoke<[string, number][]>('list_languages'),
  setInShelf: (shelf: number, book: BookId, member: boolean) => invoke<void>('set_in_shelf', { shelf, book, member }),
  removeFromShelf: (shelf: number, books: BookId[]) => invoke<number>('remove_from_shelf', { shelf, books }),
  bookShelves: (id: BookId) => invoke<number[]>('book_shelves', { id }),

  setProgress: (id: BookId, locator: string, percent: number, finished: boolean) =>
    invoke<void>('set_progress', { id, locator, percent, finished }),

  addAnnotation: (annotation: NewAnnotation) => invoke<number>('add_annotation', { annotation }),
  updateAnnotation: (id: number, note: string | null, color: string | null) =>
    invoke<void>('update_annotation', { id, note, color }),
  deleteAnnotation: (id: number) => invoke<void>('delete_annotation', { id }),
  bookAnnotations: (id: BookId) => invoke<Annotation[]>('book_annotations', { id }),
  listAnnotations: (query: AnnotationQuery) => invoke<Page<AnnotationEntry>>('list_annotations', { query }),
  annotationCounts: () => invoke<[number, number]>('annotation_counts'),

  listDictionaries: () => invoke<DictionaryInfo[]>('list_dictionaries'),
  define: (word: string) => invoke<Definition[]>('define', { word }),
  installDictionary: (path: string) => invoke<DictionaryInfo[]>('install_dictionary', { path }),
  removeDictionary: (id: string) => invoke<DictionaryInfo[]>('remove_dictionary', { id }),

  analyzeBook: (id: BookId) => invoke<BookStats>('analyze_book', { id }),
  getUiSettings: () => invoke<unknown>('get_ui_settings'),
  setUiSettings: (value: object) => invoke<void>('set_ui_settings', { value }),
  dataDir: () => invoke<string>('data_dir'),

  getReaderSettings: () => invoke<unknown>('get_reader_settings'),
  setReaderSettings: (value: object) => invoke<void>('set_reader_settings', { value }),
}

/** URL of a cover thumbnail, served from disk by the `thumb://` scheme. */
export const thumbUrl = (coverKey: string) => convertFileSrc(coverKey, 'thumb')

/** URL of a book file, served by the `book://` scheme. */
export const bookUrl = (id: BookId) => convertFileSrc(String(id), 'book')

/** Readable message for anything a command (or the runtime) may throw. */
export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e
  if (e && typeof e === 'object' && 'message' in e) return String((e as CmdError).message)
  return 'Unexpected error'
}

export const isCancelled = (e: unknown) =>
  !!e && typeof e === 'object' && (e as CmdError).code === 'cancelled'
