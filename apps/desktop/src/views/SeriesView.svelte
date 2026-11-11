<script lang="ts">
  import { untrack } from 'svelte'
  import Icon from '../components/Icon.svelte'
  import VirtualList from '../components/VirtualList.svelte'
  import { api } from '../lib/api'
  import { formatCount, placeholderHue } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import { nav, seriesRoute } from '../lib/nav.svelte'
  import { PageCache } from '../lib/pageCache'

  type Entry = [string, number]

  let version = $state(0)
  const cache = new PageCache<Entry>((offset, limit) => api.listSeries(offset, limit), () => version++, 100)
  const total = $derived.by(() => (void version, cache.total))

  let lastRevision = untrack(() => library.revision)
  $effect(() => {
    if (library.revision !== lastRevision) {
      lastRevision = library.revision
      cache.refresh()
    }
  })
</script>

<div class="screen">
  <header in:rise={{ y: 6 }}>
    <div class="titles">
      <span class="subtitle">Browse</span>
      <h1>Series</h1>
    </div>
    <span class="count">{total !== null ? `${formatCount(total)} series` : ''}</span>
  </header>

  <div class="content">
    {#if total === 0}
      <p class="empty">No series yet. Books with series information (EPUB, comics) appear here.</p>
    {:else}
      <VirtualList {cache} {version} rowHeight={64}>
        {#snippet row([name, count]: Entry)}
          <button class="entry" onclick={() => nav.go(seriesRoute(name))}>
            <span class="stack" style:--hue={placeholderHue(name)} aria-hidden="true">
              <span></span><span></span><span></span>
            </span>
            <span class="name">{name}</span>
            <span class="books">{formatCount(count)} {count === 1 ? 'book' : 'books'}</span>
            <Icon name="chevronRight" size={16} />
          </button>
        {/snippet}
      </VirtualList>
    {/if}
  </div>
</div>

<style>
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 22px 28px 10px;
  }

  .subtitle {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--accent);
  }

  h1 {
    margin: 0;
    font-family: var(--serif);
    font-size: 26px;
    font-weight: 600;
  }

  .count {
    flex: 1;
    color: var(--muted);
    padding-top: 14px;
  }

  .content {
    flex: 1;
    min-height: 0;
    padding: 0 20px;
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    height: 56px;
    padding: 0 12px;
    border: none;
    border-radius: 10px;
    background: transparent;
    text-align: left;
    color: var(--muted);
  }

  .entry:hover:not(:disabled) {
    background: var(--surface);
    color: var(--text);
  }

  /* Three fanned "books" standing for the series. */
  .stack {
    position: relative;
    width: 38px;
    height: 40px;
    flex-shrink: 0;
  }

  .stack span {
    position: absolute;
    bottom: 0;
    width: 22px;
    height: 34px;
    border-radius: 2px 4px 4px 2px;
    background: linear-gradient(160deg, hsl(var(--hue) 40% 48%), hsl(var(--hue) 45% 28%));
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.3);
    transition: transform 260ms var(--ease);
  }

  .stack span:nth-child(1) {
    left: 0;
    filter: brightness(0.75);
  }

  .stack span:nth-child(2) {
    left: 7px;
    filter: brightness(0.88);
  }

  .stack span:nth-child(3) {
    left: 14px;
  }

  .entry:hover .stack span:nth-child(1) {
    transform: rotate(-10deg) translateX(-3px);
  }

  .entry:hover .stack span:nth-child(3) {
    transform: rotate(8deg) translateX(3px);
  }

  .name {
    flex: 1;
    min-width: 0;
    color: var(--text);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .books {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .empty {
    color: var(--muted);
    text-align: center;
    margin-top: 60px;
  }
</style>
