// Keyboard shortcuts: the list shown in the help sheet and in Settings, and
// the few that work everywhere (help, full screen, ambience).

import { getCurrentWindow } from '@tauri-apps/api/window'
import { ambience } from './ambience/store.svelte'

export interface Shortcut {
  keys: string[][] // alternatives, each a chord: [['Ctrl', 'F'], ['/']]
  label: string
}

export interface ShortcutGroup {
  title: string
  items: Shortcut[]
}

// "Ctrl+F" → ['Ctrl', 'F']; a "+" key stays whole ("Ctrl++" → ['Ctrl', '+']).
export const k = (...alternatives: string[]) => alternatives.map((a) => a.split(/\+(?=.)/))

export const SHORTCUTS: ShortcutGroup[] = [
  {
    title: 'Everywhere',
    items: [
      { keys: k('?', 'F1'), label: 'Show keyboard shortcuts' },
      { keys: k('F11'), label: 'Full screen' },
      { keys: k('Ctrl+M'), label: 'Play or pause the ambience' },
    ],
  },
  {
    title: 'Library',
    items: [
      { keys: k('Ctrl+F', '/'), label: 'Search' },
      { keys: k('Ctrl+B'), label: 'Show or hide the sidebar' },
      { keys: k('Ctrl+1'), label: 'Home' },
      { keys: k('Ctrl+2'), label: 'All books' },
      { keys: k('Ctrl+3'), label: 'Reading now' },
      { keys: k('Ctrl+4'), label: 'Favourites' },
      { keys: k('Ctrl+5'), label: 'Notes' },
      { keys: k('Ctrl+6'), label: 'Reading stats' },
      { keys: k('Ctrl+7'), label: 'Authors' },
      { keys: k('Ctrl+8'), label: 'Collections' },
      { keys: k('Ctrl+,'), label: 'Settings' },
      { keys: k('Alt+←', 'Esc'), label: 'Back' },
    ],
  },
  {
    title: 'Book grid',
    items: [
      { keys: k('←', '→', '↑', '↓'), label: 'Move between books' },
      { keys: k('Home', 'End'), label: 'First or last book' },
      { keys: k('PgUp', 'PgDn'), label: 'A screen up or down' },
      { keys: k('Space'), label: 'Show details' },
      { keys: k('Enter'), label: 'Read' },
      { keys: k('Ctrl+A'), label: 'Select all (when selecting)' },
    ],
  },
  {
    title: 'Reading',
    items: [
      { keys: k('←', '→'), label: 'Previous or next page' },
      { keys: k('Space', 'Shift+Space'), label: 'Next or previous page' },
      { keys: k('Home', 'End'), label: 'Start or end of the book' },
      { keys: k('[', ']'), label: 'Previous or next chapter' },
      { keys: k('Alt+←'), label: 'Back after following a link' },
      { keys: k('T'), label: 'Contents' },
      { keys: k('N'), label: 'Highlights and bookmarks' },
      { keys: k('B'), label: 'Bookmark this page' },
      { keys: k('Ctrl+F'), label: 'Search in the book' },
      { keys: k('G'), label: 'Go to page' },
      { keys: k('A'), label: 'Reading settings' },
      { keys: k('M'), label: 'Ambience panel' },
      { keys: k('Ctrl++', 'Ctrl+-'), label: 'Bigger or smaller text (zoom in PDFs)' },
      { keys: k('Ctrl+0'), label: 'Reset text size (fit width in PDFs)' },
      { keys: k('H'), label: 'Hide or show the bars' },
      { keys: k('Esc'), label: 'Close a panel, then the book' },
    ],
  },
]

/** True while typing in a field: single-key shortcuts must not fire. */
export function typing(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null
  return !!t?.closest?.('input, select, textarea, [contenteditable="true"]')
}

/** A plain key, without Ctrl, Alt or Meta (Shift is allowed for "?"). */
export const plain = (e: KeyboardEvent) => !e.ctrlKey && !e.metaKey && !e.altKey

class Help {
  open = $state(false)
}
export const shortcutsHelp = new Help()

export async function toggleFullscreen() {
  try {
    const w = getCurrentWindow()
    await w.setFullscreen(!(await w.isFullscreen()))
  } catch {
    // Browsers (the dev harness): the page's own full screen.
    if (document.fullscreenElement) await document.exitFullscreen()
    else await document.documentElement.requestFullscreen?.()
  }
}

/**
 * Shortcuts that work on every screen, the readers included. Returns true
 * when the key was used (and its default prevented).
 */
export function handleGlobalKey(e: KeyboardEvent): boolean {
  const ctrl = e.ctrlKey || e.metaKey
  // While the help sheet is open it takes every key; Esc, ? or F1 close it.
  if (shortcutsHelp.open) {
    if (e.key === 'Escape' || e.key === '?' || e.key === 'F1') shortcutsHelp.open = false
    if (e.key !== 'Tab') e.preventDefault()
    return true
  }
  let used = true
  if (e.key === 'F1' || (e.key === '?' && plain(e) && !typing(e))) shortcutsHelp.open = !shortcutsHelp.open
  else if (e.key === 'F11') void toggleFullscreen()
  else if (ctrl && !e.shiftKey && (e.key === 'm' || e.key === 'M')) void ambience.toggle()
  else used = false
  if (used) e.preventDefault()
  return used
}
