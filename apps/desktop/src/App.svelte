<script lang="ts">
  import { onMount } from 'svelte'
  import CoverGrid from './components/CoverGrid.svelte'
  import BookDetail from './components/BookDetail.svelte'
  import ScanPanel from './components/ScanPanel.svelte'
  import EpubReader from './reader/EpubReader.svelte'
  import { api, errorMessage } from './lib/api'
  import { PageCache, type Fetcher } from './lib/pageCache'
  import { READABLE, formatCount } from './lib/format'
  import { scan } from './lib/scan.svelte'
  import type { Book, BookSummary, LibraryStats, SortBy } from './lib/types'

  const SEARCH_DEBOUNCE_MS = 150
  const REFRESH_DURING_SCAN_MS = 2000

  let roots = $state<string[]>([])
  let stats = $state<LibraryStats | null>(null)
  let search = $state('')
  let sort = $state<SortBy>('title')
  let selectedId = $state<number | null>(null)
  let reading = $state<Book | null>(null)
  let showScan = $state(false)
  let startupError = $state<string | null>(null)
  let version = $state(0)
  let grid = $state<CoverGrid>()

  function makeFetcher(text: string, sortBy: SortBy): Fetcher<BookSummary> {
    const query = text.trim()
    return (offset, limit) =>
      api.listBooks({
        text: query || null,
        sort: query && sortBy === 'title' ? 'relevance' : sortBy,
        offset,
        limit,
      })
  }

  const cache = new PageCache<BookSummary>(makeFetcher('', 'title'), () => version++)
  // The cache is plain TypeScript; `version` makes its fields reactive.
  const total = $derived.by(() => (void version, cache.total))
  const loadError = $derived.by(() => (void version, cache.error))

  // Re-query (debounced) when the search text or the sort order changes.
  let firstQuery = true
  $effect(() => {
    const fetcher = makeFetcher(search, sort)
    if (firstQuery) {
      firstQuery = false
      return
    }
    const t = setTimeout(() => {
      cache.reset(fetcher)
      grid?.scrollToTop()
    }, SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(t)
  })

  async function refreshStats() {
    try {
      stats = await api.stats()
    } catch (e) {
      console.error('stats failed', e)
    }
  }

  // Show new books while a scan is running, without hammering the database.
  let lastRefresh = 0
  scan.onProgress = () => {
    const now = Date.now()
    if (now - lastRefresh < REFRESH_DURING_SCAN_MS) return
    lastRefresh = now
    cache.refresh()
    refreshStats()
  }
  scan.onFinished = () => {
    cache.refresh()
    refreshStats()
  }

  onMount(async () => {
    try {
      roots = await api.getRoots()
      await refreshStats()
      if (roots.length === 0) showScan = true
      else if (!(await api.isScanning())) scan.start() // cheap incremental rescan
    } catch (e) {
      startupError = errorMessage(e)
    }
  })

  async function openBook(summary: BookSummary) {
    if (!READABLE.has(summary.format)) {
      selectedId = summary.id
      return
    }
    try {
      reading = await api.getBook(summary.id)
    } catch {
      selectedId = summary.id // the detail panel shows the error
    }
  }

  function closeReader() {
    reading = null
    // Pick up the saved progress in the grid.
    cache.refresh()
  }

  function onGlobalKey(e: KeyboardEvent) {
    if (reading || showScan) return
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
      e.preventDefault()
      document.getElementById('search')?.focus()
    } else if (e.key === 'Escape') {
      if (search) search = ''
      else selectedId = null
    }
  }
</script>

<svelte:window onkeydown={onGlobalKey} />

<div class="app" class:with-detail={selectedId !== null}>
  <header class="toolbar">
    <h1>Biblion</h1>
    <input
      id="search"
      type="search"
      placeholder="Search titles, authors, series, tags…"
      autocomplete="off"
      spellcheck="false"
      bind:value={search}
    />
    <label class="sort">
      <span class="muted">Sort</span>
      <select bind:value={sort}>
        <option value="title">{search.trim() ? 'Relevance' : 'Title'}</option>
        <option value="author">Author</option>
        <option value="series">Series</option>
        <option value="added_desc">Recently added</option>
        <option value="recently_read">Recently read</option>
      </select>
    </label>
    <span class="count muted">
      {#if total !== null}
        {formatCount(total)}{search.trim() ? ' results' : ' books'}
      {/if}
    </span>
    <button class="scan-button" onclick={() => (showScan = true)}>
      {#if scan.running}
        <span class="spinner" aria-hidden="true"></span>
        {scan.phase === 'processing' ? `Scanning ${Math.floor((scan.fraction ?? 0) * 100)}%` : 'Scanning…'}
      {:else}
        Folders
      {/if}
    </button>
  </header>

  <main>
    {#if startupError}
      <div class="empty">
        <p class="error">Biblion could not start: {startupError}</p>
      </div>
    {:else if total === 0 && !search.trim()}
      <div class="empty">
        {#if scan.running}
          <p>Looking for books…</p>
        {:else}
          <h2>Your library is empty</h2>
          <p class="muted">Add a folder with EPUB, PDF, MOBI, AZW3 or CBZ/CBR files.</p>
          <button class="primary" onclick={() => (showScan = true)}>Add a folder</button>
        {/if}
      </div>
    {:else if total === 0}
      <div class="empty"><p class="muted">No books match “{search.trim()}”.</p></div>
    {:else if loadError && total === null}
      <div class="empty"><p class="error">Could not load the library: {errorMessage(loadError)}</p></div>
    {:else}
      <CoverGrid
        bind:this={grid}
        {cache}
        {version}
        {selectedId}
        onselect={(b) => (selectedId = b.id)}
        onopen={openBook}
      />
    {/if}
  </main>

  {#if selectedId !== null}
    <BookDetail id={selectedId} onread={(b) => (reading = b)} onclose={() => (selectedId = null)} />
  {/if}
</div>

{#if showScan}
  <ScanPanel {roots} onrootschange={(r) => (roots = r)} onclose={() => (showScan = false)} />
{/if}

{#if reading}
  <EpubReader book={reading} onclose={closeReader} />
{/if}

{#if stats && stats.books > 0}
  <span class="sr-only" aria-live="polite">{formatCount(stats.books)} books in the library</span>
{/if}

<style>
  .app {
    display: grid;
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr;
    height: 100%;
  }

  .app.with-detail {
    grid-template-columns: 1fr 360px;
  }

  .toolbar {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  h1 {
    font-family: var(--serif);
    font-size: 20px;
    margin: 0 8px 0 0;
    color: var(--accent);
  }

  #search {
    flex: 1;
    max-width: 520px;
  }

  .sort {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .count {
    flex: 1;
    font-variant-numeric: tabular-nums;
  }

  .scan-button {
    display: flex;
    align-items: center;
    gap: 8px;
    font-variant-numeric: tabular-nums;
  }

  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  main {
    min-width: 0;
    min-height: 0;
  }

  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 8px;
  }

  .empty h2 {
    font-family: var(--serif);
    margin: 0;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
