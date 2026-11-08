<script lang="ts">
  // EPUB / MOBI / AZW3 reader built on foliate-js. The book file is fetched
  // from the `book://` scheme; the position is saved as an EPUB CFI.
  import { onDestroy, onMount, untrack } from 'svelte'
  import { api, bookUrl, errorMessage } from '../lib/api'
  import type { Book } from '../lib/types'

  interface Props {
    book: Book
    onclose: () => void
  }

  let { book, onclose }: Props = $props()

  const SAVE_DELAY_MS = 1000
  /** Treat the book as finished from this fraction on. */
  const FINISHED_AT = 0.995

  // The foliate-view element (an untyped custom element).
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let view: any = null
  let host: HTMLDivElement
  let status = $state<'loading' | 'ready' | 'error'>('loading')
  let error = $state('')
  // The reader is recreated for each book, so the initial value is enough.
  let fraction = $state(untrack(() => book.progress?.percent ?? 0))
  let chapter = $state('')

  let pending: { cfi: string; fraction: number } | null = null
  let saveTimer: ReturnType<typeof setTimeout> | undefined

  const title = $derived(book.metadata.title ?? book.path.split('/').pop() ?? '')

  const READER_CSS = `
    html { color-scheme: light; }
    body { line-height: 1.55; hyphens: auto; }
    p { text-align: justify; }
    img, svg { max-width: 100%; height: auto; }
  `

  function flushProgress() {
    clearTimeout(saveTimer)
    if (!pending) return
    const { cfi, fraction: f } = pending
    pending = null
    api.setProgress(book.id, cfi, f, f >= FINISHED_AT).catch((e) => console.error('saving progress failed', e))
  }

  function onRelocate(e: CustomEvent) {
    const { fraction: f, cfi, tocItem } = e.detail ?? {}
    if (typeof f === 'number') fraction = f
    chapter = tocItem?.label?.trim() ?? ''
    if (cfi) {
      pending = { cfi, fraction: typeof f === 'number' ? f : fraction }
      clearTimeout(saveTimer)
      saveTimer = setTimeout(flushProgress, SAVE_DELAY_MS)
    }
  }

  function onKey(e: KeyboardEvent) {
    if (!view || status !== 'ready') return
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
        onclose()
        break
      default:
        return
    }
    e.preventDefault()
  }

  async function load() {
    await import('../../vendor/foliate-js/view.js')
    const res = await fetch(bookUrl(book.id))
    if (!res.ok) throw new Error((await res.text()) || `HTTP ${res.status}`)
    const blob = await res.blob()
    const file = new File([blob], `book.${book.format}`, { type: blob.type })

    view = document.createElement('foliate-view')
    view.addEventListener('relocate', onRelocate)
    // Key presses inside the book's iframe do not reach the window.
    view.addEventListener('load', (e: CustomEvent) => e.detail?.doc?.addEventListener('keydown', onKey))
    host.append(view)
    await view.open(file)
    view.renderer.setAttribute('max-inline-size', '720px')
    view.renderer.setAttribute('gap', '6%')
    view.renderer.setStyles?.(READER_CSS)

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
    view?.close?.()
    view?.remove()
  })
</script>

<div class="reader" role="dialog" aria-modal="true" aria-label="Reading {title}">
  <header>
    <button class="ghost" onclick={onclose}>← Library</button>
    <div class="heading">
      <span class="title">{title}</span>
      {#if chapter}<span class="chapter muted">{chapter}</span>{/if}
    </div>
    <span class="muted percent">{Math.round(fraction * 100)}%</span>
  </header>

  <div class="stage">
    <button class="nav left" onclick={() => view?.goLeft()} aria-label="Previous page" disabled={status !== 'ready'}>‹</button>
    <div class="book" bind:this={host}>
      {#if status === 'loading'}
        <p class="message muted">Opening book…</p>
      {:else if status === 'error'}
        <div class="message">
          <p class="error">This book could not be opened.</p>
          <p class="muted">{error}</p>
          <button onclick={onclose}>Back to the library</button>
        </div>
      {/if}
    </div>
    <button class="nav right" onclick={() => view?.goRight()} aria-label="Next page" disabled={status !== 'ready'}>›</button>
  </div>

  <div class="progress" aria-hidden="true"><div style:width="{fraction * 100}%"></div></div>
</div>

<style>
  .reader {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    background: #fbf8f1;
  }

  header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
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
  }

  .percent {
    min-width: 48px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .stage {
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

  .nav {
    width: 56px;
    border: none;
    border-radius: 0;
    background: transparent;
    font-size: 32px;
    color: var(--muted);
  }

  .nav:hover:not(:disabled) {
    background: rgb(0 0 0 / 0.03);
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
  }

  .progress {
    height: 3px;
    background: var(--surface-2);
  }

  .progress div {
    height: 100%;
    background: var(--accent);
  }
</style>
