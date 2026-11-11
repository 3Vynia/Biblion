import { describe, expect, it } from 'vitest'
import { DEFAULT_UI, normalizeUi } from './uiSettings.svelte'

describe('normalizeUi', () => {
  it('falls back to defaults for missing or invalid values', () => {
    expect(normalizeUi(null)).toEqual(DEFAULT_UI)
    expect(normalizeUi({ theme: 'neon', accent: 'gold', coverSize: 5, animations: 'yes' })).toEqual(DEFAULT_UI)
  })

  it('keeps valid values', () => {
    const v = normalizeUi({ theme: 'dark', accent: 'teal', sidebarCollapsed: true, coverSize: 'large', scanOnStartup: false })
    expect(v).toMatchObject({ theme: 'dark', accent: 'teal', sidebarCollapsed: true, coverSize: 'large', scanOnStartup: false })
  })
})
