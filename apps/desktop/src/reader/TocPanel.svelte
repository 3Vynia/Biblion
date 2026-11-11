<script lang="ts">
  export interface TocItem {
    label?: string
    href?: string
    subitems?: TocItem[]
  }

  interface Props {
    toc: TocItem[]
    currentHref: string | null
    ongo: (href: string) => void
  }

  let { toc, currentHref, ongo }: Props = $props()

  // Bring the current chapter into view, scrolling only the panel (not the window).
  function scrollIntoViewIfCurrent(el: HTMLElement, current: boolean) {
    if (!current) return
    const panel = el.closest<HTMLElement>('.panel')
    if (panel) panel.scrollTop = el.offsetTop - panel.clientHeight / 2
  }
</script>

{#snippet items(list: TocItem[], depth: number)}
  <ul style:--depth={depth}>
    {#each list as item, i (i)}
      <li>
        {#if item.href}
          <button
            class="entry"
            class:current={item.href === currentHref}
            onclick={() => ongo(item.href!)}
            use:scrollIntoViewIfCurrent={item.href === currentHref}
          >
            {item.label?.trim() || 'Untitled'}
          </button>
        {:else}
          <span class="entry heading">{item.label?.trim() || 'Untitled'}</span>
        {/if}
        {#if item.subitems?.length}
          {@render items(item.subitems, depth + 1)}
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

{#if toc.length}
  <nav aria-label="Table of contents">
    {@render items(toc, 0)}
  </nav>
{:else}
  <p class="empty">This book has no table of contents.</p>
{/if}

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .entry {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    border-radius: 6px;
    background: none;
    padding: 7px 10px 7px calc(10px + var(--depth) * 16px);
    line-height: 1.35;
    color: inherit;
  }

  .entry:hover {
    background: var(--r-hover);
  }

  .entry.current {
    color: var(--r-link);
    font-weight: 600;
  }

  .heading {
    color: var(--r-muted);
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .empty {
    color: var(--r-muted);
    padding: 10px;
  }
</style>
