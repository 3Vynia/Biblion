<script lang="ts" generics="T">
  // Virtualised list with fixed-height rows, backed by a PageCache, so lists
  // of tens of thousands of authors or series stay light.
  import type { Snippet } from 'svelte'
  import type { PageCache } from '../lib/pageCache'

  interface Props {
    cache: PageCache<T>
    version: number
    rowHeight?: number
    row: Snippet<[T, number]>
    placeholder?: Snippet<[number]>
  }

  let { cache, version, rowHeight = 60, row, placeholder }: Props = $props()

  const OVERSCAN = 6
  const PADDING = 8

  let viewport: HTMLDivElement
  let height = $state(0)
  let scrollTop = $state(0)

  const total = $derived.by(() => (void version, cache.total ?? 0))
  const first = $derived(Math.max(0, Math.floor((scrollTop - PADDING) / rowHeight) - OVERSCAN))
  const last = $derived(Math.min(total - 1, Math.ceil((scrollTop + height) / rowHeight) + OVERSCAN))

  $effect(() => {
    void version
    if (cache.total === null) cache.ensure(0, Math.ceil(height / rowHeight) + OVERSCAN)
    else if (last >= first) cache.ensure(first, last + 1)
  })

  const rows = $derived.by(() => {
    void version
    const out: { index: number; item: T | undefined }[] = []
    for (let i = first; i <= last; i++) out.push({ index: i, item: cache.get(i) })
    return out
  })

  export function scrollToTop() {
    if (viewport) viewport.scrollTop = 0
    scrollTop = 0
  }
</script>

<div class="viewport" bind:this={viewport} bind:clientHeight={height} onscroll={() => (scrollTop = viewport.scrollTop)}>
  <div class="spacer" style:height="{total * rowHeight + PADDING * 2}px">
    {#each rows as r (r.index)}
      <div class="row" style:height="{rowHeight}px" style:transform="translateY({PADDING + r.index * rowHeight}px)">
        {#if r.item !== undefined}
          {@render row(r.item, r.index)}
        {:else if placeholder}
          {@render placeholder(r.index)}
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .viewport {
    height: 100%;
    overflow-y: auto;
    contain: strict;
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
  }
</style>
