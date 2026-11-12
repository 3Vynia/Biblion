<script lang="ts">
  // PDF reader on pdf.js: a continuous column of pages where only the pages
  // near the screen are rendered. The file is read in ranges, the position is
  // saved as "page:N", and pages carry a text layer for selection, copy and
  // the dictionary.
  import { onDestroy, onMount, untrack } from 'svelte'
  import Icon from '../components/Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { wordFromSelection } from '../lib/dictionary'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import type { Book } from '../lib/types'
  import { ui } from '../lib/uiSettings.svelte'
  import { pageFromLocator, pageLocator } from './comicLayout'
  import DictionaryCard from './DictionaryCard.svelte'
  import { openPdf, pdfjs, renderCover, type PDFDocumentProxy } from './pdf'
  import { THEMES, type ThemeId } from './readerSettings'
  import SelectionMenu from './SelectionMenu.svelte'
  import { readerSettings } from './settingsStore.svelte'
  import AmbiencePanel from '../components/AmbiencePanel.svelte'
  import { ambience } from '../lib/ambience/store.svelte'

  interface Props {
    book: Book
    startAt?: string
    onclose: () => void
  }

  let { book, startAt, onclose }: Props = $props()

  const GAP = 16
  const PADDING = 24
  const SAVE_DELAY_MS = 800
  const ZOOM_STEPS = [0.5, 0.67, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4]
  /** How many screens above and below the viewport stay rendered. */
  const OVERSCAN = 1

  type FitMode = 'width' | 'page' | 'custom'
  type Panel = 'outline' | 'search' | 'view' | 'ambience' | null
  interface OutlineItem {
    title: string
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    dest: any
    items: OutlineItem[]
  }

  let doc: PDFDocumentProxy | null = null
  let count = $state(0)
  let status = $state<'loading' | 'ready' | 'error'>('loading')
  let error = $state('')
  /** Unscaled page sizes (points); filled from page 1, refined as pages load. */
  let sizes = $state<{ w: number; h: number }[]>([])
  let fit = $state<FitMode>('width')
  let customScale = $state(1)
  let current = $state(0)
  let panel = $state<Panel>(null)
  let barsVisible = $state(true)
  let outline = $state<OutlineItem[]>([])
  let scroller = $state<HTMLDivElement>()
  let viewWidth = $state(0)
  let viewHeight = $state(0)
  let scrollTop = $state(0)
  let pageInput = $state('')

  const title = $derived(book.metadata.title ?? book.path.split('/').pop() ?? '')
  const s = $derived(readerSettings.value)
  const theme = $derived(THEMES[s.theme])
  const FILTERS: Record<ThemeId, string> = {
    day: 'none',
    sepia: 'sepia(0.35) contrast(0.95)',
    night: 'invert(0.9) hue-rotate(180deg)',
    console: 'invert(1) hue-rotate(180deg) contrast(1.15)',
  }

  const widest = $derived(Math.max(1, ...sizes.map((p) => p.w)))
  const scale = $derived.by(() => {
    if (fit === 'custom') return customScale
    const byWidth = Math.max(0.2, (viewWidth - PADDING * 2) / widest)
    if (fit === 'width') return Math.min(byWidth, 4)
    const first = sizes[current] ?? sizes[0] ?? { w: 1, h: 1 }
    return Math.min(byWidth, Math.max(0.2, (viewHeight - PADDING * 2) / first.h))
  })
  /** Top of each page in the scroll area. */
  const tops = $derived.by(() => {
    const out: number[] = []
    let y = PADDING
    for (const p of sizes) {
      out.push(y)
      y += p.h * scale + GAP
    }
    out.push(y - GAP + PADDING)
    return out
  })
  const totalHeight = $derived(tops.at(-1) ?? 0)

  function pageAt(y: number): number {
    let lo = 0
    let hi = count - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (tops[mid] <= y) lo = mid
      else hi = mid - 1
    }
    return Math.max(0, lo)
  }

  // Current page: the one crossing a line a third down the viewport.
  $effect(() => {
    if (status === 'ready') current = pageAt(scrollTop + viewHeight / 3)
  })

  // The reading position as page + fraction of it, so it survives any change
  // of scale (zoom, a panel opening, the window resizing).
  let anchor = { page: 0, within: 0 }
  let lastScale = 0

  function saveAnchor() {
    if (!scroller || count === 0) return
    const page = pageAt(scroller.scrollTop)
    anchor = { page, within: (scroller.scrollTop - tops[page]) / Math.max(1, sizes[page].h * scale) }
  }

  $effect(() => {
    const now = scale
    void tops
    if (status !== 'ready' || !scroller) return
    if (lastScale && Math.abs(now - lastScale) > 0.0001) {
      const { page, within } = anchor
      scroller.scrollTop = tops[page] + within * sizes[page].h * now
    }
    lastScale = now
  })

  // ── Rendering ────────────────────────────────────────────────────────────
  interface Rendered {
    scale: number
    cancel: () => void
  }
  const rendered = new Map<number, Rendered>()
  const textCache = new Map<number, string>()
  let highlightQuery = $state('')

  function slot(p: number) {
    return scroller?.querySelector<HTMLElement>(`[data-page="${p}"]`) ?? null
  }

  function release(p: number) {
    rendered.get(p)?.cancel()
    rendered.delete(p)
    const el = slot(p)
    el?.querySelector('canvas')?.remove()
    el?.querySelector('.textLayer')?.remove()
  }

  async function render(p: number, at: number) {
    if (!doc) return
    const el = slot(p)
    if (!el) return
    let cancelled = false
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let task: any = null
    rendered.set(p, { scale: at, cancel: () => ((cancelled = true), task?.cancel()) })
    try {
      const page = await doc.getPage(p + 1)
      if (cancelled) return
      const base = page.getViewport({ scale: 1 })
      if (Math.abs(base.width - sizes[p].w) > 0.5 || Math.abs(base.height - sizes[p].h) > 0.5) {
        sizes[p] = { w: base.width, h: base.height }
      }
      const viewport = page.getViewport({ scale: at })
      const dpr = window.devicePixelRatio || 1
      const canvas = document.createElement('canvas')
      canvas.width = Math.floor(viewport.width * dpr)
      canvas.height = Math.floor(viewport.height * dpr)
      task = page.render({ canvas, viewport, transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined })
      await task.promise
      if (cancelled) return
      el.querySelector('canvas')?.remove()
      el.querySelector('.textLayer')?.remove()
      el.prepend(canvas)
      const layer = document.createElement('div')
      layer.className = 'textLayer'
      layer.style.setProperty('--scale-factor', String(at))
      layer.style.setProperty('--total-scale-factor', String(at))
      el.append(layer)
      await new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container: layer, viewport }).render()
      markHits(layer)
    } catch (e) {
      if (!cancelled && (e as { name?: string })?.name !== 'RenderingCancelledException') {
        console.warn(`page ${p + 1} could not be rendered`, e)
      }
    }
  }

  // Render what is near the screen; free what is far away.
  $effect(() => {
    if (status !== 'ready') return
    const at = scale
    const from = pageAt(scrollTop - viewHeight * OVERSCAN)
    const to = pageAt(scrollTop + viewHeight * (1 + OVERSCAN))
    untrack(() => {
      for (const p of [...rendered.keys()]) if (p < from - 1 || p > to + 1) release(p)
      for (let p = from; p <= to; p++) {
        const r = rendered.get(p)
        if (!r || Math.abs(r.scale - at) > 0.001) render(p, at)
      }
    })
  })

  // ── Navigation and zoom ──────────────────────────────────────────────────
  function goToPage(p: number) {
    const target = Math.max(0, Math.min(count - 1, p))
    anchor = { page: target, within: 0 }
    if (scroller) scroller.scrollTop = tops[target] - PADDING / 2
  }

  /** Changes the zoom; the anchor effect keeps the reading position. */
  function zoomTo(mode: FitMode, value?: number) {
    saveAnchor()
    fit = mode
    if (value) customScale = Math.max(0.25, Math.min(5, value))
  }

  function zoomStep(dir: 1 | -1) {
    const now = scale
    const next = dir > 0 ? ZOOM_STEPS.find((z) => z > now + 0.01) : [...ZOOM_STEPS].reverse().find((z) => z < now - 0.01)
    if (next) zoomTo('custom', next)
  }

  function onWheel(e: WheelEvent) {
    if (!e.ctrlKey) return
    e.preventDefault()
    zoomStep(e.deltaY < 0 ? 1 : -1)
  }

  function onKey(e: KeyboardEvent) {
    if (status !== 'ready' || !scroller) return
    const target = e.target as HTMLElement | null
    if (target?.closest?.('input, select, textarea')) {
      if (e.key === 'Escape') panel = null
      return
    }
    const ctrl = e.ctrlKey || e.metaKey
    if (ctrl && (e.key === '=' || e.key === '+')) zoomStep(1)
    else if (ctrl && e.key === '-') zoomStep(-1)
    else if (ctrl && e.key === '0') zoomTo('width')
    else if (ctrl && e.key === 'f') panel = 'search'
    else if (e.key === 'PageDown' || (e.key === ' ' && !e.shiftKey)) scroller.scrollBy({ top: viewHeight - 60 })
    else if (e.key === 'PageUp' || (e.key === ' ' && e.shiftKey)) scroller.scrollBy({ top: -(viewHeight - 60) })
    else if (e.key === 'ArrowRight') goToPage(current + 1)
    else if (e.key === 'ArrowLeft') goToPage(current - 1)
    else if (e.key === 'Home') goToPage(0)
    else if (e.key === 'End') goToPage(count - 1)
    else if (e.key === 'Escape') {
      if (defining) defining = null
      else if (selection) selection = null
      else if (panel) panel = null
      else onclose()
    } else return
    e.preventDefault()
  }

  // ── Outline ──────────────────────────────────────────────────────────────
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  async function goToDest(dest: any) {
    if (!doc || !dest) return
    try {
      const explicit = typeof dest === 'string' ? await doc.getDestination(dest) : dest
      if (!explicit) return
      const ref = explicit[0]
      const index = typeof ref === 'number' ? ref : await doc.getPageIndex(ref)
      panel = null
      goToPage(index)
    } catch (e) {
      console.warn('could not follow the outline entry', e)
    }
  }

  // ── Search ───────────────────────────────────────────────────────────────
  const fold = (t: string) => t.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()

  async function pageText(p: number): Promise<string> {
    const cached = textCache.get(p)
    if (cached !== undefined) return cached
    const page = await doc!.getPage(p + 1)
    const content = await page.getTextContent()
    const text = content.items.map((i) => ('str' in i ? i.str : '')).join(' ').replace(/\s+/g, ' ')
    textCache.set(p, text)
    return text
  }

  let query = $state('')
  let results = $state<{ page: number; snippet: string }[]>([])
  let searching = $state(false)
  let searchProgress = $state(0)
  let searchRun = 0

  async function search(e?: Event) {
    e?.preventDefault()
    const q = fold(query.trim())
    const run = ++searchRun
    results = []
    highlightQuery = q
    if (q.length < 2 || !doc) return
    searching = true
    for (let p = 0; p < count && run === searchRun; p++) {
      const text = await pageText(p)
      const folded = fold(text)
      let at = folded.indexOf(q)
      while (at >= 0 && results.length < 300) {
        const start = Math.max(0, at - 40)
        results = [...results, { page: p, snippet: (start ? '…' : '') + text.slice(start, at + q.length + 60) + '…' }]
        at = folded.indexOf(q, at + q.length)
      }
      searchProgress = (p + 1) / count
    }
    if (run === searchRun) searching = false
    for (const [p] of rendered) slot(p)?.querySelectorAll('.textLayer').forEach((l) => markHits(l as HTMLElement))
  }

  function markHits(layer: HTMLElement) {
    const q = untrack(() => highlightQuery)
    layer.querySelectorAll('span.hit').forEach((s) => s.classList.remove('hit'))
    if (q.length < 2) return
    for (const span of layer.querySelectorAll('span')) if (fold(span.textContent ?? '').includes(q)) span.classList.add('hit')
  }

  // ── Selection: copy and dictionary ───────────────────────────────────────
  let selection = $state<{ x: number; y: number; text: string } | null>(null)
  let defining = $state<{ word: string; x: number; y: number } | null>(null)

  function onPointerUp() {
    setTimeout(() => {
      const sel = window.getSelection()
      const text = sel?.toString().trim() ?? ''
      if (!sel || sel.isCollapsed || !text || !scroller?.contains(sel.anchorNode)) {
        selection = null
        return
      }
      const r = sel.getRangeAt(0).getBoundingClientRect()
      defining = null
      selection = { x: r.left + r.width / 2, y: r.top, text }
    }, 10)
  }

  async function copySelection() {
    if (selection) await navigator.clipboard?.writeText(selection.text).catch(() => {})
    window.getSelection()?.removeAllRanges()
    selection = null
  }

  function define() {
    const word = selection && wordFromSelection(selection.text)
    if (!selection || !word) return
    defining = { word, x: selection.x, y: selection.y }
    window.getSelection()?.removeAllRanges()
    selection = null
  }

  // ── Progress ─────────────────────────────────────────────────────────────
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let dirty = false

  function flush() {
    clearTimeout(saveTimer)
    if (!dirty || count === 0) return
    dirty = false
    const p = untrack(() => current)
    const last = Math.max(0, count - 1)
    api.setProgress(book.id, pageLocator(p), last ? p / last : 1, p >= last).catch((e) => console.error('saving progress failed', e))
  }

  let lastPage = -1
  $effect(() => {
    void current
    if (untrack(() => status) !== 'ready') return
    if (lastPage >= 0 && current !== lastPage) ambience.pageTurned()
    lastPage = current
    dirty = true
    clearTimeout(saveTimer)
    saveTimer = setTimeout(flush, SAVE_DELAY_MS)
  })

  function jump(e: Event) {
    e.preventDefault()
    const n = Number(pageInput)
    if (Number.isFinite(n)) goToPage(n - 1)
    pageInput = ''
    ;(document.activeElement as HTMLElement | null)?.blur()
  }

  // ── Loading ──────────────────────────────────────────────────────────────
  onMount(() => {
    window.addEventListener('keydown', onKey)
    ambience.bookOpened()
    ;(async () => {
      await readerSettings.load()
      doc = await openPdf(book.id)
      const first = (await doc.getPage(1)).getViewport({ scale: 1 })
      count = doc.numPages
      sizes = Array.from({ length: count }, () => ({ w: first.width, h: first.height }))
      doc.getOutline().then((o) => (outline = (o ?? []) as OutlineItem[])).catch(() => {})
      status = 'ready'
      const saved = pageFromLocator(startAt) ?? pageFromLocator(book.progress?.locator) ?? 0
      queueMicrotask(() => goToPage(Math.min(saved, count - 1)))
      // PDFs get their cover from page 1 the first time they are opened.
      if (!book.has_cover) {
        renderCover(doc)
          .then((jpeg) => api.setCover(book.id, jpeg))
          .then(() => library.changed())
          .catch((e) => console.warn('could not make a cover for this PDF', e))
      }
    })().catch((e) => {
      status = 'error'
      error = errorMessage(e)
    })
  })

  onDestroy(() => {
    window.removeEventListener('keydown', onKey)
    flush()
    searchRun++
    for (const p of [...rendered.keys()]) release(p)
    doc?.loadingTask.destroy()
  })
</script>

{#snippet outlineItems(items: OutlineItem[], depth: number)}
  <ul style:--depth={depth}>
    {#each items as item, i (i)}
      <li>
        <button class="entry" onclick={() => goToDest(item.dest)}>{item.title || 'Untitled'}</button>
        {#if item.items?.length}{@render outlineItems(item.items, depth + 1)}{/if}
      </li>
    {/each}
  </ul>
{/snippet}

<div
  class="reader"
  role="dialog"
  aria-modal="true"
  aria-label="Reading {title}"
  in:rise={{ y: 0, scale: 0.985, duration: 320 }}
  style:--r-bg={theme.dark ? theme.bg : theme.chrome}
  style:--r-fg={theme.fg}
  style:--r-link={theme.link}
  style:--r-chrome={theme.chrome}
  style:--r-border={theme.chromeBorder}
  style:--r-muted={theme.muted}
  style:--r-hover={theme.dark ? 'rgb(255 255 255 / 0.06)' : 'rgb(0 0 0 / 0.05)'}
  style:--page-filter={FILTERS[s.theme]}
>
  <header class:hidden={!barsVisible}>
    <button class="icon" onclick={onclose} title="Back to the library (Esc)" aria-label="Back to the library"
      ><Icon name="chevronLeft" size={20} /></button
    >
    <div class="heading">
      <span class="title">{title}</span>
      {#if count}<span class="sub">Page {current + 1} of {count}</span>{/if}
    </div>
    <div class="zoom" role="group" aria-label="Zoom">
      <button class="icon" onclick={() => zoomStep(-1)} title="Zoom out (Ctrl −)" aria-label="Zoom out"><Icon name="minus" size={16} /></button>
      <span class="zoom-value">{Math.round(scale * 100)}%</span>
      <button class="icon" onclick={() => zoomStep(1)} title="Zoom in (Ctrl +)" aria-label="Zoom in"><Icon name="plus" size={16} /></button>
    </div>
    <button class="icon" class:on={panel === 'outline'} onclick={() => (panel = panel === 'outline' ? null : 'outline')} title="Contents" aria-label="Contents"
      ><Icon name="menu" size={18} /></button
    >
    <button class="icon" class:on={panel === 'search'} onclick={() => (panel = panel === 'search' ? null : 'search')} title="Search (Ctrl+F)" aria-label="Search in document"
      ><Icon name="search" size={18} /></button
    >
    <button class="icon" class:on={panel === 'ambience'} class:playing={ambience.playing} onclick={() => (panel = panel === 'ambience' ? null : 'ambience')} title="Ambience" aria-label="Ambience"
      ><Icon name="sound" size={18} /></button
    >
    <button class="icon" class:on={panel === 'view'} onclick={() => (panel = panel === 'view' ? null : 'view')} title="View settings" aria-label="View settings"
      ><Icon name="settings" size={18} /></button
    >
  </header>

  <div class="stage">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="scroller"
      bind:this={scroller}
      bind:clientWidth={viewWidth}
      bind:clientHeight={viewHeight}
      onscroll={() => {
        if (!scroller) return
        scrollTop = scroller.scrollTop
        saveAnchor()
      }}
      onwheel={onWheel}
      onpointerup={onPointerUp}
      ondblclick={() => (barsVisible = !barsVisible)}
    >
      {#if status === 'loading'}
        <p class="message">Opening PDF…</p>
      {:else if status === 'error'}
        <div class="message">
          <p class="error">This PDF could not be opened.</p>
          <p>{error}</p>
          <button onclick={onclose}>Back to the library</button>
        </div>
      {:else}
        <div class="column" style:height="{totalHeight}px">
          {#each sizes as size, p (p)}
            <div
              class="page"
              data-page={p}
              style:top="{tops[p]}px"
              style:width="{size.w * scale}px"
              style:height="{size.h * scale}px"
              aria-label="Page {p + 1}"
            ></div>
          {/each}
        </div>
      {/if}
    </div>

    {#if panel && status === 'ready'}
      <aside class="panel" aria-label={panel}>
        {#if panel === 'outline'}
          <h3>Contents</h3>
          {#if outline.length}
            {@render outlineItems(outline, 0)}
          {:else}
            <p class="muted-text">This PDF has no table of contents.</p>
          {/if}
        {:else if panel === 'ambience'}
          <h3>Ambience</h3>
          <AmbiencePanel compact />
        {:else if panel === 'search'}
          <h3>Search</h3>
          <form onsubmit={search}>
            <!-- svelte-ignore a11y_autofocus -->
            <input type="search" placeholder="Search in this document" bind:value={query} autofocus />
            <button type="submit" disabled={query.trim().length < 2}>Search</button>
          </form>
          {#if searching}<div class="bar"><div style:width="{searchProgress * 100}%"></div></div>{/if}
          {#if !searching && highlightQuery.length >= 2}
            <p class="muted-text">{results.length ? `${results.length}${results.length >= 300 ? '+' : ''} matches` : 'No matches.'}</p>
          {/if}
          {#each results as r, i (i)}
            <button class="hit-row" onclick={() => goToPage(r.page)}>
              <span class="hit-page">Page {r.page + 1}</span>
              <span>{r.snippet}</span>
            </button>
          {/each}
        {:else}
          <h3>View</h3>
          <span class="label">Fit</span>
          <div class="segmented">
            <button class:on={fit === 'width'} onclick={() => zoomTo('width')}>Width</button>
            <button class:on={fit === 'page'} onclick={() => zoomTo('page')}>Whole page</button>
          </div>
          <span class="label">Theme</span>
          <div class="themes">
            {#each Object.entries(THEMES) as [id, t] (id)}
              <button
                class="theme"
                class:on={s.theme === id}
                style:background={t.bg}
                style:color={t.fg}
                aria-label="{t.label} theme"
                title={t.label}
                onclick={() => readerSettings.update({ theme: id as ThemeId })}>Aa</button
              >
            {/each}
          </div>
          <p class="muted-text small">Night and console themes invert the page colours.</p>
        {/if}
      </aside>
    {/if}
  </div>

  <footer class:hidden={!barsVisible}>
    <form class="goto" onsubmit={jump}>
      <span>Page</span>
      <input inputmode="numeric" placeholder={String(current + 1)} bind:value={pageInput} aria-label="Go to page" />
      <span>of {count}</span>
    </form>
    <input
      type="range"
      min="0"
      max={Math.max(0, count - 1)}
      value={current}
      onchange={(e) => goToPage(Number(e.currentTarget.value))}
      disabled={status !== 'ready'}
      aria-label="Position in document"
    />
    <span class="meta">{count > 1 ? Math.round((current / (count - 1)) * 100) : 100}%</span>
  </footer>

  {#if selection}
    <SelectionMenu
      x={selection.x}
      y={selection.y}
      annotate={false}
      onhighlight={() => {}}
      onnote={() => {}}
      oncopy={copySelection}
      ondefine={wordFromSelection(selection.text) ? define : undefined}
    />
  {/if}
  {#if defining}
    {#key defining.word}
      <DictionaryCard
        word={defining.word}
        x={defining.x}
        y={defining.y}
        online={ui.value.onlineDictionary}
        bookLanguage={book.metadata.language}
        onclose={() => (defining = null)}
      />
    {/key}
  {/if}
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
    gap: 8px;
    padding: 6px 12px;
    background: var(--r-chrome);
    transition:
      margin 160ms ease,
      opacity 160ms ease;
    z-index: 2;
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
    margin-bottom: -45px;
    opacity: 0;
    pointer-events: none;
  }

  .icon {
    display: grid;
    place-items: center;
    min-width: 36px;
    height: 36px;
    border: none;
    background: transparent;
    color: var(--r-fg);
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

  .title {
    max-width: 100%;
    font-family: var(--serif);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sub {
    font-size: 12px;
    color: var(--r-muted);
    font-variant-numeric: tabular-nums;
  }

  .icon.playing {
    color: var(--r-link);
  }

  .zoom {
    display: flex;
    align-items: center;
    margin-right: 6px;
    border: 1px solid var(--r-border);
    border-radius: 10px;
  }

  .zoom .icon {
    min-width: 30px;
    height: 30px;
  }

  .zoom-value {
    min-width: 46px;
    text-align: center;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .stage {
    position: relative;
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .scroller {
    position: relative;
    flex: 1;
    overflow: auto;
  }

  .column {
    position: relative;
    min-width: 100%;
  }

  .page {
    position: absolute;
    left: 50%;
    translate: -50% 0;
    background: #fff;
    box-shadow: 0 2px 14px rgb(0 0 0 / 0.25);
    overflow: hidden;
  }

  .page :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
    filter: var(--page-filter);
  }

  .page :global(.textLayer) {
    position: absolute;
    inset: 0;
    overflow: clip;
    line-height: 1;
    text-size-adjust: none;
    forced-color-adjust: none;
    transform-origin: 0 0;
    z-index: 1;
  }

  .page :global(.textLayer span),
  .page :global(.textLayer br) {
    color: transparent;
    position: absolute;
    white-space: pre;
    cursor: text;
    transform-origin: 0% 0%;
  }

  .page :global(.textLayer > :not(.markedContent)),
  .page :global(.textLayer .markedContent span:not(.markedContent)) {
    --font-height: 0;
    font-size: calc(var(--total-scale-factor) * var(--font-height));
    --scale-x: 1;
    --rotate: 0deg;
    transform: rotate(var(--rotate)) scaleX(var(--scale-x));
  }

  .page :global(.textLayer .markedContent) {
    display: contents;
  }

  .page :global(.textLayer ::selection) {
    background: color-mix(in srgb, var(--r-link) 35%, transparent);
  }

  .page :global(.textLayer .hit) {
    background: rgb(244 207 61 / 0.45);
    border-radius: 2px;
  }

  .message {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--r-muted);
    text-align: center;
  }

  .message .error {
    color: var(--danger);
  }

  .panel {
    width: 320px;
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

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .entry {
    display: block;
    width: 100%;
    padding: 7px 10px 7px calc(10px + var(--depth) * 16px);
    border: none;
    border-radius: 6px;
    background: none;
    color: inherit;
    text-align: left;
  }

  .entry:hover,
  .hit-row:hover {
    background: var(--r-hover);
  }

  form {
    display: flex;
    gap: 6px;
    margin-bottom: 10px;
  }

  form input {
    flex: 1;
    min-width: 0;
    background: var(--r-bg);
    color: var(--r-fg);
    border-color: var(--r-border);
  }

  form button,
  .segmented button {
    background: var(--r-bg);
    color: var(--r-fg);
    border-color: var(--r-border);
  }

  .bar {
    height: 3px;
    margin-bottom: 8px;
    background: var(--r-border);
  }

  .bar div {
    height: 100%;
    background: var(--r-link);
  }

  .hit-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 7px 8px;
    border: none;
    border-radius: 8px;
    background: none;
    color: inherit;
    text-align: left;
    font-size: 13px;
    line-height: 1.4;
  }

  .hit-page {
    font-size: 11px;
    font-weight: 600;
    color: var(--r-link);
  }

  .muted-text {
    color: var(--r-muted);
    font-size: 13px;
  }

  .small {
    font-size: 12px;
  }

  .label {
    display: block;
    margin: 10px 0 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--r-muted);
  }

  .segmented {
    display: flex;
    gap: 4px;
  }

  .segmented button {
    flex: 1;
    padding: 6px;
    border-radius: 8px;
  }

  .segmented .on,
  .theme.on {
    border-color: var(--r-link);
    box-shadow: inset 0 0 0 1px var(--r-link);
    font-weight: 600;
  }

  .themes {
    display: flex;
    gap: 6px;
  }

  .theme {
    flex: 1;
    padding: 6px 0;
    border-radius: 8px;
    border-color: var(--r-border);
    font-family: var(--serif);
  }

  .goto {
    margin: 0;
    align-items: center;
    font-size: 12px;
    color: var(--r-muted);
    white-space: nowrap;
  }

  .goto input {
    width: 52px;
    flex: none;
    padding: 3px 6px;
    text-align: center;
  }

  footer input[type='range'] {
    flex: 1;
    accent-color: var(--r-link);
  }

  .meta {
    min-width: 44px;
    text-align: right;
    font-size: 12px;
    color: var(--r-muted);
    font-variant-numeric: tabular-nums;
  }
</style>
