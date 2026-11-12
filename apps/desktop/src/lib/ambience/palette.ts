// Colours of the ambience visual: by mode, shifted by the time of day.

import type { Mode } from './settings'

const BASE: Record<Mode, [string, string, string]> = {
  read: ['#f2b56b', '#d9785b', '#7c4f8c'],
  focus: ['#6fc3c0', '#4a86c9', '#3b4c8f'],
  relax: ['#9ad0a4', '#e0b26a', '#c7738c'],
  sleep: ['#6d6fc4', '#3a3f86', '#1e2150'],
}

/** Three colours for the orb; evenings get warmer, nights deeper. */
export function palette(mode: Mode, hour: number): [string, string, string] {
  const [a, b, c] = BASE[mode]
  const h = ((hour % 24) + 24) % 24
  if (mode !== 'sleep' && (h >= 21 || h < 5)) return [b, c, '#1c1d3a']
  if (h >= 17 && h < 21) return [a, '#e08a5b', c]
  return [a, b, c]
}
