<script lang="ts">
  import CollectionTile from '../components/CollectionTile.svelte'
  import Icon from '../components/Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import { favorites, finished, nav, readingNow, shelfRoute } from '../lib/nav.svelte'
  import { deleteCollection, renameCollection } from '../lib/collections'

  let creating = $state(false)
  let newName = $state('')
  let error = $state<string | null>(null)

  async function create(e: Event) {
    e.preventDefault()
    error = null
    const name = newName.trim()
    if (!name) return
    try {
      const id = await api.createShelf(name)
      newName = ''
      creating = false
      await library.refresh()
      nav.go(shelfRoute(id, name))
    } catch (err) {
      error = errorMessage(err)
    }
  }

  const TINTS = ['#c2410c', '#0f766e', '#7c3aed', '#be185d', '#1d4ed8', '#4d7c0f', '#b45309']
</script>

<div class="screen">
  <header in:rise={{ y: 6 }}>
    <div>
      <span class="subtitle">Browse</span>
      <h1>Collections</h1>
    </div>
  </header>

  <h2>My reading</h2>
  <div class="grid">
    <div in:rise={{ delay: 40 }}>
      <CollectionTile
        name="Reading now"
        icon="reading"
        count={library.counts.reading}
        query={{ reading_only: true }}
        tint="#d97706"
        onclick={() => nav.go(readingNow())}
      />
    </div>
    {#if library.toRead}
      {@const toRead = library.toRead}
      <div in:rise={{ delay: 80 }}>
        <CollectionTile
          name="To read"
          icon="bookmark"
          count={toRead.book_count}
          query={{ shelf_id: toRead.id }}
          tint="#2563eb"
          onclick={() => nav.go(shelfRoute(toRead.id, 'To read', 'to-read'))}
        />
      </div>
    {/if}
    <div in:rise={{ delay: 120 }}>
      <CollectionTile
        name="Favourites"
        icon="heart"
        count={library.counts.favorites}
        query={{ favorites_only: true }}
        tint="#e11d48"
        onclick={() => nav.go(favorites())}
      />
    </div>
    <div in:rise={{ delay: 160 }}>
      <CollectionTile
        name="Finished"
        icon="check"
        count={library.counts.finished}
        query={{ finished_only: true }}
        tint="#059669"
        onclick={() => nav.go(finished())}
      />
    </div>
  </div>

  <h2>My collections</h2>
  <div class="grid">
    {#each library.custom as shelf, i (shelf.id)}
      <div in:rise={{ delay: 40 + i * 40 }}>
        <CollectionTile
          name={shelf.name}
          icon="collection"
          count={shelf.book_count}
          query={{ shelf_id: shelf.id, sort: 'added_desc' }}
          tint={TINTS[shelf.id % TINTS.length]}
          onclick={() => nav.go(shelfRoute(shelf.id, shelf.name))}
          onrename={() => renameCollection(shelf)}
          ondelete={() => deleteCollection(shelf)}
        />
      </div>
    {/each}

    {#if creating}
      <form class="new editing" onsubmit={create} in:rise={{ y: 6, duration: 180 }}>
        <!-- svelte-ignore a11y_autofocus -->
        <input placeholder="Collection name" bind:value={newName} maxlength="80" autofocus />
        {#if error}<p class="error">{error}</p>{/if}
        <div class="actions">
          <button type="button" class="ghost" onclick={() => ((creating = false), (error = null))}>Cancel</button>
          <button type="submit" class="primary" disabled={!newName.trim()}>Create</button>
        </div>
      </form>
    {:else}
      <button class="new" onclick={() => (creating = true)}>
        <span class="plus"><Icon name="plus" size={22} /></span>
        New collection
      </button>
    {/if}
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
    margin: 0;
    font-family: var(--serif);
    font-size: 26px;
    font-weight: 600;
  }

  h2 {
    margin: 26px 0 12px;
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 16px;
  }

  .new {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 222px;
    border: 1.5px dashed var(--border);
    border-radius: 16px;
    background: transparent;
    color: var(--muted);
    font-weight: 600;
  }

  .new:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }

  .plus {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--surface-2);
    transition: transform 300ms var(--ease);
  }

  .new:hover .plus {
    transform: rotate(90deg);
  }

  .new.editing {
    justify-content: center;
    align-items: stretch;
    padding: 18px;
    border-style: solid;
    border-color: var(--accent);
    background: var(--surface);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }

  .error {
    margin: 0;
    font-size: 12px;
    font-weight: 400;
  }
</style>
