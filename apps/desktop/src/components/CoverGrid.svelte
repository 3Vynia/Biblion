<script lang="ts">
  // Virtualised cover grid: only the rows in view (plus a few above and
  // below) exist in the DOM, whatever the size of the library. Data comes
  // from a PageCache that loads pages of 200 books on demand.
  import type { PageCache } from '../lib/pageCache'
  import type { BookSummary } from '../lib/types'
  import { CAPTION, GAP, gridMetrics, visibleRows } from '../lib/gridLayout'
  import BookCover from './BookCover.svelte'
  import { COVER_WIDTH, ui } from '../lib/uiSettings.svelte'

  interface Props {
    cache: PageCache<BookSummary>
    /** Bumped by the parent whenever the cache changes. */
    version: number
    selectedId: number | null
    onselect: (book: BookSummary) => void
    onopen: (book: BookSummary) => void
    /** Selection mode: clicks tick books instead of opening them. */
    selecting?: boolean
    picked?: ReadonlySet<number>
    ontoggle?: (book: BookSummary) => void
  }

  let { cache, version, selectedId, onselect, onopen, selecting = false, picked, ontoggle }: Props = $props()

  const PADDING = 24

  let viewport: HTMLDivElement
  let width = $state(0)
  let height = $state(0)
  let scrollTop = $state(0)

  const total = $derived.by(() => (void version, cache.total ?? 0))
  const captions = $derived(ui.value.showCaptions)
  const metrics = $derived(gridMetrics(width - PADDING * 2, total, COVER_WIDTH[ui.value.coverSize], captions ? CAPTION : 0))
  const range = $derived(visibleRows(metrics, Math.max(0, scrollTop - PADDING), height))

  $effect(() => {
    void version
    const cols = metrics.columns
    if (cache.total === null) {
      // First page of a new query: load what fits on screen.
      const rowsOnScreen = Math.ceil(height / metrics.rowHeight) + 1
      cache.ensure(0, cols * rowsOnScreen)
    } else if (range.lastRow >= range.firstRow) {
      cache.ensure(range.firstRow * cols, (range.lastRow + 1) * cols)
    }
  })

  const cells = $derived.by(() => {
    void version
    const out: { index: number; row: number; col: number; book: BookSummary | undefined }[] = []
    for (let row = range.firstRow; row <= range.lastRow; row++) {
      for (let col = 0; col < metrics.columns; col++) {
        const index = row * metrics.columns + col
        if (index >= total) break
        out.push({ index, row, col, book: cache.get(index) })
      }
    }
    return out
  })

  // ── Keyboard: arrows move between books, Enter reads, Space selects ──
  let cursor = $state(0)
  // Roving focus: one card is reachable with Tab, the cursor's or else the
  // first one on screen.
  const tabStop = $derived(cells.find((c) => c.index === cursor && c.book)?.index ?? cells.find((c) => c.book)?.index ?? -1)

  /** Scrolls a book into view and focuses its card once it is loaded. */
  function moveTo(index: number) {
    const last = total - 1
    if (last < 0) return
    index = Math.max(0, Math.min(last, index))
    cursor = index
    const row = Math.floor(index / metrics.columns)
    const top = PADDING + row * metrics.rowHeight
    if (top < viewport.scrollTop) viewport.scrollTop = top - PADDING
    else if (top + metrics.rowHeight > viewport.scrollTop + height) viewport.scrollTop = top + metrics.rowHeight - height + PADDING
    scrollTop = viewport.scrollTop
    cache.ensure(index, index + 1)
    const started = performance.now()
    const focus = () => {
      if (cursor !== index) return
      const card = viewport.querySelector<HTMLButtonElement>(`[data-index="${index}"]`)
      if (card) {
        card.focus({ preventScroll: true })
        // The details panel, when open, follows the cursor.
        const book = cache.get(index)
        if (book && selectedId !== null && !selecting) onselect(book)
      } else if (performance.now() - started < 3000) requestAnimationFrame(focus)
    }
    requestAnimationFrame(focus)
  }

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey || e.altKey) return
    const cols = metrics.columns
    const page = Math.max(1, Math.floor(height / metrics.rowHeight)) * cols
    const moves: Record<string, number> = {
      ArrowRight: 1,
      ArrowLeft: -1,
      ArrowDown: cols,
      ArrowUp: -cols,
      PageDown: page,
      PageUp: -page,
    }
    if (e.key in moves) moveTo(cursor + moves[e.key])
    else if (e.key === 'Home') moveTo(0)
    else if (e.key === 'End') moveTo(total - 1)
    else if (e.key === 'Enter' && !selecting) {
      const book = cache.get(cursor)
      if (book) onopen(book)
    } else return
    e.preventDefault()
  }

  export function scrollToTop() {
    if (viewport) viewport.scrollTop = 0
    scrollTop = 0
  }
</script>

<div
  class="viewport"
  bind:this={viewport}
  bind:clientWidth={width}
  bind:clientHeight={height}
  onscroll={() => (scrollTop = viewport.scrollTop)}
  role="grid"
  aria-rowcount={metrics.rows}
  aria-label="Books"
  tabindex="-1"
  onkeydown={onKey}
  onfocusin={(e) => {
    const i = (e.target as HTMLElement).dataset?.index
    if (i !== undefined) cursor = Number(i)
  }}
>
  <div class="spacer" style:height="{metrics.totalHeight + PADDING * 2}px">
    {#each cells as cell (cell.index)}
      {@const x = PADDING + cell.col * (metrics.cardWidth + GAP)}
      {@const y = PADDING + cell.row * metrics.rowHeight}
      <div
        class="cell"
        style:width="{metrics.cardWidth}px"
        style:transform="translate({x}px, {y}px)"
        role="gridcell"
        aria-rowindex={cell.row + 1}
      >
        {#if cell.book}
          {@const book = cell.book}
          {@const isPicked = selecting && !!picked?.has(book.id)}
          <button
            class="card"
            class:selected={!selecting && book.id === selectedId}
            class:selecting
            class:picked={isPicked}
            aria-pressed={selecting ? isPicked : undefined}
            onclick={() => (selecting ? ontoggle?.(book) : onselect(book))}
            ondblclick={() => !selecting && onopen(book)}
            data-index={cell.index}
            tabindex={cell.index === tabStop ? 0 : -1}
            title={book.title}
          >
            {#if selecting}
              <span class="tick" aria-hidden="true">
                {#if isPicked}<svg viewBox="0 0 24 24" width="14" height="14"><path d="M5 12.5 10 17 19 7" /></svg>{/if}
              </span>
            {/if}
            <div class="cover" style:height="{metrics.coverHeight}px">
              <BookCover title={book.title} author={book.authors[0]} coverKey={book.cover_key} />
              {#if book.favorite}<span class="fav" aria-label="Favourite">♥</span>{/if}
              {#if book.progress !== null && book.progress > 0}
                <div class="progress" style:width="{Math.round(book.progress * 100)}%"></div>
              {/if}
              <span class="badge">{book.format.toUpperCase()}</span>
            </div>
            {#if captions}
              <div class="caption">
                <div class="title">{book.title}</div>
                <div class="author">{book.authors.join(', ') || 'Unknown author'}</div>
              </div>
            {/if}
          </button>
        {:else}
          <div class="skeleton" style:height="{metrics.coverHeight}px"></div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .viewport {
    position: relative;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    contain: strict;
  }

  .spacer {
    position: relative;
    width: 100%;
  }

  .cell {
    position: absolute;
    top: 0;
    left: 0;
    will-change: transform;
  }

  .card {
    display: block;
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
    border-radius: var(--radius);
  }

  .card:hover:not(:disabled) {
    background: none;
  }

  .cover {
    position: relative;
    border-radius: 3px 6px 6px 3px;
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: var(--shadow);
    transition:
      transform 260ms var(--ease),
      box-shadow 260ms var(--ease);
  }

  /* Spine highlight, like light on a real book. */
  .cover::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, rgb(0 0 0 / 0.18), rgb(255 255 255 / 0.12) 3%, transparent 7%);
    pointer-events: none;
  }

  .card:hover .cover {
    transform: translateY(-5px) rotate(-0.5deg);
    box-shadow: var(--shadow-lift);
  }

  .card.selected .cover {
    outline: 3px solid var(--accent);
    outline-offset: 3px;
  }

  /* Selection mode. */
  .tick {
    position: absolute;
    top: 8px;
    left: 8px;
    z-index: 2;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid #fff;
    background: rgb(0 0 0 / 0.35);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.4);
    transition:
      background 160ms ease,
      transform 220ms var(--ease);
  }

  .tick svg {
    fill: none;
    stroke: var(--accent-text);
    stroke-width: 3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .card.picked .tick {
    background: var(--accent);
    border-color: var(--accent);
    animation: pop 320ms var(--ease);
  }

  @keyframes pop {
    40% {
      transform: scale(1.25);
    }
  }

  .card.selecting .cover {
    transition:
      transform 220ms var(--ease),
      box-shadow 260ms var(--ease),
      opacity 200ms ease;
  }

  .card.selecting:hover .cover {
    transform: none;
  }

  .card.picked .cover {
    transform: scale(0.94);
    outline: 3px solid var(--accent);
    outline-offset: 3px;
  }

  .card.picked:hover .cover {
    transform: scale(0.94);
  }

  .card {
    position: relative;
  }

  .fav {
    position: absolute;
    top: 6px;
    left: 8px;
    z-index: 1;
    color: #ff6b6b;
    font-size: 15px;
    text-shadow: 0 1px 3px rgb(0 0 0 / 0.6);
  }

  .progress {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 4px;
    background: var(--accent);
  }

  .badge {
    position: absolute;
    top: 6px;
    right: 6px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    padding: 2px 5px;
    border-radius: 3px;
    background: rgb(0 0 0 / 0.55);
    color: #fff;
  }

  .caption {
    padding: 8px 2px 0;
    height: 52px;
  }

  .title,
  .author {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 600;
  }

  .author {
    color: var(--muted);
    font-size: 12px;
    margin-top: 2px;
  }

  .skeleton {
    border-radius: 4px;
    background: var(--surface-2);
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.55;
    }
  }
</style>
