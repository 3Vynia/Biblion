import { describe, expect, it } from 'vitest'
import { addDays, byMonth, calendar, formatReadingTime, level, niceMax, streaks } from './readingStats'

describe('reading stats', () => {
  it('adds days across months, years and daylight saving', () => {
    expect(addDays('2026-03-28', 2)).toBe('2026-03-30')
    expect(addDays('2026-12-31', 1)).toBe('2027-01-01')
    expect(addDays('2026-03-01', -1)).toBe('2026-02-28')
  })

  it('counts streaks', () => {
    const days = ['2026-11-01', '2026-11-02', '2026-11-03', '2026-11-08', '2026-11-10', '2026-11-11']
    expect(streaks(days, '2026-11-11')).toEqual({ current: 2, longest: 3 })
    // Not read yet today: yesterday's streak still counts.
    expect(streaks(days, '2026-11-12')).toEqual({ current: 2, longest: 3 })
    expect(streaks(days, '2026-11-13')).toEqual({ current: 0, longest: 3 })
    expect(streaks([], '2026-11-13')).toEqual({ current: 0, longest: 0 })
  })

  it('shades days by minutes read', () => {
    expect([0, 60, 15 * 60, 30 * 60, 2 * 3600].map(level)).toEqual([0, 1, 2, 3, 4])
  })

  it('lays the year out in Monday-first weeks', () => {
    const weeks = calendar(2026, [{ day: '2026-01-01', seconds: 600, pages: 3 }], '2026-06-01')
    // 1 Jan 2026 is a Thursday: three days of 2025 pad the first week.
    expect(weeks[0].map((c) => c.day)).toEqual(['2025-12-29', '2025-12-30', '2025-12-31', '2026-01-01', '2026-01-02', '2026-01-03', '2026-01-04'])
    expect(weeks[0][2].outside).toBe(true)
    expect(weeks[0][3]).toMatchObject({ seconds: 600, outside: false, future: false })
    expect(weeks.at(-1)!.some((c) => c.day === '2026-12-31')).toBe(true)
    expect(weeks.every((w) => w.length === 7)).toBe(true)
    expect(weeks.at(-1)![0].future).toBe(true)
  })

  it('totals months', () => {
    const m = byMonth([
      { day: '2026-02-01', seconds: 60, pages: 1 },
      { day: '2026-02-03', seconds: 120, pages: 2 },
    ])
    expect(m[1]).toEqual({ month: 1, seconds: 180, pages: 3, days: 2 })
    expect(m[0].seconds).toBe(0)
  })

  it('formats times and axis tops', () => {
    expect(formatReadingTime(35 * 60)).toBe('35 min')
    expect(formatReadingTime(42 * 3600 + 10 * 60)).toBe('42 h 10 min')
    expect(formatReadingTime(3 * 3600)).toBe('3 h')
    expect(niceMax(7 * 3600)).toBe(10 * 3600)
    expect(niceMax(13 * 3600)).toBe(20 * 3600)
    expect(niceMax(10)).toBe(3600)
  })
})
