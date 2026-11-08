<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { api, errorMessage } from '../lib/api'
  import { formatCount, formatDuration } from '../lib/format'
  import { scan } from '../lib/scan.svelte'

  interface Props {
    roots: string[]
    onrootschange: (roots: string[]) => void
    onclose: () => void
  }

  let { roots, onrootschange, onclose }: Props = $props()
  let folderError = $state<string | null>(null)

  async function addFolder() {
    folderError = null
    const picked = await open({ directory: true, multiple: false, title: 'Choose a folder with books' })
    if (typeof picked !== 'string') return
    try {
      onrootschange(await api.addRoot(picked))
      scan.start()
    } catch (e) {
      folderError = errorMessage(e)
    }
  }

  async function removeFolder(path: string) {
    folderError = null
    try {
      onrootschange(await api.removeRoot(path))
    } catch (e) {
      folderError = errorMessage(e)
    }
  }
</script>

<div class="backdrop" onclick={onclose} role="presentation"></div>
<div class="panel" role="dialog" aria-modal="true" aria-labelledby="scan-title">
  <header>
    <h2 id="scan-title">Library folders</h2>
    <button class="ghost" onclick={onclose} aria-label="Close">✕</button>
  </header>

  {#if roots.length === 0}
    <p class="muted">No folders yet. Add the folder where you keep your books; subfolders are included.</p>
  {:else}
    <ul class="roots">
      {#each roots as root (root)}
        <li>
          <span class="path" title={root}>{root}</span>
          <button class="ghost" disabled={scan.running} onclick={() => removeFolder(root)}>Remove</button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if folderError}<p class="error">{folderError}</p>{/if}

  <div class="buttons">
    <button onclick={addFolder} disabled={scan.running}>Add folder…</button>
    <span class="spacer"></span>
    {#if scan.running}
      <button onclick={() => scan.cancel()}>Cancel scan</button>
    {:else}
      <button
        disabled={roots.length === 0}
        onclick={() => scan.start(true)}
        title="Read every file again, even unchanged ones">Full rescan</button
      >
      <button class="primary" disabled={roots.length === 0} onclick={() => scan.start()}>Scan now</button>
    {/if}
  </div>

  {#if scan.phase !== 'idle'}
    <section class="status" aria-live="polite">
      {#if scan.phase === 'walking'}
        <div class="bar indeterminate"><div></div></div>
        <p>Looking for books… {formatCount(scan.found)} found</p>
      {:else if scan.phase === 'processing'}
        <div class="bar"><div style:width="{(scan.fraction ?? 0) * 100}%"></div></div>
        <p>Reading {formatCount(scan.done)} of {formatCount(scan.total)} new or changed books…</p>
      {:else if scan.phase === 'cancelled'}
        <p>Scan cancelled. Books read so far were kept.</p>
      {:else if scan.phase === 'failed'}
        <p class="error">The scan failed: {scan.error}</p>
      {:else if scan.phase === 'finished' && scan.report}
        {@const r = scan.report}
        <p>
          <strong>Done in {formatDuration(r.elapsed_ms)}.</strong>
          {formatCount(r.found)} books found: {formatCount(r.added)} new, {formatCount(r.updated)} updated,
          {formatCount(r.unchanged)} unchanged{#if r.missing}, {formatCount(r.missing)} missing{/if}.
        </p>
        {#if r.errors}
          <details>
            <summary class="error">{formatCount(r.errors)} files could not be read completely</summary>
            <p class="muted small">They are still in the library, with the details taken from the file name.</p>
            <ul class="errors">
              {#each r.error_samples as [path, message] (path)}
                <li><span class="path">{path}</span><span class="muted">{message}</span></li>
              {/each}
            </ul>
          </details>
        {/if}
      {/if}
    </section>
  {/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(43 33 24 / 0.3);
    z-index: 10;
  }

  .panel {
    position: fixed;
    top: 64px;
    left: 50%;
    transform: translateX(-50%);
    width: min(640px, calc(100vw - 32px));
    max-height: calc(100vh - 96px);
    overflow-y: auto;
    padding: 20px 24px;
    background: var(--surface);
    border-radius: 12px;
    box-shadow: 0 12px 40px rgb(43 33 24 / 0.25);
    z-index: 11;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h2 {
    font-size: 18px;
    margin: 0;
  }

  .roots {
    list-style: none;
    padding: 0;
    margin: 16px 0;
  }

  .roots li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    border-bottom: 1px solid var(--border);
  }

  .path {
    flex: 1;
    font-family: ui-monospace, monospace;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .buttons {
    display: flex;
    gap: 8px;
    margin-top: 16px;
  }

  .spacer {
    flex: 1;
  }

  .status {
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }

  .bar {
    height: 8px;
    border-radius: 4px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 150ms linear;
  }

  .bar.indeterminate div {
    width: 30%;
    animation: slide 1.1s ease-in-out infinite;
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }

  .small {
    font-size: 12px;
  }

  .errors {
    list-style: none;
    padding: 0;
    max-height: 220px;
    overflow-y: auto;
    font-size: 12px;
  }

  .errors li {
    display: flex;
    flex-direction: column;
    padding: 4px 0;
    border-bottom: 1px solid var(--border);
  }
</style>
