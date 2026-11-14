// Backup actions shared by Settings: save, restore (after a preview and a
// confirmation), and export every note as Markdown.

import { open, save } from '@tauri-apps/plugin-dialog'
import { api } from './api'
import { confirm } from './dialog.svelte'
import { formatCount, formatDate, plural } from './format'
import { annotationsToMarkdown } from './highlights'
import type { AnnotationEntry, BackupSummary, RestoreReport } from './types'

const today = () => new Date().toISOString().slice(0, 10)

/** Asks where to save and writes a backup there. Null when cancelled. */
export async function backUpNow(): Promise<BackupSummary | null> {
  const path = await save({
    title: 'Save a backup',
    defaultPath: `biblion-backup-${today()}.json`,
    filters: [{ name: 'Biblion backup', extensions: ['json'] }],
  })
  if (!path) return null
  return api.exportBackup(path)
}

/** The lines of a restore preview or result, in plain words. */
export function describe(r: RestoreReport): string[] {
  const lines: string[] = []
  const add = (n: number, one: string, many = `${one}s`) => n > 0 && lines.push(`${formatCount(n)} ${plural(n, one, many)}`)
  add(r.annotations, 'highlight or note', 'highlights and notes')
  add(r.progress, 'book with newer progress', 'books with newer progress')
  add(r.shelves_created, 'new collection')
  add(r.shelf_books, 'book added to a collection', 'books added to collections')
  add(r.marks, 'favourite or rating', 'favourites and ratings')
  add(r.reading_days, 'day of reading time', 'days of reading time')
  add(r.edits ?? 0, 'book with edited details', 'books with edited details')
  return lines
}

/**
 * Restores a backup file: shows what it would change, asks, then merges it.
 * Returns the report, or null when the user stopped.
 */
export async function restoreFrom(path: string, withSettings: boolean): Promise<RestoreReport | null> {
  const preview = await api.inspectBackup(path)
  const r = preview.report
  const changes = describe(r)
  const found = `${formatCount(r.matched)} of its ${formatCount(r.books)} ${plural(r.books, 'book')} are in this library.`
  const missing = r.matched < r.books ? `\nNot found: ${r.missing_samples.join(', ')}${r.books - r.matched > r.missing_samples.length ? '…' : ''}` : ''
  const settings = withSettings && preview.has_settings ? '\nYour preferences (reading, appearance, ambience) are replaced by the backup’s.' : ''
  const ok = await confirm({
    title: `Restore the backup of ${formatDate(preview.created_at)}?`,
    message: changes.length
      ? `${found}${missing}\n\nIt adds:\n• ${changes.join('\n• ')}${settings}\n\nNothing in your library is deleted.`
      : `${found}${missing}\n\nYour library already has everything in it.${settings}`,
    confirmLabel: 'Restore',
  })
  if (!ok) return null
  return api.restoreBackup(path, withSettings && preview.has_settings)
}

export async function pickBackupFile(): Promise<string | null> {
  const path = await open({ title: 'Choose a backup', multiple: false, filters: [{ name: 'Biblion backup', extensions: ['json'] }] })
  return typeof path === 'string' ? path : null
}

/** Every highlight and note in the library, as one Markdown document. */
export async function notesMarkdown(): Promise<{ text: string; books: number; notes: number }> {
  const all: AnnotationEntry[] = []
  for (let offset = 0; ; offset += 500) {
    const page = await api.listAnnotations({ kind: 'highlight', offset, limit: 500 })
    all.push(...page.items)
    if (all.length >= page.total || page.items.length === 0) break
  }
  const byBook = new Map<number, AnnotationEntry[]>()
  for (const e of all) byBook.set(e.annotation.book_id, [...(byBook.get(e.annotation.book_id) ?? []), e])
  const sections = [...byBook.values()]
    .sort((a, b) => a[0].book_title.localeCompare(b[0].book_title))
    .map((entries) =>
      annotationsToMarkdown(
        entries[0].book_title,
        entries[0].book_authors,
        entries.map((e) => e.annotation).sort((a, b) => a.percent - b.percent),
      ),
    )
  return { text: sections.join('\n---\n\n'), books: byBook.size, notes: all.length }
}

export async function exportNotes(): Promise<{ books: number; notes: number } | null> {
  const { text, books, notes } = await notesMarkdown()
  if (notes === 0) {
    await confirm({ title: 'No notes yet', message: 'Highlight passages while reading and they can be exported here.', confirmLabel: 'OK' })
    return null
  }
  const path = await save({
    title: 'Export notes',
    defaultPath: `biblion-notes-${today()}.md`,
    filters: [{ name: 'Markdown', extensions: ['md'] }],
  })
  if (!path) return null
  await api.saveTextFile(path, text)
  return { books, notes }
}
