// Reading aloud: sentences come from the open book (a SpeechSource), become
// speech in the backend (Kokoro), and play one after another. The next
// sentences are prepared while the current one plays, so there are no gaps.

import { api, errorMessage } from './api'
import { ambience } from './ambience/store.svelte'
import type { TtsVoice } from './types'

export interface Segment {
  /** Name of the SSML mark before it, used to highlight it in the book. */
  mark: string | null
  text: string
}

/** What a reader gives to be read: SSML blocks (paragraphs) with marks. */
export interface SpeechSource {
  /** The block at the current page. */
  start(): Promise<string | null>
  /** The block after the last one given; null at the end of the chapter. */
  next(): Promise<string | null>
  /**
   * Goes to the next chapter and gives its first block; null at the end of
   * the book. Only called once the chapter has been read to the end, so the
   * page does not turn early.
   */
  nextChapter(): Promise<string | null>
  /** Highlights a mark (and turns the page to it); null clears. */
  mark(name: string | null): void
}

const ENTITIES: Record<string, string> = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" }
const unescape = (s: string) =>
  s.replace(/&(#x[0-9a-f]+|#\d+|\w+);/gi, (m, e: string) =>
    e[0] === '#' ? String.fromCodePoint(e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : Number(e.slice(1))) : (ENTITIES[e] ?? m),
  )

/** Sentences of an SSML block, each with the mark that precedes it. */
export function ssmlToSegments(ssml: string): Segment[] {
  const out: Segment[] = []
  let current: Segment = { mark: null, text: '' }
  const push = () => {
    const text = current.text.replace(/\s+/g, ' ').trim()
    if (/[\p{L}\p{N}]/u.test(text)) out.push({ mark: current.mark, text })
  }
  for (const part of ssml.split(/(<[^>]*>)/)) {
    if (!part) continue
    if (part.startsWith('<')) {
      const mark = part.match(/^<mark\b[^>]*\bname="([^"]*)"/)
      if (mark) {
        push()
        current = { mark: unescape(mark[1]), text: '' }
      } else if (/^<break\b/.test(part)) current.text += ' '
    } else current.text += unescape(part)
  }
  push()
  return out
}

/** The language a voice reads, from a book language ("es-ES" → "es", "en" → "en-us"). */
export function voiceLang(lang: string | null | undefined): string | null {
  const l = (lang ?? '').toLowerCase()
  if (l.startsWith('es')) return 'es'
  if (l === 'en-gb' || l.startsWith('en-gb')) return 'en-gb'
  if (l.startsWith('en')) return 'en-us'
  return null
}

const DEFAULT_VOICE: Record<string, string> = { es: 'ef_dora', 'en-us': 'af_heart', 'en-gb': 'bf_emma' }

/** The chosen voice for a language, or its default; Spanish when unknown. */
export function pickVoice(voices: TtsVoice[], lang: string | null, chosen: Record<string, string>): string {
  const key = lang ?? 'es'
  const valid = (id: string | undefined) => !!id && voices.some((v) => v.id === id)
  if (valid(chosen[key])) return chosen[key]
  if (valid(DEFAULT_VOICE[key])) return DEFAULT_VOICE[key]
  return voices.find((v) => v.lang === key)?.id ?? voices[0]?.id ?? 'ef_dora'
}

/** How many sentences are prepared ahead of the one playing. */
const AHEAD = 2

class ReadAloud {
  status = $state<'off' | 'starting' | 'playing' | 'paused'>('off')
  error = $state<string | null>(null)
  /** The sentence being read, shown in the player. */
  sentence = $state('')
  voice = $state('ef_dora')
  speed = $state(1)

  #source: SpeechSource | null = null
  #queue: Segment[] = []
  #index = 0
  #audio = new Map<number, Promise<AudioBuffer | null>>()
  /** The chapter has no more blocks; the next ones need turning the chapter. */
  #chapterEnd = false
  #ctx: AudioContext | null = null
  #node: AudioBufferSourceNode | null = null
  #sampleRate = 24_000
  /** Bumped on every stop or jump, so stale work is dropped. */
  #run = 0

  get active() {
    return this.status !== 'off'
  }

  async start(source: SpeechSource, voice: string, speed: number, sampleRate: number) {
    this.stop()
    const run = ++this.#run
    this.#source = source
    this.voice = voice
    this.speed = speed
    this.#sampleRate = sampleRate
    this.error = null
    this.status = 'starting'
    this.#ctx ??= new AudioContext()
    await this.#ctx.resume().catch(() => {})
    const first = await source.start().catch(() => null)
    if (run !== this.#run) return
    this.#queue = first ? ssmlToSegments(first) : []
    this.#index = 0
    this.#chapterEnd = false
    ambience.duck(true)
    await this.#play(run)
  }

  pause() {
    if (this.status !== 'playing') return
    this.status = 'paused'
    void this.#ctx?.suspend()
  }

  resume() {
    if (this.status !== 'paused') return
    this.status = 'playing'
    void this.#ctx?.resume()
  }

  toggle() {
    if (this.status === 'playing') this.pause()
    else if (this.status === 'paused') this.resume()
  }

  stop() {
    this.#run++
    this.#stopNode()
    this.#audio.clear()
    this.#queue = []
    if (this.status !== 'off') {
      this.#source?.mark(null)
      ambience.duck(false)
    }
    this.#source = null
    this.status = 'off'
    this.sentence = ''
  }

  /** Jumps by sentences (−1 previous, +1 next). */
  async skip(by: number) {
    if (!this.#source || this.status === 'off') return
    const run = ++this.#run
    this.#stopNode()
    this.#index = Math.max(0, this.#index + by)
    await this.#play(run)
  }

  /** A new voice or pace applies from the current sentence. */
  async change(voice: string, speed: number) {
    this.voice = voice
    this.speed = speed
    if (!this.#source || this.status === 'off') return
    const run = ++this.#run
    this.#stopNode()
    this.#audio.clear()
    await this.#play(run)
  }

  #stopNode() {
    if (this.#node) {
      this.#node.onended = null
      try {
        this.#node.stop()
      } catch {
        // already stopped
      }
      this.#node = null
    }
  }

  /** Makes sure the queue holds the sentences needed from `index` on. */
  async #fill(index: number, run: number) {
    while (!this.#chapterEnd && this.#queue.length <= index + AHEAD && run === this.#run) {
      const block = await this.#source?.next().catch(() => null)
      if (run !== this.#run) return
      if (block == null) this.#chapterEnd = true
      else this.#queue.push(...ssmlToSegments(block))
    }
  }

  #speech(index: number): Promise<AudioBuffer | null> {
    let p = this.#audio.get(index)
    if (!p) {
      const seg = this.#queue[index]
      const [voice, speed, rate] = [this.voice, this.speed, this.#sampleRate]
      p = api.ttsSpeak(seg.text, voice, speed).then((bytes) => {
        const samples = new Float32Array(bytes)
        if (!samples.length || !this.#ctx) return null
        const buffer = this.#ctx.createBuffer(1, samples.length, rate)
        buffer.copyToChannel(samples, 0)
        return buffer
      })
      this.#audio.set(index, p)
    }
    return p
  }

  async #play(run: number): Promise<void> {
    await this.#fill(this.#index, run)
    if (run !== this.#run) return
    if (!this.#queue[this.#index] && this.#chapterEnd) {
      // The chapter is read: on to the next one.
      const block = await this.#source?.nextChapter().catch(() => null)
      if (run !== this.#run) return
      if (block != null) {
        this.#chapterEnd = false
        this.#queue.push(...ssmlToSegments(block))
        return this.#play(run)
      }
    }
    const seg = this.#queue[this.#index]
    if (!seg) {
      // The end of the book.
      this.stop()
      return
    }
    // Free what is behind, prepare what is ahead.
    for (const i of this.#audio.keys()) if (i < this.#index - 1) this.#audio.delete(i)
    let buffer: AudioBuffer | null
    try {
      buffer = await this.#speech(this.#index)
    } catch (e) {
      if (run !== this.#run) return
      this.error = errorMessage(e)
      this.stop()
      return
    }
    if (run !== this.#run) return
    for (let i = 1; i <= AHEAD; i++) {
      void this.#fill(this.#index + i, run).then(() => {
        if (run === this.#run && this.#queue[this.#index + i]) this.#speech(this.#index + i).catch(() => {})
      })
    }
    this.sentence = seg.text
    this.#source?.mark(seg.mark)
    if (this.status !== 'paused') this.status = 'playing'
    if (!buffer || !this.#ctx) {
      this.#index++
      return this.#play(run)
    }
    const node = this.#ctx.createBufferSource()
    node.buffer = buffer
    node.connect(this.#ctx.destination)
    node.onended = () => {
      if (run !== this.#run) return
      this.#node = null
      this.#index++
      void this.#play(run)
    }
    this.#node = node
    node.start()
  }
}

export const readAloud = new ReadAloud()
