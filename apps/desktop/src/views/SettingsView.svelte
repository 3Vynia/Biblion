<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { confirm } from '../lib/dialog.svelte'
  import type { DictionaryInfo } from '../lib/types'
  import Icon, { type IconName } from '../components/Icon.svelte'
  import Switch from '../components/Switch.svelte'
  import SettingsPanel from '../reader/SettingsPanel.svelte'
  import AmbiencePanel from '../components/AmbiencePanel.svelte'
  import { FONTS, THEMES } from '../reader/readerSettings'
  import { readerSettings } from '../reader/settingsStore.svelte'
  import { api, errorMessage } from '../lib/api'
  import { formatBytes, formatCount, formatDuration } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { rise } from '../lib/motion'
  import { scan } from '../lib/scan.svelte'
  import { ACCENTS, ui, type AccentId, type CoverSize, type ThemeMode } from '../lib/uiSettings.svelte'

  interface Props {
    roots: string[]
    onrootschange: (roots: string[]) => void
  }

  let { roots, onrootschange }: Props = $props()

  const SECTIONS: { id: string; label: string; icon: IconName }[] = [
    { id: 'appearance', label: 'Appearance', icon: 'sparkle' },
    { id: 'library', label: 'Library', icon: 'folder' },
    { id: 'reading', label: 'Reading', icon: 'reading' },
    { id: 'ambience', label: 'Ambience', icon: 'sound' },
    { id: 'about', label: 'About', icon: 'info' },
  ]

  let scroller: HTMLDivElement
  let current = $state('appearance')
  let folderError = $state<string | null>(null)
  let dataDir = $state('')
  let copied = $state(false)
  let dictionaries = $state<DictionaryInfo[]>([])
  let dictError = $state<string | null>(null)
  let installing = $state(false)

  async function addDictionary() {
    dictError = null
    const picked = await open({
      multiple: false,
      title: 'Choose a StarDict dictionary',
      filters: [{ name: 'StarDict dictionary', extensions: ['ifo', 'zip', 'gz', 'tgz', 'xz', 'txz', 'tar'] }],
    })
    if (typeof picked !== 'string') return
    installing = true
    try {
      dictionaries = await api.installDictionary(picked)
    } catch (e) {
      dictError = errorMessage(e)
    } finally {
      installing = false
    }
  }

  async function removeDictionary(d: DictionaryInfo) {
    const ok = await confirm({
      title: `Remove “${d.name}”?`,
      message: "Its files are deleted from Biblion's data folder. You can add it again later.",
      confirmLabel: 'Remove dictionary',
      danger: true,
    })
    if (!ok) return
    try {
      dictionaries = await api.removeDictionary(d.id)
    } catch (e) {
      dictError = errorMessage(e)
    }
  }

  onMount(() => {
    readerSettings.load()
    api.dataDir().then((d) => (dataDir = d)).catch(() => {})
    api.listDictionaries().then((d) => (dictionaries = d)).catch(() => {})
    // Highlight the section being read in the side index.
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((e) => e.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)
        if (visible[0]) current = visible[0].target.id
      },
      { root: scroller, rootMargin: '0px 0px -60% 0px' },
    )
    scroller.querySelectorAll('section[id]').forEach((s) => observer.observe(s))
    return () => observer.disconnect()
  })

  function goTo(id: string) {
    // Scroll only this panel: scrollIntoView would also shift the whole window.
    const section = scroller.querySelector<HTMLElement>(`#${id}`)
    if (section) scroller.scrollTo({ top: section.offsetTop - scroller.offsetTop - 12, behavior: 'smooth' })
  }

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

  async function copyDataDir() {
    await navigator.clipboard?.writeText(dataDir).catch(() => {})
    copied = true
    setTimeout(() => (copied = false), 1400)
  }

  const THEME_OPTIONS: { id: ThemeMode; label: string }[] = [
    { id: 'system', label: 'System' },
    { id: 'light', label: 'Light' },
    { id: 'dark', label: 'Dark' },
  ]
  const SIZES: { id: CoverSize; label: string; w: number }[] = [
    { id: 'small', label: 'Small', w: 16 },
    { id: 'medium', label: 'Medium', w: 21 },
    { id: 'large', label: 'Large', w: 27 },
  ]

  const r = $derived(readerSettings.value)
  const readerTheme = $derived(THEMES[r.theme])
</script>

<div class="settings">
  <nav class="index" aria-label="Settings sections" in:rise={{ y: 6 }}>
    <span class="subtitle">Preferences</span>
    <h1>Settings</h1>
    {#each SECTIONS as s (s.id)}
      <button class="ghost" class:current={current === s.id} onclick={() => goTo(s.id)}>
        <Icon name={s.icon} size={16} />
        {s.label}
      </button>
    {/each}
  </nav>

  <div class="scroller" bind:this={scroller}>
    <!-- Appearance -->
    <section id="appearance" in:rise={{ delay: 40 }}>
      <h2>Appearance</h2>

      <h3>Theme</h3>
      <div class="themes" role="radiogroup" aria-label="Theme">
        {#each THEME_OPTIONS as t (t.id)}
          <button
            class="theme-card"
            class:active={ui.value.theme === t.id}
            role="radio"
            aria-checked={ui.value.theme === t.id}
            onclick={() => ui.update({ theme: t.id })}
          >
            <span class="mock {t.id}" aria-hidden="true">
              <span class="mock-side"></span>
              <span class="mock-main">
                <span class="mock-bar"></span>
                <span class="mock-books"><i></i><i></i><i></i></span>
              </span>
            </span>
            <span class="theme-label">{t.label}</span>
          </button>
        {/each}
      </div>

      <h3>Accent colour</h3>
      <div class="accents" role="radiogroup" aria-label="Accent colour">
        {#each Object.entries(ACCENTS) as [id, a] (id)}
          <button
            class="swatch"
            class:active={ui.value.accent === id}
            style:--c={ui.dark ? a.dark : a.light}
            role="radio"
            aria-checked={ui.value.accent === id}
            title={a.label}
            aria-label={a.label}
            onclick={() => ui.update({ accent: id as AccentId })}
          >
            {#if ui.value.accent === id}<Icon name="check" size={14} strokeWidth={3} />{/if}
          </button>
        {/each}
      </div>

      <h3>Cover size</h3>
      <div class="segmented">
        {#each SIZES as s (s.id)}
          <button class:active={ui.value.coverSize === s.id} onclick={() => ui.update({ coverSize: s.id })}>
            <span class="size-preview" style:width="{s.w}px" style:height="{s.w * 1.5}px"></span>
            {s.label}
          </button>
        {/each}
      </div>

      <div class="card">
        <Switch
          label="Titles under covers"
          description="Show the title and author below each cover in the grid."
          checked={ui.value.showCaptions}
          onchange={(v) => ui.update({ showCaptions: v })}
        />
        <Switch
          label="Animations"
          description="Transitions, hover effects and counters. The system “reduce motion” setting is always respected."
          checked={ui.value.animations}
          onchange={(v) => ui.update({ animations: v })}
        />
        <Switch
          label="Compact sidebar"
          description="Show only icons in the sidebar. Toggle it any time with Ctrl+B."
          checked={ui.value.sidebarCollapsed}
          onchange={(v) => ui.update({ sidebarCollapsed: v })}
        />
      </div>
    </section>

    <!-- Library -->
    <section id="library">
      <h2>Library</h2>
      <h3>Folders</h3>
      <p class="hint">Biblion looks for EPUB, PDF, MOBI, AZW3, CBZ and CBR files in these folders and all their subfolders.</p>
      <div class="card">
        {#each roots as root (root)}
          <div class="folder" in:rise={{ y: 4, duration: 200 }}>
            <Icon name="folder" size={17} />
            <span class="path" title={root}>{root}</span>
            <button class="ghost" disabled={scan.running} onclick={() => removeFolder(root)}>Remove</button>
          </div>
        {:else}
          <p class="muted">No folders yet.</p>
        {/each}
        {#if folderError}<p class="error">{folderError}</p>{/if}
        <div class="buttons">
          <button onclick={addFolder} disabled={scan.running}><Icon name="plus" size={15} /> Add folder…</button>
          <span class="spacer"></span>
          {#if scan.running}
            <button onclick={() => scan.cancel()}>Cancel scan</button>
          {:else}
            <button disabled={roots.length === 0} onclick={() => scan.start(true)} title="Read every file again"
              >Full rescan</button
            >
            <button class="primary" disabled={roots.length === 0} onclick={() => scan.start()}>Scan now</button>
          {/if}
        </div>
        {#if scan.running}
          <div class="progress"><div style:width="{(scan.fraction ?? 0.05) * 100}%"></div></div>
          <p class="hint">
            {scan.phase === 'walking'
              ? `Looking for books… ${formatCount(scan.found)} found`
              : `Reading ${formatCount(scan.done)} of ${formatCount(scan.total)}…`}
          </p>
        {:else if scan.report}
          <p class="hint">
            Last scan: {formatCount(scan.report.found)} books in {formatDuration(scan.report.elapsed_ms)},
            {formatCount(scan.report.added)} new{#if scan.report.errors}, {formatCount(scan.report.errors)} could not be read fully{/if}.
          </p>
        {/if}
      </div>
      <div class="card">
        <Switch
          label="Look for new books at startup"
          description="A quick check of the folders each time Biblion opens. Unchanged files are skipped, so it takes seconds even for large libraries."
          checked={ui.value.scanOnStartup}
          onchange={(v) => ui.update({ scanOnStartup: v })}
        />
      </div>
    </section>

    <!-- Reading -->
    <section id="reading">
      <h2>Reading</h2>
      <p class="hint">Defaults for every book. You can also change them while reading, from the Aa button.</p>
      <div class="reading">
        <div
          class="card reader-settings"
          style:--r-bg="var(--surface)"
          style:--r-fg="var(--text)"
          style:--r-link="var(--accent)"
          style:--r-border="var(--border)"
          style:--r-muted="var(--muted)"
          style:--r-hover="color-mix(in srgb, var(--text) 6%, transparent)"
        >
          <SettingsPanel />
        </div>
        <div
          class="preview"
          style:background={readerTheme.bg}
          style:color={readerTheme.fg}
          style:font-family={FONTS[r.font].stack ?? 'var(--serif)'}
          style:font-size="{(r.fontSize / 100) * 16}px"
          style:line-height={r.lineHeight}
          style:text-align={r.justify ? 'justify' : 'start'}
          style:hyphens={r.hyphenate ? 'auto' : 'manual'}
          lang="en"
        >
          <span class="preview-label" style:color={readerTheme.muted}>Preview</span>
          <h4>Chapter One</h4>
          <p>
            It was a quiet evening, and the library smelled of paper and rain. Somewhere between the shelves a page turned,
            as if the books themselves were reading one another in the dark.
          </p>
          <p>She chose a volume at random, opened it near the middle, and began to read.</p>
        </div>
      </div>

      <h3>Dictionaries</h3>
      <p class="hint">
        Select a word while reading and choose <em>Define</em>. Offline dictionaries in StarDict format are searched first;
        free ones for many languages are available from
        <button class="text-link" onclick={() => openUrl('https://freedict.org/downloads/')}>FreeDict</button>.
      </p>
      <div class="card">
        {#each dictionaries as d (d.id)}
          <div class="folder" in:rise={{ y: 4, duration: 200 }}>
            <Icon name="define" size={17} />
            <span class="dict-name">{d.name}</span>
            <span class="muted dict-count">{formatCount(d.word_count)} words</span>
            <button class="ghost" onclick={() => removeDictionary(d)}>Remove</button>
          </div>
        {:else}
          <p class="muted">No offline dictionaries yet.</p>
        {/each}
        {#if dictError}<p class="error">{dictError}</p>{/if}
        <div class="buttons">
          <button onclick={addDictionary} disabled={installing}>
            <Icon name="plus" size={15} />
            {installing ? 'Installing…' : 'Add dictionary…'}
          </button>
          <span class="hint inline">.ifo, .zip, .tar.gz or .tar.xz</span>
        </div>
      </div>
      <div class="card">
        <Switch
          label="Look words up online"
          description="When no offline dictionary knows a word, ask Wiktionary. Only the word is sent."
          checked={ui.value.onlineDictionary}
          onchange={(v) => ui.update({ onlineDictionary: v })}
        />
      </div>
    </section>

    <!-- Ambience -->
    <section id="ambience">
      <h2>Ambience</h2>
      <p class="hint">
        A soundscape made live for reading: it never repeats, follows the time of day and answers your page turns. Mix in
        rain, a fireplace or the sea, save your own scenes, and let it fade out with the sleep timer.
      </p>
      <div class="card ambience-card">
        <AmbiencePanel />
      </div>
    </section>

    <!-- About -->
    <section id="about">
      <h2>About</h2>
      <div class="card about">
        <div class="logo" aria-hidden="true"><Icon name="reading" size={26} strokeWidth={2} /></div>
        <div>
          <strong>Biblion</strong> <span class="muted">version 0.1.0</span>
          <p class="hint">A local library manager for large collections. Everything stays on this computer.</p>
        </div>
      </div>
      {#if library.stats}
        <div class="facts">
          <div><span>{formatCount(library.stats.books)}</span>books</div>
          <div><span>{formatCount(library.stats.authors)}</span>authors</div>
          <div><span>{formatCount(library.stats.with_cover)}</span>with cover</div>
          <div><span>{formatBytes(library.stats.total_bytes)}</span>of books</div>
        </div>
      {/if}
      <h3>Library data</h3>
      <div class="card folder">
        <Icon name="folder" size={17} />
        <span class="path" title={dataDir}>{dataDir || '…'}</span>
        <button class="ghost" onclick={copyDataDir}><Icon name={copied ? 'check' : 'copy'} size={15} /> {copied ? 'Copied' : 'Copy'}</button>
      </div>
      <h3>Keyboard shortcuts</h3>
      <dl class="keys">
        <dt><kbd>Ctrl</kbd> <kbd>F</kbd></dt><dd>Search the library (or the open book)</dd>
        <dt><kbd>Ctrl</kbd> <kbd>B</kbd></dt><dd>Show or hide the sidebar</dd>
        <dt><kbd>Esc</kbd></dt><dd>Close search, panels or the reader; go back</dd>
        <dt><kbd>Alt</kbd> <kbd>←</kbd></dt><dd>Back</dd>
        <dt><kbd>←</kbd> <kbd>→</kbd> <kbd>Space</kbd></dt><dd>Turn pages while reading</dd>
        <dt><kbd>Shift</kbd> <kbd>Enter</kbd></dt><dd>Open the focused book</dd>
      </dl>
    </section>
  </div>
</div>

<style>
  .settings {
    display: grid;
    grid-template-columns: 210px 1fr;
    height: 100%;
  }

  .index {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 22px 12px 22px 28px;
  }

  .subtitle {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--accent);
  }

  h1 {
    margin: 0 0 18px;
    font-family: var(--serif);
    font-size: 26px;
    font-weight: 600;
  }

  .index button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 8px;
    color: var(--muted);
    text-align: left;
  }

  .index button.current {
    color: var(--accent);
    background: var(--accent-soft);
    font-weight: 600;
  }

  .scroller {
    position: relative;
    overflow-y: auto;
    padding: 22px 36px 80px 12px;
    scroll-behavior: smooth;
  }

  section {
    max-width: 760px;
    padding-bottom: 28px;
    margin-bottom: 8px;
    scroll-margin-top: 12px;
  }

  h2 {
    margin: 4px 0 6px;
    font-family: var(--serif);
    font-size: 22px;
    font-weight: 600;
  }

  h3 {
    margin: 22px 0 10px;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  .hint {
    margin: 4px 0 10px;
    font-size: 12.5px;
    color: var(--muted);
    line-height: 1.5;
  }

  .card {
    padding: 6px 18px;
    margin-top: 12px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--surface);
  }

  /* Theme cards with a tiny mock-up of the app. */
  .themes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
  }

  .theme-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border-radius: 14px;
    border: 2px solid var(--border);
    background: var(--surface);
    transition:
      border-color 200ms ease,
      transform 220ms var(--ease);
  }

  .theme-card:hover:not(:disabled) {
    transform: translateY(-2px);
    background: var(--surface);
  }

  .theme-card.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
  }

  .mock {
    display: flex;
    height: 86px;
    border-radius: 8px;
    overflow: hidden;
    --m-bg: #f6f1e7;
    --m-side: #efe7d8;
    --m-line: #ddd0b8;
    --m-book: #c9b48e;
  }

  .mock.dark {
    --m-bg: #17181a;
    --m-side: #1b1c1f;
    --m-line: #34353a;
    --m-book: #5a5d66;
  }

  .mock.system {
    background: linear-gradient(115deg, #f6f1e7 50%, #17181a 50%);
    --m-bg: transparent;
    --m-side: rgb(128 128 128 / 0.2);
    --m-line: rgb(128 128 128 / 0.45);
    --m-book: rgb(128 128 128 / 0.55);
  }

  .mock-side {
    width: 24%;
    background: var(--m-side);
  }

  .mock-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    background: var(--m-bg);
  }

  .mock-bar {
    width: 60%;
    height: 7px;
    border-radius: 4px;
    background: var(--m-line);
  }

  .mock-books {
    display: flex;
    gap: 6px;
  }

  .mock-books i {
    width: 18px;
    height: 27px;
    border-radius: 2px;
    background: var(--m-book);
  }

  .mock-books i:nth-child(2) {
    background: var(--accent);
  }

  .theme-label {
    font-weight: 600;
    text-align: center;
  }

  .accents {
    display: flex;
    gap: 12px;
  }

  .swatch {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    padding: 0;
    border-radius: 50%;
    border: none;
    background: var(--c);
    color: #fff;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.15);
    transition: transform 220ms var(--ease);
  }

  .swatch:hover:not(:disabled) {
    background: var(--c);
    transform: scale(1.12);
  }

  .swatch.active {
    transform: scale(1.12);
    box-shadow:
      0 0 0 3px var(--bg),
      0 0 0 5px var(--c);
  }

  .segmented {
    display: inline-flex;
    gap: 6px;
  }

  .segmented button {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-radius: 10px;
  }

  .segmented button.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .size-preview {
    border-radius: 2px;
    background: currentColor;
    opacity: 0.6;
  }

  .folder {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
  }

  .card.folder {
    padding: 8px 14px;
    border-bottom: 1px solid var(--border);
  }

  .path {
    flex: 1;
    min-width: 0;
    font-family: ui-monospace, monospace;
    font-size: 12px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .buttons {
    display: flex;
    gap: 8px;
    padding: 12px 0;
  }

  .buttons button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .spacer {
    flex: 1;
  }

  .progress {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .progress div {
    height: 100%;
    background: var(--accent);
    transition: width 200ms linear;
  }

  .reading {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(290px, 1fr));
    gap: 18px;
    align-items: start;
  }

  .reader-settings {
    padding: 16px;
  }

  .preview {
    position: sticky;
    top: 0;
    margin-top: 12px;
    padding: 28px 32px;
    border-radius: 14px;
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    transition:
      background 300ms ease,
      color 300ms ease,
      font-size 200ms ease;
  }

  .preview-label {
    font-family: var(--font);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .preview h4 {
    margin: 10px 0;
    font-size: 1.4em;
  }

  .about {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px 18px;
  }

  .logo {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    border-radius: 14px;
    color: var(--accent-text);
    background: linear-gradient(140deg, var(--accent), color-mix(in srgb, var(--accent) 60%, #5a2d0c));
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
    margin-top: 12px;
  }

  .facts div {
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--muted);
    font-size: 12px;
  }

  .facts span {
    font-family: var(--serif);
    font-size: 20px;
    font-weight: 600;
    color: var(--text);
  }

  .folder button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .ambience-card {
    padding: 20px;
  }

  .dict-name {
    flex: 1;
    min-width: 0;
    color: var(--text);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dict-count {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .text-link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    text-decoration: underline;
    font-size: inherit;
  }

  .hint.inline {
    margin: 0;
    align-self: center;
  }

  .keys {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 10px 18px;
    align-items: center;
  }

  .keys dd {
    margin: 0;
    color: var(--muted);
  }

  kbd {
    font-family: inherit;
    font-size: 11px;
    padding: 2px 7px;
    border-radius: 5px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    background: var(--surface);
  }
</style>
