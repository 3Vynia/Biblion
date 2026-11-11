<script lang="ts">
  // A book cover: the cached thumbnail, or a generated placeholder with the
  // title when there is none (or it fails to load). Fills its container.
  import { thumbUrl } from '../lib/api'
  import { placeholderHue } from '../lib/format'

  interface Props {
    title: string
    author?: string
    coverKey: string | null
    lazy?: boolean
  }

  let { title, author, coverKey, lazy = true }: Props = $props()

  // Remember which key loaded or failed, so a new cover starts fresh without
  // resetting state in an effect (which could run after the load event).
  let loadedKey = $state<string | null>(null)
  let brokenKey = $state<string | null>(null)
  let img = $state<HTMLImageElement>()

  const loaded = $derived(!!coverKey && loadedKey === coverKey)
  const broken = $derived(!!coverKey && brokenKey === coverKey)

  // Images served from the cache can be complete before handlers run.
  $effect(() => {
    if (img?.complete && img.naturalWidth > 0 && coverKey) loadedKey = coverKey
  })
</script>

<div class="cover" style:--hue={placeholderHue(title)}>
  {#if coverKey && !broken}
    <img
      src={thumbUrl(coverKey)}
      alt=""
      loading={lazy ? 'lazy' : 'eager'}
      decoding="async"
      draggable="false"
      class:loaded
      bind:this={img}
      onload={() => (loadedKey = coverKey)}
      onerror={() => (brokenKey = coverKey)}
    />
  {:else}
    <div class="placeholder">
      <span class="title">{title}</span>
      {#if author}<span class="author">{author}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .cover {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: linear-gradient(160deg, hsl(var(--hue) 25% 82%), hsl(var(--hue) 25% 72%));
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    opacity: 0;
    transform: scale(1.02);
    transition:
      opacity 260ms ease,
      transform 400ms var(--ease);
  }

  img.loaded {
    opacity: 1;
    transform: none;
  }

  .placeholder {
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 12% 10%;
    background:
      radial-gradient(120% 80% at 20% 0%, hsl(var(--hue) 45% 48% / 0.9), transparent 60%),
      linear-gradient(160deg, hsl(var(--hue) 35% 36%), hsl(var(--hue) 40% 20%));
    color: hsl(var(--hue) 30% 92%);
    font-family: var(--serif);
    box-shadow: inset 6px 0 10px -6px rgb(0 0 0 / 0.5);
  }

  .title {
    font-size: clamp(11px, 1.1em, 17px);
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 6;
    line-clamp: 6;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .author {
    font-size: 0.8em;
    opacity: 0.85;
  }
</style>
