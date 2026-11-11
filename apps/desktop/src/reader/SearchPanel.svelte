<script lang="ts">
  // Full-text search inside the open book (foliate-js `view.search`).
  import { onDestroy } from 'svelte'

  interface Excerpt {
    pre: string
    match: string
    post: string
  }
  interface Hit {
    cfi: string
    excerpt: Excerpt
  }
  interface Group {
    label: string
    subitems: Hit[]
  }

  interface Props {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    view: any
    ongo: (cfi: string) => void
  }

  let { view, ongo }: Props = $props()

  const MAX_RESULTS = 500

  let query = $state('')
  let groups = $state<Group[]>([])
  let count = $state(0)
  let progress = $state(0)
  let searching = $state(false)
  let truncated = $state(false)
  /** Query of the last finished or running search. */
  let searched = $state('')
  let run = 0

  async function search(e?: Event) {
    e?.preventDefault()
    const q = query.trim()
    const id = ++run
    groups = []
    count = 0
    truncated = false
    view.clearSearch()
    searched = q
    if (q.length < 2) return
    searching = true
    progress = 0
    try {
      for await (const result of view.search({ query: q, matchCase: false, matchDiacritics: false, matchWholeWords: false })) {
        if (id !== run) return
        if (result === 'done') break
        if (result.progress !== undefined) progress = result.progress
        if (result.subitems) {
          groups = [...groups, { label: result.label || 'Section', subitems: result.subitems }]
          count += result.subitems.length
          if (count >= MAX_RESULTS) {
            truncated = true
            break
          }
        }
      }
    } catch (err) {
      console.error('search failed', err)
    } finally {
      if (id === run) searching = false
    }
  }

  onDestroy(() => {
    run++
    view?.clearSearch?.()
  })
</script>

<form onsubmit={search}>
  <!-- svelte-ignore a11y_autofocus -->
  <input type="search" placeholder="Search in this book" bind:value={query} autofocus />
  <button type="submit" disabled={query.trim().length < 2}>Search</button>
</form>

{#if searching}
  <div class="bar"><div style:width="{progress * 100}%"></div></div>
{/if}

{#if count > 0 || (!searching && searched.length >= 2)}
  <p class="summary">
    {count === 0 ? 'No matches.' : `${count}${truncated ? '+' : ''} matches`}
  </p>
{/if}

{#each groups as group, gi (gi)}
  <section>
    <h4>{group.label}</h4>
    {#each group.subitems as hit (hit.cfi)}
      <button class="hit" onclick={() => ongo(hit.cfi)}>
        …{hit.excerpt.pre}<mark>{hit.excerpt.match}</mark>{hit.excerpt.post}…
      </button>
    {/each}
  </section>
{/each}

<style>
  form {
    display: flex;
    gap: 6px;
    margin-bottom: 10px;
  }

  input {
    flex: 1;
    min-width: 0;
    background: var(--r-bg);
    color: var(--r-fg);
    border-color: var(--r-border);
  }

  form button {
    background: var(--r-bg);
    color: var(--r-fg);
    border-color: var(--r-border);
  }

  .bar {
    height: 3px;
    background: var(--r-border);
    margin-bottom: 8px;
  }

  .bar div {
    height: 100%;
    background: var(--r-link);
  }

  .summary {
    color: var(--r-muted);
    font-size: 12px;
    margin: 4px 0 8px;
  }

  h4 {
    font-size: 12px;
    color: var(--r-muted);
    margin: 14px 0 4px;
    font-weight: 600;
  }

  .hit {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    border-radius: 6px;
    background: none;
    padding: 6px 8px;
    line-height: 1.4;
    color: inherit;
    font-size: 13px;
  }

  .hit:hover {
    background: var(--r-hover);
  }

  mark {
    background: color-mix(in srgb, var(--r-link) 35%, transparent);
    color: inherit;
    border-radius: 2px;
  }
</style>
