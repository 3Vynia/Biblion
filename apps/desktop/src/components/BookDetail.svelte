<script lang="ts">
  import { api, errorMessage, thumbUrl } from '../lib/api'
  import { FORMAT_LABEL, READABLE, formatBytes, formatDate, placeholderHue, plainText } from '../lib/format'
  import type { Book } from '../lib/types'

  interface Props {
    id: number
    onread: (book: Book) => void
    onclose: () => void
  }

  let { id, onread, onclose }: Props = $props()

  let book = $state<Book | null>(null)
  let error = $state<string | null>(null)
  let coverBroken = $state(false)

  $effect(() => {
    const current = id
    book = null
    error = null
    coverBroken = false
    api
      .getBook(current)
      .then((b) => {
        if (current === id) book = b
      })
      .catch((e) => {
        if (current === id) error = errorMessage(e)
      })
  })

  const description = $derived(book?.metadata.description ? plainText(book.metadata.description) : '')
  const canRead = $derived(!!book && !book.missing && READABLE.has(book.format))
</script>

<aside class="detail" aria-label="Book details">
  <button class="ghost close" onclick={onclose} aria-label="Close details">✕</button>

  {#if error}
    <p class="error">Could not load this book: {error}</p>
  {:else if !book}
    <p class="muted">Loading…</p>
  {:else}
    {@const m = book.metadata}
    {@const title = m.title ?? book.path.split('/').pop() ?? 'Untitled'}
    <div class="cover">
      {#if book.cover_key && !coverBroken}
        <img src={thumbUrl(book.cover_key)} alt="Cover of {title}" onerror={() => (coverBroken = true)} />
      {:else}
        <div class="placeholder" style:--hue={placeholderHue(title)}>{title}</div>
      {/if}
    </div>

    <h2>{title}</h2>
    <p class="authors">{m.authors.join(', ') || 'Unknown author'}</p>
    {#if m.series}
      <p class="series">{m.series}{m.series_index !== null ? ` · #${m.series_index}` : ''}</p>
    {/if}

    {#if book.missing}
      <p class="error">The file is missing from disk. It will be matched again if it reappears.</p>
    {/if}

    <div class="actions">
      <button class="primary" disabled={!canRead} onclick={() => book && onread(book)}>
        {book.progress && book.progress.percent > 0 && !book.progress.finished ? 'Continue reading' : 'Read'}
      </button>
      {#if !READABLE.has(book.format)}
        <span class="muted small">{FORMAT_LABEL[book.format]} reader coming soon</span>
      {/if}
    </div>

    {#if book.progress}
      <div class="progress-row">
        <div class="bar"><div style:width="{Math.round(book.progress.percent * 100)}%"></div></div>
        <span class="muted small">{book.progress.finished ? 'Finished' : `${Math.round(book.progress.percent * 100)}%`}</span>
      </div>
    {/if}

    {#if description}
      <p class="description">{description}</p>
    {/if}

    <dl>
      <dt>Format</dt>
      <dd>{FORMAT_LABEL[book.format]} · {formatBytes(book.size)}</dd>
      {#if m.publisher}<dt>Publisher</dt><dd>{m.publisher}</dd>{/if}
      {#if m.published}<dt>Published</dt><dd>{m.published}</dd>{/if}
      {#if m.language}<dt>Language</dt><dd>{m.language}</dd>{/if}
      {#if m.page_count}<dt>Pages</dt><dd>{m.page_count}</dd>{/if}
      {#if m.isbn}<dt>ISBN</dt><dd>{m.isbn}</dd>{/if}
      {#if m.tags.length}<dt>Tags</dt><dd>{m.tags.join(', ')}</dd>{/if}
      <dt>Added</dt>
      <dd>{formatDate(book.added_at)}</dd>
      <dt>File</dt>
      <dd class="path" title={book.path}>{book.path}</dd>
    </dl>
  {/if}
</aside>

<style>
  .detail {
    position: relative;
    height: 100%;
    overflow-y: auto;
    padding: 24px;
    background: var(--surface);
    border-left: 1px solid var(--border);
  }

  .close {
    position: absolute;
    top: 10px;
    right: 10px;
  }

  .cover {
    width: 180px;
    margin: 8px auto 16px;
    aspect-ratio: 2 / 3;
    border-radius: 4px;
    overflow: hidden;
    box-shadow: var(--shadow);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .placeholder {
    height: 100%;
    padding: 16px;
    background: linear-gradient(160deg, hsl(var(--hue) 35% 38%), hsl(var(--hue) 40% 24%));
    color: hsl(var(--hue) 30% 92%);
    font-family: var(--serif);
    font-size: 17px;
  }

  h2 {
    font-family: var(--serif);
    font-size: 21px;
    line-height: 1.25;
    margin: 0 0 4px;
  }

  .authors {
    margin: 0;
    font-size: 15px;
  }

  .series {
    margin: 4px 0 0;
    color: var(--muted);
    font-style: italic;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 18px 0 10px;
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

  .description {
    white-space: pre-line;
    line-height: 1.5;
    margin: 16px 0;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 14px;
    margin: 16px 0 0;
    font-size: 13px;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .path {
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }
</style>
