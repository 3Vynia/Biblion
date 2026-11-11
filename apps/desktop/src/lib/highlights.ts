// Highlight colours, shared by the reader and the Notes screen.

export const HIGHLIGHT_COLORS = {
  yellow: { label: 'Yellow', value: '#f4cf3d' },
  green: { label: 'Green', value: '#6fcf82' },
  blue: { label: 'Blue', value: '#5fb0f2' },
  pink: { label: 'Pink', value: '#f28bb3' },
  purple: { label: 'Purple', value: '#ad8bf2' },
} as const

export type HighlightColor = keyof typeof HIGHLIGHT_COLORS

export const DEFAULT_HIGHLIGHT: HighlightColor = 'yellow'

export function highlightValue(color: string | null | undefined): string {
  return HIGHLIGHT_COLORS[(color ?? DEFAULT_HIGHLIGHT) as HighlightColor]?.value ?? HIGHLIGHT_COLORS.yellow.value
}

/** Markdown for a book's annotations, in reading order. */
export function annotationsToMarkdown(
  title: string,
  authors: string[],
  items: { kind: string; text: string; note: string | null; chapter: string | null }[],
): string {
  const lines = [`# ${title}`, authors.length ? `*${authors.join(', ')}*` : '', '']
  let chapter: string | null = null
  for (const a of items.filter((i) => i.kind === 'highlight')) {
    if (a.chapter && a.chapter !== chapter) {
      chapter = a.chapter
      lines.push(`## ${chapter}`, '')
    }
    lines.push(...a.text.split('\n').map((l) => `> ${l}`))
    if (a.note) lines.push('', a.note)
    lines.push('')
  }
  return lines.join('\n').replace(/\n{3,}/g, '\n\n').trim() + '\n'
}
