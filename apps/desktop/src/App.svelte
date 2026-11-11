<script lang="ts">
  import { onMount } from 'svelte'
  import Sidebar from './components/Sidebar.svelte'
  import BookDetail from './components/BookDetail.svelte'
  import ScanPanel from './components/ScanPanel.svelte'
  import Icon from './components/Icon.svelte'
  import EpubReader from './reader/EpubReader.svelte'
  import HomeView from './views/HomeView.svelte'
  import BooksView from './views/BooksView.svelte'
  import AuthorsView from './views/AuthorsView.svelte'
  import SeriesView from './views/SeriesView.svelte'
  import CollectionsView from './views/CollectionsView.svelte'
  import FormatsView from './views/FormatsView.svelte'
  import SettingsView from './views/SettingsView.svelte'
  import { api, errorMessage } from './lib/api'
  import { READABLE } from './lib/format'
  import { library } from './lib/library.svelte'
  import { rise } from './lib/motion'
  import { nav, routeKey } from './lib/nav.svelte'
  import { scan } from './lib/scan.svelte'
  import type { Book, BookSummary } from './lib/types'
  import { ui } from './lib/uiSettings.svelte'

  const SEARCH_DEBOUNCE_MS = 180
  const REFRESH_DURING_SCAN_MS = 2500

  let roots = $state<string[]>([])
  let searchInput = $state('')
  let search = $state('')
  let selectedId = $state<number | null>(null)
  let reading = $state<Book | null>(null)
  let showScan = $state(false)
  let startupError = $state<string | null>(null)

  // Debounce the global search box.
  $effect(() => {
    const value = searchInput
    const t = setTimeout(() => (search = value.trim()), SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(t)
  })

  // Show new books while a scan runs, without hammering the database.
  let lastRefresh = 0
  scan.onProgress = () => {
    const now = Date.now()
    if (now - lastRefresh < REFRESH_DURING_SCAN_MS) return
    lastRefresh = now
    library.changed()
  }
  scan.onFinished = () => library.changed()

  onMount(async () => {
    try {
      await ui.load()
      roots = await api.getRoots()
      await library.refresh()
      if (roots.length === 0) showScan = true
      else if (ui.value.scanOnStartup && !(await api.isScanning())) scan.start() // cheap incremental rescan
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
    library.changed() // progress changed
  }

  function onGlobalKey(e: KeyboardEvent) {
    if (reading || showScan) return
    const inField = (e.target as HTMLElement | null)?.closest?.('input, select, textarea')
    if ((e.ctrlKey || e.metaKey) && e.key === 'b') {
      e.preventDefault()
      ui.update({ sidebarCollapsed: !ui.value.sidebarCollapsed })
    } else if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
      e.preventDefault()
      document.getElementById('search')?.focus()
    } else if (e.key === 'Escape') {
      if (searchInput) searchInput = ''
      else if (selectedId !== null) selectedId = null
      else if (!inField && nav.canGoBack) nav.back()
    } else if (e.altKey && e.key === 'ArrowLeft' && nav.canGoBack) {
      nav.back()
    }
  }

  const route = $derived(nav.route)
  const viewKey = $derived(search ? `search:${search}` : routeKey(route))
</script>

<svelte:window onkeydown={onGlobalKey} />

<div class="app" class:with-detail={selectedId !== null}>
  <Sidebar onfolders={() => (showScan = true)} />

  <div class="main">
    <header class="topbar">
      <div class="search" class:active={!!searchInput}>
        <Icon name="search" size={17} />
        <input
          id="search"
          type="search"
          placeholder="Search titles, authors, series, tags…"
          autocomplete="off"
          spellcheck="false"
          bind:value={searchInput}
        />
        {#if searchInput}
          <button class="ghost clear" onclick={() => (searchInput = '')} aria-label="Clear search"
            ><Icon name="close" size={14} /></button
          >
        {:else}
          <kbd>Ctrl F</kbd>
        {/if}
      </div>
    </header>

    <main>
      {#if startupError}
        <div class="fatal"><p class="error">Biblion could not start: {startupError}</p></div>
      {:else}
        {#key viewKey}
          <div class="view" in:rise={{ y: 8, duration: 280 }}>
            {#if search}
              <BooksView
                title="Search"
                filter={{}}
                text={search}
                {selectedId}
                onselect={(b) => (selectedId = b.id)}
                onopen={openBook}
              />
            {:else if route.name === 'home'}
              <HomeView onselect={(b) => (selectedId = b.id)} onopen={openBook} onfolders={() => (showScan = true)} />
            {:else if route.name === 'books'}
              <BooksView
                title={route.title}
                subtitle={route.subtitle}
                filter={route.filter}
                sort={route.sort}
                {selectedId}
                onselect={(b) => (selectedId = b.id)}
                onopen={openBook}
              />
            {:else if route.name === 'authors'}
              <AuthorsView />
            {:else if route.name === 'series'}
              <SeriesView />
            {:else if route.name === 'collections'}
              <CollectionsView />
            {:else if route.name === 'formats'}
              <FormatsView />
            {:else if route.name === 'settings'}
              <SettingsView {roots} onrootschange={(r) => (roots = r)} />
            {/if}
          </div>
        {/key}
      {/if}
    </main>
  </div>

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

<style>
  .app {
    display: grid;
    grid-template-columns: auto 1fr;
    height: 100%;
  }

  .app.with-detail {
    grid-template-columns: auto 1fr 360px;
  }

  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .topbar {
    display: flex;
    align-items: center;
    padding: 12px 28px 0;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    width: min(560px, 100%);
    padding: 0 8px 0 14px;
    border-radius: 22px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
    transition:
      border-color 160ms ease,
      box-shadow 220ms var(--ease);
  }

  .search:focus-within,
  .search.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
    color: var(--accent);
  }

  .search input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    padding: 9px 0;
    outline: none;
    color: var(--text);
  }

  .search input::-webkit-search-cancel-button {
    display: none;
  }

  .clear {
    padding: 4px;
    border-radius: 50%;
  }

  kbd {
    font-family: inherit;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 5px;
    border: 1px solid var(--border);
    color: var(--muted);
  }

  main {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .view {
    position: absolute;
    inset: 0;
  }

  .fatal {
    height: 100%;
    display: grid;
    place-items: center;
  }
</style>
