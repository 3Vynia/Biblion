<script lang="ts">
  // Sheet with every keyboard shortcut. Opens with ? or F1, anywhere.
  import Icon from './Icon.svelte'
  import ShortcutList from './ShortcutList.svelte'
  import { shortcutsHelp } from '../lib/shortcuts.svelte'
  import { ms } from '../lib/motion'
  import { fade, scale } from 'svelte/transition'

  let sheet = $state<HTMLDivElement>()
  $effect(() => {
    if (shortcutsHelp.open) sheet?.focus()
  })
</script>

{#if shortcutsHelp.open}
  <div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && (shortcutsHelp.open = false)} transition:fade={{ duration: ms(150) }}>
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
      tabindex="-1"
      bind:this={sheet}
      transition:scale={{ start: 0.96, duration: ms(180) }}
    >
      <header>
        <h2>Keyboard shortcuts</h2>
        <button class="ghost close" onclick={() => (shortcutsHelp.open = false)} aria-label="Close"><Icon name="close" size={16} /></button>
      </header>
      <div class="body"><ShortcutList /></div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.45);
    backdrop-filter: blur(4px);
  }

  .sheet {
    display: flex;
    flex-direction: column;
    width: min(880px, 100%);
    max-height: min(720px, 100%);
    border: 1px solid var(--border);
    border-radius: 18px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.35);
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px 8px;
  }

  h2 {
    margin: 0;
    font-family: var(--serif);
    font-size: 20px;
  }

  .close {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
  }

  .body {
    overflow-y: auto;
    padding: 8px 20px 16px;
  }
</style>
