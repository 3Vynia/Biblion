<script lang="ts">
  import BookCover from './BookCover.svelte'
  import Icon from './Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import {
    FORMAT_LABEL,
    READABLE,
    formatBytes,
    formatCount,
    formatDate,
    formatMinutesShort,
    formatPublished,
    identifierLabel,
    languageName,
    plainText,
    plural,
    roleLabel,
  } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { slideIn } from '../lib/motion'
  import { authorRoute, nav, seriesRoute, tagRoute } from '../lib/nav.svelte'
  import type { Book, BookStats } from '../lib/types'

  interface Props {
    id: number
    onread: (book: Book) => void
    onclose: () => void
  }

  let { id, onread, onclose }: Props = $props()

  let book = $state<Book | null>(null)
  let error = $state<string | null>(null)
  let memberOf = $state<number[]>([])
  let heartPop = $state(0)
  let stats = $state<BookStats | null>(null)
  let statsState = $state<'idle' | 'loading' | 'done' | 'unavailable'>('idle')
  let copiedId = $state<string | null>(null)

  const ANALYZABLE = new Set(['epub', 'cbz', 'cbr'])

  async function analyze(b: Book) {
    stats = null
    if (!ANALYZABLE.has(b.format) || b.missing) {
      statsState = 'unavailable'
      return
    }
    statsState = 'loading'
    try {
      const s = await api.analyzeBook(b.id)
      if (b.id !== id) return
      stats = s
      statsState = 'done'
    } catch {
      if (b.id === id) statsState = 'unavailable'
    }
  }

  async function copy(value: string) {
    await navigator.clipboard?.writeText(value).catch(() => {})
    copiedId = value
    setTimeout(() => (copiedId = copiedId === value ? null : copiedId), 1400)
  }

  // Generated stats shown as one row of equal tiles.
  const statItems = $derived.by(() => {
    if (!stats || !book) return []
    const comic = book.format === 'cbz' || book.format === 'cbr'
    const items: { icon: 'clock' | 'text' | 'reading' | 'image'; value: string; label: string }[] = []
    if (stats.words > 0) {
      items.push({ icon: 'clock', value: formatMinutesShort(stats.reading_minutes), label: 'to read' })
      items.push({ icon: 'text', value: formatCount(stats.words), label: plural(stats.words, 'word') })
    }
    if (stats.chapters > 0) items.push({ icon: 'reading', value: formatCount(stats.chapters), label: plural(stats.chapters, 'chapter') })
    if (stats.images > 0)
      items.push({ icon: 'image', value: formatCount(stats.images), label: comic ? plural(stats.images, 'page') : plural(stats.images, 'image') })
    return items
  })

  // Contributors grouped by role: "Translated by A, B".
  const contributorGroups = $derived.by(() => {
    const groups = new Map<string, string[]>()
    for (const c of book?.metadata.contributors ?? []) groups.set(c.role, [...(groups.get(c.role) ?? []), c.name])
    return [...groups].map(([role, names]) => ({ label: roleLabel(role), names }))
  })

  async function load(current: number) {
    try {
      const [b, shelves] = await Promise.all([api.getBook(current), api.bookShelves(current)])
      if (current !== id) return
      const first = book?.id !== b.id
      book = b
      memberOf = shelves
      if (first) analyze(b)
    } catch (e) {
      if (current === id) error = errorMessage(e)
    }
  }

  $effect(() => {
    const current = id
    void library.revision
    error = null
    load(current)
  })

  async function toggleFavorite() {
    if (!book) return
    const next = !book.favorite
    book = { ...book, favorite: next }
    if (next) heartPop++
    try {
      await api.setFavorite(book.id, next)
      library.changed()
    } catch (e) {
      book = { ...book, favorite: !next }
      error = errorMessage(e)
    }
  }

  async function toggleShelf(shelf: number) {
    if (!book) return
    const member = !memberOf.includes(shelf)
    memberOf = member ? [...memberOf, shelf] : memberOf.filter((s) => s !== shelf)
    try {
      await api.setInShelf(shelf, book.id, member)
      library.changed()
    } catch (e) {
      error = errorMessage(e)
    }
  }

  async function openAuthor(name: string) {
    try {
      const page = await api.listAuthors(name, 0, 20)
      const match = page.items.find((a) => a.name.toLowerCase() === name.toLowerCase())
      if (match) nav.go(authorRoute(match.id, match.name))
    } catch (e) {
      console.error('could not open author', e)
    }
  }

  const description = $derived(book?.metadata.description ? plainText(book.metadata.description) : '')
  const canRead = $derived(!!book && !book.missing && READABLE.has(book.format))
  const started = $derived(!!book?.progress && book.progress.percent > 0 && !book.progress.finished)
</script>

<aside class="detail" aria-label="Book details" in:slideIn>
  <button class="ghost close" onclick={onclose} aria-label="Close details"><Icon name="close" size={18} /></button>

  {#if error && !book}
    <p class="error">Could not load this book: {error}</p>
  {:else if !book}
    <div class="loading"><div class="cover skeleton"></div></div>
  {:else}
    {@const m = book.metadata}
    {@const title = m.title ?? book.path.split('/').pop() ?? 'Untitled'}
    <div class="backdrop" aria-hidden="true">
      <BookCover {title} coverKey={book.cover_key} lazy={false} />
    </div>

    <div class="cover">
      <BookCover {title} author={m.authors[0]} coverKey={book.cover_key} lazy={false} />
    </div>

    <h2>{title}</h2>
    {#if m.subtitle}<p class="subtitle">{m.subtitle}</p>{/if}
    <p class="authors">
      {#each m.authors as author, i (author)}
        <button class="link" onclick={() => openAuthor(author)}>{author}</button>{i < m.authors.length - 1 ? ', ' : ''}
      {:else}
        <span class="muted">Unknown author</span>
      {/each}
    </p>
    {#each contributorGroups as g (g.label)}
      <p class="contributors"><span class="muted">{g.label}</span> {g.names.join(', ')}</p>
    {/each}
    {#if m.series}
      <button class="link series" onclick={() => nav.go(seriesRoute(m.series!))}>
        <Icon name="layers" size={14} />
        {m.series}{m.series_index !== null ? ` · #${m.series_index}` : ''}
      </button>
    {/if}

    {#if book.missing}
      <p class="error">The file is missing from disk. It will be matched again if it reappears.</p>
    {/if}

    <div class="actions">
      <button class="primary read" disabled={!canRead} onclick={() => book && onread(book)}>
        <Icon name="play" size={14} filled />
        {started ? 'Continue reading' : book.progress?.finished ? 'Read again' : 'Read'}
      </button>
      <button
        class="icon-btn heart"
        class:on={book.favorite}
        onclick={toggleFavorite}
        aria-pressed={book.favorite}
        title={book.favorite ? 'Remove from favourites' : 'Add to favourites'}
      >
        {#key heartPop}
          <span class="heart-icon" class:pop={heartPop > 0 && book.favorite}><Icon name="heart" size={18} filled={book.favorite} /></span>
        {/key}
      </button>
    </div>
    {#if !READABLE.has(book.format)}
      <p class="muted small">{FORMAT_LABEL[book.format]} reader coming soon</p>
    {/if}

    {#if book.progress}
      <div class="progress-row">
        <div class="bar"><div style:width="{Math.round(book.progress.percent * 100)}%"></div></div>
        <span class="muted small">{book.progress.finished ? 'Finished' : `${Math.round(book.progress.percent * 100)}%`}</span>
      </div>
    {/if}

    <div class="shelves">
      {#each library.shelves as shelf (shelf.id)}
        {@const member = memberOf.includes(shelf.id)}
        <button class="chip" class:on={member} onclick={() => toggleShelf(shelf.id)} aria-pressed={member}>
          <Icon name={member ? 'check' : shelf.id === library.toRead?.id ? 'bookmark' : 'plus'} size={13} strokeWidth={2.2} />
          {shelf.name}
        </button>
      {/each}
    </div>

    {#if statsState === 'loading'}
      <div class="stats loading" aria-label="Analysing the book"><div class="shimmer"></div></div>
    {:else if statsState === 'done' && statItems.length}
      <div class="stats" style:--n={statItems.length}>
        {#each statItems as item, i (item.label)}
          <div class="stat" style:animation-delay="{i * 60}ms">
            <Icon name={item.icon} size={15} />
            <strong>{item.value}</strong>
            <small>{item.label}</small>
          </div>
        {/each}
      </div>
    {/if}

    {#if description}
      <h3>About this book</h3>
      <p class="description">{description}</p>
    {/if}

    {#if m.tags.length}
      <div class="tags">
        {#each m.tags as tag (tag)}
          <button class="tag" onclick={() => nav.go(tagRoute(tag))}><Icon name="tag" size={12} /> {tag}</button>
        {/each}
      </div>
    {/if}

    <h3>Details</h3>

    <dl>
      <dt>Format</dt>
      <dd>{FORMAT_LABEL[book.format]} · {formatBytes(book.size)}</dd>
      {#if m.format_version}<dt>Edition</dt><dd>{m.format_version}</dd>{/if}
      {#if m.publisher}<dt>Publisher</dt><dd>{m.publisher}</dd>{/if}
      {#if m.published}<dt>Published</dt><dd>{formatPublished(m.published)}</dd>{/if}
      {#if m.language}<dt>Language</dt><dd>{languageName(m.language)}</dd>{/if}
      {#if m.page_count}<dt>Pages</dt><dd>{formatCount(m.page_count)}</dd>{/if}
      {#if m.modified}<dt>Revised</dt><dd>{formatPublished(m.modified)}</dd>{/if}
      {#if m.rights}<dt>Rights</dt><dd>{m.rights}</dd>{/if}
      {#if m.source}<dt>Source</dt><dd class="small-text">{m.source}</dd>{/if}
      {#if m.identifiers?.length || m.isbn}
        <dt>Identifiers</dt>
        <dd class="ids">
          {#each m.identifiers?.length ? m.identifiers : [{ scheme: 'isbn', value: m.isbn ?? '' }] as ident (ident.scheme + ident.value)}
            <button class="ident" onclick={() => copy(ident.value)} title="Copy {identifierLabel(ident.scheme)}">
              <span class="scheme">{identifierLabel(ident.scheme)}</span>
              <span class="value">{ident.value}</span>
              <Icon name={copiedId === ident.value ? 'check' : 'copy'} size={12} />
            </button>
          {/each}
        </dd>
      {/if}
      <dt>Added</dt>
      <dd>{formatDate(book.added_at)}</dd>
      <dt>File</dt>
      <dd class="path" title={book.path}>{book.path}</dd>
    </dl>
    {#if error}<p class="error small">{error}</p>{/if}
  {/if}
</aside>

<style>
  .detail {
    position: relative;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 28px 24px;
    background: var(--surface);
    border-left: 1px solid var(--border);
    isolation: isolate;
  }

  .backdrop {
    position: absolute;
    inset: -60px -60px auto;
    height: 340px;
    z-index: -1;
    opacity: 0.35;
    filter: blur(40px) saturate(1.4);
    mask-image: linear-gradient(to bottom, #000 30%, transparent);
  }

  .close {
    position: absolute;
    top: 10px;
    right: 10px;
    padding: 6px;
    border-radius: 50%;
    z-index: 1;
  }

  .cover {
    width: 170px;
    height: 255px;
    margin: 4px auto 20px;
    border-radius: 4px 8px 8px 4px;
    overflow: hidden;
    box-shadow: var(--shadow-lift);
    animation: settle 600ms var(--ease) both;
  }

  @keyframes settle {
    from {
      transform: translateY(10px) rotate(-3deg) scale(0.96);
      opacity: 0;
    }
  }

  .loading .cover {
    animation: none;
  }

  .skeleton {
    background: var(--surface-2);
  }

  h2 {
    font-family: var(--serif);
    font-size: 22px;
    line-height: 1.25;
    margin: 0 0 6px;
  }

  .authors {
    margin: 0;
    font-size: 15px;
  }

  .subtitle {
    margin: -2px 0 8px;
    font-family: var(--serif);
    font-style: italic;
    color: var(--muted);
    font-size: 15px;
  }

  .contributors {
    margin: 4px 0 0;
    font-size: 13px;
  }

  h3 {
    margin: 22px 0 8px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(var(--n, 1), 1fr);
    margin-top: 18px;
    border-radius: 12px;
    border: 1px solid var(--border);
    background: var(--bg);
    overflow: hidden;
  }

  .stat {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    min-width: 0;
    padding: 12px 4px 10px;
    color: var(--accent);
    animation: settle 420ms var(--ease) both;
  }

  .stat + .stat {
    border-left: 1px solid var(--border);
  }

  .stat strong {
    color: var(--text);
    font-size: 15px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .stat small {
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
  }

  .stats.loading {
    height: 70px;
  }

  .shimmer {
    height: 100%;
    background: linear-gradient(90deg, var(--surface-2), var(--bg), var(--surface-2));
    background-size: 200% 100%;
    animation: shimmer 1.2s linear infinite;
  }

  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }

  .tag {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 10px;
    border-radius: 12px;
    font-size: 12px;
    background: var(--bg);
  }

  .tag:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .ids {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .ident {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 12px;
    text-align: left;
    color: var(--muted);
  }

  .ident .scheme {
    font-weight: 700;
    font-size: 10px;
    letter-spacing: 0.05em;
    color: var(--accent);
  }

  .ident .value {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
    font-family: ui-monospace, monospace;
  }

  .small-text {
    font-size: 12px;
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, currentColor 30%, transparent);
    text-underline-offset: 3px;
  }

  .link:hover:not(:disabled) {
    background: none;
    color: var(--accent);
    text-decoration-color: currentColor;
  }

  .series {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    color: var(--muted);
    font-style: italic;
    text-decoration: none;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 20px 0 10px;
  }

  .read {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 9px 18px;
    border-radius: 20px;
    font-weight: 600;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    padding: 0;
    border-radius: 50%;
  }

  .heart.on {
    color: #e11d48;
    border-color: color-mix(in srgb, #e11d48 40%, transparent);
    background: color-mix(in srgb, #e11d48 10%, var(--surface));
  }

  .heart-icon {
    display: grid;
  }

  .heart-icon.pop {
    animation: pop 480ms var(--ease);
  }

  @keyframes pop {
    30% {
      transform: scale(1.45);
    }
    60% {
      transform: scale(0.9);
    }
  }

  .small {
    font-size: 12px;
  }

  .progress-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .bar {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
  }

  .shelves {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 16px 0 4px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 11px;
    border-radius: 14px;
    font-size: 12px;
    color: var(--muted);
  }

  .chip.on {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: var(--accent-soft);
    font-weight: 600;
  }

  .description {
    white-space: pre-line;
    line-height: 1.6;
    margin: 0;
  }

  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 6px 14px;
    margin: 16px 0 0;
    font-size: 13px;
  }

  dt {
    color: var(--muted);
  }

  dd {
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
  }

  .path {
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }
</style>
