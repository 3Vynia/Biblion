<script lang="ts">
  import Icon from './Icon.svelte'
  import { dialog } from '../lib/dialog.svelte'
  import { rise } from '../lib/motion'

  let input = $state<HTMLInputElement>()
  let confirmButton = $state<HTMLButtonElement>()
  let value = $state('')

  $effect(() => {
    const d = dialog.current
    if (!d) return
    if (d.kind === 'prompt') value = d.value
    // Focus the field, or the action; for deletions the safe choice stays put.
    queueMicrotask(() => {
      if (d.kind === 'prompt') {
        input?.focus()
        input?.select()
      } else if (!d.danger) confirmButton?.focus()
    })
  })

  function onKey(e: KeyboardEvent) {
    if (dialog.current && e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      dialog.cancel()
    }
  }

  function submit(e: Event) {
    e.preventDefault()
    if (dialog.current?.kind === 'prompt' && !value.trim()) return
    dialog.accept(value)
  }
</script>

<svelte:window onkeydowncapture={onKey} />

{#if dialog.current}
  {@const d = dialog.current}
  {@const danger = d.kind === 'confirm' && d.danger}
  <div class="backdrop" role="presentation" onclick={() => dialog.cancel()}></div>
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="dialog-title" in:rise={{ y: 12, scale: 0.97, duration: 220 }}>
    <form onsubmit={submit}>
      <div class="icon" class:danger><Icon name={danger ? 'trash' : 'edit'} size={20} /></div>
      <h2 id="dialog-title">{d.title}</h2>
      {#if d.kind === 'confirm'}
        <p>{d.message}</p>
      {:else}
        <label>
          <span>{d.label}</span>
          <input bind:this={input} bind:value maxlength={d.maxLength ?? 80} spellcheck="false" />
        </label>
      {/if}
      <div class="actions">
        <button type="button" onclick={() => dialog.cancel()}>Cancel</button>
        <button
          type="submit"
          class={danger ? 'danger' : 'primary'}
          bind:this={confirmButton}
          disabled={d.kind === 'prompt' && (!value.trim() || value.trim() === d.value.trim())}>{d.confirmLabel}</button
        >
      </div>
    </form>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.35);
    backdrop-filter: blur(2px);
    animation: fade 180ms ease both;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .dialog {
    position: fixed;
    z-index: 41;
    top: 50%;
    left: 50%;
    translate: -50% -50%;
    width: min(420px, calc(100vw - 32px));
    padding: 24px;
    border-radius: 16px;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: 0 24px 60px rgb(0 0 0 / 0.35);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border-radius: 12px;
    color: var(--accent);
    background: var(--accent-soft);
  }

  .icon.danger {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }

  h2 {
    margin: 14px 0 6px;
    font-size: 17px;
  }

  p {
    margin: 0;
    color: var(--muted);
    line-height: 1.5;
    white-space: pre-line;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 10px;
    font-size: 12px;
    color: var(--muted);
  }

  input {
    padding: 9px 12px;
    border-radius: 9px;
    font-size: 14px;
    color: var(--text);
  }

  input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 22px;
  }

  .actions button {
    padding: 8px 16px;
    border-radius: 9px;
    font-weight: 600;
  }

  .danger {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  .danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 85%, black);
  }
</style>
