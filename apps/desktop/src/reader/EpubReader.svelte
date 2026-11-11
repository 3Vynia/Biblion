<script lang="ts">
  // EPUB / MOBI / AZW3 reader built on foliate-js. The book file is
  // fetched from the `book://` scheme; the position is saved as an EPUB CFI.
  import { onDestroy, onMount, untrack } from 'svelte'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { api, bookUrl, errorMessage } from '../lib/api'
  import type { Book } from '../lib/types'
  import { THEMES, formatMinutes, readerCss, rendererAttributes } from './readerSettings'
  import { readerSettings } from './settingsStore.svelte'
  import { rise } from '../lib/motion'
  import TocPanel, { type TocItem } from './TocPanel.svelte'
  import SearchPanel from './SearchPanel.svelte'
  import SettingsPanel from './SettingsPanel.svelte'

  interface Props {
    book: Book
    onclose: () => void
  }

  let { book, onclose }: Props = $props()

  const SAVE_DELAY_MS = 1000
  /** Treat the book as finished from this fraction on. */
  const FINISHED_AT = 0.995
  /** Share of the page width that turns pages when clicked (each side). */
  const TAP_ZONE = 0.25

  type Panel = 'toc' | 'search' | 'settings' | null

  // foliate-js elements are untyped custom elements.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let view: any = $state.raw(null)
  let host: HTMLDivElement
  let footnoteHost = $state<HTMLDivElement>()
  let bookWidth = $state(0)
  let status = $state<'loading' | 'ready' | 'error'>('loading')
  let error = $state('')
  // The reader is recreated for each book, so the initial value is enough.
  let fraction = $state(untrack(() => book.progress?.percent ?? 0))
  let sliderValue = $state<number | null>(null)
  let chapter = $state('')
  let tocHref = $state<string | null>(null)
  let page = $state<{ current: number; total: number } | null>(null)
  let minutesLeft = $state<number | null>(null)
  let toc = $state<TocItem[]>([])
  let panel = $state<Panel>(null)
  let barsVisible = $state(true)
  let footnote = $state<{ visible: boolean; type: string | null }>({ visible: false, type: null })

  let pending: { cfi: string; fraction: number } | null = null
  let saveTimer: ReturnType<typeof setTimeout> | undefined

  const title = $derived(book.metadata.title ?? book.path.split('/').pop() ?? '')
  const settings = $derived(readerSettings.value)
  const theme = $derived(THEMES[settings.theme])

  // Apply layout and typography whenever the settings change.
  $effect(() => {
    const s = settings
    const width = bookWidth
    if (status !== 'ready' || !view?.renderer) return
    for (const [name, value] of Object.entries(rendererAttributes(s, width))) view.renderer.setAttribute(name, value)
    view.renderer.setStyles?.(readerCss(s))
  })

  function flushProgress() {
    clearTimeout(saveTimer)
    if (!pending) return
    const { cfi, fraction: f } = pending
    pending = null
    api.setProgress(book.id, cfi, f, f >= FINISHED_AT).catch((e) => console.error('saving progress failed', e))
  }

  function onRelocate(e: CustomEvent) {
    const d = e.detail ?? {}
    if (typeof d.fraction === 'number') fraction = d.fraction
    chapter = d.tocItem?.label?.trim() ?? ''
    tocHref = d.tocItem?.href ?? null
    page = d.location?.total ? { current: d.location.current + 1, total: d.location.total } : null
    minutesLeft = typeof d.time?.section === 'number' ? d.time.section : null
    if (d.cfi) {
      pending = { cfi: d.cfi, fraction: typeof d.fraction === 'number' ? d.fraction : fraction }
      clearTimeout(saveTimer)
      saveTimer = setTimeout(flushProgress, SAVE_DELAY_MS)
    }
  }

  function go(target: string) {
    view?.goTo(target)?.catch?.((e: unknown) => console.error('navigation failed', e))
    panel = null
  }

  function togglePanel(p: Exclude<Panel, null>) {
    panel = panel === p ? null : p
    barsVisible = true
  }

  function onKey(e: KeyboardEvent) {
    if (status !== 'ready') return
    const target = e.target as HTMLElement | null
    if (target?.closest?.('input, select, textarea')) {
      if (e.key === 'Escape') panel = null
      return
    }
    switch (e.key) {
      case 'ArrowLeft':
      case 'PageUp':
        view.goLeft()
        break
      case 'ArrowRight':
      case 'PageDown':
      case ' ':
        view.goRight()
        break
      case 'Escape':
        if (footnote.visible) closeFootnote()
        else if (panel) panel = null
        else onclose()
        break
      case 'f':
        if (e.ctrlKey || e.metaKey) togglePanel('search')
        else return
        break
      default:
        return
    }
    e.preventDefault()
  }

  /** Clicks inside the book: side zones turn pages, the centre toggles the bars. */
  function onDocClick(e: MouseEvent) {
    const target = e.target as Element | null
    if (target?.closest?.('a[href]')) return
    const doc = (target as Node | null)?.ownerDocument
    if (doc?.getSelection()?.toString()) return
    const frame = doc?.defaultView?.frameElement
    if (!frame) return
    const x = frame.getBoundingClientRect().left + e.clientX
    const bounds = host.getBoundingClientRect()
    const rel = (x - bounds.left) / bounds.width
    if (panel) panel = null
    else if (rel < TAP_ZONE) view.goLeft()
    else if (rel > 1 - TAP_ZONE) view.goRight()
    else barsVisible = !barsVisible
  }

  function commitSlider() {
    if (sliderValue === null) return
    const f = sliderValue / 1000
    sliderValue = null
    view?.goToFraction(f)
  }

  // ── Footnotes ──────────────────────────────────────────────────────────
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let footnoteHandler: any = null
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let footnoteView: any = null

  function closeFootnote() {
    footnote = { visible: false, type: null }
    footnoteView?.close?.()
    footnoteView?.remove()
    footnoteView = null
  }

  async function setupFootnotes() {
    const { FootnoteHandler } = await import('../../vendor/foliate-js/footnotes.js')
    footnoteHandler = new FootnoteHandler()
    footnoteHandler.addEventListener('before-render', (e: CustomEvent) => {
      const v = e.detail.view
      v.addEventListener('link', (ev: CustomEvent) => {
        ev.preventDefault()
        closeFootnote()
        go(ev.detail.href)
      })
      v.addEventListener('external-link', onExternalLink)
    })
    footnoteHandler.addEventListener('render', (e: CustomEvent) => {
      const { view: v, type } = e.detail
      closeFootnote()
      footnoteView = v
      v.renderer.setAttribute('flow', 'scrolled')
      v.renderer.setAttribute('margin', '0px')
      v.renderer.setAttribute('gap', '5%')
      v.renderer.setStyles?.(readerCss({ ...settings, fontSize: Math.min(settings.fontSize, 120) }))
      footnoteHost?.replaceChildren(v)
      footnote = { visible: true, type }
    })
  }

  function onLink(e: CustomEvent) {
    // Footnote references open in a popup instead of jumping away.
    footnoteHandler?.handle(view.book, e)?.catch((err: unknown) => {
      console.warn('footnote failed, following the link instead', err)
      view.goTo(e.detail.href)
    })
  }

  function onExternalLink(e: CustomEvent) {
    e.preventDefault()
    const href: string = e.detail?.href_ ?? e.detail?.a?.href ?? ''
    if (/^(https?|mailto):/i.test(href)) openUrl(href).catch((err) => console.error('cannot open link', err))
  }

  // ── Loading ────────────────────────────────────────────────────────────

  async function load() {
    await Promise.all([import('../../vendor/foliate-js/view.js'), readerSettings.load(), setupFootnotes()])
    const res = await fetch(bookUrl(book.id))
    if (!res.ok) throw new Error((await res.text()) || `HTTP ${res.status}`)
    const blob = await res.blob()
    const file = new File([blob], `book.${book.format}`, { type: blob.type })

    view = document.createElement('foliate-view')
    view.addEventListener('relocate', onRelocate)
    view.addEventListener('link', onLink)
    view.addEventListener('external-link', onExternalLink)
    // Key presses and clicks inside the book's iframes do not reach the window.
    view.addEventListener('load', (e: CustomEvent) => {
      const doc: Document | undefined = e.detail?.doc
      doc?.addEventListener('keydown', onKey)
      doc?.addEventListener('click', onDocClick)
    })
    host.append(view)
    await view.open(file)
    toc = view.book?.toc ?? []
    for (const [name, value] of Object.entries(rendererAttributes(settings, bookWidth))) view.renderer.setAttribute(name, value)
    view.renderer.setStyles?.(readerCss(settings))

    const lastLocation = book.progress?.locator || undefined
    try {
      await view.init({ lastLocation, showTextStart: !lastLocation })
    } catch (e) {
      // A saved position that no longer resolves (e.g. the file changed).
      console.warn('could not restore position, starting from the beginning', e)
      await view.init({ showTextStart: true })
    }
    status = 'ready'
  }

  onMount(() => {
    window.addEventListener('keydown', onKey)
    load().catch((e) => {
      status = 'error'
      error = errorMessage(e)
    })
  })

  onDestroy(() => {
    window.removeEventListener('keydown', onKey)
    flushProgress()
    closeFootnote()
    view?.close?.()
    view?.remove()
  })
</script>

<div
  class="reader"
  role="dialog"
  aria-modal="true"
  aria-label="Reading {title}"
  in:rise={{ y: 0, scale: 0.985, duration: 320 }}
  style:--r-bg={theme.bg}
  style:--r-fg={theme.fg}
  style:--r-link={theme.link}
  style:--r-chrome={theme.chrome}
  style:--r-border={theme.chromeBorder}
  style:--r-muted={theme.muted}
  style:--r-hover={theme.dark ? 'rgb(255 255 255 / 0.06)' : 'rgb(0 0 0 / 0.05)'}
  style:color-scheme={theme.dark ? 'dark' : 'light'}
>
  <header class:hidden={!barsVisible}>
    <button class="icon" onclick={onclose} title="Back to the library (Esc)">←</button>
    <div class="heading">
      <span class="title">{title}</span>
      {#if chapter}<span class="chapter">{chapter}</span>{/if}
    </div>
    <button class="icon" class:on={panel === 'toc'} onclick={() => togglePanel('toc')} title="Contents" aria-label="Contents"
      >☰</button
    >
    <button
      class="icon"
      class:on={panel === 'search'}
      onclick={() => togglePanel('search')}
      title="Search in book (Ctrl+F)"
      aria-label="Search in book">⌕</button
    >
    <button
      class="icon"
      class:on={panel === 'settings'}
      onclick={() => togglePanel('settings')}
      title="Reading settings"
      aria-label="Reading settings">Aa</button
    >
  </header>

  <div class="stage">
    <div class="book" bind:this={host} bind:clientWidth={bookWidth}>
      {#if status === 'loading'}
        <p class="message">Opening book…</p>
      {:else if status === 'error'}
        <div class="message">
          <p class="error">This book could not be opened.</p>
          <p>{error}</p>
          <button onclick={onclose}>Back to the library</button>
        </div>
      {/if}
    </div>

    {#if panel && status === 'ready'}
      <aside class="panel" aria-label={panel}>
        {#if panel === 'toc'}
          <h3>Contents</h3>
          <TocPanel {toc} currentHref={tocHref} ongo={go} />
        {:else if panel === 'search'}
          <h3>Search</h3>
          <SearchPanel {view} ongo={(cfi) => view.goTo(cfi)} />
        {:else}
          <h3>Reading settings</h3>
          <SettingsPanel />
        {/if}
      </aside>
    {/if}

    <div class="footnote" class:visible={footnote.visible} role="dialog" aria-label="Footnote">
      <div class="footnote-head">
        <span>{footnote.type === 'endnote' ? 'Endnote' : footnote.type === 'definition' ? 'Definition' : 'Note'}</span>
        <button class="icon" onclick={closeFootnote} aria-label="Close note">✕</button>
      </div>
      <div class="footnote-body" bind:this={footnoteHost}></div>
    </div>
  </div>

  <footer class:hidden={!barsVisible}>
    <span class="meta">{page ? `Page ${page.current} of ${page.total}` : ''}</span>
    <input
      type="range"
      min="0"
      max="1000"
      step="1"
      value={sliderValue ?? Math.round(fraction * 1000)}
      oninput={(e) => (sliderValue = Number(e.currentTarget.value))}
      onchange={commitSlider}
      disabled={status !== 'ready'}
      aria-label="Position in book"
    />
    <span class="meta right">
      {Math.round((sliderValue !== null ? sliderValue / 1000 : fraction) * 100)}%
      {#if minutesLeft && sliderValue === null}
        · {formatMinutes(minutesLeft)} left in chapter{/if}
    </span>
  </footer>
</div>

<style>
  .reader {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    background: var(--r-bg);
    color: var(--r-fg);
  }

  header,
  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: var(--r-chrome);
    transition:
      margin 160ms ease,
      opacity 160ms ease;
  }

  header {
    border-bottom: 1px solid var(--r-border);
  }

  footer {
    border-top: 1px solid var(--r-border);
  }

  header.hidden {
    margin-top: -49px;
    opacity: 0;
    pointer-events: none;
  }

  footer.hidden {
    margin-bottom: -41px;
    opacity: 0;
    pointer-events: none;
  }

  .icon {
    min-width: 36px;
    height: 36px;
    border: none;
    background: transparent;
    color: var(--r-fg);
    font-size: 17px;
    border-radius: 8px;
  }

  .icon:hover:not(:disabled),
  .icon.on {
    background: var(--r-hover);
  }

  .heading {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .title,
  .chapter {
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    font-family: var(--serif);
    font-weight: 600;
  }

  .chapter {
    font-size: 12px;
    color: var(--r-muted);
  }

  .stage {
    position: relative;
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .book {
    position: relative;
    flex: 1;
    min-width: 0;
  }

  .book :global(foliate-view) {
    display: block;
    width: 100%;
    height: 100%;
  }

  .message {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 24px;
    color: var(--r-muted);
  }

  .message .error {
    color: var(--danger);
  }

  .panel {
    width: 340px;
    flex-shrink: 0;
    overflow-y: auto;
    padding: 14px 16px 24px;
    background: var(--r-chrome);
    border-left: 1px solid var(--r-border);
  }

  h3 {
    font-size: 15px;
    margin: 0 0 12px;
  }

  .footnote {
    position: absolute;
    left: 50%;
    bottom: 16px;
    width: min(560px, calc(100% - 32px));
    max-height: 45%;
    display: flex;
    flex-direction: column;
    transform: translate(-50%, 20px);
    opacity: 0;
    pointer-events: none;
    background: var(--r-bg);
    border: 1px solid var(--r-border);
    border-radius: 12px;
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.25);
    transition:
      opacity 140ms ease,
      transform 140ms ease;
  }

  .footnote.visible {
    opacity: 1;
    transform: translate(-50%, 0);
    pointer-events: auto;
  }

  .footnote-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px 0 16px;
    font-size: 12px;
    color: var(--r-muted);
  }

  .footnote-body {
    flex: 1;
    min-height: 120px;
  }

  .footnote-body :global(foliate-view) {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 120px;
  }

  .meta {
    min-width: 110px;
    font-size: 12px;
    color: var(--r-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .meta.right {
    min-width: 200px;
    text-align: right;
  }

  input[type='range'] {
    flex: 1;
    accent-color: var(--r-link);
  }

  button:not(.icon) {
    background: var(--r-bg);
    color: var(--r-fg);
    border-color: var(--r-border);
  }
</style>
