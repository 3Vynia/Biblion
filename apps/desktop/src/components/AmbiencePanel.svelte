<script lang="ts">
  // Controls for the reading ambience. Colours come from --a-* variables so
  // it fits both the reader (its theme) and the main window.
  import AmbienceOrb from './AmbienceOrb.svelte'
  import Icon from './Icon.svelte'
  import Switch from './Switch.svelte'
  import { prompt } from '../lib/dialog.svelte'
  import { ambience } from '../lib/ambience/store.svelte'
  import { palette } from '../lib/ambience/palette'
  import { LAYERS, MODES, type LayerId, type Mode, type Scene } from '../lib/ambience/settings'
  import { rise } from '../lib/motion'
  import { rangeFill } from '../lib/rangeFill'
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { errorMessage } from '../lib/api'

  interface Props {
    /** Smaller layout for the reader's side panel. */
    compact?: boolean
    /** Split the controls into tabs below the "now playing" header (Settings). */
    tabs?: boolean
  }

  let { compact = false, tabs = false }: Props = $props()

  const TABS = [
    { id: 'soundscape', label: 'Soundscape' },
    { id: 'sounds', label: 'Sounds & scenes' },
    { id: 'music', label: 'Your music' },
    { id: 'timer', label: 'Timer & options' },
  ] as const
  type Tab = (typeof TABS)[number]['id']
  let tab = $state<Tab>('soundscape')
  const show = (t: Tab) => !tabs || tab === t

  function tabKey(e: KeyboardEvent) {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
    if (!step) return
    e.preventDefault()
    const i = (TABS.findIndex((t) => t.id === tab) + step + TABS.length) % TABS.length
    tab = TABS[i].id
    ;(e.currentTarget as HTMLElement).querySelector<HTMLElement>(`[data-tab="${tab}"]`)?.focus()
  }

  const s = $derived(ambience.value)
  const active = $derived(ambience.active)
  const hour = new Date().getHours()
  const TIMERS = [0, 15, 30, 45, 60, 90]
  const groups = ['Nature', 'Indoors', 'Noise'] as const

  ambience.load()

  let now = $state(Date.now())
  $effect(() => {
    if (!ambience.timerEnds) return
    const id = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(id)
  })
  const remaining = $derived(ambience.timerEnds ? Math.max(0, Math.ceil((ambience.timerEnds - now) / 60000)) : 0)

  async function saveMix() {
    const name = await prompt({ title: 'Save this mix', label: 'Scene name', value: '', confirmLabel: 'Save scene', maxLength: 60 })
    if (name) ambience.saveScene(name)
  }

  const pct = (v: number) => Math.round(v * 100)

  // ── Music folder ──
  const lib = $derived(ambience.musicLibrary)
  let musicError = $state<string | null>(null)
  onMount(() => {
    ambience.loadMusic()
  })

  async function pickMusicFolder() {
    musicError = null
    const path = await open({ directory: true, multiple: false, title: 'Choose a folder with music' })
    if (typeof path !== 'string') return
    try {
      await ambience.chooseMusicFolder(path)
    } catch (e) {
      musicError = errorMessage(e)
    }
  }
</script>

<div class="ambience" class:compact>
  <div class="hero">
    <div class="orb">
      <AmbienceOrb size={compact ? 150 : 190} />
      <button
        class="play"
        class:on={ambience.playing}
        onclick={() => ambience.toggle()}
        aria-label={ambience.playing ? 'Pause ambience' : 'Play ambience'}
        title={ambience.playing ? 'Pause' : 'Play'}
      >
        <Icon name={ambience.playing ? 'pause' : 'play'} size={22} filled />
      </button>
    </div>
    <div class="now">
      <span class="eyebrow">{ambience.playing ? 'Now playing' : 'Ambience'}</span>
      <strong>{active?.name ?? (s.soundscape.on ? `${MODES.find((m) => m.id === s.soundscape.mode)?.label} soundscape` : 'Your mix')}</strong>
      <label class="slider">
        <Icon name="sound" size={15} />
        <input type="range" min="0" max="1" step="0.01" value={s.master} use:rangeFill={s.master} oninput={(e) => ambience.setMaster(Number(e.currentTarget.value))} aria-label="Master volume" />
        <span>{pct(s.master)}</span>
      </label>
      {#if ambience.timerEnds}
        <span class="timer-left"><Icon name="moon" size={13} /> Fades out in {remaining} min</span>
      {/if}
    </div>
  </div>

  {#if tabs}
    <div class="tabs" role="tablist" aria-label="Ambience settings" tabindex="-1" onkeydown={tabKey}>
      {#each TABS as t (t.id)}
        <button
          role="tab"
          data-tab={t.id}
          aria-selected={tab === t.id}
          tabindex={tab === t.id ? 0 : -1}
          class:on={tab === t.id}
          onclick={() => (tab = t.id)}
        >
          {t.label}
          {#if t.id === 'soundscape' && s.soundscape.on}<span class="live" aria-hidden="true"></span>{/if}
          {#if t.id === 'sounds' && LAYERS.some((l) => s.layers[l.id].on)}<span class="live" aria-hidden="true"></span>{/if}
          {#if t.id === 'music' && s.music.on && lib?.tracks.length}<span class="live" aria-hidden="true"></span>{/if}
          {#if t.id === 'timer' && ambience.timerEnds}<span class="badge">{remaining}m</span>{/if}
        </button>
      {/each}
    </div>
  {/if}

  {#if show('soundscape')}
    <section>
      <h4>Soundscape</h4>
      <div class="modes" role="radiogroup" aria-label="Mode">
        {#each MODES as m (m.id)}
          {@const [a, b] = palette(m.id, hour)}
          <button
            class="mode"
            class:on={s.soundscape.on && s.soundscape.mode === m.id}
            style:--m1={a}
            style:--m2={b}
            role="radio"
            aria-label={m.label}
            title={m.hint}
            aria-checked={s.soundscape.on && s.soundscape.mode === m.id}
            onclick={() => ambience.setSoundscape({ on: true, mode: m.id as Mode })}
          >
            <span class="swatch"></span>
            <span class="mode-name">{m.label}</span>
            {#if !compact}<span class="mode-hint">{m.hint}</span>{/if}
          </button>
        {/each}
      </div>
      {#if s.soundscape.on}
        <div class="controls" in:rise={{ y: 4, duration: 200 }}>
          <label class="slider labeled">
            <span class="name">Volume</span>
            <input type="range" min="0" max="1" step="0.01" value={s.soundscape.volume} use:rangeFill={s.soundscape.volume} oninput={(e) => ambience.setSoundscape({ volume: Number(e.currentTarget.value) })} />
          </label>
          <label class="slider labeled">
            <span class="name">Energy</span>
            <input type="range" min="0" max="1" step="0.01" value={s.soundscape.energy} use:rangeFill={s.soundscape.energy} oninput={(e) => ambience.setSoundscape({ energy: Number(e.currentTarget.value) })} />
            <span class="ends"><span>Sparse</span><span>Lively</span></span>
          </label>
          <label class="slider labeled">
            <span class="name">Warmth</span>
            <input type="range" min="0" max="1" step="0.01" value={s.soundscape.warmth} use:rangeFill={s.soundscape.warmth} oninput={(e) => ambience.setSoundscape({ warmth: Number(e.currentTarget.value) })} />
            <span class="ends"><span>Dark</span><span>Bright</span></span>
          </label>
          <Switch
            label="Follow the time of day"
            description="Brighter in the morning, deeper at night."
            checked={s.soundscape.adaptToTime}
            onchange={(v) => ambience.setSoundscape({ adaptToTime: v })}
          />
          <Switch
            label="Respond to my reading"
            description="A soft note when you turn a page; calmer when you pause."
            checked={s.soundscape.reactToReading}
            onchange={(v) => ambience.setSoundscape({ reactToReading: v })}
          />
          <button class="text" onclick={() => ambience.setSoundscape({ on: false })}>Turn the soundscape off</button>
        </div>
      {/if}
    </section>
  {/if}

  {#if show('sounds')}
    <section>
      <div class="row-head">
        <h4>Scenes</h4>
        <button class="text" onclick={saveMix} title="Save the current mix as a scene"><Icon name="save" size={14} /> Save mix</button>
      </div>
      <div class="scenes">
        {#each ambience.scenes as scene (scene.id)}
          <span class="scene-wrap">
            <button class="scene" class:on={active?.id === scene.id} onclick={() => ambience.useScene(scene)}>{scene.name}</button>
            {#if scene.id.startsWith('user-')}
              <button class="remove" onclick={() => ambience.deleteScene(scene as Scene)} aria-label="Delete scene {scene.name}" title="Delete scene"
                ><Icon name="close" size={11} /></button
              >
            {/if}
          </span>
        {/each}
      </div>
    </section>
  {/if}

  {#if show('music')}
    <section class="music">
      <div class="row-head">
        <h4>Your music</h4>
        {#if lib?.folder}
          <span class="folder-actions">
            <button class="text" onclick={pickMusicFolder} title={lib.folder}>Change folder</button>
            <button class="text muted" onclick={() => ambience.forgetMusicFolder()}>Stop using it</button>
          </span>
        {/if}
      </div>
      {#if !lib?.folder}
        <button class="choose" onclick={pickMusicFolder}>
          <Icon name="music" size={16} />
          <span>
            <strong>Choose a music folder…</strong>
            <small>Your own songs, mixed with the ambience. MP3, OGG, Opus, FLAC, M4A or WAV.</small>
          </span>
        </button>
      {:else if lib.tracks.length === 0}
        <p class="empty-music">No music found in {lib.folder}.</p>
      {:else}
        <div class="layer" class:on={s.music.on}>
          <button class="toggle" role="switch" aria-checked={s.music.on} aria-label="Play my music" onclick={() => ambience.setMusic({ on: !s.music.on })}>
            <span class="dot"></span>
            Play my music
            <span class="count">{lib.tracks.length} {lib.tracks.length === 1 ? 'track' : 'tracks'}</span>
          </button>
          {#if s.music.on}
            <div class="playing-now" in:rise={{ y: 4, duration: 200 }}>
              <div class="track">
                <span class="track-title">{ambience.track?.title ?? '—'}</span>
                {#if ambience.track?.folder}<span class="track-folder">{ambience.track.folder}</span>{/if}
              </div>
              <div class="transport">
                <button class="icon" onclick={() => ambience.prevTrack()} aria-label="Previous track" title="Previous track"><Icon name="skip-back" size={14} /></button>
                <button class="icon" onclick={() => ambience.nextTrack()} aria-label="Next track" title="Next track"><Icon name="skip-forward" size={14} /></button>
                <button
                  class="icon"
                  class:on={s.music.shuffle}
                  onclick={() => ambience.setMusic({ shuffle: !s.music.shuffle })}
                  aria-label="Shuffle"
                  aria-pressed={s.music.shuffle}
                  title="Shuffle"><Icon name="shuffle" size={14} /></button
                >
              </div>
            </div>
            <label class="slider">
              <span class="mini">Vol</span>
              <input type="range" min="0" max="1" step="0.01" value={s.music.volume} use:rangeFill={s.music.volume} oninput={(e) => ambience.setMusic({ volume: Number(e.currentTarget.value) })} aria-label="Music volume" />
            </label>
          {/if}
        </div>
      {/if}
      {#if musicError}<p class="error">{musicError}</p>{/if}
    </section>
  {/if}

  {#if show('sounds')}
    <section>
      <h4>Textures</h4>
      {#each groups as group (group)}
        <p class="group">{group}</p>
        {#each LAYERS.filter((l) => l.group === group) as layer (layer.id)}
          {@const st = s.layers[layer.id]}
          <div class="layer" class:on={st.on}>
            <button
              class="toggle"
              role="switch"
              aria-checked={st.on}
              aria-label={layer.label}
              onclick={() => ambience.setLayer(layer.id as LayerId, { on: !st.on })}
            >
              <span class="dot"></span>
              {layer.label}
            </button>
            {#if st.on}
              <div class="layer-controls" in:rise={{ y: 4, duration: 180 }}>
                <label class="slider">
                  <span class="mini">Vol</span>
                  <input type="range" min="0" max="1" step="0.01" value={st.volume} use:rangeFill={st.volume} oninput={(e) => ambience.setLayer(layer.id, { volume: Number(e.currentTarget.value) })} aria-label="{layer.label} volume" />
                </label>
                {#if layer.tone}
                  <label class="slider labeled">
                    <input type="range" min="0" max="1" step="0.01" value={st.tone} use:rangeFill={st.tone} oninput={(e) => ambience.setLayer(layer.id, { tone: Number(e.currentTarget.value) })} aria-label="{layer.label} character" />
                    <span class="ends"><span>{layer.tone[0]}</span><span>{layer.tone[1]}</span></span>
                  </label>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      {/each}
    </section>
  {/if}

  {#if show('timer')}
    <section>
      <h4>Sleep timer</h4>
      <div class="timers" role="radiogroup" aria-label="Sleep timer">
        {#each TIMERS as m (m)}
          <button class:on={s.timerMinutes === m} role="radio" aria-checked={s.timerMinutes === m} onclick={() => ambience.setTimer(m)}>{m ? `${m} min` : 'Off'}</button>
        {/each}
      </div>
    </section>

    <section>
      <h4>Options</h4>
      <Switch label="Start when I open a book" checked={s.autoStart} onchange={(v) => ambience.setOptions({ autoStart: v })} />
      <Switch label="Page-turn sound" description="A soft paper sound on every page." checked={s.pageTurn} onchange={(v) => ambience.setOptions({ pageTurn: v })} />
      {#if s.pageTurn}
        <label class="slider labeled">
          <span class="name">Page sound volume</span>
          <input type="range" min="0" max="1" step="0.01" value={s.pageTurnVolume} use:rangeFill={s.pageTurnVolume} oninput={(e) => ambience.setOptions({ pageTurnVolume: Number(e.currentTarget.value) })} />
        </label>
      {/if}
    </section>
  {/if}
</div>

<style>
  .ambience {
    --a-fg: var(--r-fg, var(--text));
    --a-muted: var(--r-muted, var(--muted));
    --a-border: var(--r-border, var(--border));
    --a-accent: var(--r-link, var(--accent));
    --a-card: var(--r-bg, var(--surface));
    --a-hover: var(--r-hover, var(--surface-2));
    color: var(--a-fg);
    container-type: inline-size;
  }

  .hero {
    display: flex;
    align-items: center;
    gap: 18px;
    margin-bottom: 8px;
  }

  .compact .hero {
    flex-direction: column;
    text-align: center;
    gap: 6px;
  }

  .orb {
    position: relative;
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }

  .play {
    position: absolute;
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    padding: 0;
    border-radius: 50%;
    border: 1px solid rgb(255 255 255 / 0.5);
    background: rgb(255 255 255 / 0.75);
    color: #2b2118;
    box-shadow: 0 8px 26px rgb(0 0 0 / 0.25);
    backdrop-filter: blur(6px);
    transition: transform 220ms var(--ease);
  }

  .play:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.9);
    transform: scale(1.06);
  }

  .play.on {
    background: rgb(255 255 255 / 0.55);
  }

  .now {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .compact .now {
    width: 100%;
  }

  .eyebrow {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--a-accent);
  }

  .now strong {
    font-family: var(--serif);
    font-size: 19px;
  }

  .timer-left {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--a-muted);
  }

  section {
    margin-top: 18px;
  }

  .tabs {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
    margin: 18px 0 4px;
    padding: 4px;
    border-radius: 12px;
    background: var(--a-hover);
  }

  .tabs button {
    min-width: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 10px;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: var(--a-muted);
    font-size: 13px;
    white-space: nowrap;
    transition:
      background 180ms ease,
      color 180ms ease;
  }

  .tabs button:focus-visible {
    outline: 2px solid var(--a-accent);
    outline-offset: -2px;
  }

  .tabs button:hover:not(:disabled) {
    background: transparent;
    color: var(--a-fg);
  }

  .tabs button.on,
  .tabs button.on:hover:not(:disabled) {
    background: var(--a-card);
    color: var(--a-accent);
    font-weight: 600;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.18);
  }

  /* Two rows of two when four do not fit. */
  @container (max-width: 560px) {
    .tabs {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .live {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--a-accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--a-accent) 25%, transparent);
  }

  .badge {
    padding: 0 6px;
    border-radius: 8px;
    font-size: 11px;
    background: color-mix(in srgb, var(--a-accent) 18%, transparent);
    color: var(--a-accent);
  }

  h4 {
    margin: 0 0 8px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--a-muted);
  }

  .row-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .folder-actions {
    display: flex;
    gap: 2px;
  }

  .text.muted {
    color: var(--a-muted);
  }

  .choose {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 12px 14px;
    border: 1px dashed var(--a-border);
    border-radius: 12px;
    background: transparent;
    color: var(--a-fg);
    text-align: left;
  }

  .choose:hover {
    border-color: var(--a-accent);
    background: var(--a-hover);
  }

  .choose :global(svg) {
    flex: none;
    color: var(--a-accent);
  }

  .choose span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .choose small {
    color: var(--a-muted);
    font-size: 12px;
  }

  .empty-music {
    margin: 0;
    font-size: 12.5px;
    color: var(--a-muted);
  }

  .count {
    margin-left: auto;
    font-size: 11.5px;
    font-weight: 400;
    color: var(--a-muted);
  }

  .playing-now {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 8px 0 2px;
  }

  .track {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .track-title,
  .track-folder {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .track-title {
    font-size: 13px;
  }

  .track-folder {
    font-size: 11.5px;
    color: var(--a-muted);
  }

  .transport {
    display: flex;
    gap: 2px;
  }

  .transport .icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--a-fg);
  }

  .transport .icon:hover {
    background: var(--a-hover);
  }

  .transport .icon.on {
    color: var(--a-accent);
  }

  .error {
    margin: 6px 0 0;
    font-size: 12px;
    color: #dc2626;
  }

  .row-head h4 {
    margin: 0;
  }

  .modes {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }

  .compact .modes {
    grid-template-columns: repeat(2, 1fr);
  }

  .mode {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 10px;
    border-radius: 12px;
    border: 1px solid var(--a-border);
    background: var(--a-card);
    color: var(--a-fg);
    text-align: left;
    transition:
      transform 200ms var(--ease),
      border-color 200ms ease;
  }

  .mode:hover:not(:disabled) {
    background: var(--a-card);
    transform: translateY(-2px);
  }

  .mode.on {
    border-color: var(--m1);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--m1) 30%, transparent);
  }

  .swatch {
    width: 100%;
    height: 26px;
    border-radius: 8px;
    background: linear-gradient(120deg, var(--m1), var(--m2));
  }

  .mode-name {
    font-weight: 600;
  }

  .mode-hint {
    font-size: 11.5px;
    color: var(--a-muted);
    line-height: 1.3;
  }

  .controls {
    margin-top: 10px;
  }

  .slider {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--a-muted);
    font-size: 12px;
  }

  .slider input {
    flex: 1;
    min-width: 0;
    --range-accent: var(--a-accent);
    --range-track: var(--a-border);
  }

  .slider.labeled {
    display: grid;
    grid-template-columns: 90px 1fr;
    grid-template-areas: 'name input' '. ends';
    row-gap: 0;
    margin: 6px 0;
  }

  .slider.labeled .name {
    grid-area: name;
    color: var(--a-fg);
    font-size: 13px;
  }

  .slider.labeled input {
    grid-area: input;
  }

  .layer-controls .slider.labeled {
    grid-template-columns: 0 1fr;
  }

  .ends {
    grid-area: ends;
    display: flex;
    justify-content: space-between;
    font-size: 10.5px;
    color: var(--a-muted);
  }

  .scenes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }

  .scene-wrap {
    position: relative;
    display: inline-flex;
  }

  .scene {
    padding: 6px 12px;
    border-radius: 16px;
    border: 1px solid var(--a-border);
    background: var(--a-card);
    color: var(--a-fg);
    font-size: 12.5px;
  }

  .scene.on {
    border-color: var(--a-accent);
    color: var(--a-accent);
    font-weight: 600;
  }

  .scene-wrap:has(.remove) .scene {
    padding-right: 28px;
  }

  .remove {
    position: absolute;
    right: 5px;
    top: 50%;
    translate: 0 -50%;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--a-muted);
  }

  .remove:hover:not(:disabled) {
    background: var(--a-hover);
    color: var(--danger);
  }

  .group {
    margin: 10px 0 4px;
    font-size: 12px;
    color: var(--a-muted);
  }

  .layer {
    border-radius: 10px;
    transition: background 200ms ease;
  }

  .layer.on {
    background: color-mix(in srgb, var(--a-accent) 8%, transparent);
    padding-bottom: 6px;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border: none;
    border-radius: 10px;
    background: transparent;
    color: var(--a-fg);
    text-align: left;
  }

  .toggle:hover:not(:disabled) {
    background: var(--a-hover);
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1.5px solid var(--a-muted);
    transition: all 200ms ease;
  }

  .layer.on .dot {
    border-color: var(--a-accent);
    background: var(--a-accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--a-accent) 25%, transparent);
  }

  .layer-controls {
    padding: 0 12px 0 30px;
  }

  .mini {
    width: 24px;
    font-size: 11px;
  }

  .timers {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .timers button {
    padding: 5px 11px;
    border-radius: 14px;
    border: 1px solid var(--a-border);
    background: var(--a-card);
    color: var(--a-fg);
    font-size: 12.5px;
  }

  .timers button.on {
    border-color: var(--a-accent);
    color: var(--a-accent);
    font-weight: 600;
  }

  .text {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 6px;
    border: none;
    background: transparent;
    color: var(--a-accent);
    font-size: 12.5px;
  }

  .text:hover:not(:disabled) {
    background: transparent;
    text-decoration: underline;
  }

  .ambience :global(.row) {
    color: var(--a-fg);
  }

  .ambience :global(.description) {
    color: var(--a-muted);
  }
</style>
