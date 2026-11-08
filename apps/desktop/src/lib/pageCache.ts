// Page cache for the virtualised grid.
//
// The grid never holds the whole library: it asks for the items it is about
// to draw and the cache loads the fixed-size pages that contain them
// (offset/limit), keeping at most `maxPages` in memory (LRU). Pages that are
// visible are never evicted.
//
// Two ways to change the data:
// - `reset(fetcher)`: a new query (search text, sort…). Everything is dropped
//   and responses from the old query are ignored when they arrive.
// - `refresh()`: same query, fresher data (e.g. during a scan). Old pages keep
//   being shown until their replacement arrives, so the grid never flickers.

export interface PageResult<T> {
  items: T[]
  total: number
}

export type Fetcher<T> = (offset: number, limit: number) => Promise<PageResult<T>>

interface Entry<T> {
  items: T[]
  generation: number
}

export class PageCache<T> {
  readonly pageSize: number
  readonly maxPages: number
  /** Total number of items, or null until the first page arrives. */
  total: number | null = null
  /** Last load error (cleared by the next successful load). */
  error: unknown = null

  #fetcher: Fetcher<T>
  #pages = new Map<number, Entry<T>>()
  #inflight = new Map<number, number>() // page -> generation being fetched
  #generation = 0
  #protected: [number, number] = [0, -1]
  #onChange: () => void

  constructor(fetcher: Fetcher<T>, onChange: () => void, pageSize = 200, maxPages = 12) {
    this.#fetcher = fetcher
    this.#onChange = onChange
    this.pageSize = pageSize
    this.maxPages = maxPages
  }

  /** Item at `index`, or undefined while its page is loading. */
  get(index: number): T | undefined {
    const page = Math.floor(index / this.pageSize)
    const entry = this.#pages.get(page)
    return entry?.items[index - page * this.pageSize]
  }

  /** Makes sure the pages covering items [start, end) are loaded or loading. */
  ensure(start: number, end: number): void {
    if (end <= start) end = start + 1
    if (this.total !== null) end = Math.min(end, Math.max(this.total, 1))
    const first = Math.floor(Math.max(0, start) / this.pageSize)
    const last = Math.floor((end - 1) / this.pageSize)
    this.#protected = [first, last]
    for (let p = first; p <= last; p++) {
      const entry = this.#pages.get(p)
      if (entry) {
        // Touch for LRU order.
        this.#pages.delete(p)
        this.#pages.set(p, entry)
      }
      const fresh = entry?.generation === this.#generation
      if (!fresh && this.#inflight.get(p) !== this.#generation) this.#load(p)
    }
  }

  /** New query: drop everything and ignore responses for the old one. */
  reset(fetcher?: Fetcher<T>): void {
    if (fetcher) this.#fetcher = fetcher
    this.#generation++
    this.#pages.clear()
    this.#inflight.clear()
    this.total = null
    this.error = null
    this.#onChange()
  }

  /** Same query, fresher data: keep showing old pages until replaced. */
  refresh(): void {
    this.#generation++
    const [first, last] = this.#protected
    if (last >= first) this.ensure(first * this.pageSize, (last + 1) * this.pageSize)
  }

  get loadedPages(): number[] {
    return [...this.#pages.keys()]
  }

  async #load(page: number) {
    const generation = this.#generation
    this.#inflight.set(page, generation)
    try {
      const result = await this.#fetcher(page * this.pageSize, this.pageSize)
      if (generation !== this.#generation) return
      this.#pages.delete(page)
      this.#pages.set(page, { items: result.items, generation })
      this.total = result.total
      this.error = null
      this.#evict()
    } catch (e) {
      if (generation !== this.#generation) return
      this.error = e
    } finally {
      if (this.#inflight.get(page) === generation) this.#inflight.delete(page)
    }
    this.#onChange()
  }

  #evict() {
    const [first, last] = this.#protected
    for (const page of this.#pages.keys()) {
      if (this.#pages.size <= this.maxPages) break
      if (page >= first && page <= last) continue
      this.#pages.delete(page)
    }
  }
}
