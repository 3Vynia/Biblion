import { describe, expect, it } from 'vitest'
import { lowShare, noiseSamples, rms, rng } from './noise'

describe('noise', () => {
  it('is repeatable for a seed and stays within [-1, 1]', () => {
    expect(noiseSamples('pink', 64, 7)).toEqual(noiseSamples('pink', 64, 7))
    for (const color of ['white', 'pink', 'brown'] as const) {
      const s = noiseSamples(color, 48_000, 3)
      expect(Math.max(...s.map(Math.abs))).toBeLessThanOrEqual(0.9 + 1e-6)
      expect(rms(s)).toBeGreaterThan(0.05)
    }
  })

  it('gets darker from white to pink to brown', () => {
    const [w, p, b] = (['white', 'pink', 'brown'] as const).map((c) => lowShare(noiseSamples(c, 48_000, 5)))
    expect(p).toBeGreaterThan(w)
    expect(b).toBeGreaterThan(p)
  })

  it('rng is uniform enough', () => {
    const r = rng(42)
    const n = 10_000
    let sum = 0
    for (let i = 0; i < n; i++) sum += r()
    expect(sum / n).toBeGreaterThan(0.45)
    expect(sum / n).toBeLessThan(0.55)
  })
})
