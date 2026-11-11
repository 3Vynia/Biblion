import { describe, expect, it } from 'vitest'
import { wordFromSelection } from './dictionary'

describe('wordFromSelection', () => {
  it('keeps one to three words without surrounding punctuation', () => {
    expect(wordFromSelection('  lighthouse, ')).toBe('lighthouse')
    expect(wordFromSelection('«faro»')).toBe('faro')
    expect(wordFromSelection('ice\ncream')).toBe('ice cream')
    expect(wordFromSelection('a b c')).toBe('a b c')
  })

  it('rejects empty and long selections', () => {
    expect(wordFromSelection('...')).toBeNull()
    expect(wordFromSelection('one two three four')).toBeNull()
    expect(wordFromSelection('x'.repeat(61))).toBeNull()
  })
})
