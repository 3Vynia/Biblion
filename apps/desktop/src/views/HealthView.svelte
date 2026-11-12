<script lang="ts">
  // Library health: duplicate books, files that could not be read, and files
  // that are gone. Every action that removes something asks first.
  import { onMount } from 'svelte'
  import { Tween } from 'svelte/motion'
  import { cubicOut } from 'svelte/easing'
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import BookCover from '../components/BookCover.svelte'
  import Icon, { type IconName } from '../components/Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { confirm } from '../lib/dialog.svelte'
  import { FORMAT_LABEL, formatBytes, formatCount, formatDate, plural } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { ms, rise } from '../lib/motion'
  import type { DuplicateGroup, DuplicateKind, HealthBook, HealthCounts, ProblemKind } from '../lib/types'

  type Tab = 'duplicates' | ProblemKind
  const PAGE = 100

  let tab = $state<Tab>('duplicates')
  let dupKind = $state<DuplicateKind>('identical')
  let groups = $state<DuplicateGroup[] | null>(null)
  let counts = $state<HealthCounts | null>(null)
  let problems = $state<HealthBook[]>([])
  let problemTotal = $state(0)
  let loading = $state(false)
  let busy = $state<string | null>(null)
  let error = $state<string | null>(null)
  // Identical-copy groups, counted once for the tile even while "same book" is shown.
  let identicalCount = $state<number | null>(null)
  let wastedBytes = $state(0)

  const dupTween = new Tween(0, { duration: ms(700), easing: cubicOut })
  const unreadTween = new Tween(0, { duration: ms(700), easing: cubicOut })
  const missingTween = new Tween(0, { duration: ms(700), easing: cubicOut })
  $effect(() => {
    dupTween.target = identicalCount ?? 0
    unreadTween.target = counts?.unreadable ?? 0
    missingTween.target = counts?.missing ?? 0
  })

  const tiles = $derived<{ id: Tab; label: string; icon: IconName; value: number; detail: string; tint: string }[]>([
    {
      id: 'duplicates',
      label: 'Duplicates',
      icon: 'copy',
      value: dupTween.current,
      detail: identicalCount === null ? 'Comparing files…' : wastedBytes ? `${formatBytes(wastedBytes)} could be freed` : 'No identical copies',
      tint: '#7c3aed',
    },
    {
      id: 'unreadable',
      label: 'Unreadable',
      icon: 'info',
      value: unreadTween.current,
      detail: 'Damaged or still downloading',
      tint: '#c2410c',
    },
    {
      id: 'missing',
      label: 'Missing',
      icon: 'search',
      value: missingTween.current,
      detail: 'No longer on disk',
      tint: '#0f766e',
    },
  ])

  async function loadCounts() {
    counts = await api.healthCounts()
  }

  async function loadDuplicates() {
    loading = true
    error = null
    try {
      const kind = dupKind
      const result = await api.duplicates(kind)
      if (kind !== dupKind) return
      groups = result
      if (kind === 'identical') {
        identicalCount = result.length
        wastedBytes = result.reduce((n, g) => n + g.wasted, 0)
      }
    } catch (e) {
      error = errorMessage(e)
    } finally {
      loading = false
    }
  }

  async function loadProblems(more = false) {
    if (tab === 'duplicates') return
    loading = true
    error = null
    try {
      const page = await api.problemBooks(tab, more ? problems.length : 0, PAGE)
      problems = more ? [...problems, ...page.items] : page.items
      problemTotal = page.total
    } catch (e) {
      error = errorMessage(e)
    } finally {
      loading = false
    }
  }

  async function refresh() {
    await loadCounts()
    if (tab === 'duplicates') {
      await loadDuplicates()
      if (dupKind !== 'identical') {
        const ident = await api.duplicates('identical')
        identicalCount = ident.length
        wastedBytes = ident.reduce((n, g) => n + g.wasted, 0)
      }
    } else await loadProblems()
  }

  function show(next: Tab) {
    if (tab === next) return
    tab = next
    problems = []
    if (next === 'duplicates') loadDuplicates()
    else loadProblems()
  }

  function showKind(kind: DuplicateKind) {
    if (dupKind === kind) return
    dupKind = kind
    groups = null
    loadDuplicates()
  }

  onMount(() => {
    loadCounts().catch((e) => (error = errorMessage(e)))
    loadDuplicates()
  })

  /** Runs an action that changes the library, then reloads everything. */
  async function act(label: string, fn: () => Promise<void>) {
    busy = label
    try {
      await fn()
      library.changed()
      await refresh()
    } catch (e) {
      await confirm({ title: 'That did not work', message: errorMessage(e), confirmLabel: 'OK' })
    } finally {
      busy = null
    }
  }

  async function reportTrash(ids: number[]) {
    const report = await api.trashBooks(ids)
    if (report.failed.length) {
      await confirm({
        title: `${formatCount(report.failed.length)} ${plural(report.failed.length, 'file')} could not be moved`,
        message: report.failed.slice(0, 5).map(([p, why]) => `${p}: ${why}`).join('\n'),
        confirmLabel: 'OK',
      })
    }
  }

  async function trashOne(b: HealthBook) {
    const ok = await confirm({
      title: `Move “${b.title}” to the trash?`,
      message: `${b.path}\n\nThe file goes to the system trash, so you can still restore it. Biblion forgets its progress and notes.`,
      confirmLabel: 'Move to trash',
      danger: true,
    })
    if (ok) await act(`trash-${b.id}`, () => reportTrash([b.id]))
  }

  async function keepFirst(g: DuplicateGroup) {
    const [keep, ...rest] = g.books
    const ok = await confirm({
      title: `Keep one copy of “${keep.title}”?`,
      message: `${formatCount(rest.length)} other ${plural(rest.length, 'copy', 'copies')} (${formatBytes(g.wasted)}) go to the system trash, where you can still restore them.\n\nKept: ${keep.path}`,
      confirmLabel: `Trash ${formatCount(rest.length)} ${plural(rest.length, 'copy', 'copies')}`,
      danger: true,
    })
    if (ok) await act(`group-${keep.id}`, () => reportTrash(rest.map((b) => b.id)))
  }

  async function rereadOne(b: HealthBook) {
    await act(`reread-${b.id}`, async () => {
      await api.rereadBooks([b.id])
    })
  }

  async function rereadAll() {
    await act('reread-all', async () => {
      const report = await api.rereadBooks()
      const fixed = report.found - report.errors
      await confirm({
        title: fixed > 0 ? `${formatCount(fixed)} ${plural(fixed, 'book')} can be read now` : 'Still unreadable',
        message:
          report.errors > 0
            ? `${formatCount(report.errors)} ${plural(report.errors, 'file')} still could not be read. If they are downloading, try again when the download finishes.`
            : 'Every file was read fully.',
        confirmLabel: 'OK',
      })
    })
  }

  async function forgetOne(b: HealthBook) {
    const ok = await confirm({
      title: `Forget “${b.title}”?`,
      message: 'Its file is gone. Biblion removes the book with its progress, notes and collections.',
      confirmLabel: 'Forget book',
      danger: true,
    })
    if (ok) await act(`forget-${b.id}`, async () => void (await api.forgetBooks([b.id])))
  }

  async function forgetAll() {
    const n = counts?.missing ?? 0
    const ok = await confirm({
      title: `Forget ${formatCount(n)} missing ${plural(n, 'book')}?`,
      message: 'Their files are gone. Biblion removes them with their progress, notes and collections. If a drive is just disconnected, plug it in instead: the books come back on their own.',
      confirmLabel: 'Forget all',
      danger: true,
    })
    if (ok) await act('forget-all', async () => void (await api.forgetMissing()))
  }

  function reveal(path: string) {
    revealItemInDir(path).catch((e) => (error = errorMessage(e)))
  }

  const folder = (path: string) => path.slice(0, path.lastIndexOf('/') + 1)
  const file = (path: string) => path.slice(path.lastIndexOf('/') + 1)
  const tabTitle = $derived(tab === 'duplicates' ? 'Duplicates' : tab === 'unreadable' ? 'Unreadable files' : 'Missing files')
</script>

<div class="screen">
  <header in:rise={{ y: 6 }}>
    <span class="subtitle">Maintenance</span>
    <h1>Library health</h1>
  </header>

  <div class="tiles" role="tablist" aria-label="Problems">
    {#each tiles as t, i (t.id)}
      <button
        class="tile"
        class:on={tab === t.id}
        style:--t={t.tint}
        role="tab"
        aria-selected={tab === t.id}
        onclick={() => show(t.id)}
        in:rise={{ delay: 40 + i * 60 }}
      >
        <span class="glow" aria-hidden="true"></span>
        <span class="tile-icon"><Icon name={t.icon} size={18} /></span>
        <span class="tile-label">{t.label}</span>
        <span class="tile-value">{formatCount(Math.round(t.value))}</span>
        <span class="tile-detail">{t.detail}</span>
      </button>
    {/each}
  </div>

  <section class="panel" aria-label={tabTitle}>
    {#if error}<p class="error">{error}</p>{/if}

    {#if tab === 'duplicates'}
      <div class="bar">
        <div class="segmented" role="radiogroup" aria-label="Kind of duplicate">
          <button role="radio" aria-checked={dupKind === 'identical'} class:on={dupKind === 'identical'} onclick={() => showKind('identical')}
            >Identical copies</button
          >
          <button role="radio" aria-checked={dupKind === 'same_book'} class:on={dupKind === 'same_book'} onclick={() => showKind('same_book')}
            >Same book</button
          >
        </div>
        <p class="hint">
          {dupKind === 'identical'
            ? 'The same file more than once, byte for byte.'
            : 'Same title and author in different files: another edition, or another format.'}
        </p>
      </div>

      {#if groups === null}
        <div class="loading" role="status"><span class="spinner"></span>Comparing files…</div>
      {:else if groups.length === 0}
        <div class="empty" in:rise>
          <span class="empty-icon"><Icon name="check" size={26} /></span>
          <p>{dupKind === 'identical' ? 'No identical copies.' : 'No book appears twice.'}</p>
        </div>
      {:else}
        <p class="summary">
          {formatCount(groups.length)}
          {plural(groups.length, 'group')}{#if dupKind === 'identical'}&nbsp;· {formatBytes(groups.reduce((n, g) => n + g.wasted, 0))} could be freed{/if}
        </p>
        <ul class="groups">
          {#each groups as g, gi (g.books.map((b) => b.id).join('-'))}
            <li class="group" in:rise={{ delay: Math.min(gi, 10) * 30 }}>
              <div class="group-head">
                <div>
                  <h3>{g.books[0].title}</h3>
                  <p class="by">{g.books[0].authors.join(', ') || 'Unknown author'}</p>
                </div>
                <span class="pill">{g.books.length} {dupKind === 'identical' ? 'copies' : 'files'}</span>
                {#if g.kind === 'identical'}
                  <button class="primary small" disabled={!!busy} onclick={() => keepFirst(g)}>
                    <Icon name="sparkle" size={13} /> Keep one
                  </button>
                {/if}
              </div>
              {#each g.books as b, bi (b.id)}
                <div class="row" class:keep={bi === 0}>
                  <div class="cover"><BookCover title={b.title} coverKey={b.cover_key} bare /></div>
                  <div class="info">
                    <p class="path" title={b.path}><span class="dir">{folder(b.path)}</span>{file(b.path)}</p>
                    <p class="meta">
                      <span class="fmt">{FORMAT_LABEL[b.format]}</span>
                      {formatBytes(b.size)} · added {formatDate(b.added_at)}
                      {#if b.progress}· {Math.round(b.progress * 100)}% read{/if}
                      {#if b.annotations}· {b.annotations} {plural(b.annotations, 'note')}{/if}
                      {#if b.favorite}· <Icon name="heart" size={11} filled />{/if}
                    </p>
                  </div>
                  {#if bi === 0}<span class="keep-badge" title="Read the most, or has notes">Suggested to keep</span>{/if}
                  <div class="actions">
                    <button class="ghost icon" onclick={() => reveal(b.path)} aria-label="Show {file(b.path)} in folder" title="Show in folder"
                      ><Icon name="folder" size={15} /></button
                    >
                    <button class="ghost icon danger" disabled={!!busy} onclick={() => trashOne(b)} aria-label="Move {file(b.path)} to trash" title="Move to trash"
                      ><Icon name="trash" size={15} /></button
                    >
                  </div>
                </div>
              {/each}
            </li>
          {/each}
        </ul>
      {/if}
    {:else}
      <div class="bar">
        <p class="hint">
          {tab === 'unreadable'
            ? 'Usually a file that is still downloading, or damaged. Biblion reads it again on its own when it changes; you can also do it now.'
            : 'These files are no longer where they were. If they come back (a drive plugged in again, say), they are matched with their progress and notes.'}
        </p>
        <span class="spacer"></span>
        {#if tab === 'unreadable' && problemTotal > 0}
          <button class="primary" disabled={!!busy} onclick={rereadAll}>
            {#if busy === 'reread-all'}<span class="spinner small"></span>{:else}<Icon name="sparkle" size={14} />{/if}
            Read all again
          </button>
        {:else if tab === 'missing' && problemTotal > 0}
          <button class="danger-btn" disabled={!!busy} onclick={forgetAll}><Icon name="trash" size={14} /> Forget all</button>
        {/if}
      </div>

      {#if loading && problems.length === 0}
        <div class="loading" role="status"><span class="spinner"></span>Loading…</div>
      {:else if problems.length === 0}
        <div class="empty" in:rise>
          <span class="empty-icon"><Icon name="check" size={26} /></span>
          <p>{tab === 'unreadable' ? 'Every file could be read.' : 'No file is missing.'}</p>
        </div>
      {:else}
        <p class="summary">{formatCount(problemTotal)} {plural(problemTotal, 'book')}</p>
        <ul class="list">
          {#each problems as b, i (b.id)}
            <li class="row" in:rise={{ delay: Math.min(i, 12) * 20 }}>
              <span class="fmt-badge">{FORMAT_LABEL[b.format]}</span>
              <div class="info">
                <p class="title">{b.title}<span class="by-inline">{b.authors.length ? ` · ${b.authors.join(', ')}` : ''}</span></p>
                <p class="path" title={b.path}><span class="dir">{folder(b.path)}</span>{file(b.path)}</p>
                {#if b.error}<p class="why">{b.error}</p>{/if}
              </div>
              <div class="actions">
                {#if tab === 'unreadable'}
                  <button class="ghost icon" onclick={() => reveal(b.path)} aria-label="Show {file(b.path)} in folder" title="Show in folder"
                    ><Icon name="folder" size={15} /></button
                  >
                  <button class="ghost small" disabled={!!busy} onclick={() => rereadOne(b)}>
                    {#if busy === `reread-${b.id}`}<span class="spinner small"></span>{/if}Read again
                  </button>
                  <button class="ghost icon danger" disabled={!!busy} onclick={() => trashOne(b)} aria-label="Move {file(b.path)} to trash" title="Move to trash"
                    ><Icon name="trash" size={15} /></button
                  >
                {:else}
                  <button class="ghost small danger" disabled={!!busy} onclick={() => forgetOne(b)}>Forget</button>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
        {#if problems.length < problemTotal}
          <button class="more" disabled={loading} onclick={() => loadProblems(true)}>
            Show more ({formatCount(problemTotal - problems.length)} left)
          </button>
        {/if}
      {/if}
    {/if}
  </section>
</div>

<style>
  .screen {
    height: 100%;
    overflow-y: auto;
    padding: 22px 28px 48px;
  }

  .subtitle {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--accent);
  }

  h1 {
    margin: 0 0 20px;
    font-family: var(--serif);
    font-size: 26px;
    font-weight: 600;
  }

  /* ── Tiles ── */
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(200px, 100%), 1fr));
    gap: 14px;
    margin-bottom: 22px;
  }

  .tile {
    position: relative;
    overflow: hidden;
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-areas: 'icon label' 'value value' 'detail detail';
    align-items: center;
    gap: 4px 10px;
    padding: 16px 18px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition:
      transform 0.2s ease,
      border-color 0.2s ease,
      box-shadow 0.2s ease;
  }

  .tile:hover {
    transform: translateY(-2px);
    box-shadow: 0 10px 28px rgb(0 0 0 / 0.12);
  }

  .tile.on {
    border-color: color-mix(in srgb, var(--t) 60%, var(--border));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--t) 18%, transparent);
  }

  .glow {
    position: absolute;
    inset: -40% -20% auto auto;
    width: 160px;
    height: 160px;
    border-radius: 50%;
    background: radial-gradient(circle, color-mix(in srgb, var(--t) 35%, transparent), transparent 70%);
    opacity: 0.5;
    transition: opacity 0.3s ease;
    pointer-events: none;
  }

  .tile.on .glow {
    opacity: 1;
  }

  .tile-icon {
    grid-area: icon;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--t) 16%, transparent);
    color: var(--t);
  }

  .tile-label {
    grid-area: label;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }

  .tile-value {
    grid-area: value;
    margin-top: 6px;
    font-family: var(--serif);
    font-size: 34px;
    font-weight: 600;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }

  .tile-detail {
    grid-area: detail;
    font-size: 12px;
    color: var(--muted);
  }

  /* ── Panel ── */
  .bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px 16px;
    margin-bottom: 14px;
  }

  .spacer {
    flex: 1;
  }

  .hint {
    margin: 0;
    max-width: 640px;
    font-size: 13px;
    color: var(--muted);
  }

  .segmented {
    display: inline-flex;
    max-width: 100%;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
  }

  .segmented button {
    padding: 6px 14px;
    border: none;
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
    font-size: 13px;
  }

  .segmented button.on {
    background: var(--accent);
    color: var(--accent-text);
  }

  .summary {
    margin: 0 0 10px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .groups,
  .list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .list {
    gap: 0;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
    overflow: hidden;
  }

  .list .row + .row {
    border-top: 1px solid var(--border);
  }

  .group {
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
    overflow: hidden;
  }

  .group-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }

  .group-head > div {
    flex: 1;
    min-width: 0;
  }

  h3 {
    margin: 0;
    font-family: var(--serif);
    font-size: 16px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .by {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--muted);
  }

  .pill {
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    transition: background 0.15s ease;
  }

  .row:hover {
    background: var(--surface-2);
  }

  .row.keep {
    background: linear-gradient(90deg, var(--accent-soft), transparent 60%);
  }

  .cover {
    flex: none;
    width: 32px;
    height: 46px;
    border-radius: 4px;
    overflow: hidden;
  }

  .info {
    flex: 1;
    min-width: 0;
  }

  .info p {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: 14px;
    font-weight: 600;
  }

  .by-inline {
    font-weight: 400;
    color: var(--muted);
  }

  .path {
    font-size: 13px;
  }

  .dir {
    color: var(--muted);
  }

  .meta {
    margin-top: 3px !important;
    font-size: 12px;
    color: var(--muted);
  }

  .meta :global(svg) {
    display: inline-block;
    vertical-align: -1px;
    color: #e11d48;
  }

  .why {
    margin-top: 3px !important;
    font-family: ui-monospace, monospace;
    font-size: 11px;
    color: #c2410c;
  }

  .fmt,
  .fmt-badge {
    display: inline-block;
    padding: 1px 6px;
    margin-right: 4px;
    border-radius: 5px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--text);
  }

  .fmt-badge {
    flex: none;
    width: 44px;
    text-align: center;
    margin: 0;
  }

  .keep-badge {
    flex: none;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 11px;
    font-weight: 600;
  }

  .actions {
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  button.icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
  }

  button.small {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    font-size: 12px;
  }

  button.primary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .danger,
  .danger-btn {
    color: #dc2626;
  }

  .danger-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-color: color-mix(in srgb, #dc2626 40%, var(--border));
  }

  .more {
    display: block;
    margin: 14px auto 0;
  }

  .loading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 40px 0;
    justify-content: center;
    color: var(--muted);
    font-size: 14px;
  }

  .spinner {
    width: 18px;
    height: 18px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .spinner.small {
    width: 12px;
    height: 12px;
  }

  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 48px 0;
    color: var(--muted);
  }

  .empty-icon {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: color-mix(in srgb, #16a34a 14%, transparent);
    color: #16a34a;
    animation: pop 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }

  @keyframes pop {
    from {
      scale: 0.4;
      opacity: 0;
    }
  }

  .empty p {
    margin: 0;
  }

  .error {
    color: #dc2626;
  }

  @media (max-width: 720px) {
    .screen {
      padding: 16px 16px 40px;
    }

    .keep-badge {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .tile,
    .spinner,
    .empty-icon {
      transition: none;
      animation: none;
    }
  }
</style>
