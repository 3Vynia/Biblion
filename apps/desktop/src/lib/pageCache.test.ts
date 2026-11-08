import { describe, expect, it } from 'vitest'
import { PageCache, type Fetcher } from './pageCache'

const TOTAL = 200_000

/** Fake backend: item i is the number i; requests resolve when we say so. */
function fakeBackend(total = TOTAL, label = '') {
  const calls: { offset: number; resolve: () => void; reject: (e: unknown) => void }[] = []
  const fetcher: Fetcher<string> = (offset, limit) =>
    new Promise((resolve, reject) => {
      const items = Array.from({ length: Math.max(0, Math.min(limit, total - offset)) }, (_, i) => `${label}${offset + i}`)
      calls.push({ offset, resolve: () => resolve({ items, total }), reject })
    })
  const flush = async () => {
    while (calls.length) calls.shift()!.resolve()
    await new Promise((r) => setTimeout(r, 0))
  }
  return { fetcher, calls, flush }
}

describe('PageCache', () => {
  it('loads only the pages that cover the requested range', async () => {
    const b = fakeBackend()
    const cache = new PageCache(b.fetcher, () => {}, 200)
    cache.ensure(150, 450)
    expect(b.calls.map((c) => c.offset)).toEqual([0, 200, 400])
    expect(cache.get(150)).toBeUndefined()
    await b.flush()
    expect(cache.total).toBe(TOTAL)
    expect(cache.get(150)).toBe('150')
    expect(cache.get(449)).toBe('449')
  })

  it('does not fetch a page twice while it is loading or loaded', async () => {
    const b = fakeBackend()
    const cache = new PageCache(b.fetcher, () => {}, 200)
    cache.ensure(0, 100)
    cache.ensure(10, 150)
    expect(b.calls.length).toBe(1)
    await b.flush()
    cache.ensure(0, 199)
    expect(b.calls.length).toBe(0)
  })

  it('keeps memory bounded while scrolling through 200k items', async () => {
    const b = fakeBackend()
    const cache = new PageCache(b.fetcher, () => {}, 200, 12)
    for (let start = 0; start < TOTAL; start += 600) {
      cache.ensure(start, start + 120)
      await b.flush()
    }
    expect(cache.loadedPages.length).toBeLessThanOrEqual(12)
    expect(cache.get(TOTAL - 1)).toBe(String(TOTAL - 1))
  })

  it('never evicts the visible pages', async () => {
    const b = fakeBackend()
    const cache = new PageCache(b.fetcher, () => {}, 10, 2)
    cache.ensure(0, 40) // pages 0..3 visible, more than maxPages
    await b.flush()
    expect(cache.loadedPages.sort()).toEqual([0, 1, 2, 3])
  })

  it('ignores responses that belong to an old query', async () => {
    const old = fakeBackend(TOTAL, 'old-')
    const cache = new PageCache(old.fetcher, () => {}, 200)
    cache.ensure(0, 10)
    const fresh = fakeBackend(5, 'new-')
    cache.reset(fresh.fetcher)
    cache.ensure(0, 10)
    await old.flush() // late answer for the previous search
    expect(cache.get(0)).toBeUndefined()
    await fresh.flush()
    expect(cache.total).toBe(5)
    expect(cache.get(0)).toBe('new-0')
  })

  it('refresh keeps old items visible until new data arrives', async () => {
    let label = 'a-'
    const calls: (() => void)[] = []
    const fetcher: Fetcher<string> = (offset, limit) =>
      new Promise((resolve) => {
        const l = label
        calls.push(() => resolve({ items: Array.from({ length: limit }, (_, i) => `${l}${offset + i}`), total: 1000 }))
      })
    const flush = async () => {
      while (calls.length) calls.shift()!()
      await new Promise((r) => setTimeout(r, 0))
    }
    const cache = new PageCache(fetcher, () => {}, 100)
    cache.ensure(0, 50)
    await flush()
    label = 'b-'
    cache.refresh()
    expect(cache.get(0)).toBe('a-0')
    await flush()
    expect(cache.get(0)).toBe('b-0')
  })

  it('reports errors and retries on the next ensure', async () => {
    const b = fakeBackend()
    const cache = new PageCache(b.fetcher, () => {}, 200)
    cache.ensure(0, 10)
    b.calls.shift()!.reject({ code: 'database', message: 'boom' })
    await new Promise((r) => setTimeout(r, 0))
    expect(cache.error).toEqual({ code: 'database', message: 'boom' })
    cache.ensure(0, 10)
    await b.flush()
    expect(cache.error).toBeNull()
    expect(cache.get(0)).toBe('0')
  })

  it('handles an empty library', async () => {
    const b = fakeBackend(0)
    const cache = new PageCache(b.fetcher, () => {}, 200)
    cache.ensure(0, 0)
    await b.flush()
    expect(cache.total).toBe(0)
    expect(cache.get(0)).toBeUndefined()
  })
})
