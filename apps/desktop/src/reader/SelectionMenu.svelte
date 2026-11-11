<script lang="ts">
  // Floating menu over a text selection: highlight in a colour, add a note, copy.
  import Icon from '../components/Icon.svelte'
  import { HIGHLIGHT_COLORS, type HighlightColor } from '../lib/highlights'
  import { rise } from '../lib/motion'

  interface Props {
    /** Centre of the selection's top edge, in window coordinates. */
    x: number
    y: number
    onhighlight: (color: HighlightColor) => void
    onnote: () => void
    oncopy: () => void
    /** Only for short selections (a word or a short phrase). */
    ondefine?: () => void
  }

  let { x, y, onhighlight, onnote, oncopy, ondefine }: Props = $props()
  // Keep it on screen; flip below the selection near the top edge.
  const below = $derived(y < 70)
</script>

<div
  class="menu"
  class:below
  style:left="{Math.max(150, Math.min(window.innerWidth - 150, x))}px"
  style:top="{below ? y + 34 : y - 12}px"
  role="toolbar"
  aria-label="Selection"
  in:rise={{ y: below ? -6 : 6, scale: 0.94, duration: 180 }}
>
  {#each Object.entries(HIGHLIGHT_COLORS) as [id, c] (id)}
    <button
      class="swatch"
      style:--c={c.value}
      title="Highlight in {c.label.toLowerCase()}"
      aria-label="Highlight in {c.label.toLowerCase()}"
      onclick={() => onhighlight(id as HighlightColor)}
    ></button>
  {/each}
  <span class="sep"></span>
  {#if ondefine}
    <button class="action" onclick={ondefine} title="Look up in the dictionary"><Icon name="define" size={16} /> Define</button>
  {/if}
  <button class="action" onclick={onnote} title="Highlight and add a note"><Icon name="note" size={16} /> Note</button>
  <button class="action" onclick={oncopy} title="Copy text"><Icon name="copy" size={15} /> Copy</button>
</div>

<style>
  .menu {
    position: fixed;
    z-index: 30;
    translate: -50% -100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-radius: 14px;
    background: var(--r-chrome);
    border: 1px solid var(--r-border);
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.25);
    color: var(--r-fg);
  }

  .menu.below {
    translate: -50% 0;
  }

  .swatch {
    width: 24px;
    height: 24px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--c) 70%, black);
    background: var(--c);
    transition: transform 160ms var(--ease);
  }

  .swatch:hover:not(:disabled) {
    background: var(--c);
    transform: scale(1.18);
  }

  .sep {
    width: 1px;
    height: 22px;
    margin: 0 2px;
    background: var(--r-border);
  }

  .action {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 9px;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: var(--r-fg);
    font-size: 13px;
  }

  .action:hover:not(:disabled) {
    background: var(--r-hover);
  }
</style>
