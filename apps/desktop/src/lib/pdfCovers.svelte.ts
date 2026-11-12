// Background job: gives PDFs a cover by rendering their first page. PDFs have
// no embedded cover, so the scanner leaves them without one.

import { api } from './api'
import { library } from './library.svelte'

const BATCH = 10
const PER_BOOK_TIMEOUT_MS = 20_000
const REFRESH_EVERY = 12

class PdfCovers {
  running = $state(false)
  made = $state(0)
  /** Set while a book is open, so rendering doesn't compete with reading. */
  paused = false
  #failed = new Set<number>()

  async run() {
    if (this.running) return
    this.running = true
    try {
      const { openPdf, renderCover } = await import('../reader/pdf')
      let sinceRefresh = 0
      for (;;) {
        const ids = (await api.booksWithoutCover('pdf', Math.min(500, BATCH + this.#failed.size)))
          .filter((id) => !this.#failed.has(id))
          .slice(0, BATCH)
        if (ids.length === 0) break
        for (const id of ids) {
          while (this.paused) await sleep(1000)
          try {
            await withTimeout(
              (async () => {
                const doc = await openPdf(id)
                try {
                  await api.setCover(id, await renderCover(doc))
                } finally {
                  doc.loadingTask.destroy()
                }
              })(),
              PER_BOOK_TIMEOUT_MS,
            )
            this.made++
            if (++sinceRefresh >= REFRESH_EVERY) {
              sinceRefresh = 0
              library.changed()
            }
          } catch (e) {
            this.#failed.add(id)
            console.warn(`no cover for PDF ${id}`, e)
          }
          await sleep(30) // keep the interface responsive
        }
      }
      if (sinceRefresh) library.changed()
    } catch (e) {
      console.error('PDF cover generation stopped', e)
    } finally {
      this.running = false
    }
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

function withTimeout<T>(p: Promise<T>, ms: number): Promise<T> {
  return Promise.race([p, new Promise<T>((_, reject) => setTimeout(() => reject(new Error('timed out')), ms))])
}

export const pdfCovers = new PdfCovers()
