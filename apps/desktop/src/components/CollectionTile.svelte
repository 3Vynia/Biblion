<script lang="ts">
  // A collection card: three covers fanned out, a name and a count.
  import BookCover from './BookCover.svelte'
  import Icon, { type IconName } from './Icon.svelte'
  import { api } from '../lib/api'
  import { formatCount } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import type { BookQuery, BookSummary } from '../lib/types'

  interface Props {
    name: string
    count: number
    icon: IconName
    query: Partial<BookQuery>
    tint?: string
    onclick: () => void
    /** Only for collections the user made. */
    onrename?: () => void
    ondelete?: () => void
  }

  let { name, count, icon, query, tint = 'var(--accent)', onclick, onrename, ondelete }: Props = $props()
  let covers = $state<BookSummary[]>([])

  $effect(() => {
    void library.revision
    const q = query
    api
      .listBooks({ sort: 'recently_read', ...q, offset: 0, limit: 3 })
      .then((p) => (covers = p.items))
      .catch(() => (covers = []))
  })
</script>

<div class="holder">
  <button class="tile" style:--tint={tint} {onclick}>
    <div class="fan" aria-hidden="true">
      {#each covers as book, i (book.id)}
        <div class="sheet" style:--i={i - (covers.length - 1) / 2}>
          <BookCover title={book.title} author={book.authors[0]} coverKey={book.cover_key} />
        </div>
      {:else}
        <div class="sheet empty"><Icon name={icon} size={26} strokeWidth={1.5} /></div>
      {/each}
    </div>
    <div class="meta">
      <span class="icon"><Icon name={icon} size={15} /></span>
      <span class="name">{name}</span>
      <span class="count">{formatCount(count)}</span>
    </div>
  </button>
  {#if onrename || ondelete}
    <div class="actions">
      {#if onrename}
        <button class="action" onclick={onrename} title="Rename {name}" aria-label="Rename {name}"><Icon name="edit" size={14} /></button>
      {/if}
      {#if ondelete}
        <button class="action danger" onclick={ondelete} title="Delete {name}" aria-label="Delete {name}"
          ><Icon name="trash" size={14} /></button
        >
      {/if}
    </div>
  {/if}
</div>

<style>
  .holder {
    position: relative;
  }

  .actions {
    position: absolute;
    top: 10px;
    right: 10px;
    z-index: 3;
    display: flex;
    gap: 4px;
    opacity: 0;
    transform: translateY(-4px);
    transition:
      opacity 180ms ease,
      transform 220ms var(--ease);
  }

  .holder:hover .actions,
  .actions:focus-within {
    opacity: 1;
    transform: none;
  }

  .action {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border-radius: 9px;
    background: var(--surface);
    box-shadow: var(--shadow);
    color: var(--muted);
  }

  .action:hover:not(:disabled) {
    color: var(--accent);
  }

  .action.danger:hover:not(:disabled) {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, var(--surface));
  }

  .tile {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 16px 14px;
    border-radius: 16px;
    border: 1px solid var(--border);
    background:
      radial-gradient(120% 90% at 50% 0%, color-mix(in srgb, var(--tint) 16%, transparent), transparent 70%),
      var(--surface);
    text-align: left;
    transition:
      transform 260ms var(--ease),
      box-shadow 260ms var(--ease);
  }

  .tile:hover:not(:disabled) {
    background:
      radial-gradient(120% 90% at 50% 0%, color-mix(in srgb, var(--tint) 22%, transparent), transparent 70%),
      var(--surface);
    transform: translateY(-4px);
    box-shadow: var(--shadow-lift);
  }

  .fan {
    position: relative;
    height: 150px;
    display: flex;
    justify-content: center;
    align-items: flex-end;
  }

  .sheet {
    position: absolute;
    bottom: 0;
    width: 86px;
    height: 129px;
    border-radius: 3px 6px 6px 3px;
    overflow: hidden;
    box-shadow: 0 6px 16px rgb(0 0 0 / 0.25);
    transform: translateX(calc(var(--i) * 34px)) rotate(calc(var(--i) * 7deg));
    transform-origin: 50% 120%;
    transition: transform 420ms var(--ease);
  }

  .sheet:nth-child(2) {
    z-index: 1;
  }

  .tile:hover .sheet {
    transform: translateX(calc(var(--i) * 46px)) rotate(calc(var(--i) * 12deg)) translateY(-4px);
  }

  .sheet.empty {
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--tint) 14%, var(--surface-2));
    color: var(--tint);
    box-shadow: none;
    border: 1.5px dashed color-mix(in srgb, var(--tint) 45%, transparent);
    transform: none;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    color: var(--tint);
    background: color-mix(in srgb, var(--tint) 15%, transparent);
  }

  .name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .count {
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
</style>
