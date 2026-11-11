// Library-wide state shared by the sidebar, home and details: counts,
// collections and a "something changed" signal for views to refresh.

import { api } from './api'
import type { LibraryStats, Shelf } from './types'

export const TO_READ = 'To read'

class LibraryState {
  stats = $state<LibraryStats | null>(null)
  shelves = $state<Shelf[]>([])
  counts = $state({ reading: 0, favorites: 0, finished: 0, highlights: 0, bookmarks: 0 })
  /** Bumped whenever books change (scan, progress, favourites, collections). */
  revision = $state(0)

  get toRead(): Shelf | undefined {
    return this.shelves.find((s) => s.name.toLowerCase() === TO_READ.toLowerCase())
  }

  /** Other collections, without the built-in "To read". */
  get custom(): Shelf[] {
    const toRead = this.toRead
    return this.shelves.filter((s) => s !== toRead)
  }

  async refresh() {
    const count = (q: object) => api.listBooks({ ...q, offset: 0, limit: 1 }).then((p) => p.total)
    try {
      const [stats, shelves, reading, favorites, finished, [highlights, bookmarks]] = await Promise.all([
        api.stats(),
        api.listShelves(),
        count({ reading_only: true }),
        count({ favorites_only: true }),
        count({ finished_only: true }),
        api.annotationCounts(),
      ])
      this.stats = stats
      this.shelves = shelves
      this.counts = { reading, favorites, finished, highlights, bookmarks }
      if (!this.toRead) {
        await api.createShelf(TO_READ).catch(() => {})
        this.shelves = await api.listShelves()
      }
    } catch (e) {
      console.error('could not refresh library state', e)
    }
  }

  /** Call after any change to books; views listening to `revision` reload. */
  changed() {
    this.revision++
    this.refresh()
  }
}

export const library = new LibraryState()
