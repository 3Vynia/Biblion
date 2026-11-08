// Geometry for the virtualised cover grid. Pure functions so they can be
// tested without a browser.

export interface GridMetrics {
  columns: number
  cardWidth: number
  coverHeight: number
  rowHeight: number
  rows: number
  totalHeight: number
}

export const GAP = 20
export const CAPTION = 52
export const COVER_RATIO = 1.5 // 360x540 thumbnails

export function gridMetrics(width: number, total: number, minCard = 150): GridMetrics {
  const usable = Math.max(1, width)
  const columns = Math.max(1, Math.floor((usable + GAP) / (minCard + GAP)))
  const cardWidth = (usable - GAP * (columns - 1)) / columns
  const coverHeight = Math.round(cardWidth * COVER_RATIO)
  const rowHeight = coverHeight + CAPTION + GAP
  const rows = Math.ceil(Math.max(0, total) / columns)
  return { columns, cardWidth, coverHeight, rowHeight, rows, totalHeight: rows * rowHeight }
}

/** Rows to render for a viewport, plus `overscan` rows above and below. */
export function visibleRows(m: GridMetrics, scrollTop: number, viewportHeight: number, overscan = 3) {
  if (m.rows === 0) return { firstRow: 0, lastRow: -1 }
  const firstRow = Math.max(0, Math.floor(scrollTop / m.rowHeight) - overscan)
  const lastRow = Math.min(m.rows - 1, Math.ceil((scrollTop + viewportHeight) / m.rowHeight) + overscan)
  return { firstRow, lastRow }
}
