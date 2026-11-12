import { describe, expect, it } from 'vitest'
import { FLUSH_MS, IDLE_MS, ReadingTracker, localDay } from './readingLog'

function setup(start = new Date(2026, 10, 12, 21, 0, 0).getTime()) {
  let now = start
  let visible = true
  const sent: [number, string, number, number][] = []
  const tracker = new ReadingTracker(
    async (...args) => void sent.push(args),
    () => now,
    () => visible,
  )
  const advance = (ms: number) => {
    for (let i = 0; i < ms / 1000; i++) {
      now += 1000
      tracker.tick()
    }
  }
  /** The clock moves on without ticks in between (a sleeping laptop). */
  const jump = (ms: number) => {
    now += ms
    tracker.tick()
  }
  return { tracker, sent, advance, jump, hide: (v: boolean) => (visible = !v) }
}

describe('reading tracker', () => {
  it('counts active seconds and pages, flushing every half minute', () => {
    const { tracker, sent, advance } = setup()
    tracker.start(7)
    tracker.pageTurned()
    advance(FLUSH_MS)
    expect(sent).toEqual([[7, '2026-11-12', 30, 1]])
    advance(10_000)
    tracker.stop()
    expect(sent.at(-1)).toEqual([7, '2026-11-12', 10, 0])
  })

  it('stops counting when idle or hidden', () => {
    const { tracker, sent, advance, hide } = setup()
    tracker.start(1)
    advance(IDLE_MS + 60_000) // five minutes count, the last one does not
    hide(true)
    tracker.activity()
    advance(60_000)
    tracker.stop()
    expect(sent.reduce((n, s) => n + s[2], 0)).toBe(IDLE_MS / 1000)
  })

  it('splits reading across midnight', () => {
    const { tracker, sent, advance } = setup(new Date(2026, 10, 12, 23, 59, 40).getTime())
    tracker.start(3)
    advance(20_000)
    advance(15_000)
    tracker.stop()
    const on = (day: string) => sent.filter((s) => s[1] === day).reduce((n, s) => n + s[2], 0)
    expect(on('2026-11-12')).toBe(20)
    expect(on('2026-11-13')).toBe(15)
  })

  it('ignores a sleeping computer', () => {
    const { tracker, sent, advance, jump } = setup()
    tracker.start(1)
    advance(3_000)
    tracker.activity()
    jump(60 * 60_000)
    tracker.stop()
    expect(sent.reduce((n, s) => n + s[2], 0)).toBe(3)
  })

  it('formats local days', () => {
    expect(localDay(new Date(2026, 0, 5))).toBe('2026-01-05')
  })
})
