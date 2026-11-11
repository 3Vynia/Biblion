import { describe, expect, it } from 'vitest'
import { formatPublished, identifierLabel, languageName, roleLabel } from './format'

describe('metadata formatting', () => {
  it('turns MARC codes into readable roles', () => {
    expect(roleLabel('trl')).toBe('Translated by')
    expect(roleLabel('ILL')).toBe('Illustrated by')
    expect(roleLabel('xyz')).toBe('Xyz')
  })

  it('names languages', () => {
    expect(languageName('es')).toBe('Spanish')
    expect(languageName('pt-BR')).toBe('Brazilian Portuguese')
    expect(languageName('')).toBe('')
    expect(languageName('not a code!')).toBe('not a code!')
  })

  it('formats publication dates of different precision', () => {
    expect(formatPublished('2007-03-27')).toBe('March 27, 2007')
    expect(formatPublished('2007-03')).toBe('March 2007')
    expect(formatPublished('1944')).toBe('1944')
    expect(formatPublished('2011-05-02T00:00:00+00:00')).toBe('May 2, 2011')
    expect(formatPublished('Spring 1990')).toBe('Spring 1990')
  })

  it('labels identifier schemes', () => {
    expect(identifierLabel('isbn')).toBe('ISBN')
    expect(identifierLabel('mobi-asin')).toBe('MOBI-ASIN')
  })
})
