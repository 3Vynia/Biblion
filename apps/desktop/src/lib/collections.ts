// Collection edits, always confirmed by the user first.

import { api, errorMessage } from './api'
import { confirm, prompt } from './dialog.svelte'
import { formatCount } from './format'
import { library } from './library.svelte'
import type { Shelf } from './types'

/** Asks for a new name, then confirms. Returns the new name, or null if cancelled. */
export async function renameCollection(shelf: Shelf): Promise<string | null> {
  const name = await prompt({ title: 'Rename collection', label: 'New name', value: shelf.name, confirmLabel: 'Continue' })
  if (!name || name === shelf.name) return null
  const ok = await confirm({
    title: 'Rename this collection?',
    message: `“${shelf.name}” will be renamed to “${name}”.`,
    confirmLabel: 'Rename',
  })
  if (!ok) return null
  try {
    await api.renameShelf(shelf.id, name)
  } catch (e) {
    await confirm({ title: 'Could not rename the collection', message: errorMessage(e), confirmLabel: 'OK' })
    return null
  }
  await library.refresh()
  return name
}

/** Confirms, then deletes. Returns true when the collection was deleted. */
export async function deleteCollection(shelf: Shelf): Promise<boolean> {
  const books = shelf.book_count
  const ok = await confirm({
    title: `Delete “${shelf.name}”?`,
    message:
      books > 0
        ? `The collection will be removed. Its ${formatCount(books)} ${books === 1 ? 'book stays' : 'books stay'} in your library.\nThis can't be undone.`
        : `The collection is empty. This can't be undone.`,
    confirmLabel: 'Delete collection',
    danger: true,
  })
  if (!ok) return false
  try {
    await api.deleteShelf(shelf.id)
  } catch (e) {
    await confirm({ title: 'Could not delete the collection', message: errorMessage(e), confirmLabel: 'OK' })
    return false
  }
  library.changed()
  return true
}

/** Confirms, then takes the books out of the collection (they stay in the library). */
export async function removeFromCollection(shelf: Shelf, books: number[]): Promise<boolean> {
  const n = books.length
  const ok = await confirm({
    title: `Remove ${formatCount(n)} ${n === 1 ? 'book' : 'books'} from “${shelf.name}”?`,
    message: `${n === 1 ? 'It stays' : 'They stay'} in your library; only the collection changes.`,
    confirmLabel: 'Remove from collection',
    danger: true,
  })
  if (!ok) return false
  try {
    await api.removeFromShelf(shelf.id, books)
  } catch (e) {
    await confirm({ title: 'Could not update the collection', message: errorMessage(e), confirmLabel: 'OK' })
    return false
  }
  library.changed()
  return true
}
