import { describe, expect, it } from 'vitest'
import { Playlist } from './playlist'

describe('playlist', () => {
  it('plays in order and wraps around', () => {
    const p = new Playlist(3, false)
    expect([p.current, p.next(), p.next(), p.next(), p.prev()]).toEqual([0, 1, 2, 0, 2])
  })

  it('shuffles every track once before repeating', () => {
    const p = new Playlist(20, true, 7)
    const seen = [p.current, ...Array.from({ length: 19 }, () => p.next())]
    expect(new Set(seen).size).toBe(20)
    expect(seen).not.toEqual([...Array(20).keys()])
  })

  it('handles an empty folder', () => {
    const p = new Playlist(0, true)
    expect([p.current, p.next(), p.prev()]).toEqual([null, null, null])
  })

  it('jumps to a track', () => {
    const p = new Playlist(5, false)
    p.jumpTo(3)
    expect(p.next()).toBe(4)
  })
})
