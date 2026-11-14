<script lang="ts">
  // Edits a book's details by hand. They are saved in Biblion (the file is
  // never touched), kept by rescans and included in backups.
  import { onDestroy, onMount, untrack } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import BookCover from './BookCover.svelte'
  import ChipInput from './ChipInput.svelte'
  import Icon from './Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { confirm, dialog } from '../lib/dialog.svelte'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import type { Book, Metadata } from '../lib/types'

  interface Props {
    book: Book
    onclose: () => void
    ondone: (message: string) => void
  }

  let { book, onclose, ondone }: Props = $props()

  const original = untrack(() => book.metadata)
  const isbnOf = (m: Metadata) => m.isbn ?? m.identifiers?.find((i) => i.scheme === 'isbn')?.value ?? ''

  let title = $state(original.title ?? '')
  let subtitle = $state(original.subtitle ?? '')
  let authors = $state([...original.authors])
  let series = $state(original.series ?? '')
  let number = $state(original.series_index === null ? '' : String(original.series_index))
  let publisher = $state(original.publisher ?? '')
  let published = $state(original.published ?? '')
  let language = $state(original.language ?? '')
  let isbn = $state(isbnOf(original))
  let tags = $state([...original.tags])
  let description = $state(original.description ?? '')

  let coverPath = $state<string | null>(null)
  let coverPreview = $state<string | null>(null)
  let saving = $state(false)
  let error = $state<string | null>(null)
  let box: HTMLDivElement
  let first: HTMLInputElement

  const numberValue = $derived(number.trim() === '' ? null : Number(number.replace(',', '.')))
  const numberValid = $derived(numberValue === null || (Number.isFinite(numberValue) && numberValue >= 0))

  const same = (a: string | null | undefined, b: string) => (a ?? '').trim() === b.trim()
  const sameList = (a: string[], b: string[]) => a.length === b.length && a.every((x, i) => x === b[i])

  // Field labels that differ from what the book had.
  const changed = $derived(
    [
      !same(original.title, title) && 'title',
      !same(original.subtitle, subtitle) && 'subtitle',
      !sameList(original.authors, authors) && 'authors',
      (!same(original.series, series) || original.series_index !== numberValue) && 'series',
      !same(original.publisher, publisher) && 'publisher',
      !same(original.published, published) && 'date',
      !same(original.language, language) && 'language',
      !same(isbnOf(original), isbn) && 'ISBN',
      !sameList(original.tags, tags) && 'subjects',
      !same(original.description, description) && 'description',
    ].filter((x): x is string => !!x),
  )
  const all = $derived(coverPath ? ['cover', ...changed] : changed)
  const canSave = $derived(all.length > 0 && title.trim() !== '' && numberValid && !saving)

  async function chooseCover() {
    error = null
    const picked = await open({
      multiple: false,
      title: 'Choose a cover image',
      filters: [{ name: 'Image', extensions: ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp'] }],
    })
    if (typeof picked !== 'string') return
    try {
      const bytes = await api.readImage(picked)
      if (coverPreview) URL.revokeObjectURL(coverPreview)
      coverPreview = URL.createObjectURL(new Blob([bytes]))
      coverPath = picked
    } catch (e) {
      error = errorMessage(e)
    }
  }

  function undoCover() {
    if (coverPreview) URL.revokeObjectURL(coverPreview)
    coverPreview = null
    coverPath = null
  }

  onDestroy(() => {
    if (coverPreview) URL.revokeObjectURL(coverPreview)
  })

  function metadata(): Metadata {
    const clean = (s: string) => s.trim() || null
    const id = isbn.trim()
    const identifiers = (original.identifiers ?? []).filter((i) => i.scheme !== 'isbn')
    if (id) identifiers.unshift({ scheme: 'isbn', value: id })
    return {
      ...original,
      title: title.trim(),
      subtitle: clean(subtitle),
      authors,
      series: clean(series),
      series_index: clean(series) ? numberValue : null,
      publisher: clean(publisher),
      published: clean(published),
      language: clean(language),
      isbn: id || null,
      identifiers,
      tags,
      description: clean(description),
    }
  }

  async function save(e?: Event) {
    e?.preventDefault()
    if (!canSave) return
    const list = all.length > 1 ? `${all.slice(0, -1).join(', ')} and ${all.at(-1)}` : all[0]
    const ok = await confirm({
      title: `Save changes to “${original.title ?? 'this book'}”?`,
      message: `Its ${list} will change in Biblion. The file itself stays as it is, and the changes are kept after a rescan and in backups.`,
      confirmLabel: 'Save changes',
    })
    if (!ok) return
    saving = true
    error = null
    try {
      if (changed.length) await api.updateMetadata(book.id, metadata())
      if (coverPath) await api.coverFromFile(book.id, coverPath)
      library.changed()
      ondone('Details saved')
    } catch (err) {
      error = errorMessage(err)
    } finally {
      saving = false
    }
  }

  async function close() {
    if (all.length) {
      const ok = await confirm({ title: 'Discard your changes?', message: 'The book keeps the details it had.', confirmLabel: 'Discard', danger: true })
      if (!ok) return
    }
    onclose()
  }

  function onKey(e: KeyboardEvent) {
    if (dialog.current) return
    if (e.key === 'Escape') {
      e.preventDefault()
      close()
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      save()
    } else if (e.key === 'Tab') {
      const items = [...box.querySelectorAll<HTMLElement>('button:not(:disabled), input, textarea')]
      const i = items.indexOf(document.activeElement as HTMLElement)
      const next = e.shiftKey ? (i <= 0 ? items.length - 1 : i - 1) : i === items.length - 1 ? 0 : i + 1
      e.preventDefault()
      items[next]?.focus()
    }
    e.stopPropagation()
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node)
    return { destroy: () => node.remove() }
  }

  onMount(() => first?.focus())
</script>

<div use:portal>
  <div class="backdrop" role="presentation" onclick={close}></div>
  <div
    class="editor"
    role="dialog"
    aria-modal="true"
    aria-labelledby="edit-title"
    tabindex="-1"
    bind:this={box}
    onkeydown={onKey}
    in:rise={{ y: 16, scale: 0.98, duration: 260 }}
  >
    <header>
      <div class="badge"><Icon name="edit" size={19} /></div>
      <div>
        <h2 id="edit-title">Edit details</h2>
        <p>Saved in Biblion. The file stays as it is.</p>
      </div>
      <button class="ghost close" onclick={close} aria-label="Close"><Icon name="close" size={18} /></button>
    </header>

    <form class="body" onsubmit={save}>
      <div class="cover-col">
        <div class="cover" class:new={!!coverPreview}>
          {#if coverPreview}
            <img src={coverPreview} alt="New cover" />
          {:else}
            <BookCover title={title || original.title || ''} author={authors[0]} coverKey={book.cover_key} lazy={false} />
          {/if}
        </div>
        <button type="button" onclick={chooseCover}><Icon name="image" size={15} /> Choose image…</button>
        {#if coverPath}
          <button type="button" class="ghost small" onclick={undoCover}>Keep the old cover</button>
        {/if}
      </div>

      <div class="grid">
        <label class="wide">
          <span>Title</span>
          <input bind:this={first} bind:value={title} required spellcheck="false" />
        </label>
        <label class="wide">
          <span>Subtitle</span>
          <input bind:value={subtitle} spellcheck="false" />
        </label>
        <div class="wide field">
          <span>Authors <small>Enter after each name</small></span>
          <ChipInput label="Authors" values={authors} commaAdds={false} placeholder="Add an author" onchange={(v) => (authors = v)} />
        </div>
        <label>
          <span>Series</span>
          <input bind:value={series} spellcheck="false" placeholder="e.g. Crossed" />
        </label>
        <label class:invalid={!numberValid}>
          <span>Number in series</span>
          <input bind:value={number} inputmode="decimal" placeholder="e.g. 12" disabled={!series.trim()} />
        </label>
        <label>
          <span>Publisher</span>
          <input bind:value={publisher} spellcheck="false" />
        </label>
        <label>
          <span>Published</span>
          <input bind:value={published} placeholder="2011 or 2011-04-20" />
        </label>
        <label>
          <span>Language</span>
          <input bind:value={language} placeholder="en, es, ja…" spellcheck="false" />
        </label>
        <label>
          <span>ISBN</span>
          <input bind:value={isbn} inputmode="numeric" spellcheck="false" />
        </label>
        <div class="wide field">
          <span>Subjects</span>
          <ChipInput label="Subjects" values={tags} placeholder="Horror, Manhwa, Fantasy…" onchange={(v) => (tags = v)} />
        </div>
        <label class="wide">
          <span>Description</span>
          <textarea bind:value={description} rows="5"></textarea>
        </label>
      </div>
    </form>

    <footer>
      {#if error}<p class="error">{error}</p>{/if}
      <span class="status">
        {#if all.length}
          {all.length} {all.length === 1 ? 'change' : 'changes'}: {all.join(', ')}
        {:else}
          No changes yet
        {/if}
      </span>
      <button onclick={close}>Cancel</button>
      <button class="primary" disabled={!canSave} onclick={() => save()}>{saving ? 'Saving…' : 'Save changes…'}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgb(0 0 0 / 0.45);
    backdrop-filter: blur(4px);
    animation: fade 200ms ease both;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .editor {
    position: fixed;
    z-index: 31;
    top: 50%;
    left: 50%;
    translate: -50% -50%;
    display: flex;
    flex-direction: column;
    width: min(860px, calc(100vw - 32px));
    max-height: calc(100vh - 48px);
    border-radius: 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.45);
    overflow: hidden;
    outline: none;
  }

  .editor::before {
    content: '';
    position: absolute;
    inset: -120px -80px auto;
    height: 260px;
    background: radial-gradient(closest-side, color-mix(in srgb, var(--accent) 20%, transparent), transparent);
    pointer-events: none;
  }

  header {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 20px 22px 16px;
    border-bottom: 1px solid var(--border);
  }

  .badge {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 13px;
    color: var(--accent-text);
    background: linear-gradient(140deg, var(--accent), color-mix(in srgb, var(--accent) 60%, #5a2d0c));
    box-shadow: 0 6px 18px color-mix(in srgb, var(--accent) 35%, transparent);
  }

  h2 {
    margin: 0;
    font-family: var(--serif);
    font-size: 21px;
  }

  header p {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--muted);
  }

  .close {
    margin-left: auto;
    align-self: flex-start;
    padding: 6px;
    border-radius: 50%;
  }

  .body {
    position: relative;
    display: grid;
    grid-template-columns: 170px 1fr;
    gap: 24px;
    padding: 20px 22px;
    overflow-y: auto;
  }

  .cover-col {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }

  .cover {
    width: 170px;
    height: 255px;
    margin-bottom: 6px;
    border-radius: 4px 8px 8px 4px;
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: var(--shadow-lift);
    transition:
      transform 300ms var(--ease),
      box-shadow 300ms ease;
  }

  .cover.new {
    transform: rotate(-1.5deg) scale(1.02);
    box-shadow:
      0 0 0 3px var(--accent),
      var(--shadow-lift);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: fade 300ms ease both;
  }

  .cover-col button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }

  .small {
    font-size: 12px;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px 14px;
    align-content: start;
  }

  .wide {
    grid-column: 1 / -1;
  }

  label,
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
    font-size: 11.5px;
    color: var(--muted);
  }

  small {
    margin-left: 6px;
    opacity: 0.75;
  }

  input,
  textarea {
    padding: 8px 11px;
    border-radius: 9px;
    font: inherit;
    font-size: 13.5px;
    color: var(--text);
  }

  textarea {
    resize: vertical;
    min-height: 96px;
    line-height: 1.5;
    border: 1px solid var(--border);
    background: var(--bg);
  }

  input:focus,
  textarea:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  input:disabled {
    opacity: 0.5;
  }

  .invalid input {
    border-color: var(--danger);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 22px;
    border-top: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg) 50%, var(--surface));
  }

  .status {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  footer .error {
    margin: 0;
    font-size: 12.5px;
  }

  @media (max-width: 680px) {
    .body {
      grid-template-columns: 1fr;
    }
    .cover-col {
      align-items: center;
    }
  }
</style>
