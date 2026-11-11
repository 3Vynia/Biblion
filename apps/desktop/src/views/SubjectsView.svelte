<script lang="ts">
  import { untrack } from 'svelte'
  import Icon from '../components/Icon.svelte'
  import VirtualList from '../components/VirtualList.svelte'
  import { api } from '../lib/api'
  import { formatCount, placeholderHue } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import { nav, tagRoute } from '../lib/nav.svelte'
  import { PageCache, type Fetcher } from '../lib/pageCache'
  import type { TagEntry } from '../lib/types'

  let filter = $state('')
  let version = $state(0)
  let list = $state<VirtualList<TagEntry>>()

  const fetcher = (prefix: string): Fetcher<TagEntry> => (offset, limit) => api.listTags(prefix.trim() || null, offset, limit)
  const cache = new PageCache<TagEntry>(fetcher(''), () => untrack(() => version++), 100)
  const total = $derived.by(() => (void version, cache.total))

  let first = true
  $effect(() => {
    const f = fetcher(filter)
    if (first) {
      first = false
      return
    }
    const t = setTimeout(() => {
      untrack(() => {
        cache.reset(f)
        list?.scrollToTop()
      })
    }, 150)
    return () => clearTimeout(t)
  })
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
    <div>
      <span class="subtitle">Browse</span>
      <h1>Subjects</h1>
    </div>
    <span class="count">{total !== null ? `${formatCount(total)} subjects` : ''}</span>
    <div class="filter">
      <Icon name="search" size={16} />
      <input placeholder="Science fiction, history…" bind:value={filter} spellcheck="false" />
    </div>
  </header>

  <div class="content">
    {#if total === 0}
      <p class="empty">
        {filter.trim() ? `No subjects starting with “${filter.trim()}”.` : 'No subjects yet. They come from the books’ own metadata (EPUB subjects, comic genres).'}
      </p>
    {:else}
      <VirtualList bind:this={list} {cache} {version} rowHeight={56}>
        {#snippet row(t: TagEntry)}
          <button class="entry" onclick={() => nav.go(tagRoute(t.name))}>
            <span class="dot" style:--hue={placeholderHue(t.name)}><Icon name="tag" size={15} /></span>
            <span class="name">{t.name}</span>
            <span class="books">{formatCount(t.book_count)} {t.book_count === 1 ? 'book' : 'books'}</span>
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

  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 18px;
    background: var(--surface);
    color: var(--muted);
  }

  .filter input {
    border: none;
    background: transparent;
    padding: 7px 2px;
    width: 200px;
    outline: none;
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
    height: 48px;
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

  .dot {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    border-radius: 10px;
    color: hsl(var(--hue) 55% 45%);
    background: hsl(var(--hue) 50% 50% / 0.14);
    transition: transform 220ms var(--ease);
  }

  .entry:hover .dot {
    transform: rotate(-10deg) scale(1.08);
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
