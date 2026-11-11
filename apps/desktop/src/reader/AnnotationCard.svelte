<script lang="ts">
  // Popover for one highlight: colour, note, copy and delete. Changing an
  // existing note or deleting asks for confirmation first.
  import { onMount } from 'svelte'
  import Icon from '../components/Icon.svelte'
  import { confirm } from '../lib/dialog.svelte'
  import { HIGHLIGHT_COLORS, highlightValue, type HighlightColor } from '../lib/highlights'
  import { rise } from '../lib/motion'
  import type { Annotation } from '../lib/types'

  interface Props {
    annotation: Annotation
    x: number
    y: number
    /** Open with the note editor ready (right after "Note" in the selection menu). */
    editing?: boolean
    onrecolor: (color: HighlightColor) => void
    onsavenote: (note: string) => Promise<void> | void
    ondelete: () => void
    onclose: () => void
  }

  let { annotation, x, y, editing: startEditing = false, onrecolor, onsavenote, ondelete, onclose }: Props = $props()

  let editing = $state(false)
  let draft = $state('')
  let textarea = $state<HTMLTextAreaElement>()
  let copied = $state(false)

  onMount(() => {
    if (startEditing) edit()
  })

  function edit() {
    draft = annotation.note ?? ''
    editing = true
    queueMicrotask(() => textarea?.focus())
  }

  async function save() {
    const next = draft.trim()
    const previous = annotation.note ?? ''
    if (next === previous) {
      editing = false
      return
    }
    if (previous) {
      const ok = await confirm({
        title: next ? 'Save changes to this note?' : 'Remove this note?',
        message: next ? 'The previous text of the note will be replaced.' : 'The highlight stays; only the note is removed.',
        confirmLabel: next ? 'Save note' : 'Remove note',
        danger: !next,
      })
      if (!ok) return
    }
    await onsavenote(next)
    editing = false
  }

  async function copy() {
    await navigator.clipboard?.writeText(annotation.text).catch(() => {})
    copied = true
    setTimeout(() => (copied = false), 1200)
  }

  const below = $derived(y < 260)
</script>

<div
  class="card"
  class:below
  style:left="{Math.max(180, Math.min(window.innerWidth - 180, x))}px"
  style:top="{below ? y + 30 : y - 12}px"
  style:--c={highlightValue(annotation.color)}
  role="dialog"
  aria-label="Highlight"
  in:rise={{ y: below ? -8 : 8, scale: 0.96, duration: 200 }}
>
  <blockquote>{annotation.text}</blockquote>

  {#if editing}
    <textarea
      bind:this={textarea}
      bind:value={draft}
      rows="4"
      placeholder="Write a note…"
      onkeydown={(e) => {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) save()
        if (e.key === 'Escape') {
          e.stopPropagation()
          editing = false
        }
      }}
    ></textarea>
    <div class="row end">
      <button class="ghost" onclick={() => (editing = false)}>Cancel</button>
      <button class="primary-r" onclick={save}>Save</button>
    </div>
  {:else if annotation.note}
    <button class="note" onclick={edit} title="Edit note"><Icon name="note" size={14} /> {annotation.note}</button>
  {/if}

  {#if !editing}
    <div class="row">
      {#each Object.entries(HIGHLIGHT_COLORS) as [id, c] (id)}
        <button
          class="swatch"
          class:on={(annotation.color ?? 'yellow') === id}
          style:--s={c.value}
          aria-label="{c.label} highlight"
          title={c.label}
          onclick={() => onrecolor(id as HighlightColor)}
        ></button>
      {/each}
      <span class="spacer"></span>
      {#if !annotation.note}
        <button class="icon" onclick={edit} title="Add a note" aria-label="Add a note"><Icon name="note" size={16} /></button>
      {/if}
      <button class="icon" onclick={copy} title="Copy text" aria-label="Copy text"><Icon name={copied ? 'check' : 'copy'} size={15} /></button>
      <button class="icon danger" onclick={ondelete} title="Delete highlight" aria-label="Delete highlight"
        ><Icon name="trash" size={15} /></button
      >
      <button class="icon" onclick={onclose} title="Close" aria-label="Close"><Icon name="close" size={15} /></button>
    </div>
  {/if}
</div>

<style>
  .card {
    position: fixed;
    z-index: 30;
    translate: -50% -100%;
    width: 340px;
    padding: 14px;
    border-radius: 14px;
    background: var(--r-chrome);
    border: 1px solid var(--r-border);
    box-shadow: 0 14px 40px rgb(0 0 0 / 0.28);
    color: var(--r-fg);
  }

  .card.below {
    translate: -50% 0;
  }

  blockquote {
    margin: 0 0 10px;
    padding: 2px 0 2px 12px;
    border-left: 3px solid var(--c);
    font-family: var(--serif);
    font-size: 14px;
    line-height: 1.5;
    max-height: 7.5em;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
  }

  .note {
    display: flex;
    gap: 8px;
    width: 100%;
    margin-bottom: 10px;
    padding: 8px 10px;
    border-radius: 9px;
    border: 1px dashed var(--r-border);
    background: transparent;
    color: var(--r-fg);
    text-align: left;
    white-space: pre-wrap;
    line-height: 1.45;
  }

  .note:hover:not(:disabled) {
    background: var(--r-hover);
  }

  textarea {
    width: 100%;
    resize: vertical;
    padding: 8px 10px;
    border-radius: 9px;
    border: 1px solid var(--r-border);
    background: var(--r-bg);
    color: var(--r-fg);
    font: inherit;
    line-height: 1.45;
  }

  textarea:focus {
    outline: none;
    border-color: var(--c);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 30%, transparent);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .row.end {
    justify-content: flex-end;
    margin-top: 8px;
  }

  .spacer {
    flex: 1;
  }

  .swatch {
    width: 20px;
    height: 20px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid transparent;
    background: var(--s);
    transition: transform 160ms var(--ease);
  }

  .swatch:hover:not(:disabled) {
    background: var(--s);
    transform: scale(1.15);
  }

  .swatch.on {
    box-shadow:
      0 0 0 2px var(--r-chrome),
      0 0 0 4px var(--s);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--r-muted);
  }

  .icon:hover:not(:disabled) {
    background: var(--r-hover);
    color: var(--r-fg);
  }

  .icon.danger:hover:not(:disabled) {
    color: var(--danger);
  }

  .ghost {
    background: transparent;
    border-color: var(--r-border);
    color: var(--r-fg);
  }

  .primary-r {
    background: var(--c);
    border-color: var(--c);
    color: #1a1a1a;
    font-weight: 600;
  }
</style>
