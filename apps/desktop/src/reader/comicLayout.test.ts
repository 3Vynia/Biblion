import { describe, expect, it } from 'vitest'
import { pageFromLocator, pageLocator, spreadOf, spreads, wantsDouble } from './comicLayout'

describe('comic spreads', () => {
  it('keeps the cover alone and pairs the rest', () => {
    expect(spreads(6, true)).toEqual([[0], [1, 2], [3, 4], [5]])
    expect(spreads(5, true)).toEqual([[0], [1, 2], [3, 4]])
    expect(spreads(1, true)).toEqual([[0]])
    expect(spreads(0, true)).toEqual([])
  })

  it('shows one page per spread when not doubling', () => {
    expect(spreads(3, false)).toEqual([[0], [1], [2]])
  })

  it('finds the spread of a page', () => {
    const s = spreads(6, true)
    expect(spreadOf(s, 0)).toBe(0)
    expect(spreadOf(s, 2)).toBe(1)
    expect(spreadOf(s, 5)).toBe(3)
    expect(spreadOf(s, 99)).toBe(0)
  })

  it('doubles only in wide areas', () => {
    expect(wantsDouble(1600, 900)).toBe(true)
    expect(wantsDouble(900, 1000)).toBe(false)
  })
})

describe('page locators', () => {
  it('round-trips and rejects other locators', () => {
    expect(pageFromLocator(pageLocator(12))).toBe(12)
    expect(pageFromLocator('epubcfi(/6/2)')).toBeNull()
    expect(pageFromLocator(null)).toBeNull()
  })
})
