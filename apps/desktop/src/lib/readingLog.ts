// Counts the time spent reading a book and the pages turned, and sends them
// to the backend in small chunks (per local day) for the statistics screen.
//
// Only active time counts: the window must be visible and the reader must
// have done something (turned a page, scrolled, pressed a key) in the last
// few minutes, so a book left open overnight adds nothing.

import { api } from './api'

/** No activity for this long and the clock stops. */
export const IDLE_MS = 5 * 60_000
/** How often the counted time is sent. */
export const FLUSH_MS = 30_000
/** Longest gap between two ticks that still counts (a sleeping laptop does not). */
const MAX_TICK_MS = 5_000

/** Local date as YYYY-MM-DD. */
export function localDay(t: number | Date = Date.now()): string {
  const d = new Date(t)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

export type Send = (id: number, day: string, seconds: number, pages: number) => Promise<unknown>

export class ReadingTracker {
  #book: number | null = null
  #ms = 0
  #pages = 0
  #day = ''
  #lastTick = 0
  #lastActivity = 0
  #lastFlush = 0

  constructor(
    private send: Send,
    private now: () => number = Date.now,
    private visible: () => boolean = () => true,
  ) {}

  get book() {
    return this.#book
  }

  start(id: number) {
    if (this.#book !== null) this.stop()
    const t = this.now()
    this.#book = id
    this.#ms = 0
    this.#pages = 0
    this.#day = localDay(t)
    this.#lastTick = this.#lastActivity = this.#lastFlush = t
  }

  activity() {
    this.#lastActivity = this.now()
  }

  pageTurned() {
    if (this.#book === null) return
    this.#pages++
    this.activity()
  }

  /** Call about once a second. */
  tick() {
    if (this.#book === null) return
    const t = this.now()
    const step = t - this.#lastTick
    this.#lastTick = t
    if (step > 0 && step <= MAX_TICK_MS && this.visible() && t - this.#lastActivity <= IDLE_MS) this.#ms += step
    if (localDay(t) !== this.#day) {
      // Midnight: what was read so far belongs to the day before.
      this.flush()
      this.#day = localDay(t)
    }
    if (t - this.#lastFlush >= FLUSH_MS) this.flush()
  }

  flush() {
    if (this.#book === null) return
    this.#lastFlush = this.now()
    const seconds = Math.floor(this.#ms / 1000)
    if (seconds === 0 && this.#pages === 0) return
    this.#ms -= seconds * 1000
    const pages = this.#pages
    this.#pages = 0
    this.send(this.#book, this.#day, seconds, pages).catch((e) => console.warn('could not log reading time', e))
  }

  stop() {
    this.tick()
    this.flush()
    this.#book = null
  }
}

/** The app's tracker, wired to the backend and the window. */
export const readingLog = new ReadingTracker(
  (id, day, seconds, pages) => api.logReading(id, day, seconds, pages),
  Date.now,
  () => document.visibilityState === 'visible',
)

/** Starts tracking a book; returns the function that stops it (for onMount). */
export function trackReading(id: number): () => void {
  readingLog.start(id)
  const onActivity = () => readingLog.activity()
  const events = ['keydown', 'pointerdown', 'wheel', 'touchstart'] as const
  for (const e of events) window.addEventListener(e, onActivity, { passive: true, capture: true })
  const timer = setInterval(() => readingLog.tick(), 1000)
  return () => {
    clearInterval(timer)
    for (const e of events) window.removeEventListener(e, onActivity, { capture: true })
    if (readingLog.book === id) readingLog.stop()
  }
}
