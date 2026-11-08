<script lang="ts">
  // Virtualised cover grid: only the rows in view (plus a few above and
  // below) exist in the DOM, whatever the size of the library. Data comes
  // from a PageCache that loads pages of 200 books on demand.
  import type { PageCache } from '../lib/pageCache'
  import type { BookSummary } from '../lib/types'
  import { GAP, gridMetrics, visibleRows } from '../lib/gridLayout'
  import { thumbUrl } from '../lib/api'
  import { placeholderHue } from '../lib/format'

  interface Props {
    cache: PageCache<BookSummary>
    /** Bumped by the parent whenever the cache changes. */
    version: number
    selectedId: number | null
    onselect: (book: BookSummary) => void
    onopen: (book: BookSummary) => void
  }

  let { cache, version, selectedId, onselect, onopen }: Props = $props()

  const PADDING = 24

  let viewport: HTMLDivElement
  let width = $state(0)
  let height = $state(0)
  let scrollTop = $state(0)
  /** Cover keys whose thumbnail failed to load (show the placeholder). */
  let brokenCovers = $state(new Set<string>())

  const total = $derived.by(() => (void version, cache.total ?? 0))
  const metrics = $derived(gridMetrics(width - PADDING * 2, total))
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

  export function scrollToTop() {
    if (viewport) viewport.scrollTop = 0
    scrollTop = 0
  }

  function coverFailed(key: string) {
    brokenCovers = new Set(brokenCovers).add(key)
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
          <button
            class="card"
            class:selected={book.id === selectedId}
            onclick={() => onselect(book)}
            ondblclick={() => onopen(book)}
            onkeydown={(e) => e.key === 'Enter' && e.shiftKey && onopen(book)}
            title={book.title}
          >
            <div class="cover" style:height="{metrics.coverHeight}px">
              {#if book.cover_key && !brokenCovers.has(book.cover_key)}
                <img
                  src={thumbUrl(book.cover_key)}
                  alt=""
                  loading="lazy"
                  decoding="async"
                  draggable="false"
                  onerror={() => coverFailed(book.cover_key!)}
                />
              {:else}
                <div class="placeholder" style:--hue={placeholderHue(book.title)}>
                  <span class="ph-title">{book.title}</span>
                  {#if book.authors.length}<span class="ph-author">{book.authors[0]}</span>{/if}
                </div>
              {/if}
              {#if book.progress !== null && book.progress > 0}
                <div class="progress" style:width="{Math.round(book.progress * 100)}%"></div>
              {/if}
              <span class="badge">{book.format.toUpperCase()}</span>
            </div>
            <div class="caption">
              <div class="title">{book.title}</div>
              <div class="author">{book.authors.join(', ') || 'Unknown author'}</div>
            </div>
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
    border-radius: 4px;
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: var(--shadow);
    transition: transform 120ms ease;
  }

  .card:hover .cover {
    transform: translateY(-2px);
  }

  .card.selected .cover {
    outline: 3px solid var(--accent);
    outline-offset: 2px;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .placeholder {
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 14px 12px;
    background: linear-gradient(160deg, hsl(var(--hue) 35% 38%), hsl(var(--hue) 40% 24%));
    color: hsl(var(--hue) 30% 92%);
    font-family: var(--serif);
  }

  .ph-title {
    font-size: 15px;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 6;
    line-clamp: 6;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .ph-author {
    font-size: 12px;
    opacity: 0.85;
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
