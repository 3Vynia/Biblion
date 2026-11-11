// Interface preferences: theme, accent colour, motion, sidebar and grid.
// Saved to settings.json through the backend and applied to <html>.

import { api } from './api'

export type ThemeMode = 'system' | 'light' | 'dark'
export type CoverSize = 'small' | 'medium' | 'large'
export type AccentId = 'amber' | 'teal' | 'indigo' | 'rose' | 'forest' | 'plum'

export interface UiSettings {
  theme: ThemeMode
  accent: AccentId
  animations: boolean
  sidebarCollapsed: boolean
  coverSize: CoverSize
  showCaptions: boolean
  scanOnStartup: boolean
  /** Ask Wiktionary when no offline dictionary knows a word. */
  onlineDictionary: boolean
}

export const DEFAULT_UI: UiSettings = {
  theme: 'system',
  accent: 'amber',
  animations: true,
  sidebarCollapsed: false,
  coverSize: 'medium',
  showCaptions: true,
  scanOnStartup: true,
  onlineDictionary: true,
}

/** Light / dark variants of each accent. */
export const ACCENTS: Record<AccentId, { label: string; light: string; dark: string }> = {
  amber: { label: 'Amber', light: '#9a6b1f', dark: '#d9a24a' },
  teal: { label: 'Teal', light: '#0f766e', dark: '#2dd4bf' },
  indigo: { label: 'Indigo', light: '#4f46e5', dark: '#818cf8' },
  rose: { label: 'Rose', light: '#be185d', dark: '#f472b6' },
  forest: { label: 'Forest', light: '#3f6212', dark: '#a3e635' },
  plum: { label: 'Plum', light: '#7e22ce', dark: '#c084fc' },
}

export const COVER_WIDTH: Record<CoverSize, number> = { small: 118, medium: 150, large: 196 }

const SAVE_DELAY_MS = 300

export function normalizeUi(raw: unknown): UiSettings {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>
  const pick = <T extends string>(v: unknown, options: readonly T[], d: T): T => (options.includes(v as T) ? (v as T) : d)
  const bool = (v: unknown, d: boolean) => (typeof v === 'boolean' ? v : d)
  const d = DEFAULT_UI
  return {
    theme: pick(r.theme, ['system', 'light', 'dark'] as const, d.theme),
    accent: pick(r.accent, Object.keys(ACCENTS) as AccentId[], d.accent),
    animations: bool(r.animations, d.animations),
    sidebarCollapsed: bool(r.sidebarCollapsed, d.sidebarCollapsed),
    coverSize: pick(r.coverSize, ['small', 'medium', 'large'] as const, d.coverSize),
    showCaptions: bool(r.showCaptions, d.showCaptions),
    scanOnStartup: bool(r.scanOnStartup, d.scanOnStartup),
    onlineDictionary: bool(r.onlineDictionary, d.onlineDictionary),
  }
}

class UiStore {
  value = $state<UiSettings>({ ...DEFAULT_UI })
  systemDark = $state(typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches)
  #timer: ReturnType<typeof setTimeout> | undefined

  get dark() {
    return this.value.theme === 'dark' || (this.value.theme === 'system' && this.systemDark)
  }

  async load() {
    if (typeof matchMedia === 'function') {
      matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
        this.systemDark = e.matches
        this.apply()
      })
    }
    try {
      this.value = normalizeUi(await api.getUiSettings())
    } catch (e) {
      console.error('could not load interface settings', e)
    }
    this.apply()
  }

  update(patch: Partial<UiSettings>) {
    this.value = normalizeUi({ ...this.value, ...patch })
    this.apply()
    clearTimeout(this.#timer)
    const snapshot = $state.snapshot(this.value)
    this.#timer = setTimeout(
      () => api.setUiSettings(snapshot).catch((e) => console.error('could not save interface settings', e)),
      SAVE_DELAY_MS,
    )
  }

  /** Pushes the settings to <html> as attributes and CSS variables. */
  apply() {
    const root = document.documentElement
    const v = this.value
    if (v.theme === 'system') root.removeAttribute('data-theme')
    else root.setAttribute('data-theme', v.theme)
    root.classList.toggle('no-motion', !v.animations)
    const accent = ACCENTS[v.accent][this.dark ? 'dark' : 'light']
    root.style.setProperty('--accent', accent)
    root.style.setProperty('--accent-strong', `color-mix(in srgb, ${accent} 85%, ${this.dark ? 'white' : 'black'})`)
    root.style.setProperty('--accent-soft', `color-mix(in srgb, ${accent} ${this.dark ? 16 : 12}%, transparent)`)
  }
}

export const ui = new UiStore()
