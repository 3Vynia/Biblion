<script lang="ts">
  import { untrack } from 'svelte'
  import Icon from '../components/Icon.svelte'
  import VirtualList from '../components/VirtualList.svelte'
  import { api } from '../lib/api'
  import { formatCount, placeholderHue } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import { authorRoute, nav } from '../lib/nav.svelte'
  import { PageCache, type Fetcher } from '../lib/pageCache'
  import type { AuthorEntry } from '../lib/types'

  let filter = $state('')
  let version = $state(0)
  let list = $state<VirtualList<AuthorEntry>>()

  const fetcher = (prefix: string): Fetcher<AuthorEntry> => (offset, limit) =>
    api.listAuthors(prefix.trim() || null, offset, limit)
  const cache = new PageCache<AuthorEntry>(fetcher(''), () => version++, 100)
  const total = $derived.by(() => (void version, cache.total))

  let first = true
  $effect(() => {
    const f = fetcher(filter)
    if (first) {
      first = false
      return
    }
    const t = setTimeout(() => {
      cache.reset(f)
      list?.scrollToTop()
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

  const initials = (name: string) =>
    name
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase())
      .join('')
</script>

<div class="screen">
  <header in:rise={{ y: 6 }}>
    <div class="titles">
      <span class="subtitle">Browse</span>
      <h1>Authors</h1>
    </div>
    <span class="count">{total !== null ? `${formatCount(total)} authors` : ''}</span>
    <div class="filter">
      <Icon name="search" size={16} />
      <input placeholder="Filter by name" bind:value={filter} spellcheck="false" />
    </div>
  </header>

  <div class="content">
    {#if total === 0}
      <p class="empty">No authors{filter.trim() ? ` starting with “${filter.trim()}”` : ''}.</p>
    {:else}
      <VirtualList bind:this={list} {cache} {version} rowHeight={64}>
        {#snippet row(a: AuthorEntry)}
          <button class="entry" onclick={() => nav.go(authorRoute(a.id, a.name))}>
            <span class="avatar" style:--hue={placeholderHue(a.name)}>{initials(a.name)}</span>
            <span class="name">{a.name}</span>
            <span class="books">{formatCount(a.book_count)} {a.book_count === 1 ? 'book' : 'books'}</span>
            <Icon name="chevronRight" size={16} />
          </button>
        {/snippet}
        {#snippet placeholder()}
          <div class="entry ghost-row"><span class="avatar skeleton"></span><span class="bar skeleton"></span></div>
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
    width: 180px;
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

  .avatar {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    flex-shrink: 0;
    border-radius: 50%;
    font-weight: 700;
    font-size: 13px;
    letter-spacing: 0.02em;
    color: hsl(var(--hue) 40% 95%);
    background: linear-gradient(140deg, hsl(var(--hue) 40% 50%), hsl(var(--hue) 45% 32%));
    transition: transform 220ms var(--ease);
  }

  .entry:hover .avatar {
    transform: scale(1.08) rotate(-4deg);
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

  .ghost-row {
    pointer-events: none;
  }

  .skeleton {
    background: var(--surface-2);
    animation: pulse 1.2s ease-in-out infinite;
  }

  .bar {
    width: 40%;
    height: 12px;
    border-radius: 6px;
  }

  @keyframes pulse {
    50% {
      opacity: 0.5;
    }
  }

  .empty {
    color: var(--muted);
    text-align: center;
    margin-top: 60px;
  }
</style>
