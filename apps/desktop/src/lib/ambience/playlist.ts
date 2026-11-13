// Order of the user's music: in folder order or shuffled (every track once
// before any repeats), with next and previous.

import { rng } from './noise'

export class Playlist {
  #order: number[] = []
  #pos = 0

  constructor(
    readonly size: number,
    readonly shuffle: boolean,
    seed = Date.now() % 2 ** 31,
  ) {
    this.#order = Array.from({ length: size }, (_, i) => i)
    if (shuffle) {
      const random = rng(seed)
      for (let i = size - 1; i > 0; i--) {
        const j = Math.floor(random() * (i + 1))
        ;[this.#order[i], this.#order[j]] = [this.#order[j], this.#order[i]]
      }
    }
  }

  /** The track now, or null for an empty folder. */
  get current(): number | null {
    return this.size ? this.#order[this.#pos] : null
  }

  next(): number | null {
    if (!this.size) return null
    this.#pos = (this.#pos + 1) % this.size
    return this.current
  }

  prev(): number | null {
    if (!this.size) return null
    this.#pos = (this.#pos - 1 + this.size) % this.size
    return this.current
  }

  /** Starts from a given track (keeps the order of the rest). */
  jumpTo(track: number) {
    const at = this.#order.indexOf(track)
    if (at >= 0) this.#pos = at
  }
}
