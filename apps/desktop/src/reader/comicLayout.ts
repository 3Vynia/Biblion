// Which pages a comic shows at once. Pure, so it can be tested.

/**
 * Spreads (one or two page indexes) for a comic of `count` pages. The cover
 * stays alone, then pages pair up: [0], [1, 2], [3, 4]… A trailing odd page is
 * alone too. With `double` false every page is its own spread.
 */
export function spreads(count: number, double: boolean): number[][] {
  if (count <= 0) return []
  if (!double) return Array.from({ length: count }, (_, i) => [i])
  const out: number[][] = [[0]]
  for (let i = 1; i < count; i += 2) out.push(i + 1 < count ? [i, i + 1] : [i])
  return out
}

/** Index of the spread that contains `page`. */
export function spreadOf(list: number[][], page: number): number {
  const i = list.findIndex((s) => s.includes(page))
  return i < 0 ? 0 : i
}

/** Two pages fit when the area is clearly wider than tall. */
export const wantsDouble = (width: number, height: number) => width > height * 1.15

/** Locator stored as reading progress for comics and PDFs: "page:12". */
export const pageLocator = (page: number) => `page:${page}`

export function pageFromLocator(locator: string | null | undefined): number | null {
  const m = /^page:(\d+)$/.exec(locator ?? '')
  return m ? Number(m[1]) : null
}
