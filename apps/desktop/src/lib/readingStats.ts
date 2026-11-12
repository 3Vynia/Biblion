// Calculations for the reading statistics screen. Days are local dates
// (YYYY-MM-DD), as the reader logs them.

import type { DayReading } from './types'
import { localDay } from './readingLog'

const DAY_MS = 86_400_000

/** Noon of a local day, so adding days never trips over daylight saving. */
export function dayToDate(day: string): Date {
  const [y, m, d] = day.split('-').map(Number)
  return new Date(y, m - 1, d, 12)
}

export const addDays = (day: string, n: number) => localDay(dayToDate(day).getTime() + n * DAY_MS)

/** The year as day strings and as unix seconds [start, end). */
export function yearBounds(year: number) {
  return {
    from: `${year}-01-01`,
    to: `${year}-12-31`,
    start: Math.floor(new Date(year, 0, 1).getTime() / 1000),
    end: Math.floor(new Date(year + 1, 0, 1).getTime() / 1000),
  }
}

/**
 * Current streak: days in a row with reading, ending today (or yesterday, so
 * the streak isn't lost before today's reading). Longest: the best run.
 */
export function streaks(days: string[], today: string): { current: number; longest: number } {
  const set = new Set(days)
  let longest = 0
  for (const day of set) {
    if (set.has(addDays(day, -1))) continue // not the start of a run
    let n = 1
    while (set.has(addDays(day, n))) n++
    longest = Math.max(longest, n)
  }
  let end = set.has(today) ? today : set.has(addDays(today, -1)) ? addDays(today, -1) : null
  let current = 0
  while (end && set.has(end)) {
    current++
    end = addDays(end, -1)
  }
  return { current, longest }
}

/** Minutes thresholds for the four shades of the calendar. */
export const LEVELS = [1, 15, 30, 60] as const

export function level(seconds: number): 0 | 1 | 2 | 3 | 4 {
  const min = seconds / 60
  if (seconds <= 0) return 0
  if (min < LEVELS[1]) return 1
  if (min < LEVELS[2]) return 2
  if (min < LEVELS[3]) return 3
  return 4
}

export interface CalendarCell {
  day: string
  seconds: number
  pages: number
  /** Not in the year: drawn empty, keeps the grid square. */
  outside: boolean
  future: boolean
}

/** Weeks (columns) of seven days, Monday first, covering the whole year. */
export function calendar(year: number, days: DayReading[], today: string): CalendarCell[][] {
  const byDay = new Map(days.map((d) => [d.day, d]))
  const first = dayToDate(`${year}-01-01`)
  const offset = (first.getDay() + 6) % 7 // Monday = 0
  let day = addDays(`${year}-01-01`, -offset)
  const weeks: CalendarCell[][] = []
  while (Number(day.slice(0, 4)) <= year) {
    const week: CalendarCell[] = []
    for (let i = 0; i < 7; i++) {
      const d = byDay.get(day)
      week.push({
        day,
        seconds: d?.seconds ?? 0,
        pages: d?.pages ?? 0,
        outside: Number(day.slice(0, 4)) !== year,
        future: day > today,
      })
      day = addDays(day, 1)
    }
    weeks.push(week)
  }
  return weeks
}

export interface MonthTotal {
  month: number
  seconds: number
  pages: number
  days: number
}

export function byMonth(days: DayReading[]): MonthTotal[] {
  const out: MonthTotal[] = Array.from({ length: 12 }, (_, month) => ({ month, seconds: 0, pages: 0, days: 0 }))
  for (const d of days) {
    const m = out[Number(d.day.slice(5, 7)) - 1]
    if (!m) continue
    m.seconds += d.seconds
    m.pages += d.pages
    if (d.seconds > 0) m.days++
  }
  return out
}

/** "42 h 10 min", "35 min", "0 min". */
export function formatReadingTime(seconds: number): string {
  const min = Math.round(seconds / 60)
  if (min < 60) return `${min} min`
  const h = Math.floor(min / 60)
  const m = min % 60
  return m ? `${h} h ${m} min` : `${h} h`
}

/** Short form for axes and tight spots: "42h", "35m". */
export function formatShortTime(seconds: number): string {
  const min = Math.round(seconds / 60)
  return min < 60 ? `${min}m` : `${Math.round(min / 60)}h`
}

/** A tidy top for a time axis, in seconds: 1, 2, 5 or 10 times a power of ten, in hours. */
export function niceMax(seconds: number): number {
  const hours = Math.max(1, seconds / 3600)
  const p = 10 ** Math.floor(Math.log10(hours))
  const step = [1, 2, 5, 10].find((s) => s * p >= hours) ?? 10
  return step * p * 3600
}
