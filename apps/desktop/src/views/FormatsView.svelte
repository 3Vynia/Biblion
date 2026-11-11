<script lang="ts">
  import CollectionTile from '../components/CollectionTile.svelte'
  import { FORMAT_LABEL } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import { formatRoute, nav } from '../lib/nav.svelte'
  import type { Format } from '../lib/types'

  const TINT: Record<Format, string> = {
    epub: '#0f766e',
    pdf: '#b91c1c',
    mobi: '#7c3aed',
    azw3: '#c2410c',
    cbz: '#1d4ed8',
    cbr: '#be185d',
  }

  const formats = $derived([...(library.stats?.by_format ?? [])].filter(([, n]) => n > 0).sort((a, b) => b[1] - a[1]))
</script>

<div class="screen">
  <header in:rise={{ y: 6 }}>
    <span class="subtitle">Browse</span>
    <h1>Formats</h1>
  </header>
  <div class="grid">
    {#each formats as [format, n], i (format)}
      <div in:rise={{ delay: 40 + i * 50 }}>
        <CollectionTile
          name={FORMAT_LABEL[format]}
          icon="file"
          count={n}
          query={{ format, sort: 'added_desc' }}
          tint={TINT[format]}
          onclick={() => nav.go(formatRoute(format, FORMAT_LABEL[format]))}
        />
      </div>
    {:else}
      <p class="muted">No books yet.</p>
    {/each}
  </div>
</div>

<style>
  .screen {
    height: 100%;
    overflow-y: auto;
    padding: 22px 28px 40px;
  }

  .subtitle {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--accent);
  }

  h1 {
    margin: 0 0 22px;
    font-family: var(--serif);
    font-size: 26px;
    font-weight: 600;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 16px;
  }
</style>
