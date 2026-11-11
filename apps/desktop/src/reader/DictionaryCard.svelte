<script lang="ts">
  // Definition of a selected word: offline dictionaries first, Wiktionary as
  // a fallback. Everything is shown as plain text.
  import { onMount } from 'svelte'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import Icon from '../components/Icon.svelte'
  import { lookup, markupToText, type LookupResult } from '../lib/dictionary'
  import { rise } from '../lib/motion'

  interface Props {
    word: string
    x: number
    y: number
    online: boolean
    bookLanguage: string | null
    onclose: () => void
  }

  let { word, x, y, online, bookLanguage, onclose }: Props = $props()

  let result = $state<LookupResult | null>(null)
  let tab = $state(0)

  onMount(() => {
    let alive = true
    lookup(word, { online, bookLanguage }).then((r) => alive && (result = r))
    return () => (alive = false)
  })

  // Offline results grouped by dictionary, one tab each.
  const groups = $derived.by(() => {
    const map = new Map<string, { word: string; text: string }[]>()
    for (const d of result?.offline ?? []) {
      const text = d.markup ? markupToText(d.body) : d.body
      map.set(d.dictionary, [...(map.get(d.dictionary) ?? []), { word: d.word, text }])
    }
    return [...map].map(([name, entries]) => ({ name, entries }))
  })

  const below = $derived(y < 340)
</script>

<div
  class="card"
  class:below
  style:left="{Math.max(200, Math.min(window.innerWidth - 200, x))}px"
  style:top="{below ? y + 34 : y - 12}px"
  role="dialog"
  aria-label="Definition of {word}"
  in:rise={{ y: below ? -8 : 8, scale: 0.96, duration: 200 }}
>
  <header>
    <h3>{word}</h3>
    <button class="icon" onclick={onclose} aria-label="Close"><Icon name="close" size={15} /></button>
  </header>

  {#if !result}
    <div class="loading"><span class="spinner"></span> Looking up…</div>
  {:else if result.source === 'offline'}
    {#if groups.length > 1}
      <div class="tabs" role="tablist">
        {#each groups as g, i (g.name)}
          <button role="tab" aria-selected={tab === i} class:on={tab === i} onclick={() => (tab = i)}>{g.name}</button>
        {/each}
      </div>
    {/if}
    {@const group = groups[Math.min(tab, groups.length - 1)]}
    <div class="body">
      {#each group.entries as e, i (i)}
        {#if e.word.toLowerCase() !== word.toLowerCase()}<p class="headword">{e.word}</p>{/if}
        <p class="definition">{e.text}</p>
      {/each}
    </div>
    <footer>{group.name}</footer>
  {:else if result.source === 'wiktionary'}
    <div class="body">
      {#each result.online as lang (lang.language)}
        <p class="language">{lang.language}</p>
        {#each lang.senses as s, si (si)}
          <p class="pos">{s.partOfSpeech}</p>
          <ol>
            {#each s.definitions as d, di (di)}<li>{d}</li>{/each}
          </ol>
        {/each}
      {/each}
    </div>
    <footer>
      From <button class="link" onclick={() => openUrl(`https://en.wiktionary.org/wiki/${encodeURIComponent(word)}`)}>Wiktionary</button>,
      CC BY-SA
    </footer>
  {:else}
    <div class="empty">
      <p>No definition found for “{word}”.</p>
      {#if result.error}
        <p class="hint">The online dictionary could not be reached ({result.error}).</p>
      {/if}
      <p class="hint">
        Add offline dictionaries in Settings → Reading{online ? '' : ', or turn on online lookups there'}.
      </p>
    </div>
  {/if}
</div>

<style>
  .card {
    position: fixed;
    z-index: 30;
    translate: -50% -100%;
    width: 380px;
    max-height: 320px;
    display: flex;
    flex-direction: column;
    border-radius: 14px;
    background: var(--r-chrome);
    border: 1px solid var(--r-border);
    box-shadow: 0 14px 40px rgb(0 0 0 / 0.28);
    color: var(--r-fg);
    overflow: hidden;
  }

  .card.below {
    translate: -50% 0;
  }

  header {
    display: flex;
    align-items: center;
    padding: 12px 10px 4px 16px;
  }

  h3 {
    flex: 1;
    margin: 0;
    font-family: var(--serif);
    font-size: 19px;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--r-muted);
  }

  .icon:hover:not(:disabled) {
    background: var(--r-hover);
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding: 4px 12px;
    overflow-x: auto;
  }

  .tabs button {
    flex-shrink: 0;
    padding: 3px 10px;
    border-radius: 12px;
    border: 1px solid var(--r-border);
    background: transparent;
    color: var(--r-muted);
    font-size: 12px;
  }

  .tabs button.on {
    color: var(--r-fg);
    border-color: var(--r-link);
  }

  .body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 16px 10px;
    font-size: 14px;
    line-height: 1.5;
  }

  .definition {
    margin: 4px 0 10px;
    white-space: pre-line;
  }

  .headword,
  .language {
    margin: 8px 0 0;
    font-weight: 700;
  }

  .language {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--r-link);
  }

  .pos {
    margin: 6px 0 2px;
    font-style: italic;
    color: var(--r-muted);
  }

  ol {
    margin: 0 0 6px;
    padding-left: 20px;
  }

  li {
    margin-bottom: 3px;
  }

  footer {
    padding: 8px 16px;
    border-top: 1px solid var(--r-border);
    font-size: 11.5px;
    color: var(--r-muted);
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--r-link);
    text-decoration: underline;
    font-size: inherit;
  }

  .loading,
  .empty {
    padding: 10px 16px 16px;
    color: var(--r-muted);
  }

  .loading {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .empty p {
    margin: 0 0 6px;
  }

  .hint {
    font-size: 12.5px;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--r-border);
    border-top-color: var(--r-link);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
