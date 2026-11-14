<script lang="ts">
  // Finds a book in every online source at once (Open Library, AniList) and
  // lets the user pick, field by field, what to take from it. Nothing changes
  // until they confirm.
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { onDestroy, onMount, untrack } from 'svelte'
  import BookCover from './BookCover.svelte'
  import Icon from './Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { confirm, dialog } from '../lib/dialog.svelte'
  import { formatCount, plainText, plural } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { applyChanges, proposedChanges, rankCandidates, type FieldId } from '../lib/metadataMatch'
  import { rise } from '../lib/motion'
  import type { Book, MdCandidate, MdDetails, SourceResults } from '../lib/types'

  interface Props {
    book: Book
    onclose: () => void
    ondone: (message: string) => void
  }

  let { book, onclose, ondone }: Props = $props()

  const m = $derived(book.metadata)
  // The search starts from what the book has; the user can change it.
  const start = untrack(() => book.metadata)
  let title = $state(start.title ?? '')
  let author = $state(start.authors[0] ?? '')
  let isbn = $state(start.isbn ?? start.identifiers?.find((i) => i.scheme === 'isbn')?.value ?? '')

  let searching = $state(false)
  let searchError = $state<string | null>(null)
  let answers = $state<SourceResults[] | null>(null)
  let only = $state<string | null>(null)
  let picked = $state<MdCandidate | null>(null)
  let work = $state<MdDetails | null>(null)
  let workState = $state<'loading' | 'done' | 'failed'>('loading')
  let chosen = $state(new Set<FieldId>())
  let useCover = $state(false)
  let saving = $state(false)
  let saveError = $state<string | null>(null)

  let box: HTMLDivElement
  let firstField: HTMLInputElement

  // ── Cover previews, fetched one by one and kept as blob URLs ──
  let covers = $state<Record<string, string | 'none'>>({})
  const urls: string[] = []
  let alive = true

  async function loadCovers(list: MdCandidate[]) {
    for (const c of list) {
      if (!alive) return
      const id = c.cover
      if (!id || covers[id]) continue
      try {
        const bytes = await api.mdCover(id)
        const url = URL.createObjectURL(new Blob([bytes], { type: 'image/jpeg' }))
        urls.push(url)
        covers[id] = url
      } catch {
        covers[id] = 'none'
      }
    }
  }

  onDestroy(() => {
    alive = false
    urls.forEach((u) => URL.revokeObjectURL(u))
  })

  async function search(e?: Event) {
    e?.preventDefault()
    if (searching || (!title.trim() && !author.trim() && !isbn.trim())) return
    searching = true
    searchError = null
    picked = null
    try {
      answers = await api.mdSearch(title, author, isbn)
      only = null
      // Covers of the best matches first.
      loadCovers(rankCandidates(answers, title, author))
      const failed = answers.filter((a) => a.error)
      if (failed.length === answers.length && failed.length) searchError = failed[0].error
    } catch (err) {
      searchError = errorMessage(err)
      answers = null
    } finally {
      searching = false
    }
  }

  const ranked = $derived(answers ? rankCandidates(answers, title, author) : [])
  const results = $derived(answers ? ranked.filter((c) => !only || c.source === only) : null)
  const sourceLabel = (source: string) => answers?.find((a) => a.source === source)?.label ?? source

  async function pick(c: MdCandidate) {
    picked = c
    work = null
    workState = 'loading'
    useCover = !book.has_cover && !!c.cover
    chosen = new Set(proposedChanges(m, c, null).filter((ch) => ch.suggested).map((ch) => ch.field))
    try {
      const w = await api.mdDetails(c.source, c.key)
      if (picked !== c) return
      work = w
      workState = 'done'
      // The description and subjects arrive now: suggest them too.
      for (const ch of proposedChanges(m, c, w)) if (ch.suggested && (ch.field === 'description' || ch.field === 'tags')) chosen.add(ch.field)
      chosen = new Set(chosen)
    } catch {
      if (picked === c) workState = 'failed'
    }
  }

  const changes = $derived(picked ? proposedChanges(m, picked, work) : [])
  const count = $derived(changes.filter((c) => chosen.has(c.field)).length + (useCover ? 1 : 0))

  function toggle(field: FieldId) {
    const next = new Set(chosen)
    if (next.has(field)) next.delete(field)
    else next.add(field)
    chosen = next
  }

  async function apply() {
    if (!picked || !count || saving) return
    const names = [...(useCover ? ['cover'] : []), ...changes.filter((c) => chosen.has(c.field)).map((c) => c.label.toLowerCase())]
    const list = names.length > 1 ? `${names.slice(0, -1).join(', ')} and ${names.at(-1)}` : names[0]
    const ok = await confirm({
      title: `Update “${m.title ?? 'this book'}”?`,
      message: `Its ${list} will come from ${sourceLabel(picked.source)}. The book file itself is not changed, and Biblion keeps these details after a rescan.`,
      confirmLabel: 'Update book',
    })
    if (!ok) return
    saving = true
    saveError = null
    try {
      const fields = changes.filter((c) => chosen.has(c.field))
      if (fields.length) await api.updateMetadata(book.id, applyChanges(m, changes, chosen))
      if (useCover && picked.cover) await api.mdApplyCover(book.id, picked.cover)
      library.changed()
      ondone(`Updated ${count} ${plural(count, 'detail')} from ${sourceLabel(picked.source)}`)
    } catch (err) {
      saveError = errorMessage(err)
    } finally {
      saving = false
    }
  }

  // Keys stay inside: Esc goes back or closes, Tab cycles through the window.
  function onKey(e: KeyboardEvent) {
    if (dialog.current) return
    if (e.key === 'Escape') {
      e.preventDefault()
      if (picked) picked = null
      else onclose()
    } else if (e.key === 'Tab') {
      const items = [...box.querySelectorAll<HTMLElement>('button:not(:disabled), input, [tabindex="0"]')]
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

  onMount(() => {
    firstField?.focus()
    firstField?.select()
    search()
  })

  const meta = (c: MdCandidate) =>
    [
      c.year,
      c.volumes ? `${formatCount(c.volumes)} ${plural(c.volumes, 'volume')}` : null,
      c.editions > 1 ? `${formatCount(c.editions)} editions` : null,
      c.pages ? `${formatCount(c.pages)} pages` : null,
    ]
      .filter(Boolean)
      .join(' · ')
</script>

<div use:portal>
  <div class="backdrop" role="presentation" onclick={onclose}></div>
  <div
    class="lookup"
    role="dialog"
    aria-modal="true"
    aria-labelledby="lookup-title"
    tabindex="-1"
    bind:this={box}
    onkeydown={onKey}
    in:rise={{ y: 16, scale: 0.98, duration: 260 }}
  >
    <header>
      <div class="badge"><Icon name="globe" size={20} /></div>
      <div>
        <h2 id="lookup-title">Find details online</h2>
        <p>From Open Library and AniList, all at once. Only what you type below is sent.</p>
      </div>
      <button class="ghost close" onclick={onclose} aria-label="Close"><Icon name="close" size={18} /></button>
    </header>

    <form class="query" onsubmit={search}>
      <label class="title-field">
        <span>Title</span>
        <input bind:this={firstField} bind:value={title} spellcheck="false" />
      </label>
      <label>
        <span>Author</span>
        <input bind:value={author} spellcheck="false" />
      </label>
      <label class="isbn-field">
        <span>ISBN</span>
        <input bind:value={isbn} spellcheck="false" inputmode="numeric" />
      </label>
      <button class="primary" type="submit" disabled={searching}>
        <Icon name="search" size={15} />
        {searching ? 'Searching…' : 'Search'}
      </button>
    </form>

    <div class="body">
      {#if picked}
        {@const c = picked}
        <div class="compare" in:rise={{ y: 8, duration: 220 }}>
          <button class="back ghost" onclick={() => (picked = null)}><Icon name="chevron-left" size={15} /> All results</button>

          <div class="covers">
            <figure>
              <div class="cover-frame"><BookCover title={m.title ?? ''} author={m.authors[0]} coverKey={book.cover_key} lazy={false} /></div>
              <figcaption>Now</figcaption>
            </figure>
            <span class="arrow" class:on={useCover} aria-hidden="true"><Icon name="chevron-right" size={22} /></span>
            <figure>
              <div class="cover-frame new" class:on={useCover}>
                {#if c.cover && covers[c.cover] && covers[c.cover] !== 'none'}
                  <img src={covers[c.cover]} alt="Cover from {sourceLabel(c.source)}" />
                {:else if c.cover && !covers[c.cover]}
                  <span class="shimmer"></span>
                {:else}
                  <span class="no-cover">No cover</span>
                {/if}
              </div>
              <label class="check" class:disabled={!c.cover || covers[c.cover] === 'none'}>
                <input type="checkbox" bind:checked={useCover} disabled={!c.cover || covers[c.cover] === 'none'} />
                <span class="box"><Icon name="check" size={12} strokeWidth={3} /></span>
                Use this cover
              </label>
            </figure>
          </div>

          <div class="fields">
            <div class="tags-row">
              <span class="src {c.source}">{sourceLabel(c.source)}</span>
              {#if c.kind}<span class="kind">{c.kind}</span>{/if}
              {#if c.url}
                <button class="ghost open-src" onclick={() => c.url && openUrl(c.url)}>View on {sourceLabel(c.source)} <Icon name="chevron-right" size={12} /></button>
              {/if}
            </div>
            <h3>{c.title}{#if c.authors.length}<span class="h-by">{c.authors.join(', ')}</span>{/if}</h3>
            {#each changes as ch, i (ch.field)}
              {@const on = chosen.has(ch.field)}
              <label class="field" class:on style:animation-delay="{i * 40}ms">
                <input type="checkbox" checked={on} onchange={() => toggle(ch.field)} />
                <span class="box"><Icon name="check" size={12} strokeWidth={3} /></span>
                <span class="field-text">
                  <span class="field-label">{ch.label}</span>
                  {#if ch.field === 'tags'}
                    <span class="chips">
                      {#each m.tags as t (t)}<span class="chip">{t}</span>{/each}
                      {#each ch.added ?? [] as t (t)}<span class="chip new">+ {t}</span>{/each}
                    </span>
                  {:else}
                    {#if ch.current}<span class="was" class:struck={on}>{ch.field === 'description' ? plainText(ch.current) : ch.current}</span>{/if}
                    <span class="next" class:long={ch.field === 'description'} class:replaces={!!ch.current}>{ch.next}</span>
                  {/if}
                </span>
              </label>
            {:else}
              {#if workState !== 'loading'}
                <p class="nothing"><Icon name="check" size={16} /> This book already has everything {sourceLabel(c.source)} knows about it{c.cover ? ', apart from the cover' : ''}.</p>
              {/if}
            {/each}
            {#if workState === 'loading'}
              <div class="field loading"><span class="shimmer"></span></div>
            {:else if workState === 'failed'}
              <p class="hint">The description and subjects could not be loaded.</p>
            {/if}
          </div>
        </div>
      {:else if searching && !answers}
        <div class="grid">
          {#each Array(6) as _, i (i)}<div class="result skeleton"><span class="shimmer"></span></div>{/each}
        </div>
      {:else if searchError}
        <div class="empty">
          <Icon name="globe" size={28} />
          <p>{searchError}</p>
          <button onclick={() => search()}>Try again</button>
        </div>
      {:else if answers && ranked.length === 0}
        <div class="empty">
          <Icon name="search" size={28} />
          <p>Nothing found. Try a shorter title, or only the author’s last name.</p>
        </div>
      {:else if results}
        <div class="sources">
          <button class:on={!only} onclick={() => (only = null)}>All <span>{ranked.length}</span></button>
          {#each answers ?? [] as a (a.source)}
            <button class:on={only === a.source} class:failed={!!a.error} disabled={!!a.error} title={a.error ?? undefined} onclick={() => (only = a.source)}>
              <span class="dot {a.source}"></span>{a.label} <span>{a.error ? '!' : a.candidates.length}</span>
            </button>
          {/each}
          <span class="pick-hint">Pick the one that is this book.</span>
        </div>
        {#each (answers ?? []).filter((a) => a.error) as a (a.source)}
          <p class="source-error"><Icon name="globe" size={13} /> {a.error}</p>
        {/each}
        <div class="grid" class:dim={searching}>
          {#each results as c, i (c.source + c.key)}
            <button class="result" onclick={() => pick(c)} in:rise={{ y: 10, delay: i * 35, duration: 240 }}>
              <span class="thumb">
                {#if c.cover && covers[c.cover] && covers[c.cover] !== 'none'}
                  <img src={covers[c.cover]} alt="" />
                {:else if c.cover && !covers[c.cover]}
                  <span class="shimmer"></span>
                {:else}
                  <Icon name="reading" size={20} />
                {/if}
              </span>
              <span class="result-text">
                <span class="tags-row">
                  <span class="src {c.source}">{sourceLabel(c.source)}</span>
                  {#if c.kind}<span class="kind">{c.kind}</span>{/if}
                </span>
                <strong>{c.title}</strong>
                {#if c.alt_titles[0]}<span class="alt">{c.alt_titles[0]}</span>{/if}
                {#if c.authors.length}<span class="by">{c.authors.slice(0, 2).join(', ')}</span>{/if}
                <span class="meta">{meta(c)}</span>
                {#if c.subjects.length}
                  <span class="chips">{#each c.subjects.slice(0, 3) as t (t)}<span class="chip">{t}</span>{/each}</span>
                {/if}
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    {#if picked}
      <footer>
        {#if saveError}<p class="error">{saveError}</p>{/if}
        <span class="selected">{count ? `${count} ${plural(count, 'change')} selected` : 'Nothing selected'}</span>
        <button onclick={onclose}>Cancel</button>
        <button class="primary" disabled={!count || saving} onclick={apply}>{saving ? 'Updating…' : 'Update book…'}</button>
      </footer>
    {/if}
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

  .lookup {
    position: fixed;
    z-index: 31;
    top: 50%;
    left: 50%;
    translate: -50% -50%;
    display: flex;
    flex-direction: column;
    width: min(880px, calc(100vw - 32px));
    height: min(720px, calc(100vh - 48px));
    border-radius: 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.45);
    overflow: hidden;
    outline: none;
  }

  /* A soft glow of the accent behind the header. */
  .lookup::before {
    content: '';
    position: absolute;
    inset: -120px -80px auto;
    height: 260px;
    background: radial-gradient(closest-side, color-mix(in srgb, var(--accent) 22%, transparent), transparent);
    pointer-events: none;
  }

  header {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 20px 22px 14px;
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

  .query {
    position: relative;
    display: grid;
    grid-template-columns: 2fr 1.4fr 1fr auto;
    gap: 10px;
    align-items: end;
    padding: 0 22px 16px;
    border-bottom: 1px solid var(--border);
  }

  .query label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
    font-size: 11.5px;
    color: var(--muted);
  }

  .query input {
    padding: 8px 11px;
    border-radius: 9px;
    font-size: 13.5px;
    color: var(--text);
  }

  .query input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .query button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
  }

  @media (max-width: 720px) {
    .query {
      grid-template-columns: 1fr 1fr;
    }
    .title-field {
      grid-column: 1 / -1;
    }
  }

  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 22px 22px;
  }

  .count {
    margin: 0 0 12px;
    font-size: 12.5px;
    color: var(--muted);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 10px;
    transition: opacity 200ms ease;
  }

  .grid.dim {
    opacity: 0.5;
  }

  .result {
    display: flex;
    gap: 12px;
    padding: 10px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--bg);
    text-align: left;
    transition:
      transform 220ms var(--ease),
      border-color 200ms ease,
      box-shadow 220ms ease;
  }

  .result:hover:not(:disabled),
  .result:focus-visible {
    transform: translateY(-2px);
    border-color: var(--accent);
    background: var(--bg);
    box-shadow: 0 10px 24px rgb(0 0 0 / 0.18);
  }

  .result.skeleton {
    height: 116px;
    padding: 0;
    overflow: hidden;
  }

  .thumb {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 62px;
    height: 94px;
    border-radius: 3px 6px 6px 3px;
    overflow: hidden;
    color: var(--muted);
    background: var(--surface-2);
    box-shadow: 0 3px 10px rgb(0 0 0 / 0.25);
  }

  .thumb img,
  .cover-frame img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: fade 300ms ease both;
  }

  .sources {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: 12px;
  }

  .sources button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 11px;
    border-radius: 14px;
    font-size: 12.5px;
    color: var(--muted);
  }

  .sources button span:last-child {
    font-variant-numeric: tabular-nums;
    opacity: 0.75;
  }

  .sources button.on {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: var(--accent-soft);
    font-weight: 600;
  }

  .sources button.failed {
    opacity: 0.6;
  }

  .pick-hint {
    margin-left: auto;
    font-size: 12.5px;
    color: var(--muted);
  }

  .source-error {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: -4px 0 10px;
    font-size: 12px;
    color: var(--muted);
  }

  /* Each source keeps one colour, in the filter and on its results. */
  .dot,
  .src {
    --src: var(--muted);
  }

  .openlibrary {
    --src: #3b82f6;
  }

  .anilist {
    --src: #02a9ff;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--src);
  }

  .tags-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    margin-bottom: 3px;
  }

  .src,
  .kind {
    padding: 1px 7px;
    border-radius: 6px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .src {
    color: var(--src);
    background: color-mix(in srgb, var(--src) 14%, transparent);
  }

  .src.openlibrary {
    --src: #5b8def;
  }

  .kind {
    color: var(--accent);
    background: var(--accent-soft);
  }

  .alt {
    font-size: 11.5px;
    color: var(--muted);
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .open-src {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    margin-left: auto;
    padding: 2px 6px;
    font-size: 12px;
    color: var(--muted);
  }

  .result-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .result-text strong {
    font-family: var(--serif);
    font-size: 15px;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .by {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    font-size: 11.5px;
    color: var(--muted);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
  }

  .chip {
    padding: 1px 8px;
    border-radius: 10px;
    font-size: 11px;
    color: var(--muted);
    background: var(--surface-2);
  }

  .chip.new {
    color: var(--accent);
    background: var(--accent-soft);
    font-weight: 600;
  }

  .shimmer {
    display: block;
    width: 100%;
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

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 60px 20px;
    text-align: center;
    color: var(--muted);
  }

  .empty p {
    max-width: 360px;
    margin: 0;
    line-height: 1.5;
  }

  /* ── Compare ── */
  .compare {
    display: grid;
    grid-template-columns: 250px 1fr;
    grid-template-rows: auto 1fr;
    gap: 6px 26px;
  }

  .back {
    grid-column: 1 / -1;
    justify-self: start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px 4px 4px;
    font-size: 12.5px;
    color: var(--muted);
  }

  .covers {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }

  figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  figcaption {
    font-size: 12px;
    color: var(--muted);
    padding-top: 2px;
  }

  .cover-frame {
    display: grid;
    place-items: center;
    width: 104px;
    height: 156px;
    border-radius: 3px 7px 7px 3px;
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.3);
    transition:
      transform 300ms var(--ease),
      box-shadow 300ms ease;
  }

  .cover-frame.new.on {
    transform: scale(1.04) rotate(1.5deg);
    box-shadow:
      0 0 0 3px var(--accent),
      0 12px 30px rgb(0 0 0 / 0.35);
  }

  .no-cover {
    font-size: 12px;
    color: var(--muted);
  }

  .arrow {
    margin-top: 66px;
    color: var(--muted);
    transition:
      color 200ms ease,
      transform 300ms var(--ease);
  }

  .arrow.on {
    color: var(--accent);
    transform: translateX(3px);
  }

  .fields h3 {
    margin: 0 0 10px;
    font-family: var(--serif);
    font-size: 18px;
  }

  .fields h3 .h-by::before {
    content: '·';
    margin: 0 6px;
  }

  .fields h3 span {
    font-family: var(--font);
    font-size: 13px;
    font-weight: 400;
    color: var(--muted);
  }

  .field {
    display: flex;
    gap: 12px;
    padding: 12px;
    margin-bottom: 8px;
    border-radius: 12px;
    border: 1px solid var(--border);
    cursor: pointer;
    animation: rise-in 300ms var(--ease) both;
    transition:
      border-color 200ms ease,
      background 200ms ease;
  }

  @keyframes rise-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  .field.on {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
  }

  .field.loading {
    height: 74px;
    padding: 0;
    overflow: hidden;
    cursor: default;
  }

  input[type='checkbox'] {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .box {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 6px;
    border: 1.5px solid var(--muted);
    color: transparent;
    transition: all 180ms ease;
  }

  input:checked + .box {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-text);
  }

  input:focus-visible + .box {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .field-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .field-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  .was {
    font-size: 12.5px;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    transition: opacity 200ms ease;
  }

  .was.struck {
    text-decoration: line-through;
    opacity: 0.6;
  }

  .next {
    font-size: 14px;
  }

  .next.replaces::before {
    content: '→ ';
    color: var(--accent);
    font-weight: 600;
  }

  .next.long {
    font-size: 13px;
    line-height: 1.55;
    white-space: pre-line;
    max-height: 9.3em;
    overflow-y: auto;
  }

  .check {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    cursor: pointer;
  }

  .check.disabled {
    opacity: 0.5;
    cursor: default;
  }

  .nothing {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px;
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--text);
    font-size: 13px;
  }

  .nothing :global(svg) {
    flex-shrink: 0;
    color: var(--accent);
  }

  .hint {
    font-size: 12.5px;
    color: var(--muted);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 22px;
    border-top: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg) 50%, var(--surface));
  }

  .selected {
    margin-right: auto;
    font-size: 12.5px;
    color: var(--muted);
  }

  footer .error {
    margin: 0;
    font-size: 12.5px;
  }

  @media (max-width: 720px) {
    .compare {
      grid-template-columns: 1fr;
    }
  }
</style>
