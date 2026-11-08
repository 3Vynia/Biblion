// Typed wrappers around the Tauri commands in src-tauri/src/commands.rs.

import { Channel, convertFileSrc, invoke } from '@tauri-apps/api/core'
import type {
  Book,
  BookId,
  BookQuery,
  BookSummary,
  CmdError,
  LibraryStats,
  Page,
  ScanEvent,
  ScanReport,
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

  setProgress: (id: BookId, locator: string, percent: number, finished: boolean) =>
    invoke<void>('set_progress', { id, locator, percent, finished }),
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
