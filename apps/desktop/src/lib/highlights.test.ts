import { describe, expect, it } from 'vitest'
import { annotationsToMarkdown, highlightValue } from './highlights'

describe('highlights', () => {
  it('falls back to yellow for unknown colours', () => {
    expect(highlightValue('blue')).toBe('#5fb0f2')
    expect(highlightValue('neon')).toBe('#f4cf3d')
    expect(highlightValue(null)).toBe('#f4cf3d')
  })

  it('exports highlights as Markdown grouped by chapter, skipping bookmarks', () => {
    const md = annotationsToMarkdown('The Reef', ['Ann Author'], [
      { kind: 'highlight', text: 'First line\nsecond line', note: null, chapter: 'One' },
      { kind: 'bookmark', text: 'ignored', note: null, chapter: 'One' },
      { kind: 'highlight', text: 'Another', note: 'My thought', chapter: 'Two' },
    ])
    expect(md).toBe('# The Reef\n*Ann Author*\n\n## One\n\n> First line\n> second line\n\n## Two\n\n> Another\n\nMy thought\n')
  })
})
