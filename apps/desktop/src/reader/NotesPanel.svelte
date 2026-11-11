<script lang="ts">
  // The open book's highlights and bookmarks, in reading order.
  import Icon from '../components/Icon.svelte'
  import { highlightValue } from '../lib/highlights'
  import { rise } from '../lib/motion'
  import type { Annotation } from '../lib/types'

  interface Props {
    annotations: Annotation[]
    ongo: (a: Annotation) => void
    ondelete: (a: Annotation) => void
    oncopyall: () => Promise<void>
  }

  let { annotations, ongo, ondelete, oncopyall }: Props = $props()

  let tab = $state<'highlight' | 'bookmark'>('highlight')
  let copied = $state(false)

  const highlights = $derived(annotations.filter((a) => a.kind === 'highlight'))
  const bookmarks = $derived(annotations.filter((a) => a.kind === 'bookmark'))
  const shown = $derived(tab === 'highlight' ? highlights : bookmarks)

  async function copyAll() {
    await oncopyall()
    copied = true
    setTimeout(() => (copied = false), 1400)
  }
</script>

<div class="tabs" role="tablist">
  <button role="tab" aria-selected={tab === 'highlight'} class:on={tab === 'highlight'} onclick={() => (tab = 'highlight')}>
    Highlights <span>{highlights.length}</span>
  </button>
  <button role="tab" aria-selected={tab === 'bookmark'} class:on={tab === 'bookmark'} onclick={() => (tab = 'bookmark')}>
    Bookmarks <span>{bookmarks.length}</span>
  </button>
</div>

{#if shown.length === 0}
  <div class="empty">
    <Icon name={tab === 'highlight' ? 'quote' : 'bookmark'} size={28} strokeWidth={1.4} />
    <p>
      {tab === 'highlight'
        ? 'Select text in the book to highlight it or add a note.'
        : 'Use the bookmark button at the top to mark a page.'}
    </p>
  </div>
{:else}
  <ul>
    {#each shown as a, i (a.id)}
      <li in:rise={{ delay: Math.min(i, 8) * 30, y: 6, duration: 200 }}>
        <button class="entry" style:--c={highlightValue(a.color)} onclick={() => ongo(a)}>
          <span class="meta">{a.chapter ?? 'Untitled'} · {Math.round(a.percent * 100)}%</span>
          {#if a.kind === 'highlight'}
            <span class="text">{a.text}</span>
            {#if a.note}<span class="note"><Icon name="note" size={12} /> {a.note}</span>{/if}
          {:else}
            <span class="text plain">{a.text || 'Bookmarked page'}</span>
          {/if}
        </button>
        <button class="remove" onclick={() => ondelete(a)} title="Delete" aria-label="Delete"><Icon name="trash" size={14} /></button>
      </li>
    {/each}
  </ul>
  {#if tab === 'highlight'}
    <button class="copy-all" onclick={copyAll}>
      <Icon name={copied ? 'check' : 'copy'} size={14} />
      {copied ? 'Copied' : 'Copy all as Markdown'}
    </button>
  {/if}
{/if}

<style>
  .tabs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
    padding: 3px;
    margin-bottom: 12px;
    border-radius: 10px;
    background: var(--r-hover);
  }

  .tabs button {
    padding: 6px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--r-muted);
    font-size: 13px;
  }

  .tabs button.on {
    background: var(--r-bg);
    color: var(--r-fg);
    font-weight: 600;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.12);
  }

  .tabs span {
    margin-left: 4px;
    font-size: 11px;
    color: var(--r-muted);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    position: relative;
  }

  .entry {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    padding: 10px 34px 10px 12px;
    margin-bottom: 6px;
    border: none;
    border-left: 3px solid var(--c);
    border-radius: 4px 10px 10px 4px;
    background: var(--r-bg);
    color: var(--r-fg);
    text-align: left;
  }

  .entry:hover:not(:disabled) {
    background: var(--r-hover);
  }

  .meta {
    font-size: 11px;
    color: var(--r-muted);
  }

  .text {
    font-family: var(--serif);
    font-size: 14px;
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .text.plain {
    font-family: inherit;
    font-size: 13px;
    color: var(--r-muted);
  }

  .note {
    display: flex;
    gap: 6px;
    font-size: 12.5px;
    color: var(--r-muted);
    white-space: pre-wrap;
  }

  .remove {
    position: absolute;
    top: 8px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--r-muted);
    opacity: 0;
    transition: opacity 150ms ease;
  }

  li:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }

  .remove:hover:not(:disabled) {
    color: var(--danger);
    background: var(--r-hover);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 30px 10px;
    text-align: center;
    color: var(--r-muted);
  }

  .copy-all {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    justify-content: center;
    margin-top: 8px;
    background: var(--r-bg);
    color: var(--r-fg);
    border-color: var(--r-border);
  }
</style>
