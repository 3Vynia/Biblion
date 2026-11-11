<script lang="ts">
  import { Tween } from 'svelte/motion'
  import { cubicOut } from 'svelte/easing'
  import BookCover from '../components/BookCover.svelte'
  import Icon from '../components/Icon.svelte'
  import ProgressRing from '../components/ProgressRing.svelte'
  import ShelfRow from '../components/ShelfRow.svelte'
  import { api } from '../lib/api'
  import { FORMAT_LABEL, formatBytes, formatCount } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { ms, rise } from '../lib/motion'
  import { allBooks, favorites, formatRoute, nav, readingNow } from '../lib/nav.svelte'
  import type { BookQuery, BookSummary } from '../lib/types'

  interface Props {
    onselect: (book: BookSummary) => void
    onopen: (book: BookSummary) => void
    onfolders: () => void
  }

  let { onselect, onopen, onfolders }: Props = $props()

  let reading = $state<BookSummary[]>([])
  let recent = $state<BookSummary[]>([])
  let favs = $state<BookSummary[]>([])
  let loaded = $state(false)

  const list = (q: Omit<BookQuery, 'offset'>) => api.listBooks({ offset: 0, ...q }).then((p) => p.items)

  $effect(() => {
    void library.revision
    Promise.all([
      list({ reading_only: true, sort: 'recently_read', limit: 10 }),
      list({ sort: 'added_desc', limit: 20 }),
      list({ favorites_only: true, sort: 'title', limit: 20 }),
    ])
      .then(([r, a, f]) => {
        reading = r
        recent = a
        favs = f
        loaded = true
      })
      .catch((e) => console.error('home failed to load', e))
  })

  const greeting = (() => {
    const h = new Date().getHours()
    return h < 6 ? 'Good night' : h < 12 ? 'Good morning' : h < 19 ? 'Good afternoon' : 'Good evening'
  })()

  // Count-up numbers for the stats tiles.
  const tween = () => new Tween(0, { duration: ms(1100), easing: cubicOut })
  const books = tween()
  const authors = tween()
  const series = tween()
  const finishedCount = tween()
  $effect(() => {
    books.target = library.stats?.books ?? 0
    authors.target = library.stats?.authors ?? 0
    series.target = library.stats?.series ?? 0
    finishedCount.target = library.counts.finished
  })

  const formats = $derived(
    [...(library.stats?.by_format ?? [])].sort((a, b) => b[1] - a[1]).filter(([, n]) => n > 0),
  )
  const maxFormat = $derived(Math.max(1, ...formats.map(([, n]) => n)))
  const hero = $derived(reading[0])
</script>

<div class="home">
  <header class="intro" in:rise={{ y: 8 }}>
    <p class="eyebrow">{greeting}</p>
    <h1>
      {#if hero}Pick up where you left off{:else if (library.stats?.books ?? 0) > 0}What will you read next?{:else}Welcome to Biblion{/if}
    </h1>
  </header>

  {#if loaded && (library.stats?.books ?? 0) === 0}
    <div class="welcome" in:rise={{ delay: 80 }}>
      <Icon name="sparkle" size={28} />
      <h2>Your library is empty</h2>
      <p>Add a folder with EPUB, PDF, MOBI, AZW3 or CBZ/CBR files. Subfolders are included.</p>
      <button class="primary" onclick={onfolders}>Add a folder</button>
    </div>
  {/if}

  {#if hero}
    <button class="hero" in:rise={{ delay: 60, y: 16 }} onclick={() => onopen(hero)}>
      <div class="hero-bg" aria-hidden="true">
        <BookCover title={hero.title} coverKey={hero.cover_key} lazy={false} />
      </div>
      <div class="hero-cover">
        <BookCover title={hero.title} author={hero.authors[0]} coverKey={hero.cover_key} lazy={false} />
      </div>
      <div class="hero-text">
        <span class="label">Continue reading</span>
        <span class="hero-title">{hero.title}</span>
        <span class="hero-author">{hero.authors.join(', ') || 'Unknown author'}</span>
        <div class="hero-progress">
          <ProgressRing value={hero.progress ?? 0} size={52} stroke={5} />
          <span class="cta"><Icon name="play" size={14} filled /> Continue</span>
        </div>
      </div>
    </button>
  {/if}

  {#if reading.length > 1}
    <section class="continue">
      <header>
        <h2>Also reading</h2>
        <button class="ghost all" onclick={() => nav.go(readingNow())}>See all</button>
      </header>
      <div class="cards">
        {#each reading.slice(1, 7) as book, i (book.id)}
          <button class="card" in:rise={{ delay: 120 + i * 50, y: 14 }} onclick={() => onopen(book)}>
            <div class="mini-cover">
              <BookCover title={book.title} author={book.authors[0]} coverKey={book.cover_key} />
            </div>
            <div class="card-text">
              <span class="title">{book.title}</span>
              <span class="author">{book.authors[0] ?? 'Unknown author'}</span>
              <div class="bar"><div style:width="{(book.progress ?? 0) * 100}%"></div></div>
              <span class="pct">{Math.round((book.progress ?? 0) * 100)}% read</span>
            </div>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  {#if recent.length}
    <ShelfRow
      title="Recently added"
      books={recent}
      {onselect}
      {onopen}
      onall={() => nav.go({ ...allBooks(), title: 'Recently added', sort: 'added_desc' })}
    />
  {/if}

  {#if favs.length}
    <ShelfRow title="Favourites" books={favs} {onselect} {onopen} onall={() => nav.go(favorites())} />
  {/if}

  {#if library.stats && library.stats.books > 0}
    <section class="stats" in:rise={{ delay: 200 }}>
      <h2>Your library</h2>
      <div class="tiles">
        <div class="tile"><span class="num">{formatCount(Math.round(books.current))}</span><span>books</span></div>
        <div class="tile"><span class="num">{formatCount(Math.round(authors.current))}</span><span>authors</span></div>
        <div class="tile"><span class="num">{formatCount(Math.round(series.current))}</span><span>series</span></div>
        <div class="tile"><span class="num">{formatCount(Math.round(finishedCount.current))}</span><span>finished</span></div>
        <div class="tile wide">
          <span class="num small">{formatBytes(library.stats.total_bytes)}</span><span>on disk</span>
        </div>
      </div>
      <div class="formats">
        {#each formats as [format, n], i (format)}
          <button class="format" onclick={() => nav.go(formatRoute(format, FORMAT_LABEL[format]))}>
            <span class="fname">{FORMAT_LABEL[format]}</span>
            <span class="track"
              ><span class="fill" style:--w="{(n / maxFormat) * 100}%" style:animation-delay="{300 + i * 70}ms"></span></span
            >
            <span class="fcount">{formatCount(n)}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .home {
    height: 100%;
    overflow-y: auto;
    padding: 28px 36px 48px;
  }

  .eyebrow {
    margin: 0 0 4px;
    color: var(--accent);
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  h1 {
    margin: 0;
    font-family: var(--serif);
    font-size: 30px;
    font-weight: 600;
    line-height: 1.2;
  }

  h2 {
    margin: 0;
    font-family: var(--serif);
    font-size: 20px;
    font-weight: 600;
  }

  .welcome {
    margin-top: 32px;
    padding: 40px;
    border-radius: 16px;
    text-align: center;
    background: linear-gradient(160deg, var(--accent-soft), transparent 70%), var(--surface);
    border: 1px solid var(--border);
    color: var(--accent);
  }

  .welcome h2 {
    color: var(--text);
    margin: 8px 0 4px;
  }

  .welcome p {
    color: var(--muted);
    margin: 0 0 18px;
  }

  /* Hero "continue reading" card with a blurred cover backdrop. */
  .hero {
    position: relative;
    display: flex;
    align-items: center;
    gap: 28px;
    width: 100%;
    margin-top: 22px;
    padding: 24px;
    border: none;
    border-radius: 18px;
    overflow: hidden;
    text-align: left;
    color: #fff;
    background: #2b2118;
    box-shadow: var(--shadow-lift);
    isolation: isolate;
  }

  .hero:hover:not(:disabled) {
    background: #2b2118;
  }

  .hero-bg {
    position: absolute;
    inset: -40px;
    z-index: -1;
    filter: blur(36px) saturate(1.3) brightness(0.55);
    transform: scale(1.1);
    transition: transform 1.2s var(--ease);
  }

  .hero:hover .hero-bg {
    transform: scale(1.18);
  }

  .hero-cover {
    flex: 0 0 120px;
    height: 180px;
    border-radius: 4px 7px 7px 4px;
    overflow: hidden;
    box-shadow: 0 14px 30px rgb(0 0 0 / 0.45);
    transition: transform 400ms var(--ease);
  }

  .hero:hover .hero-cover {
    transform: translateY(-4px) rotate(-1.2deg) scale(1.02);
  }

  .hero-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .label {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    opacity: 0.75;
  }

  .hero-title {
    font-family: var(--serif);
    font-size: 26px;
    font-weight: 600;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .hero-author {
    opacity: 0.8;
  }

  .hero-progress {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-top: 14px;
    --text: #fff;
  }

  .cta {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 20px;
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 600;
    transition: transform 200ms var(--ease);
  }

  .hero:hover .cta {
    transform: translateX(4px);
  }

  /* Other books in progress. */
  .continue {
    margin-top: 28px;
  }

  .continue header {
    display: flex;
    align-items: center;
    margin-bottom: 10px;
  }

  .continue h2 {
    flex: 1;
  }

  .all {
    color: var(--accent);
    font-weight: 600;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 12px;
  }

  .card {
    display: flex;
    gap: 14px;
    padding: 12px;
    border-radius: 12px;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--border);
    transition:
      transform 220ms var(--ease),
      box-shadow 220ms var(--ease),
      background 140ms ease;
  }

  .card:hover:not(:disabled) {
    background: var(--surface);
    transform: translateY(-3px);
    box-shadow: var(--shadow);
  }

  .mini-cover {
    flex: 0 0 56px;
    height: 84px;
    border-radius: 3px 5px 5px 3px;
    overflow: hidden;
    box-shadow: var(--shadow);
  }

  .card-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
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

  .author,
  .pct {
    font-size: 12px;
    color: var(--muted);
  }

  .bar {
    height: 4px;
    margin-top: auto;
    border-radius: 2px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
  }

  /* Stats. */
  .stats {
    margin-top: 34px;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 12px;
    margin-top: 12px;
  }

  .tile {
    display: flex;
    flex-direction: column;
    padding: 16px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--muted);
  }

  .num {
    font-family: var(--serif);
    font-size: 28px;
    font-weight: 600;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }

  .num.small {
    font-size: 22px;
  }

  .formats {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 16px;
    max-width: 560px;
  }

  .format {
    display: grid;
    grid-template-columns: 56px 1fr 70px;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border: none;
    background: transparent;
    border-radius: 8px;
  }

  .fname {
    font-weight: 600;
    font-size: 12px;
    text-align: left;
  }

  .track {
    height: 8px;
    border-radius: 4px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .fill {
    display: block;
    height: 100%;
    width: var(--w);
    border-radius: 4px;
    background: linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 60%, #e6c27a));
    transform-origin: left;
    animation: grow 900ms var(--ease) both;
  }

  @keyframes grow {
    from {
      transform: scaleX(0);
    }
  }

  .fcount {
    text-align: right;
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
</style>
