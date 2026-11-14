<script lang="ts">
  // A list of names typed one by one: Enter or comma adds, Backspace on an
  // empty field takes the last one back for editing.
  import Icon from './Icon.svelte'

  interface Props {
    values: string[]
    label: string
    placeholder?: string
    /** Commas are part of names like "Tolkien, J.R.R.": only Enter adds. */
    commaAdds?: boolean
    onchange: (values: string[]) => void
  }

  let { values, label, placeholder = '', commaAdds = true, onchange }: Props = $props()
  let text = $state('')
  let input: HTMLInputElement

  function add() {
    const v = text.trim().replace(/,$/, '').trim()
    text = ''
    if (!v || values.some((x) => x.toLowerCase() === v.toLowerCase())) return
    onchange([...values, v])
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' || (commaAdds && e.key === ',')) {
      e.preventDefault()
      add()
    } else if (e.key === 'Backspace' && !text && values.length) {
      e.preventDefault()
      text = values.at(-1)!
      onchange(values.slice(0, -1))
    }
  }
</script>

<!-- A click anywhere in the box focuses its field; keyboard users reach the field directly. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events -->
<div class="chips" role="group" aria-label={label} onclick={() => input.focus()}>
  {#each values as v, i (v)}
    <span class="chip">
      {v}
      <button type="button" aria-label="Remove {v}" onclick={() => onchange(values.filter((_, j) => j !== i))}><Icon name="close" size={11} /></button>
    </span>
  {/each}
  <input bind:this={input} bind:value={text} onkeydown={onKey} onblur={add} aria-label={label} placeholder={values.length ? '' : placeholder} spellcheck="false" />
</div>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    min-height: 38px;
    padding: 5px 8px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--bg);
    cursor: text;
    transition:
      border-color 160ms ease,
      box-shadow 160ms ease;
  }

  .chips:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 4px 2px 10px;
    border-radius: 12px;
    font-size: 12.5px;
    color: var(--accent);
    background: var(--accent-soft);
    animation: pop 220ms var(--ease) both;
  }

  @keyframes pop {
    from {
      transform: scale(0.85);
      opacity: 0;
    }
  }

  .chip button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: inherit;
  }

  .chip button:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
  }

  input {
    flex: 1;
    min-width: 120px;
    padding: 3px 2px;
    border: none;
    background: none;
    outline: none;
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
  }
</style>
