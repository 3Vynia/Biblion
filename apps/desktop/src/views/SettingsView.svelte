<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'
  import { confirm } from '../lib/dialog.svelte'
  import type { AutoBackup, DictionaryInfo } from '../lib/types'
  import { backUpNow, describe, exportNotes, pickBackupFile, restoreFrom } from '../lib/backup'
  import { ambience } from '../lib/ambience/store.svelte'
  import Icon, { type IconName } from '../components/Icon.svelte'
  import Switch from '../components/Switch.svelte'
  import SettingRow from '../components/SettingRow.svelte'
  import SettingsPanel from '../reader/SettingsPanel.svelte'
  import AmbiencePanel from '../components/AmbiencePanel.svelte'
  import ShortcutList from '../components/ShortcutList.svelte'
  import { FONTS, THEMES } from '../reader/readerSettings'
  import { readerSettings } from '../reader/settingsStore.svelte'
  import { api, errorMessage } from '../lib/api'
  import { formatBytes, formatCount, formatDate, formatDuration, plural } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { nav } from '../lib/nav.svelte'
  import { rise } from '../lib/motion'
  import { scan } from '../lib/scan.svelte'
  import { ACCENTS, ui, type AccentId, type CoverSize, type ThemeMode } from '../lib/uiSettings.svelte'

  interface Props {
    roots: string[]
    onrootschange: (roots: string[]) => void
  }

  let { roots, onrootschange }: Props = $props()

  interface Section {
    id: string
    label: string
    icon: IconName
    /** Extra words the search box should find this section by. */
    keywords: string
  }

  const GROUPS: { title: string; sections: Section[] }[] = [
    {
      title: 'General',
      sections: [
        { id: 'appearance', label: 'Appearance', icon: 'sparkle', keywords: 'theme dark light system accent colour color cover size titles captions animations motion sidebar compact' },
        { id: 'library', label: 'Library', icon: 'folder', keywords: 'folders scan rescan watch startup new books health duplicates' },
      ],
    },
    {
      title: 'Reading',
      sections: [
        { id: 'reading', label: 'Reading', icon: 'reading', keywords: 'font size line spacing margins justify hyphenation theme day sepia night console preview' },
        { id: 'dictionaries', label: 'Dictionaries', icon: 'define', keywords: 'define word stardict freedict lookup' },
        { id: 'ambience', label: 'Ambience', icon: 'sound', keywords: 'sound soundscape rain fireplace sea scenes music folder songs shuffle sleep timer page turn volume' },
      ],
    },
    {
      title: 'Your data',
      sections: [
        { id: 'backup', label: 'Backup', icon: 'save', keywords: 'restore export notes markdown automatic copy highlights progress' },
        { id: 'privacy', label: 'Privacy & online', icon: 'globe', keywords: 'internet network wiktionary online offline' },
      ],
    },
    {
      title: 'Help',
      sections: [
        { id: 'keyboard', label: 'Keyboard', icon: 'keyboard', keywords: 'shortcuts keys hotkeys' },
        { id: 'about', label: 'About', icon: 'info', keywords: 'version data folder' },
      ],
    },
  ]
  const SECTIONS = GROUPS.flatMap((g) => g.sections)

  let scroller: HTMLDivElement
  let current = $state('appearance')
  let query = $state('')
  let folderError = $state<string | null>(null)
  let dataDir = $state('')
  let copied = $state(false)
  let dictionaries = $state<DictionaryInfo[]>([])
  let dictError = $state<string | null>(null)
  let installing = $state(false)

  const terms = $derived(query.trim().toLowerCase().split(/\s+/).filter(Boolean))
  const matches = (s: Section) => terms.every((t) => `${s.label} ${s.keywords}`.toLowerCase().includes(t))
  const shown = $derived(new Set(SECTIONS.filter(matches).map((s) => s.id)))

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

  // ── Backup ──
  let backupBusy = $state<string | null>(null)
  let backupMessage = $state<string | null>(null)
  let backupError = $state<string | null>(null)
  let restorePrefs = $state(false)
  let autoBackup = $state(true)
  let autoBackups = $state<AutoBackup[]>([])

  async function backupAction(label: string, fn: () => Promise<string | null>) {
    backupBusy = label
    backupError = null
    try {
      const message = await fn()
      if (message) backupMessage = message
    } catch (e) {
      backupError = errorMessage(e)
    } finally {
      backupBusy = null
    }
  }

  const saveBackup = () =>
    backupAction('save', async () => {
      const b = await backUpNow()
      if (!b) return null
      return `Saved ${formatBytes(b.size)}: ${formatCount(b.annotations)} highlights and notes, progress of ${formatCount(b.progress)} ${plural(b.progress, 'book')}, ${formatCount(b.shelves)} ${plural(b.shelves, 'collection')}.`
    })

  const restoreBackup = (path: string | null) =>
    backupAction('restore', async () => {
      const file = path ?? (await pickBackupFile())
      if (!file) return null
      const report = await restoreFrom(file, restorePrefs)
      if (!report) return null
      library.changed()
      if (restorePrefs) await Promise.all([ui.reload(), readerSettings.reload(), ambience.reload()])
      const changes = describe(report)
      return changes.length ? `Restored: ${changes.join(', ')}.` : 'Nothing to restore: your library already had everything.'
    })

  const exportAllNotes = () =>
    backupAction('notes', async () => {
      const done = await exportNotes()
      return done ? `Exported ${formatCount(done.notes)} ${plural(done.notes, 'note')} from ${formatCount(done.books)} ${plural(done.books, 'book')}.` : null
    })

  async function setAutoBackup(on: boolean) {
    autoBackup = on
    try {
      await api.setAutoBackup(on)
    } catch (e) {
      autoBackup = !on
      backupError = errorMessage(e)
    }
  }

  async function showBackupsFolder() {
    try {
      const first = autoBackups[0]
      await revealItemInDir(first ? first.path : await api.backupsDir())
    } catch (e) {
      backupError = errorMessage(e)
    }
  }

  let watchFolders = $state(true)
  async function setWatch(on: boolean) {
    watchFolders = on
    try {
      await api.setWatchFolders(on)
    } catch (e) {
      watchFolders = !on
      folderError = errorMessage(e)
    }
  }

  onMount(() => {
    readerSettings.load()
    api.getWatchFolders().then((v) => (watchFolders = v)).catch(() => {})
    api.getAutoBackup().then((v) => (autoBackup = v)).catch(() => {})
    api.listAutoBackups().then((v) => (autoBackups = v)).catch(() => {})
    api.dataDir().then((d) => (dataDir = d)).catch(() => {})
    api.listDictionaries().then((d) => (dictionaries = d)).catch(() => {})
  })

  // Highlight the section being read in the side index: the last one whose
  // heading has reached the top, or the last one when scrolled to the end.
  function track() {
    const sections = [...scroller.querySelectorAll<HTMLElement>('section[id]:not([hidden])')]
    if (!sections.length) return
    const top = scroller.scrollTop + scroller.offsetTop + 40
    const atEnd = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 4
    let id = sections[0].id
    for (const s of sections) if (s.offsetTop <= top) id = s.id
    current = atEnd ? sections.at(-1)!.id : id
  }

  // While a jump scrolls, the clicked section stays highlighted.
  let target: string | null = null
  let settle: ReturnType<typeof setTimeout> | undefined
  function onScroll() {
    clearTimeout(settle)
    if (target) settle = setTimeout(() => (target = null), 150)
    else track()
  }

  function goTo(id: string) {
    // Scroll only this panel: scrollIntoView would also shift the whole window.
    const section = scroller.querySelector<HTMLElement>(`#${id}`)
    if (!section) return
    current = target = id
    const top = section.offsetTop - scroller.offsetTop - 12
    if (Math.abs(scroller.scrollTop - top) < 2) target = null
    scroller.scrollTo({ top, behavior: 'smooth' })
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && query) {
      e.stopPropagation()
      query = ''
    } else if (e.key === 'Enter') {
      const first = SECTIONS.find((s) => shown.has(s.id))
      if (first) goTo(first.id)
    }
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
    const ok = await confirm({
      title: 'Stop using this folder?',
      message: `Biblion stops looking for books in ${path}. The files themselves are not touched, and you can add the folder again later.`,
      confirmLabel: 'Remove folder',
      danger: true,
    })
    if (!ok) return
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
    { id: 'small', label: 'Small', w: 10 },
    { id: 'medium', label: 'Medium', w: 13 },
    { id: 'large', label: 'Large', w: 16 },
  ]

  const r = $derived(readerSettings.value)
  const readerTheme = $derived(THEMES[r.theme])
  const onOff = (on: boolean) => (on ? 'on' : 'off')

  // One line under each heading: what this section is set to right now.
  const summary = $derived<Record<string, string>>({
    appearance: [
      `${THEME_OPTIONS.find((t) => t.id === ui.value.theme)?.label} theme`,
      `${ACCENTS[ui.value.accent].label.toLowerCase()} accent`,
      `${ui.value.coverSize} covers`,
    ].join(' · '),
    library: [
      `${roots.length} ${plural(roots.length, 'folder')}`,
      library.stats ? `${formatCount(library.stats.books)} books` : null,
      `watching ${onOff(watchFolders)}`,
    ]
      .filter(Boolean)
      .join(' · '),
    reading: `${THEMES[r.theme].label} · ${FONTS[r.font].label} font · ${r.fontSize}% text`,
    dictionaries: `${dictionaries.length} offline · online lookups ${onOff(ui.value.onlineDictionary)}`,
    ambience: ambience.playing ? `Playing · ${ambience.active?.name ?? 'your mix'}` : 'Not playing',
    backup: autoBackup
      ? `Automatic backups on${autoBackups[0] ? ` · last ${formatDate(autoBackups[0].created_at)}` : ''}`
      : 'Automatic backups off',
    privacy: ui.value.onlineDictionary ? 'Only single words go online, to Wiktionary' : 'Nothing leaves this computer',
    keyboard: 'Press ? anywhere to see them',
    about: 'Biblion 0.1.0',
  })
</script>

<div class="settings">
  <nav class="index" aria-label="Settings sections" in:rise={{ y: 6 }}>
    <span class="subtitle">Preferences</span>
    <h1>Settings</h1>
    <label class="find">
      <Icon name="search" size={15} />
      <input
        id="settings-search"
        type="search"
        placeholder="Search settings"
        aria-label="Search settings"
        bind:value={query}
        onkeydown={onSearchKey}
      />
      {#if !query}<kbd>/</kbd>{/if}
    </label>
    {#each GROUPS as g (g.title)}
      {#if g.sections.some((s) => shown.has(s.id))}
        <span class="group">{g.title}</span>
        {#each g.sections.filter((s) => shown.has(s.id)) as s (s.id)}
          <button class="ghost" class:current={current === s.id} aria-current={current === s.id ? 'true' : undefined} onclick={() => goTo(s.id)}>
            <Icon name={s.icon} size={16} />
            {s.label}
          </button>
        {/each}
      {/if}
    {/each}
    {#if shown.size === 0}<p class="no-match">No settings match “{query.trim()}”.</p>{/if}
  </nav>

  <div class="scroller" bind:this={scroller} onscroll={onScroll}>
    {#snippet head(id: string, title: string)}
      <header>
        <h2>{title}</h2>
        <p class="summary">{summary[id]}</p>
      </header>
    {/snippet}

    <!-- Appearance -->
    <section id="appearance" hidden={!shown.has('appearance')} in:rise={{ delay: 40 }}>
      {@render head('appearance', 'Appearance')}
      <div class="card">
        <SettingRow label="Theme" description="Follow the system, or keep Biblion light or dark." wide>
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
        </SettingRow>
        <SettingRow label="Accent colour" description="Buttons, progress and highlights in the library.">
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
                {#if ui.value.accent === id}<Icon name="check" size={13} strokeWidth={3} />{/if}
              </button>
            {/each}
          </div>
        </SettingRow>
        <SettingRow label="Cover size" description="How big books are in the grid.">
          <div class="segmented">
            {#each SIZES as s (s.id)}
              <button class:active={ui.value.coverSize === s.id} onclick={() => ui.update({ coverSize: s.id })}>
                <span class="size-preview" style:width="{s.w}px" style:height="{s.w * 1.5}px"></span>
                {s.label}
              </button>
            {/each}
          </div>
        </SettingRow>
        <Switch
          label="Titles under covers"
          description="Show the title and author below each cover in the grid."
          checked={ui.value.showCaptions}
          onchange={(v) => ui.update({ showCaptions: v })}
        />
        <Switch
          label="Compact sidebar"
          description="Show only icons in the sidebar. Toggle it any time with Ctrl+B."
          checked={ui.value.sidebarCollapsed}
          onchange={(v) => ui.update({ sidebarCollapsed: v })}
        />
        <Switch
          label="Animations"
          description="Transitions, hover effects and counters. The system “reduce motion” setting is always respected."
          checked={ui.value.animations}
          onchange={(v) => ui.update({ animations: v })}
        />
      </div>
    </section>

    <!-- Library -->
    <section id="library" hidden={!shown.has('library')}>
      {@render head('library', 'Library')}
      <h3>Folders</h3>
      <div class="card">
        <p class="hint">Biblion looks for EPUB, PDF, MOBI, AZW3, CBZ and CBR files in these folders and all their subfolders.</p>
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
            <button disabled={roots.length === 0} onclick={() => scan.start(true)} title="Read every file again">Full rescan</button>
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
        {/if}
      </div>

      <h3>Keeping up to date</h3>
      <div class="card">
        <Switch
          label="Watch folders for changes"
          description="Books you add, move or delete in these folders show up on their own, a few seconds later. Files still downloading are read again once they settle."
          checked={watchFolders}
          onchange={setWatch}
        />
        <Switch
          label="Look for new books at startup"
          description="A quick check each time Biblion opens. Unchanged files are skipped, so it takes seconds even for large libraries."
          checked={ui.value.scanOnStartup}
          onchange={(v) => ui.update({ scanOnStartup: v })}
        />
        {#if scan.report && !scan.running}
          <SettingRow
            label="Last scan"
            description="{formatCount(scan.report.found)} books in {formatDuration(scan.report.elapsed_ms)}, {formatCount(scan.report.added)} new{scan.report.errors
              ? `, ${formatCount(scan.report.errors)} could not be read fully`
              : ''}."
          />
        {/if}
        <SettingRow label="Library health" description="Duplicates, missing files and books that could not be read.">
          <button onclick={() => nav.jump({ name: 'health' })}><Icon name="pulse" size={15} /> Open</button>
        </SettingRow>
      </div>
    </section>

    <!-- Reading -->
    <section id="reading" hidden={!shown.has('reading')}>
      {@render head('reading', 'Reading')}
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
          <span class="preview-label" style:color={readerTheme.muted}>Live preview</span>
          <h4>Chapter One</h4>
          <p>
            It was a quiet evening, and the library smelled of paper and rain. Somewhere between the shelves a page turned,
            as if the books themselves were reading one another in the dark.
          </p>
          <p>She chose a volume at random, opened it near the middle, and began to read.</p>
        </div>
      </div>
    </section>

    <!-- Dictionaries -->
    <section id="dictionaries" hidden={!shown.has('dictionaries')}>
      {@render head('dictionaries', 'Dictionaries')}
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
      <p class="hint see-also">
        Online lookups are in <button class="text-link" onclick={() => goTo('privacy')}>Privacy &amp; online</button>.
      </p>
    </section>

    <!-- Ambience -->
    <section id="ambience" hidden={!shown.has('ambience')}>
      {@render head('ambience', 'Ambience')}
      <p class="hint">
        A soundscape made live for reading: it never repeats, follows the time of day and answers your page turns. Mix in
        rain, a fireplace or the sea, add your own music, and let it fade out with the sleep timer.
      </p>
      <div class="card ambience-card">
        <AmbiencePanel tabs />
      </div>
    </section>

    <!-- Backup -->
    <section id="backup" hidden={!shown.has('backup')}>
      {@render head('backup', 'Backup')}
      <p class="hint">
        Your highlights, notes, reading progress, collections and reading time live in Biblion’s database. A backup keeps
        them safe, and brings them back on this computer or another one, even if your books moved.
      </p>
      <div class="actions">
        <button class="action primary" disabled={!!backupBusy} onclick={saveBackup}>
          <Icon name="save" size={20} />
          <strong>Back up now…</strong>
          <small>Everything in one small file.</small>
        </button>
        <button class="action" disabled={!!backupBusy} onclick={() => restoreBackup(null)}>
          <Icon name="clock" size={20} />
          <strong>Restore from a file…</strong>
          <small>See what changes before anything does.</small>
        </button>
        <button class="action" disabled={!!backupBusy} onclick={exportAllNotes}>
          <Icon name="quote" size={20} />
          <strong>Export notes as Markdown…</strong>
          <small>Every highlight and note, book by book.</small>
        </button>
      </div>
      {#if backupMessage}<p class="hint backup-message" role="status">{backupMessage}</p>{/if}
      {#if backupError}<p class="error">{backupError}</p>{/if}
      <div class="card">
        <Switch
          label="Also restore preferences"
          description="Reading, appearance and ambience settings from the backup replace yours. Library folders never change."
          checked={restorePrefs}
          onchange={(v) => (restorePrefs = v)}
        />
        <Switch
          label="Automatic backups"
          description="A copy every day in Biblion’s data folder. The last seven are kept."
          checked={autoBackup}
          onchange={setAutoBackup}
        />
        {#if autoBackups.length}
          <ul class="auto-backups">
            {#each autoBackups as b (b.path)}
              <li>
                <Icon name="clock" size={14} />
                <span class="when">{formatDate(b.created_at)}</span>
                <span class="size">{formatBytes(b.size)}</span>
                <span class="spacer"></span>
                <button class="ghost small" disabled={!!backupBusy} onclick={() => restoreBackup(b.path)}>Restore</button>
              </li>
            {/each}
          </ul>
          <div class="buttons">
            <button class="ghost" onclick={showBackupsFolder}><Icon name="folder" size={15} /> Show folder</button>
          </div>
        {/if}
      </div>
    </section>

    <!-- Privacy -->
    <section id="privacy" hidden={!shown.has('privacy')}>
      {@render head('privacy', 'Privacy & online')}
      <p class="hint">
        Biblion works offline. It has no account and sends no usage data. These are the only things that can reach the
        internet, and only if you allow them.
      </p>
      <div class="card">
        <Switch
          label="Look words up online"
          description="When no offline dictionary knows a word, ask Wiktionary. Only the word is sent."
          checked={ui.value.onlineDictionary}
          onchange={(v) => ui.update({ onlineDictionary: v })}
        />
      </div>
    </section>

    <!-- Keyboard -->
    <section id="keyboard" hidden={!shown.has('keyboard')}>
      {@render head('keyboard', 'Keyboard')}
      <div class="card"><ShortcutList /></div>
    </section>

    <!-- About -->
    <section id="about" hidden={!shown.has('about')}>
      {@render head('about', 'About')}
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
      <div class="card">
        <SettingRow label="Library data" description={dataDir || '…'}>
          <button class="ghost" onclick={copyDataDir}><Icon name={copied ? 'check' : 'copy'} size={15} /> {copied ? 'Copied' : 'Copy path'}</button>
        </SettingRow>
      </div>
    </section>
  </div>
</div>

<style>
  .settings {
    display: grid;
    grid-template-columns: 236px 1fr;
    height: 100%;
  }

  .index {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 22px 12px 22px 28px;
    overflow-y: auto;
  }

  .subtitle {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--accent);
  }

  h1 {
    margin: 0 0 14px;
    font-family: var(--serif);
    font-size: 26px;
    font-weight: 600;
  }

  .find {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    padding: 7px 10px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
    transition:
      border-color 160ms ease,
      box-shadow 160ms ease;
  }

  .find:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .find input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: none;
    outline: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }

  .find input::-webkit-search-cancel-button {
    display: none;
  }

  .find kbd {
    padding: 0 6px;
  }

  .group {
    margin: 14px 0 4px 10px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
    opacity: 0.8;
  }

  .no-match {
    margin: 12px 10px;
    font-size: 12.5px;
    color: var(--muted);
  }

  .index button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
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
    max-width: 780px;
    padding-bottom: 36px;
    margin-bottom: 8px;
    scroll-margin-top: 12px;
  }

  section[hidden] {
    display: none;
  }

  header {
    margin-bottom: 10px;
  }

  h2 {
    margin: 4px 0 2px;
    font-family: var(--serif);
    font-size: 24px;
    font-weight: 600;
  }

  .summary {
    margin: 0;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--accent);
  }

  h3 {
    margin: 22px 0 8px;
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
    padding: 4px 20px;
    margin-top: 12px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--surface);
  }

  .card > .hint:first-child {
    margin-top: 14px;
  }

  /* Every row in a card is divided from the next. */
  .card > :global(:is(.row, .setting) + :is(.row, .setting)) {
    border-top: 1px solid var(--border);
  }

  .card > :global(.row) {
    padding: 14px 0;
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
    height: 74px;
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
    gap: 10px;
  }

  .swatch {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
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
      0 0 0 3px var(--surface),
      0 0 0 5px var(--c);
  }

  .segmented {
    display: inline-flex;
    gap: 4px;
    padding: 3px;
    border-radius: 11px;
    background: var(--surface-2);
  }

  .segmented button {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 12px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
  }

  .segmented button.active {
    background: var(--surface);
    color: var(--accent);
    font-weight: 600;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.15);
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
    flex-wrap: wrap;
    gap: 8px;
    padding: 12px 0;
  }

  .buttons button,
  .card :global(.setting button) {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  /* Backup: the three things you come here to do. */
  .actions {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    margin-top: 12px;
  }

  .action {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 16px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--surface);
    text-align: left;
    transition:
      transform 220ms var(--ease),
      border-color 200ms ease;
  }

  .action:hover:not(:disabled) {
    transform: translateY(-2px);
    border-color: var(--accent);
    background: var(--surface);
  }

  .action :global(svg) {
    margin-bottom: 6px;
    color: var(--accent);
  }

  .action.primary {
    border-color: transparent;
    background: linear-gradient(140deg, var(--accent), color-mix(in srgb, var(--accent) 70%, #5a2d0c));
    color: var(--accent-text);
  }

  .action.primary:hover:not(:disabled) {
    background: linear-gradient(140deg, var(--accent), color-mix(in srgb, var(--accent) 70%, #5a2d0c));
  }

  .action.primary :global(svg),
  .action.primary small {
    color: inherit;
  }

  .action small {
    font-size: 12px;
    color: var(--muted);
    line-height: 1.4;
  }

  .action.primary small {
    opacity: 0.85;
  }

  .backup-message {
    margin: 12px 0 0;
    color: var(--text);
  }

  .auto-backups {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .auto-backups li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
    border-top: 1px solid var(--border);
    font-size: 13px;
    color: var(--muted);
  }

  .auto-backups .when {
    color: var(--text);
  }

  button.small {
    padding: 4px 10px;
    font-size: 12px;
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

  .see-also {
    margin-top: 10px;
  }

  .hint.inline {
    margin: 0;
    align-self: center;
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
