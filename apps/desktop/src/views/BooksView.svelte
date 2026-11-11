<script lang="ts">
  // Any list of books: a section (reading now, favourites…), an author, a
  // series, a collection, a format, or search results.
  import { untrack } from 'svelte'
  import CoverGrid from '../components/CoverGrid.svelte'
  import FilterBar from '../components/FilterBar.svelte'
  import { deleteCollection, removeFromCollection, renameCollection } from '../lib/collections'
  import Icon from '../components/Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { formatCount } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import { fly } from 'svelte/transition'
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

  // In a collection, books can be picked and taken out. A collection the
  // user made (not "To read") can also be renamed or deleted.
  const shelf = $derived(filter.shelf_id ? library.shelves.find((s) => s.id === filter.shelf_id) : undefined)
  const ownShelf = $derived(shelf && shelf.id !== library.toRead?.id ? shelf : undefined)

  let selecting = $state(false)
  let picked = $state(new Set<number>())
  let selectingAll = $state(false)

  function startSelecting() {
    picked = new Set()
    selecting = true
  }

  function stopSelecting() {
    selecting = false
    picked = new Set()
  }

  function toggle(book: BookSummary) {
    const next = new Set(picked)
    if (!next.delete(book.id)) next.add(book.id)
    picked = next
  }

  /** Picks every book of the current list (with filters), page by page. */
  async function selectAll() {
    if (total === null) return
    if (picked.size === total) {
      picked = new Set()
      return
    }
    selectingAll = true
    try {
      const ids = new Set<number>()
      const query = currentQuery()
      for (let offset = 0; offset < total; offset += 1000) {
        const page = await api.listBooks({ ...query, offset, limit: 1000 })
        page.items.forEach((b) => ids.add(b.id))
        if (page.items.length === 0) break
      }
      picked = ids
    } finally {
      selectingAll = false
    }
  }

  async function removePicked() {
    if (!shelf || picked.size === 0) return
    if (await removeFromCollection(shelf, [...picked])) stopSelecting()
  }

  function onKey(e: KeyboardEvent) {
    if (selecting && e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      stopSelecting()
    }
  }

  async function rename() {
    if (!ownShelf) return
    const name = await renameCollection(ownShelf)
    if (name) nav.jump(shelfRoute(ownShelf.id, name))
  }

  async function remove() {
    if (ownShelf && (await deleteCollection(ownShelf))) nav.jump({ name: 'collections' })
  }

  const effectiveSort = $derived<SortBy>(text.trim() && useRelevance ? 'relevance' : sort)

  function currentQuery() {
    // Only set extra filters apply, so they never switch off the screen's own.
    const set = Object.fromEntries(Object.entries(extra).filter(([, v]) => v !== null && v !== undefined && v !== false))
    return { ...filter, ...set, text: text.trim() || null, sort: effectiveSort }
  }

  function fetcher(): Fetcher<BookSummary> {
    const query = currentQuery()
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

<svelte:window onkeydowncapture={onKey} />

<div class="books">
  <header in:rise={{ y: 6 }}>
    {#if nav.canGoBack}
      <button class="ghost back" onclick={() => nav.back()} aria-label="Back"><Icon name="chevronLeft" /></button>
    {/if}
    <div class="heading">
      <div class="titles">
        {#if subtitle}<span class="subtitle">{subtitle}</span>{/if}
        <h1>{text.trim() ? `Results for “${text.trim()}”` : title}</h1>
      </div>
      <span class="count">
        {#if total !== null}{formatCount(total)} {total === 1 ? 'book' : 'books'}{/if}
      </span>
    </div>
    <div class="tools">
      {#if shelf && !selecting && (total ?? 0) > 0}
        <button class="ghost icon-btn" onclick={startSelecting} title="Select books to remove" aria-label="Select books"
          ><Icon name="select" size={16} /></button
        >
      {/if}
      {#if ownShelf && !selecting}
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
    </div>
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
      <CoverGrid
        bind:this={grid}
        {cache}
        {version}
        {selectedId}
        {onselect}
        {onopen}
        {selecting}
        {picked}
        ontoggle={toggle}
      />
    {/if}
  </div>

  {#if selecting}
    <div class="selection-bar" role="toolbar" aria-label="Selection" transition:fly={{ y: 40, duration: 260 }}>
      <span class="picked-count">
        <strong>{formatCount(picked.size)}</strong>
        {picked.size === 1 ? 'book selected' : 'books selected'}
      </span>
      <button class="ghost" onclick={selectAll} disabled={selectingAll}>
        {picked.size === total && total ? 'Select none' : selectingAll ? 'Selecting…' : 'Select all'}
      </button>
      <span class="divider"></span>
      <button class="ghost" onclick={stopSelecting}>Cancel</button>
      <button class="danger-btn" onclick={removePicked} disabled={picked.size === 0}>
        <Icon name="minus" size={15} strokeWidth={2.4} /> Remove from collection
      </button>
    </div>
  {/if}
</div>

<style>
  .books {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 14px;
    padding: 22px 28px 6px;
  }

  /* Title and count keep their room; the tools wrap below when narrow. */
  .heading {
    flex: 1 1 220px;
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
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

  .sort select {
    max-width: 170px;
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
    flex-shrink: 0;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    padding-top: 6px;
  }

  .selection-bar {
    position: absolute;
    left: 50%;
    bottom: 22px;
    translate: -50% 0;
    z-index: 12;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 18px;
    border-radius: 16px;
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    backdrop-filter: blur(12px);
    box-shadow: var(--shadow-lift);
    white-space: nowrap;
  }

  .picked-count {
    color: var(--muted);
    margin-right: 6px;
    font-variant-numeric: tabular-nums;
  }

  .picked-count strong {
    color: var(--text);
  }

  .divider {
    width: 1px;
    height: 22px;
    background: var(--border);
  }

  .selection-bar button {
    border-radius: 10px;
    padding: 8px 14px;
  }

  .danger-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    color: #fff;
    background: var(--danger);
    border-color: var(--danger);
  }

  .danger-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 85%, black);
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
