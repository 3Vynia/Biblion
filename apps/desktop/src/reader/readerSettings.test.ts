import { describe, expect, it } from 'vitest'
import { DEFAULT_SETTINGS, formatMinutes, normalizeSettings, readerCss, rendererAttributes } from './readerSettings'

describe('normalizeSettings', () => {
  it('returns the defaults for missing or broken values', () => {
    expect(normalizeSettings(undefined)).toEqual(DEFAULT_SETTINGS)
    expect(normalizeSettings('nonsense')).toEqual(DEFAULT_SETTINGS)
    expect(normalizeSettings({ theme: 'neon', fontSize: 'big', flow: 3 })).toEqual(DEFAULT_SETTINGS)
  })

  it('keeps valid values and clamps numbers', () => {
    const s = normalizeSettings({ theme: 'night', fontSize: 999, lineHeight: 0.2, justify: false, spread: 'single' })
    expect(s.theme).toBe('night')
    expect(s.fontSize).toBe(250)
    expect(s.lineHeight).toBe(1)
    expect(s.justify).toBe(false)
    expect(s.spread).toBe('single')
  })

  it('rounds the line height to one decimal', () => {
    expect(normalizeSettings({ lineHeight: 1.5000000001 }).lineHeight).toBe(1.5)
    expect(normalizeSettings({ fontSize: Number.NaN }).fontSize).toBe(100)
  })
})

describe('rendererAttributes', () => {
  it('maps layout options to paginator attributes', () => {
    expect(rendererAttributes(DEFAULT_SETTINGS, 1400)).toMatchObject({ flow: 'paginated', 'max-column-count': '2', gap: '6%' })
    const scrolled = rendererAttributes({ ...DEFAULT_SETTINGS, flow: 'scrolled', spread: 'single', margin: 'wide' }, 1400)
    expect(scrolled).toMatchObject({ flow: 'scrolled', 'max-column-count': '1', gap: '10%', margin: '0px' })
  })

  it('only shows two pages when there is room for two comfortable columns', () => {
    expect(rendererAttributes(DEFAULT_SETTINGS, 950)['max-column-count']).toBe('1')
    expect(rendererAttributes(DEFAULT_SETTINGS, 1200)['max-column-count']).toBe('2')
  })
})

describe('readerCss', () => {
  it('keeps the book font unless another one is chosen', () => {
    expect(readerCss(DEFAULT_SETTINGS)).not.toContain('font-family')
    expect(readerCss({ ...DEFAULT_SETTINGS, font: 'serif' })).toContain('font-family: ')
  })

  it('applies size, spacing, alignment and theme colours', () => {
    const css = readerCss({ ...DEFAULT_SETTINGS, fontSize: 130, lineHeight: 1.8, justify: false, theme: 'sepia' })
    expect(css).toContain('font-size: 130%')
    expect(css).toContain('line-height: 1.8')
    expect(css).toContain('text-align: start')
    expect(css).toContain('#f3e9d2')
  })

  it('forces readable colours only in dark themes', () => {
    expect(readerCss(DEFAULT_SETTINGS)).not.toContain('body *:not(a)')
    expect(readerCss({ ...DEFAULT_SETTINGS, theme: 'night' })).toContain('body *:not(a)')
  })
})

describe('formatMinutes', () => {
  it('formats short and long durations', () => {
    expect(formatMinutes(0)).toBe('')
    expect(formatMinutes(undefined)).toBe('')
    expect(formatMinutes(0.4)).toBe('1 min')
    expect(formatMinutes(42)).toBe('42 min')
    expect(formatMinutes(125)).toBe('2 h 05 min')
  })
})
