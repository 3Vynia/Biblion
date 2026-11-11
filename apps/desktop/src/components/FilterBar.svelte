<script lang="ts">
  // Extra filters on top of a books screen: status, format, language and
  // subject. Filters fixed by the screen itself (e.g. an author) are hidden.
  import { onMount } from 'svelte'
  import Icon from './Icon.svelte'
  import { api } from '../lib/api'
  import { FORMAT_LABEL, formatCount, languageName } from '../lib/format'
  import { rise } from '../lib/motion'
  import type { BookFilter } from '../lib/nav.svelte'
  import type { Format, TagEntry } from '../lib/types'

  interface Props {
    /** Filters the screen already applies. */
    base: BookFilter
    value: BookFilter
    onchange: (value: BookFilter) => void
  }

  let { base, value, onchange }: Props = $props()

  type Status = 'any' | 'reading' | 'finished' | 'favorites'
  const FORMATS: Format[] = ['epub', 'pdf', 'mobi', 'azw3', 'cbz', 'cbr']

  let open = $state(false)
  let root: HTMLDivElement
  let languages = $state<[string, number][]>([])
  let subjectQuery = $state('')
  let subjects = $state<TagEntry[]>([])
  let subjectsTotal = $state(0)

  const status = $derived<Status>(
    value.reading_only ? 'reading' : value.finished_only ? 'finished' : value.favorites_only ? 'favorites' : 'any',
  )
  const fixedStatus = $derived(!!(base.reading_only || base.finished_only || base.favorites_only))
  const activeCount = $derived(
    [status !== 'any', !!value.format, !!value.language, !!value.tag].filter(Boolean).length,
  )

  onMount(() => {
    api.listLanguages().then((l) => (languages = l)).catch(() => {})
    const onDown = (e: PointerEvent) => {
      if (open && !root.contains(e.target as Node)) open = false
    }
    window.addEventListener('pointerdown', onDown)
    return () => window.removeEventListener('pointerdown', onDown)
  })

  // Subject suggestions, debounced.
  $effect(() => {
    if (!open || base.tag) return
    const q = subjectQuery.trim()
    const t = setTimeout(() => {
      api
        .listTags(q || null, 0, 40)
        .then((p) => {
          subjects = p.items
          subjectsTotal = p.total
        })
        .catch(() => {})
    }, 120)
    return () => clearTimeout(t)
  })

  // Esc closes only this panel, not the book panel or the search behind it.
  function onKey(e: KeyboardEvent) {
    if (open && e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      open = false
    }
  }

  function set(patch: BookFilter) {
    onchange({ ...value, ...patch })
  }

  function setStatus(s: Status) {
    set({ reading_only: s === 'reading', finished_only: s === 'finished', favorites_only: s === 'favorites' })
  }

  function clearAll() {
    onchange({})
  }

  const STATUS_LABEL: Record<Exclude<Status, 'any'>, string> = {
    reading: 'Reading',
    finished: 'Finished',
    favorites: 'Favourites',
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="filters" bind:this={root}>
  {#if status !== 'any'}
    <button class="chip" onclick={() => setStatus('any')} in:rise={{ y: 4, duration: 180 }}>
      {STATUS_LABEL[status]} <Icon name="close" size={12} />
    </button>
  {/if}
  {#if value.format}
    <button class="chip" onclick={() => set({ format: null })} in:rise={{ y: 4, duration: 180 }}>
      {FORMAT_LABEL[value.format]} <Icon name="close" size={12} />
    </button>
  {/if}
  {#if value.language}
    <button class="chip" onclick={() => set({ language: null })} in:rise={{ y: 4, duration: 180 }}>
      {languageName(value.language)} <Icon name="close" size={12} />
    </button>
  {/if}
  {#if value.tag}
    <button class="chip" onclick={() => set({ tag: null })} in:rise={{ y: 4, duration: 180 }}>
      <Icon name="tag" size={12} />
      {value.tag} <Icon name="close" size={12} />
    </button>
  {/if}

  <button class="toggle" class:active={open || activeCount > 0} onclick={() => (open = !open)} aria-expanded={open}>
    <Icon name="filter" size={15} />
    Filters
    {#if activeCount}<span class="badge">{activeCount}</span>{/if}
  </button>

  {#if open}
    <div class="panel" role="dialog" aria-label="Filters" in:rise={{ y: -6, duration: 200 }}>
      {#if !fixedStatus}
        <h4>Status</h4>
        <div class="segmented">
          {#each ['any', 'reading', 'finished', 'favorites'] as const as s (s)}
            <button class:on={status === s} onclick={() => setStatus(s)}>{s === 'any' ? 'Any' : STATUS_LABEL[s]}</button>
          {/each}
        </div>
      {/if}

      {#if !base.format}
        <h4>Format</h4>
        <div class="wrap">
          {#each FORMATS as f (f)}
            <button class="pill" class:on={value.format === f} onclick={() => set({ format: value.format === f ? null : f })}>
              {FORMAT_LABEL[f]}
            </button>
          {/each}
        </div>
      {/if}

      {#if !base.language && languages.length}
        <h4>Language</h4>
        <div class="wrap">
          {#each languages.slice(0, 12) as [code, n] (code)}
            <button
              class="pill"
              class:on={value.language === code}
              onclick={() => set({ language: value.language === code ? null : code })}
            >
              {languageName(code)} <span class="n">{formatCount(n)}</span>
            </button>
          {/each}
        </div>
      {/if}

      {#if !base.tag}
        <h4>Subject</h4>
        <div class="search">
          <Icon name="search" size={14} />
          <input placeholder="Science fiction, history, poetry…" bind:value={subjectQuery} spellcheck="false" />
        </div>
        <div class="subjects">
          {#each subjects as t (t.id)}
            <button class="subject" class:on={value.tag === t.name} onclick={() => set({ tag: value.tag === t.name ? null : t.name })}>
              <span>{t.name}</span><span class="n">{formatCount(t.book_count)}</span>
            </button>
          {:else}
            <p class="empty">No subjects{subjectQuery.trim() ? ` starting with “${subjectQuery.trim()}”` : ' in this library yet'}.</p>
          {/each}
          {#if subjectsTotal > subjects.length}
            <p class="empty">{formatCount(subjectsTotal - subjects.length)} more — keep typing to narrow down.</p>
          {/if}
        </div>
      {/if}

      {#if activeCount}
        <button class="clear" onclick={clearAll}>Clear all filters</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .filters {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 8px 4px 11px;
    border-radius: 14px;
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
    background: var(--accent-soft);
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 9px;
  }

  .toggle.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .badge {
    display: grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    font-size: 11px;
    font-weight: 700;
    color: var(--accent-text);
    background: var(--accent);
  }

  .panel {
    position: absolute;
    z-index: 15;
    top: calc(100% + 8px);
    right: 0;
    width: 360px;
    max-height: min(560px, 70vh);
    overflow-y: auto;
    padding: 14px 16px 16px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: var(--shadow-lift);
  }

  h4 {
    margin: 12px 0 8px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  h4:first-child {
    margin-top: 0;
  }

  .segmented {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
  }

  .segmented button,
  .pill {
    padding: 5px 8px;
    border-radius: 8px;
    font-size: 12.5px;
  }

  .wrap {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .n {
    margin-left: 4px;
    font-size: 11px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 9px;
    color: var(--muted);
  }

  .search input {
    flex: 1;
    border: none;
    background: transparent;
    padding: 7px 0;
    outline: none;
    color: var(--text);
  }

  .subjects {
    display: flex;
    flex-direction: column;
    margin-top: 6px;
    max-height: 220px;
    overflow-y: auto;
  }

  .subject {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 8px;
    border: none;
    border-radius: 7px;
    background: transparent;
    text-align: left;
  }

  .subject span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    margin: 6px 8px;
    font-size: 12px;
    color: var(--muted);
  }

  .clear {
    width: 100%;
    margin-top: 14px;
  }
</style>
