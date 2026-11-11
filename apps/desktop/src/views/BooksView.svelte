<script lang="ts">
  // Any list of books: a section (reading now, favourites…), an author, a
  // series, a collection, a format, or search results.
  import { untrack } from 'svelte'
  import CoverGrid from '../components/CoverGrid.svelte'
  import FilterBar from '../components/FilterBar.svelte'
  import { deleteCollection, renameCollection } from '../lib/collections'
  import Icon from '../components/Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { formatCount } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import { nav, shelfRoute, type BookFilter } from '../lib/nav.svelte'
  import { PageCache, type Fetcher } from '../lib/pageCache'
  import type { BookSummary, SortBy } from '../lib/types'

  interface Props {
    title: string
    subtitle?: string
    filter: BookFilter
    sort?: SortBy
    /** Search text; when set, results are ranked by relevance by default. */
    text?: string
    selectedId: number | null
    onselect: (book: BookSummary) => void
    onopen: (book: BookSummary) => void
  }

  let { title, subtitle, filter, sort: initialSort, text = '', selectedId, onselect, onopen }: Props = $props()

  const SORTS: { value: SortBy; label: string }[] = [
    { value: 'title', label: 'Title' },
    { value: 'author', label: 'Author' },
    { value: 'series', label: 'Series' },
    { value: 'added_desc', label: 'Recently added' },
    { value: 'recently_read', label: 'Recently read' },
  ]

  // The screen is recreated per route, so the initial sort is enough.
  let sort = $state<SortBy>(untrack(() => initialSort ?? 'title'))
  let useRelevance = $state(true)
  let version = $state(0)
  let grid = $state<CoverGrid>()
  /** Filters picked in the Filters panel, on top of the screen's own. */
  let extra = $state<BookFilter>({})

  // A collection the user made (not "To read") can be renamed or deleted here.
  const ownShelf = $derived(
    filter.shelf_id && filter.shelf_id !== library.toRead?.id ? library.shelves.find((s) => s.id === filter.shelf_id) : undefined,
  )

  async function rename() {
    if (!ownShelf) return
    const name = await renameCollection(ownShelf)
    if (name) nav.jump(shelfRoute(ownShelf.id, name))
  }

  async function remove() {
    if (ownShelf && (await deleteCollection(ownShelf))) nav.jump({ name: 'collections' })
  }

  const effectiveSort = $derived<SortBy>(text.trim() && useRelevance ? 'relevance' : sort)

  function fetcher(): Fetcher<BookSummary> {
    // Only set extra filters apply, so they never switch off the screen's own.
    const picked = Object.fromEntries(Object.entries(extra).filter(([, v]) => v !== null && v !== undefined && v !== false))
    const query = { ...filter, ...picked, text: text.trim() || null, sort: effectiveSort }
    return (offset, limit) => api.listBooks({ ...query, offset, limit })
  }

  // `untrack`: reset() calls this synchronously, possibly inside an effect.
  const cache = new PageCache<BookSummary>(untrack(fetcher), () => untrack(() => version++))
  const total = $derived.by(() => (void version, cache.total))
  const loadError = $derived.by(() => (void version, cache.error))

  // New query when the text or sort changes; refresh when books change.
  let firstRun = true
  $effect(() => {
    const f = fetcher()
    if (firstRun) {
      firstRun = false
      return
    }
    untrack(() => {
      cache.reset(f)
      grid?.scrollToTop()
    })
  })
  let lastRevision = untrack(() => library.revision)
  $effect(() => {
    const r = library.revision
    if (r !== lastRevision) {
      lastRevision = r
      cache.refresh()
    }
  })
</script>

<div class="books">
  <header in:rise={{ y: 6 }}>
    {#if nav.canGoBack}
      <button class="ghost back" onclick={() => nav.back()} aria-label="Back"><Icon name="chevronLeft" /></button>
    {/if}
    <div class="titles">
      {#if subtitle}<span class="subtitle">{subtitle}</span>{/if}
      <h1>{text.trim() ? `Results for “${text.trim()}”` : title}</h1>
    </div>
    <span class="count">
      {#if total !== null}{formatCount(total)} {total === 1 ? 'book' : 'books'}{/if}
    </span>
    {#if ownShelf}
      <button class="ghost icon-btn" onclick={rename} title="Rename collection" aria-label="Rename collection"
        ><Icon name="edit" size={16} /></button
      >
      <button class="ghost icon-btn danger" onclick={remove} title="Delete collection" aria-label="Delete collection"
        ><Icon name="trash" size={16} /></button
      >
    {/if}
    <FilterBar base={filter} value={extra} onchange={(v) => (extra = v)} />
    <label class="sort">
      <span class="muted">Sort</span>
      {#if text.trim()}
        <select
          value={useRelevance ? 'relevance' : sort}
          onchange={(e) => {
            const v = e.currentTarget.value as SortBy
            useRelevance = v === 'relevance'
            if (!useRelevance) sort = v
          }}
        >
          <option value="relevance">Relevance</option>
          {#each SORTS as s (s.value)}<option value={s.value}>{s.label}</option>{/each}
        </select>
      {:else}
        <select bind:value={sort}>
          {#each SORTS as s (s.value)}<option value={s.value}>{s.label}</option>{/each}
        </select>
      {/if}
    </label>
  </header>

  <div class="content">
    {#if loadError && total === null}
      <div class="empty"><p class="error">Could not load books: {errorMessage(loadError)}</p></div>
    {:else if total === 0}
      <div class="empty" in:rise>
        <Icon name={text.trim() ? 'search' : 'books'} size={34} strokeWidth={1.4} />
        <p>
          {text.trim() ? `No books match “${text.trim()}”.` : Object.keys(extra).length ? 'No books match these filters.' : 'Nothing here yet.'}
        </p>
        {#if Object.values(extra).some(Boolean)}<button onclick={() => (extra = {})}>Clear filters</button>{/if}
      </div>
    {:else}
      <CoverGrid bind:this={grid} {cache} {version} {selectedId} {onselect} {onopen} />
    {/if}
  </div>
</div>

<style>
  .books {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 22px 28px 6px;
  }

  .back {
    padding: 6px;
    border-radius: 50%;
  }

  .titles {
    display: flex;
    flex-direction: column;
    min-width: 0;
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
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .count {
    flex: 1;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    padding-top: 6px;
  }

  .icon-btn {
    padding: 7px;
    border-radius: 8px;
    color: var(--muted);
  }

  .icon-btn.danger:hover:not(:disabled) {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, transparent);
  }

  .sort {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .content {
    flex: 1;
    min-height: 0;
  }

  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--muted);
  }
</style>
