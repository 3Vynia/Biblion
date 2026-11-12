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
  import { ui } from '../lib/uiSettings.svelte'
  import { ambience } from '../lib/ambience/store.svelte'
  import { MODES } from '../lib/ambience/settings'

  interface Item {
    section: Section
    label: string
    icon: IconName
    route: () => Route | null
    count?: number | null
  }

  const active = $derived(sectionOf(nav.route))
  const collapsed = $derived(ui.value.sidebarCollapsed)

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
        {
          section: 'notes',
          label: 'Notes',
          icon: 'quote',
          route: () => ({ name: 'notes' }),
          count: library.counts.highlights + library.counts.bookmarks || null,
        },
      ],
    },
    {
      title: 'Browse',
      items: [
        { section: 'authors', label: 'Authors', icon: 'user', route: () => ({ name: 'authors' }), count: library.stats?.authors },
        { section: 'series', label: 'Series', icon: 'layers', route: () => ({ name: 'series' }), count: library.stats?.series },
        { section: 'subjects', label: 'Subjects', icon: 'tag', route: () => ({ name: 'subjects' }) },
        {
          section: 'collections',
          label: 'Collections',
          icon: 'collection',
          route: () => ({ name: 'collections' }),
          count: library.custom.length || null,
        },
        { section: 'formats', label: 'Formats', icon: 'file', route: () => ({ name: 'formats' }) },
        { section: 'health', label: 'Library health', icon: 'pulse', route: () => ({ name: 'health' }) },
      ],
    },
  ])

  // Sliding highlight behind the active entry (main list only).
  let navEl: HTMLElement
  // Separate values so the effect below only writes them (never reads).
  let indicatorTop = $state(0)
  let indicatorHeight = $state(0)
  let indicatorVisible = $state(false)

  $effect(() => {
    void active
    void groups
    void collapsed
    const update = () => {
      const el = navEl?.querySelector<HTMLElement>(`[data-section="${active}"]`)
      indicatorVisible = !!el
      if (el) {
        indicatorTop = el.offsetTop
        indicatorHeight = el.offsetHeight
      }
    }
    update()
    // Group titles change height when collapsing: measure again once settled.
    const t = setTimeout(update, 320)
    return () => clearTimeout(t)
  })

  function open(item: Item) {
    const route = item.route()
    if (route) nav.jump(route)
  }

  const scanLabel = $derived(
    scan.phase === 'processing' ? `Scanning ${Math.floor((scan.fraction ?? 0) * 100)}%` : 'Looking for books…',
  )
</script>

<aside class="sidebar" class:collapsed>
  <div class="brand">
    <span class="logo" aria-hidden="true"><Icon name="reading" size={18} strokeWidth={2} /></span>
    {#if !collapsed}<span class="name">Biblion</span>{/if}
    <button
      class="toggle"
      onclick={() => ui.update({ sidebarCollapsed: !collapsed })}
      title={collapsed ? 'Expand sidebar (Ctrl+B)' : 'Collapse sidebar (Ctrl+B)'}
      aria-label={collapsed ? 'Show sidebar' : 'Hide sidebar'}
      aria-expanded={!collapsed}
    >
      <Icon name="panel" size={17} />
    </button>
  </div>

  <nav bind:this={navEl} aria-label="Sections">
    <div
      class="indicator"
      class:visible={indicatorVisible}
      style:transform="translateY({indicatorTop}px)"
      style:height="{indicatorHeight}px"
    ></div>
    {#each groups as group, gi (gi)}
      {#if group.title}
        {#if collapsed}<hr />{:else}<h2>{group.title}</h2>{/if}
      {/if}
      {#each group.items as item (item.section)}
        <button
          class="item"
          class:active={active === item.section}
          data-section={item.section}
          aria-current={active === item.section ? 'page' : undefined}
          aria-label={collapsed ? item.label : undefined}
          title={collapsed ? item.label : undefined}
          onclick={() => open(item)}
        >
          <Icon name={item.icon} size={18} />
          {#if !collapsed}
            <span class="label">{item.label}</span>
            {#if item.count}<span class="count">{formatCount(item.count)}</span>{/if}
          {/if}
        </button>
      {/each}
    {/each}
  </nav>

  <div class="footer">
    {#if ambience.playing}
      <div class="player" class:collapsed>
        <button class="item now-playing" onclick={() => nav.jump({ name: 'settings' })} title="Ambience settings">
          <span class="bars" aria-hidden="true"><i></i><i></i><i></i></span>
          {#if !collapsed}
            <span class="label">
              {ambience.active?.name ??
                (ambience.value.soundscape.on ? MODES.find((m) => m.id === ambience.value.soundscape.mode)?.label : 'Ambience')}
            </span>
          {/if}
        </button>
        {#if !collapsed}
          <button class="pause" onclick={() => ambience.pause()} aria-label="Pause ambience" title="Pause"><Icon name="pause" size={14} filled /></button>
        {/if}
      </div>
    {/if}
    {#if scan.running}
      <button class="item scanning" onclick={() => nav.jump({ name: 'settings' })} title={scanLabel}>
        <span class="spinner" aria-hidden="true"></span>
        {#if !collapsed}<span class="label">{scanLabel}</span>{/if}
      </button>
    {/if}
    <button
      class="item"
      class:active={active === 'settings'}
      aria-current={active === 'settings' ? 'page' : undefined}
      aria-label={collapsed ? 'Settings' : undefined}
      title={collapsed ? 'Settings' : undefined}
      onclick={() => nav.jump({ name: 'settings' })}
    >
      <Icon name="settings" size={18} />
      {#if !collapsed}<span class="label">Settings</span>{/if}
    </button>
  </div>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 232px;
    height: 100%;
    padding: 14px 10px 12px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    overflow-x: hidden;
    overflow-y: auto;
    transition: width 280ms var(--ease);
  }

  .sidebar.collapsed {
    width: 68px;
  }

  /* Brand row: logo, name and the collapse toggle. */
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    margin-bottom: 18px;
    padding-left: 6px;
  }

  .collapsed .brand {
    flex-direction: column;
    height: auto;
    gap: 10px;
    padding: 0;
  }

  .logo {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    color: var(--accent-text);
    background: linear-gradient(140deg, var(--accent), color-mix(in srgb, var(--accent) 60%, #5a2d0c));
    box-shadow: 0 3px 10px color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .name {
    flex: 1;
    font-family: var(--serif);
    font-size: 20px;
    font-weight: 600;
    white-space: nowrap;
  }

  .toggle {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
  }

  .toggle:hover:not(:disabled) {
    background: color-mix(in srgb, var(--text) 7%, transparent);
    color: var(--text);
  }

  .toggle:focus-visible {
    outline-offset: 0;
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
    border-radius: 9px;
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
    margin: 18px 10px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
    white-space: nowrap;
  }

  hr {
    margin: 12px 10px;
    border: none;
    border-top: 1px solid var(--border);
  }

  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 38px;
    padding: 0 10px;
    margin-bottom: 2px;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
    text-align: left;
  }

  /* Collapsed: square, centred icons. */
  .collapsed .item {
    justify-content: center;
    padding: 0;
  }

  .item:hover:not(:disabled) {
    background: color-mix(in srgb, var(--text) 6%, transparent);
    color: var(--text);
  }

  .item.active {
    color: var(--accent);
    font-weight: 600;
  }

  nav .item.active:hover {
    background: transparent;
    color: var(--accent);
  }

  .footer .item.active {
    background: var(--accent-soft);
  }

  .label {
    flex: 1;
    min-width: 0;
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

  .footer {
    padding-top: 10px;
    margin-top: 10px;
    border-top: 1px solid var(--border);
  }

  .player {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .now-playing {
    color: var(--accent);
    font-size: 13px;
  }

  .pause {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 30px;
    height: 30px;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
  }

  .pause:hover:not(:disabled) {
    background: color-mix(in srgb, var(--text) 7%, transparent);
    color: var(--text);
  }

  /* Three bars dancing slowly: the ambience is on. */
  .bars {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    width: 16px;
    height: 14px;
    flex-shrink: 0;
  }

  .bars i {
    flex: 1;
    border-radius: 1px;
    background: currentColor;
    animation: bar 1.6s ease-in-out infinite;
  }

  .bars i:nth-child(2) {
    animation-delay: -0.5s;
  }

  .bars i:nth-child(3) {
    animation-delay: -1s;
  }

  @keyframes bar {
    0%,
    100% {
      height: 30%;
    }
    50% {
      height: 100%;
    }
  }

  .scanning {
    color: var(--accent);
    font-size: 13px;
  }

  .spinner {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    border: 2px solid color-mix(in srgb, var(--accent) 25%, transparent);
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
