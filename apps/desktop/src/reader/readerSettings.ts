// Reader preferences: defaults, validation of stored values, and how they
// turn into foliate-js renderer attributes and book CSS. Pure functions.

export type ThemeId = 'day' | 'sepia' | 'night' | 'console'
export type FontId = 'publisher' | 'serif' | 'sans' | 'mono'
export type Flow = 'paginated' | 'scrolled'
export type Spread = 'auto' | 'single'
export type Margin = 'narrow' | 'normal' | 'wide'
export type ComicMode = 'pages' | 'strip'
export type ComicFit = 'screen' | 'width' | 'height'
export type ComicSpread = 'auto' | 'single'
export type ComicDirection = 'ltr' | 'rtl'

export interface ReaderSettings {
  theme: ThemeId
  font: FontId
  /** Percent of the book's own size. */
  fontSize: number
  lineHeight: number
  margin: Margin
  justify: boolean
  hyphenate: boolean
  flow: Flow
  /** `auto` shows two pages side by side when the window is wide enough. */
  spread: Spread
  /** Comics: one or two pages at a time, or a continuous vertical strip. */
  comicMode: ComicMode
  comicFit: ComicFit
  comicSpread: ComicSpread
  /** `rtl` for manga: pages and arrows run right to left. */
  comicDirection: ComicDirection
}

export const DEFAULT_SETTINGS: ReaderSettings = {
  theme: 'day',
  font: 'publisher',
  fontSize: 100,
  lineHeight: 1.5,
  margin: 'normal',
  justify: true,
  hyphenate: true,
  flow: 'paginated',
  spread: 'auto',
  comicMode: 'pages',
  comicFit: 'screen',
  comicSpread: 'auto',
  comicDirection: 'ltr',
}

export const FONT_SIZE = { min: 60, max: 250, step: 10 }
export const LINE_HEIGHT = { min: 1.0, max: 2.4, step: 0.1 }

export interface Theme {
  label: string
  bg: string
  fg: string
  link: string
  /** Colours for the reader's own toolbars. */
  chrome: string
  chromeBorder: string
  muted: string
  dark: boolean
}

export const THEMES: Record<ThemeId, Theme> = {
  day: { label: 'Day', bg: '#fdfbf6', fg: '#1f1b16', link: '#8a5a12', chrome: '#f6f1e7', chromeBorder: '#ddd0b8', muted: '#7a6b5a', dark: false },
  sepia: { label: 'Sepia', bg: '#f3e9d2', fg: '#4a3a28', link: '#7a4a10', chrome: '#eadcbd', chromeBorder: '#d4c29c', muted: '#7d6a50', dark: false },
  night: { label: 'Night', bg: '#1d1f22', fg: '#d0ccc4', link: '#d9a95b', chrome: '#25282c', chromeBorder: '#383c42', muted: '#8d8a84', dark: true },
  console: { label: 'Console', bg: '#000000', fg: '#a8a8a8', link: '#c9a050', chrome: '#0c0c0c', chromeBorder: '#262626', muted: '#6e6e6e', dark: true },
}

export const FONTS: Record<FontId, { label: string; stack: string | null }> = {
  publisher: { label: 'Book', stack: null },
  serif: { label: 'Serif', stack: "'Literata', 'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, serif" },
  sans: { label: 'Sans', stack: "'Inter', 'Noto Sans', 'Segoe UI', Roboto, system-ui, sans-serif" },
  mono: { label: 'Mono', stack: "'JetBrains Mono', 'DejaVu Sans Mono', ui-monospace, monospace" },
}

const MARGIN_GAP: Record<Margin, string> = { narrow: '3%', normal: '6%', wide: '10%' }

/** Narrower than this, two columns would each be too thin to read well. */
export const TWO_PAGE_MIN_WIDTH = 1200

const clamp = (n: number, min: number, max: number) => Math.min(max, Math.max(min, n))
const oneOf = <T extends string>(value: unknown, options: readonly T[], fallback: T): T =>
  options.includes(value as T) ? (value as T) : fallback

/** Stored values may be missing, from an older version, or hand-edited. */
export function normalizeSettings(raw: unknown): ReaderSettings {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>
  const d = DEFAULT_SETTINGS
  const num = (v: unknown, fallback: number) => (typeof v === 'number' && Number.isFinite(v) ? v : fallback)
  return {
    theme: oneOf(r.theme, Object.keys(THEMES) as ThemeId[], d.theme),
    font: oneOf(r.font, Object.keys(FONTS) as FontId[], d.font),
    fontSize: Math.round(clamp(num(r.fontSize, d.fontSize), FONT_SIZE.min, FONT_SIZE.max)),
    lineHeight: Math.round(clamp(num(r.lineHeight, d.lineHeight), LINE_HEIGHT.min, LINE_HEIGHT.max) * 10) / 10,
    margin: oneOf(r.margin, ['narrow', 'normal', 'wide'] as const, d.margin),
    justify: typeof r.justify === 'boolean' ? r.justify : d.justify,
    hyphenate: typeof r.hyphenate === 'boolean' ? r.hyphenate : d.hyphenate,
    flow: oneOf(r.flow, ['paginated', 'scrolled'] as const, d.flow),
    spread: oneOf(r.spread, ['auto', 'single'] as const, d.spread),
    comicMode: oneOf(r.comicMode, ['pages', 'strip'] as const, d.comicMode),
    comicFit: oneOf(r.comicFit, ['screen', 'width', 'height'] as const, d.comicFit),
    comicSpread: oneOf(r.comicSpread, ['auto', 'single'] as const, d.comicSpread),
    comicDirection: oneOf(r.comicDirection, ['ltr', 'rtl'] as const, d.comicDirection),
  }
}

/** Attributes for foliate's <foliate-paginator>, given the reading area width. */
export function rendererAttributes(s: ReaderSettings, width: number): Record<string, string> {
  const twoPages = s.spread === 'auto' && s.flow === 'paginated' && width >= TWO_PAGE_MIN_WIDTH
  return {
    flow: s.flow,
    gap: MARGIN_GAP[s.margin],
    margin: s.flow === 'scrolled' ? '0px' : '40px',
    'max-inline-size': '720px',
    'max-column-count': twoPages ? '2' : '1',
  }
}

/** CSS injected into every section of the book. */
export function readerCss(s: ReaderSettings): string {
  const t = THEMES[s.theme]
  const font = FONTS[s.font].stack
  const align = s.justify ? 'justify' : 'start'
  const hyphens = s.hyphenate ? 'auto' : 'manual'
  return `
@namespace epub "http://www.idpf.org/2007/ops";
html {
  color-scheme: ${t.dark ? 'dark' : 'light'};
  background: ${t.bg} !important;
  color: ${t.fg} !important;
  font-size: ${s.fontSize}% !important;
}
body {
  background: transparent !important;
  color: inherit !important;
  ${font ? `font-family: ${font} !important;` : ''}
}
${font ? `body * { font-family: inherit !important; }\npre, code, kbd, samp, tt { font-family: ${FONTS.mono.stack} !important; }` : ''}
p, li, blockquote, dd, div {
  line-height: ${s.lineHeight} !important;
}
p, li, blockquote, dd {
  text-align: ${align} !important;
  -webkit-hyphens: ${hyphens};
  hyphens: ${hyphens};
}
a:link, a:visited { color: ${t.link} !important; }
pre { white-space: pre-wrap !important; }
img, svg, video { max-width: 100%; height: auto; }
aside[epub|type~="endnote"],
aside[epub|type~="footnote"],
aside[epub|type~="note"],
aside[epub|type~="rearnote"] { display: none; }
${t.dark ? `body *:not(a) { color: inherit !important; background-color: transparent !important; border-color: ${t.chromeBorder} !important; }
img { opacity: 0.88; }` : ''}
`
}

/** Minutes, as a short readable string ("12 min", "1 h 05 min"). */
export function formatMinutes(min: number | undefined | null): string {
  if (min === undefined || min === null || !Number.isFinite(min) || min <= 0) return ''
  const m = Math.max(1, Math.round(min))
  if (m < 60) return `${m} min`
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')} min`
}
