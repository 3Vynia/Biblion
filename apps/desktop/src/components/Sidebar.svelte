<script lang="ts">
  import Icon, { type IconName } from './Icon.svelte'
  import {
    allBooks,
    favorites,
    finished,
    nav,
    readingNow,
    sectionOf,
    shelfRoute,
    type Route,
    type Section,
  } from '../lib/nav.svelte'
  import { library } from '../lib/library.svelte'
  import { formatCount } from '../lib/format'
  import { scan } from '../lib/scan.svelte'

  interface Props {
    onfolders: () => void
  }

  let { onfolders }: Props = $props()

  interface Item {
    section: Section
    label: string
    icon: IconName
    route: () => Route | null
    count?: number | null
  }

  const active = $derived(sectionOf(nav.route))

  const groups = $derived<{ title?: string; items: Item[] }[]>([
    {
      items: [
        { section: 'home', label: 'Home', icon: 'home', route: () => ({ name: 'home' }) },
        { section: 'all', label: 'All books', icon: 'books', route: allBooks, count: library.stats?.books },
      ],
    },
    {
      title: 'My reading',
      items: [
        { section: 'reading', label: 'Reading now', icon: 'reading', route: readingNow, count: library.counts.reading },
        {
          section: 'to-read',
          label: 'To read',
          icon: 'bookmark',
          route: () => (library.toRead ? shelfRoute(library.toRead.id, 'To read', 'to-read') : null),
          count: library.toRead?.book_count,
        },
        { section: 'favorites', label: 'Favourites', icon: 'heart', route: favorites, count: library.counts.favorites },
        { section: 'finished', label: 'Finished', icon: 'check', route: finished, count: library.counts.finished },
      ],
    },
    {
      title: 'Browse',
      items: [
        { section: 'authors', label: 'Authors', icon: 'user', route: () => ({ name: 'authors' }), count: library.stats?.authors },
        { section: 'series', label: 'Series', icon: 'layers', route: () => ({ name: 'series' }), count: library.stats?.series },
        {
          section: 'collections',
          label: 'Collections',
          icon: 'collection',
          route: () => ({ name: 'collections' }),
          count: library.custom.length || null,
        },
        { section: 'formats', label: 'Formats', icon: 'file', route: () => ({ name: 'formats' }) },
      ],
    },
  ])

  // Sliding highlight behind the active entry.
  let navEl: HTMLElement
  let indicator = $state({ top: 0, height: 0, visible: false })

  $effect(() => {
    void active
    void groups
    queueMicrotask(() => {
      const el = navEl?.querySelector<HTMLElement>(`[data-section="${active}"]`)
      indicator = el ? { top: el.offsetTop, height: el.offsetHeight, visible: true } : { ...indicator, visible: false }
    })
  })

  function open(item: Item) {
    const route = item.route()
    if (route) nav.jump(route)
  }
</script>

<aside class="sidebar">
  <div class="brand">
    <span class="logo" aria-hidden="true"><Icon name="reading" size={18} strokeWidth={2} /></span>
    <span class="name">Biblion</span>
  </div>

  <nav bind:this={navEl} aria-label="Sections">
    <div
      class="indicator"
      class:visible={indicator.visible}
      style:transform="translateY({indicator.top}px)"
      style:height="{indicator.height}px"
    ></div>
    {#each groups as group, gi (gi)}
      {#if group.title}<h2>{group.title}</h2>{/if}
      {#each group.items as item (item.section)}
        <button
          class="item"
          class:active={active === item.section}
          data-section={item.section}
          aria-current={active === item.section ? 'page' : undefined}
          onclick={() => open(item)}
        >
          <Icon name={item.icon} size={18} />
          <span class="label">{item.label}</span>
          {#if item.count}<span class="count">{formatCount(item.count)}</span>{/if}
        </button>
      {/each}
    {/each}
  </nav>

  <button class="folders" onclick={onfolders}>
    {#if scan.running}
      <span class="spinner" aria-hidden="true"></span>
      <span class="label">
        {scan.phase === 'processing' ? `Scanning ${Math.floor((scan.fraction ?? 0) * 100)}%` : 'Looking for books…'}
      </span>
    {:else}
      <Icon name="folder" size={18} />
      <span class="label">Library folders</span>
    {/if}
  </button>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 232px;
    height: 100%;
    padding: 14px 10px 12px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 10px 14px;
  }

  .logo {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    color: var(--accent-text);
    background: linear-gradient(140deg, var(--accent), color-mix(in srgb, var(--accent) 60%, #5a2d0c));
    box-shadow: 0 3px 10px color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .name {
    font-family: var(--serif);
    font-size: 20px;
    font-weight: 600;
    letter-spacing: 0.01em;
  }

  nav {
    position: relative;
    flex: 1;
  }

  .indicator {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    border-radius: 8px;
    background: var(--accent-soft);
    opacity: 0;
    transition:
      transform 320ms var(--ease),
      height 320ms var(--ease),
      opacity 160ms ease;
    pointer-events: none;
  }

  .indicator.visible {
    opacity: 1;
  }

  h2 {
    margin: 16px 10px 4px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  .item,
  .folders {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
    text-align: left;
  }

  .item:hover:not(:disabled),
  .folders:hover:not(:disabled) {
    background: color-mix(in srgb, var(--text) 5%, transparent);
    color: var(--text);
  }

  .item.active {
    color: var(--accent);
    font-weight: 600;
  }

  .item.active:hover {
    background: transparent;
    color: var(--accent);
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    padding: 1px 7px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--text) 7%, transparent);
    color: var(--muted);
    font-weight: 500;
  }

  .item.active .count {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
  }

  .folders {
    margin-top: 8px;
    border-top: 1px solid var(--border);
    border-radius: 0 0 8px 8px;
    padding-top: 12px;
  }

  .spinner {
    width: 14px;
    height: 14px;
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
</style>
