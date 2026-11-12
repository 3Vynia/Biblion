<script lang="ts">
  // CBZ / CBR reader. Pages load as images from the `comic://` scheme; the
  // position is saved as "page:N".
  import { onDestroy, onMount, untrack } from 'svelte'
  import { fly } from 'svelte/transition'
  import Icon from '../components/Icon.svelte'
  import { api, comicPageUrl, errorMessage } from '../lib/api'
  import { ms, rise } from '../lib/motion'
  import type { Book } from '../lib/types'
  import { pageFromLocator, pageLocator, spreadOf, spreads, wantsDouble } from './comicLayout'
  import { THEMES, type ComicDirection, type ComicFit, type ComicMode, type ComicSpread, type ThemeId } from './readerSettings'
  import { readerSettings } from './settingsStore.svelte'

  interface Props {
    book: Book
    startAt?: string
    onclose: () => void
  }

  let { book, startAt, onclose }: Props = $props()

  const SAVE_DELAY_MS = 800
  const TAP_ZONE = 0.3

  let count = $state(0)
  let status = $state<'loading' | 'ready' | 'error'>('loading')
  let error = $state('')
  let page = $state(0)
  let direction = $state(1)
  let barsVisible = $state(true)
  let showSettings = $state(false)
  let stageWidth = $state(0)
  let stageHeight = $state(0)
  let sliderValue = $state<number | null>(null)
  let broken = $state(new Set<number>())
  let strip = $state<HTMLDivElement>()

  const title = $derived(book.metadata.title ?? book.path.split('/').pop() ?? '')
  const s = $derived(readerSettings.value)
  const theme = $derived(THEMES[s.theme])
  const rtl = $derived(s.comicDirection === 'rtl')
  const double = $derived(s.comicMode === 'pages' && s.comicSpread === 'auto' && wantsDouble(stageWidth, stageHeight))
  const list = $derived(spreads(count, double))
  const spreadIndex = $derived(spreadOf(list, page))
  const visible = $derived(list[spreadIndex] ?? [])

  // ── Progress ─────────────────────────────────────────────────────────────
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let dirty = false

  function flush() {
    clearTimeout(saveTimer)
    if (!dirty || count === 0) return
    dirty = false
    const last = Math.max(0, count - 1)
    const p = untrack(() => page)
    api.setProgress(book.id, pageLocator(p), last ? p / last : 1, p >= last).catch((e) => console.error('saving progress failed', e))
  }

  $effect(() => {
    void page
    if (status !== 'ready') return
    dirty = true
    clearTimeout(saveTimer)
    saveTimer = setTimeout(flush, SAVE_DELAY_MS)
  })

  // Preload the pages around the current spread.
  $effect(() => {
    for (const offset of [1, 2, -1]) {
      const spread = list[spreadIndex + offset]
      for (const p of spread ?? []) new Image().src = comicPageUrl(book.id, p)
    }
  })

  // ── Navigation ───────────────────────────────────────────────────────────
  function goTo(p: number) {
    const target = Math.max(0, Math.min(count - 1, p))
    direction = target >= page ? 1 : -1
    page = target
    if (s.comicMode === 'strip') scrollStripTo(target)
  }

  const next = () => (list[spreadIndex + 1] ? goTo(list[spreadIndex + 1][0]) : undefined)
  const prev = () => (list[spreadIndex - 1] ? goTo(list[spreadIndex - 1][0]) : undefined)
  const goLeft = () => (rtl ? next() : prev())
  const goRight = () => (rtl ? prev() : next())

  function onKey(e: KeyboardEvent) {
    if (status !== 'ready') return
    if ((e.target as HTMLElement | null)?.closest?.('input, select, textarea')) return
    const pages = s.comicMode === 'pages'
    switch (e.key) {
      case 'ArrowLeft':
        if (!pages) return
        goLeft()
        break
      case 'ArrowRight':
        if (!pages) return
        goRight()
        break
      case 'PageDown':
      case ' ':
        if (!pages) return
        next()
        break
      case 'PageUp':
        if (!pages) return
        prev()
        break
      case 'Home':
        goTo(0)
        break
      case 'End':
        goTo(count - 1)
        break
      case 'Escape':
        if (showSettings) showSettings = false
        else onclose()
        break
      default:
        return
    }
    e.preventDefault()
  }

  function onStageClick(e: MouseEvent) {
    if (showSettings) {
      showSettings = false
      return
    }
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const rel = (e.clientX - rect.left) / rect.width
    if (s.comicMode === 'pages' && rel < TAP_ZONE) goLeft()
    else if (s.comicMode === 'pages' && rel > 1 - TAP_ZONE) goRight()
    else barsVisible = !barsVisible
  }

  // ── Strip mode ───────────────────────────────────────────────────────────
  function scrollStripTo(p: number) {
    strip?.querySelector<HTMLElement>(`[data-page="${p}"]`)?.scrollIntoView({ block: 'start' })
  }

  /** Tracks the page at the top of the strip as it scrolls (re-observes when the page count changes). */
  function trackPages(node: HTMLElement, _count: number) {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) page = Number((e.target as HTMLElement).dataset.page)
        }
      },
      { root: node, rootMargin: '0px 0px -70% 0px' },
    )
    const observe = () => node.querySelectorAll('[data-page]').forEach((el) => observer.observe(el))
    observe()
    return { destroy: () => observer.disconnect(), update: observe }
  }

  // Keep the place when switching to the strip.
  $effect(() => {
    if (s.comicMode === 'strip' && status === 'ready') queueMicrotask(() => scrollStripTo(untrack(() => page)))
  })

  function commitSlider() {
    if (sliderValue === null) return
    goTo(sliderValue)
    sliderValue = null
  }

  function pageFailed(p: number) {
    broken = new Set(broken).add(p)
  }

  onMount(() => {
    window.addEventListener('keydown', onKey)
    Promise.all([readerSettings.load(), api.comicPages(book.id)])
      .then(([, pages]) => {
        if (!pages.length) throw new Error('This comic has no pages.')
        count = pages.length
        const saved = pageFromLocator(startAt) ?? pageFromLocator(book.progress?.locator)
        page = Math.min(saved ?? 0, count - 1)
        status = 'ready'
      })
      .catch((e) => {
        status = 'error'
        error = errorMessage(e)
      })
  })

  onDestroy(() => {
    window.removeEventListener('keydown', onKey)
    flush()
  })

  const set = readerSettings.update.bind(readerSettings)
  const MODES: [ComicMode, string][] = [
    ['pages', 'Pages'],
    ['strip', 'Strip'],
  ]
  const FITS: [ComicFit, string][] = [
    ['screen', 'Screen'],
    ['width', 'Width'],
    ['height', 'Height'],
  ]
  const SPREADS: [ComicSpread, string][] = [
    ['auto', 'Two pages'],
    ['single', 'One page'],
  ]
  const DIRECTIONS: [ComicDirection, string][] = [
    ['ltr', 'Left to right'],
    ['rtl', 'Manga (right to left)'],
  ]
</script>

<div
  class="reader"
  role="dialog"
  aria-modal="true"
  aria-label="Reading {title}"
  in:rise={{ y: 0, scale: 0.985, duration: 320 }}
  style:--r-bg={theme.dark ? theme.bg : '#1b1b1d'}
  style:--r-fg={theme.fg}
  style:--r-link={theme.link}
  style:--r-chrome={theme.chrome}
  style:--r-border={theme.chromeBorder}
  style:--r-muted={theme.muted}
  style:--r-hover={theme.dark ? 'rgb(255 255 255 / 0.06)' : 'rgb(0 0 0 / 0.05)'}
>
  <header class:hidden={!barsVisible}>
    <button class="icon" onclick={onclose} title="Back to the library (Esc)" aria-label="Back to the library"
      ><Icon name="chevronLeft" size={20} /></button
    >
    <div class="heading">
      <span class="title">{title}</span>
      {#if count}<span class="sub">Page {Math.min(page + 1, count)} of {count}</span>{/if}
    </div>
    <button
      class="icon"
      class:on={showSettings}
      onclick={() => (showSettings = !showSettings)}
      title="Comic settings"
      aria-label="Comic settings"><Icon name="settings" size={18} /></button
    >
  </header>

  <div class="stage" bind:clientWidth={stageWidth} bind:clientHeight={stageHeight}>
    {#if status === 'loading'}
      <p class="message">Opening comic…</p>
    {:else if status === 'error'}
      <div class="message">
        <p class="error">This comic could not be opened.</p>
        <p>{error}</p>
        <button onclick={onclose}>Back to the library</button>
      </div>
    {:else if s.comicMode === 'pages'}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="pages fit-{s.comicFit}" role="presentation" onclick={onStageClick}>
        {#key spreadIndex}
          <div
            class="spread"
            class:rtl
            class:two={visible.length === 2}
            in:fly={{ x: (rtl ? -direction : direction) * 60, duration: ms(220), opacity: 0 }}
          >
            {#each visible as p (p)}
              {#if broken.has(p)}
                <div class="missing">Page {p + 1} could not be read.</div>
              {:else}
                <img src={comicPageUrl(book.id, p)} alt="Page {p + 1}" draggable="false" onerror={() => pageFailed(p)} />
              {/if}
            {/each}
          </div>
        {/key}
      </div>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="strip" bind:this={strip} use:trackPages={count} role="presentation" onclick={onStageClick}>
        {#each Array.from({ length: count }, (_, i) => i) as p (p)}
          <div class="strip-page" data-page={p}>
            {#if broken.has(p)}
              <div class="missing">Page {p + 1} could not be read.</div>
            {:else}
              <img src={comicPageUrl(book.id, p)} alt="Page {p + 1}" loading="lazy" draggable="false" onerror={() => pageFailed(p)} />
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if showSettings}
      <aside class="settings" aria-label="Comic settings" in:rise={{ y: -6, duration: 180 }}>
        <span class="label">Layout</span>
        <div class="segmented">
          {#each MODES as [id, label] (id)}
            <button class:on={s.comicMode === id} onclick={() => set({ comicMode: id })}>{label}</button>
          {/each}
        </div>
        {#if s.comicMode === 'pages'}
          <span class="label">Fit</span>
          <div class="segmented">
            {#each FITS as [id, label] (id)}
              <button class:on={s.comicFit === id} onclick={() => set({ comicFit: id })}>{label}</button>
            {/each}
          </div>
          <span class="label">Spread</span>
          <div class="segmented">
            {#each SPREADS as [id, label] (id)}
              <button class:on={s.comicSpread === id} onclick={() => set({ comicSpread: id })}>{label}</button>
            {/each}
          </div>
        {/if}
        <span class="label">Direction</span>
        <div class="segmented column">
          {#each DIRECTIONS as [id, label] (id)}
            <button class:on={s.comicDirection === id} onclick={() => set({ comicDirection: id })}>{label}</button>
          {/each}
        </div>
        <span class="label">Theme</span>
        <div class="themes">
          {#each Object.entries(THEMES) as [id, t] (id)}
            <button
              class="theme"
              class:on={s.theme === id}
              style:background={t.bg}
              style:color={t.fg}
              title={t.label}
              aria-label="{t.label} theme"
              onclick={() => set({ theme: id as ThemeId })}>Aa</button
            >
          {/each}
        </div>
      </aside>
    {/if}
  </div>

  <footer class:hidden={!barsVisible}>
    <span class="meta">{count ? `${Math.min((sliderValue ?? page) + 1, count)} / ${count}` : ''}</span>
    <input
      type="range"
      min="0"
      max={Math.max(0, count - 1)}
      step="1"
      value={sliderValue ?? page}
      dir={rtl ? 'rtl' : 'ltr'}
      oninput={(e) => (sliderValue = Number(e.currentTarget.value))}
      onchange={commitSlider}
      disabled={status !== 'ready'}
      aria-label="Page"
    />
    <span class="meta right">{count ? `${Math.round((count > 1 ? page / (count - 1) : 1) * 100)}%` : ''}</span>
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
    margin-bottom: -41px;
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

  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .pages {
    position: absolute;
    inset: 0;
    display: flex;
    overflow: auto;
    user-select: none;
  }

  .spread {
    display: flex;
    gap: 2px;
    margin: auto;
  }

  .spread.rtl {
    flex-direction: row-reverse;
  }

  .spread img {
    display: block;
    box-shadow: 0 6px 30px rgb(0 0 0 / 0.45);
  }

  /* Screen: the whole spread is visible, nothing scrolls. */
  .fit-screen .spread {
    width: 100%;
    height: 100%;
    align-items: center;
    justify-content: center;
    padding: 12px;
  }

  .fit-screen img {
    min-width: 0;
    min-height: 0;
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }

  .fit-screen .two img {
    max-width: calc(50% - 1px);
  }

  .fit-width .spread {
    width: 100%;
    margin-top: 0;
  }

  .fit-width img {
    width: 100%;
    height: auto;
  }

  .fit-width .two img {
    width: 50%;
  }

  .fit-height .spread {
    height: 100%;
  }

  .fit-height img {
    height: 100%;
    width: auto;
  }

  .strip {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .strip-page {
    flex-shrink: 0;
    width: min(100%, 900px);
    min-height: 200px;
  }

  .strip-page img {
    display: block;
    width: 100%;
    height: auto;
  }

  .missing {
    display: grid;
    place-items: center;
    min-width: 240px;
    min-height: 320px;
    padding: 20px;
    border: 1px dashed var(--r-border);
    color: var(--r-muted);
    text-align: center;
  }

  .message {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    text-align: center;
    color: var(--r-muted);
  }

  .message .error {
    color: var(--danger);
  }

  .settings {
    position: absolute;
    top: 10px;
    right: 12px;
    z-index: 3;
    width: 270px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px;
    border-radius: 14px;
    background: var(--r-chrome);
    border: 1px solid var(--r-border);
    box-shadow: 0 14px 40px rgb(0 0 0 / 0.35);
  }

  .label {
    margin-top: 6px;
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

  .segmented.column {
    flex-direction: column;
  }

  .segmented button {
    flex: 1;
    padding: 6px 8px;
    border-radius: 8px;
    background: transparent;
    color: var(--r-fg);
    border-color: var(--r-border);
    font-size: 13px;
  }

  .segmented button.on,
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

  .meta {
    min-width: 80px;
    font-size: 12px;
    color: var(--r-muted);
    font-variant-numeric: tabular-nums;
  }

  .meta.right {
    text-align: right;
  }

  input[type='range'] {
    flex: 1;
    accent-color: var(--r-link);
  }
</style>
