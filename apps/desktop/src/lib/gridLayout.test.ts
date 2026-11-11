import { describe, expect, it } from 'vitest'
import { gridMetrics, visibleRows } from './gridLayout'

describe('gridMetrics', () => {
  it('fits as many columns as the minimum card width allows', () => {
    expect(gridMetrics(1000, 100, 150).columns).toBe(6)
    expect(gridMetrics(999, 100, 150).columns).toBe(5)
    expect(gridMetrics(149, 100, 150).columns).toBe(1)
    expect(gridMetrics(0, 100, 150).columns).toBe(1)
  })

  it('keeps 200k books well under the browser height limit', () => {
    // Narrowest window allowed by tauri.conf.json (720 px).
    const m = gridMetrics(680, 200_000)
    expect(m.rows).toBe(Math.ceil(200_000 / m.columns))
    expect(m.totalHeight).toBeLessThan(33_000_000)
  })

  it('never stretches a lone column into a giant cover', () => {
    const m = gridMetrics(300, 100, 150)
    expect(m.columns).toBe(1)
    expect(m.cardWidth).toBeLessThanOrEqual(150 * 1.35)
  })

  it('handles an empty library', () => {
    const m = gridMetrics(1000, 0)
    expect(m.rows).toBe(0)
    expect(visibleRows(m, 0, 800)).toEqual({ firstRow: 0, lastRow: -1 })
  })
})

describe('visibleRows', () => {
  it('renders only the rows in view plus overscan', () => {
    const m = gridMetrics(1000, 200_000)
    const { firstRow, lastRow } = visibleRows(m, m.rowHeight * 1000, 800, 2)
    expect(firstRow).toBe(998)
    expect(lastRow - firstRow).toBeLessThan(12)
  })

  it('clamps at the end of the list', () => {
    const m = gridMetrics(1000, 1000)
    const { lastRow } = visibleRows(m, m.totalHeight, 800)
    expect(lastRow).toBe(m.rows - 1)
  })
})
