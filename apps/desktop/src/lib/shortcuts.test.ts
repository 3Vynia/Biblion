import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({}) }))
vi.mock('./ambience/store.svelte', () => ({ ambience: { toggle: () => {} } }))

const { k } = await import('./shortcuts.svelte')

describe('shortcut labels', () => {
  it('splits chords, keeping a plus key whole', () => {
    expect(k('Ctrl+F', '/')).toEqual([['Ctrl', 'F'], ['/']])
    expect(k('Ctrl++', 'Ctrl+-')).toEqual([['Ctrl', '+'], ['Ctrl', '-']])
    expect(k('Shift+Space')).toEqual([['Shift', 'Space']])
  })
})
