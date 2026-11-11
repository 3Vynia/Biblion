<script lang="ts">
  // Every highlight, note and bookmark in the library, newest first. Loads
  // more as you scroll; a click opens the book at that exact place.
  import { untrack } from 'svelte'
  import BookCover from '../components/BookCover.svelte'
  import Icon from '../components/Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { confirm } from '../lib/dialog.svelte'
  import { formatCount, formatDate } from '../lib/format'
  import { HIGHLIGHT_COLORS, highlightValue, type HighlightColor } from '../lib/highlights'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import type { AnnotationEntry, AnnotationKind } from '../lib/types'

  interface Props {
    onopen: (bookId: number, locator: string) => void
  }

  let { onopen }: Props = $props()

  const PAGE = 60

  let kind = $state<AnnotationKind | null>(null)
  let color = $state<HighlightColor | null>(null)
  let search = $state('')
  let entries = $state<AnnotationEntry[]>([])
  let total = $state<number | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let copied = $state<number | null>(null)
  let generation = 0

  async function loadMore(reset = false) {
    if (loading && !reset) return
    const gen = reset ? ++generation : generation
    if (reset) {
      entries = []
      total = null
    }
    if (total !== null && entries.length >= total) return
    loading = true
    try {
      const page = await api.listAnnotations({
        kind,
        color: kind === 'bookmark' ? null : color,
        text: search.trim() || null,
        offset: reset ? 0 : entries.length,
        limit: PAGE,
      })
      if (gen !== generation) return
      entries = reset ? page.items : [...entries, ...page.items]
      total = page.total
      error = null
    } catch (e) {
      if (gen === generation) error = errorMessage(e)
    } finally {
      if (gen === generation) loading = false
    }
  }

  // Reload when filters change (search debounced) or annotations change.
  $effect(() => {
    void kind
    void color
    const q = search
    void library.revision
    const t = setTimeout(() => untrack(() => loadMore(true)), q ? 180 : 0)
    return () => clearTimeout(t)
  })

  /** Loads the next page when the end of the list comes near. */
  function loadNearEnd(node: HTMLElement) {
    const observer = new IntersectionObserver((e) => e[0]?.isIntersecting && loadMore(), { rootMargin: '600px' })
    observer.observe(node)
    return { destroy: () => observer.disconnect() }
  }

  async function copy(e: AnnotationEntry) {
    const a = e.annotation
    const text = `“${a.text}”\n— ${e.book_title}${e.book_authors[0] ? `, ${e.book_authors[0]}` : ''}${a.note ? `\n\n${a.note}` : ''}`
    await navigator.clipboard?.writeText(text).catch(() => {})
    copied = a.id
    setTimeout(() => (copied = copied === a.id ? null : copied), 1400)
  }

  async function remove(e: AnnotationEntry) {
    const a = e.annotation
    const bookmark = a.kind === 'bookmark'
    const ok = await confirm({
      title: bookmark ? 'Remove this bookmark?' : 'Delete this highlight?',
      message: bookmark
        ? `${e.book_title} · ${a.chapter ?? ''} ${Math.round(a.percent * 100)}%`
        : `“${a.text.length > 140 ? a.text.slice(0, 140) + '…' : a.text}”${a.note ? '\nIts note will be deleted too.' : ''}\nThis can't be undone.`,
      confirmLabel: bookmark ? 'Remove bookmark' : 'Delete highlight',
      danger: true,
    })
    if (!ok) return
    await api.deleteAnnotation(a.id)
    entries = entries.filter((x) => x.annotation.id !== a.id)
    if (total !== null) total -= 1
    library.refresh()
  }
</script>

<div class="screen">
  <header in:rise={{ y: 6 }}>
    <div class="titles">
      <span class="subtitle">My reading</span>
      <h1>Notes</h1>
    </div>
    <span class="count">{total !== null ? `${formatCount(total)} ${total === 1 ? 'item' : 'items'}` : ''}</span>
    <div class="search">
      <Icon name="search" size={15} />
      <input placeholder="Search quotes and notes" bind:value={search} spellcheck="false" />
    </div>
  </header>

  <div class="filters" in:rise={{ y: 6, delay: 40 }}>
    <div class="segmented" role="radiogroup" aria-label="Kind">
      {#each [null, 'highlight', 'bookmark'] as const as k (k)}
        <button role="radio" aria-checked={kind === k} class:on={kind === k} onclick={() => (kind = k)}>
          {k === null ? 'All' : k === 'highlight' ? 'Highlights' : 'Bookmarks'}
        </button>
      {/each}
    </div>
    {#if kind !== 'bookmark'}
      <div class="colors" role="radiogroup" aria-label="Colour">
        {#each Object.entries(HIGHLIGHT_COLORS) as [id, c] (id)}
          <button
            class="dot"
            class:on={color === id}
            style:--c={c.value}
            role="radio"
            aria-checked={color === id}
            aria-label={c.label}
            title={c.label}
            onclick={() => (color = color === id ? null : (id as HighlightColor))}
          ></button>
        {/each}
      </div>
    {/if}
  </div>

  <div class="scroller">
    {#if error}
      <p class="empty error">Could not load notes: {error}</p>
    {:else if total === 0}
      <div class="empty" in:rise>
        <Icon name="quote" size={40} strokeWidth={1.3} />
        {#if search.trim() || kind || color}
          <p>Nothing matches these filters.</p>
        {:else}
          <h2>No notes yet</h2>
          <p>While reading, select text to highlight it or add a note, or bookmark a page. Everything shows up here.</p>
        {/if}
      </div>
    {:else}
      <div class="masonry">
        {#each entries as e, i (e.annotation.id)}
          {@const a = e.annotation}
          <article class="quote" style:--c={highlightValue(a.color)} in:rise={{ delay: Math.min(i % PAGE, 12) * 35, y: 10 }}>
            <button class="body" onclick={() => onopen(a.book_id, a.locator)} title="Open the book here">
              {#if a.kind === 'highlight'}
                <span class="mark" aria-hidden="true">“</span>
                <span class="text">{a.text}</span>
                {#if a.note}<span class="note"><Icon name="note" size={13} /> {a.note}</span>{/if}
              {:else}
                <span class="bookmark"><Icon name="bookmark" size={14} filled /> Bookmark</span>
                <span class="text plain">{a.text || 'Bookmarked page'}</span>
              {/if}
            </button>
            <footer>
              <div class="mini-cover"><BookCover title={e.book_title} coverKey={e.cover_key} bare /></div>
              <div class="source">
                <span class="book">{e.book_title}</span>
                <span class="where">{a.chapter ? `${a.chapter} · ` : ''}{Math.round(a.percent * 100)}% · {formatDate(a.created_at)}</span>
              </div>
              <button class="icon" onclick={() => copy(e)} title="Copy" aria-label="Copy"
                ><Icon name={copied === a.id ? 'check' : 'copy'} size={14} /></button
              >
              <button class="icon danger" onclick={() => remove(e)} title="Delete" aria-label="Delete"><Icon name="trash" size={14} /></button>
            </footer>
          </article>
        {/each}
      </div>
      <div class="sentinel" use:loadNearEnd>
        {#if loading}<span class="spinner" aria-label="Loading"></span>{/if}
      </div>
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
    padding: 22px 28px 6px;
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
    padding-top: 14px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 18px;
    background: var(--surface);
    color: var(--muted);
  }

  .search input {
    width: 220px;
    border: none;
    background: transparent;
    padding: 7px 2px;
    outline: none;
  }

  .filters {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 6px 28px 12px;
  }

  .segmented {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: 10px;
    background: var(--surface-2);
  }

  .segmented button {
    padding: 5px 12px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
  }

  .segmented button.on {
    background: var(--surface);
    color: var(--text);
    font-weight: 600;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.12);
  }

  .colors {
    display: flex;
    gap: 8px;
  }

  .dot {
    width: 20px;
    height: 20px;
    padding: 0;
    border-radius: 50%;
    border: none;
    background: var(--c);
    transition: transform 160ms var(--ease);
  }

  .dot:hover:not(:disabled) {
    background: var(--c);
    transform: scale(1.15);
  }

  .dot.on {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--c);
  }

  .scroller {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 28px 40px;
  }

  .masonry {
    columns: 300px;
    column-gap: 16px;
  }

  .quote {
    break-inside: avoid;
    margin-bottom: 16px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background:
      linear-gradient(160deg, color-mix(in srgb, var(--c) 14%, transparent), transparent 55%),
      var(--surface);
    overflow: hidden;
    transition:
      transform 240ms var(--ease),
      box-shadow 240ms var(--ease);
  }

  .quote:hover {
    transform: translateY(-3px);
    box-shadow: var(--shadow-lift);
  }

  .body {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    padding: 18px 18px 14px;
    border: none;
    border-radius: 0;
    background: transparent;
    text-align: left;
  }

  .body:hover:not(:disabled) {
    background: transparent;
  }

  .mark {
    position: absolute;
    top: -8px;
    left: 10px;
    font-family: var(--serif);
    font-size: 64px;
    line-height: 1;
    color: var(--c);
    opacity: 0.55;
  }

  .text {
    position: relative;
    font-family: var(--serif);
    font-size: 15.5px;
    line-height: 1.55;
    padding-top: 10px;
    display: -webkit-box;
    -webkit-line-clamp: 10;
    line-clamp: 10;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .text.plain {
    font-family: var(--font);
    font-size: 13px;
    color: var(--muted);
    padding-top: 0;
  }

  .note {
    display: flex;
    gap: 7px;
    padding: 9px 11px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--c) 12%, var(--bg));
    font-size: 13px;
    line-height: 1.45;
    white-space: pre-wrap;
  }

  .bookmark {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px 10px 18px;
    border-top: 1px solid var(--border);
  }

  .mini-cover {
    flex-shrink: 0;
    width: 26px;
    height: 39px;
    border-radius: 2px 4px 4px 2px;
    overflow: hidden;
    box-shadow: var(--shadow);
  }

  .source {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .book,
  .where {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .book {
    font-weight: 600;
    font-size: 13px;
  }

  .where {
    font-size: 11.5px;
    color: var(--muted);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
  }

  .icon:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--text);
  }

  .icon.danger:hover:not(:disabled) {
    color: var(--danger);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    max-width: 420px;
    margin: 80px auto 0;
    text-align: center;
    color: var(--muted);
  }

  .empty h2 {
    margin: 6px 0 0;
    font-family: var(--serif);
    color: var(--text);
  }

  .sentinel {
    display: grid;
    place-items: center;
    height: 48px;
  }

  .spinner {
    width: 18px;
    height: 18px;
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
