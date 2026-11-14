<script lang="ts">
  // The read-aloud player at the bottom of the reader. Opening it starts
  // reading from the current page; the first time, the voice is downloaded.
  import { onMount } from 'svelte'
  import Icon from '../components/Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { confirm } from '../lib/dialog.svelte'
  import { formatBytes } from '../lib/format'
  import { rise } from '../lib/motion'
  import { pickVoice, readAloud, voiceLang, type SpeechSource } from '../lib/readAloud.svelte'
  import type { DownloadProgress, TtsStatus } from '../lib/types'
  import { VOICE_SPEED } from './readerSettings'
  import { readerSettings } from './settingsStore.svelte'

  interface Props {
    /** The book's language, to pick a voice that speaks it. */
    lang: string | null
    source: () => SpeechSource
    onclose: () => void
  }

  let { lang, source, onclose }: Props = $props()

  let status = $state<TtsStatus | null>(null)
  let progress = $state<DownloadProgress | null>(null)
  let problem = $state<string | null>(null)

  const voiceKey = $derived(voiceLang(lang) ?? 'es')
  const voices = $derived(status?.voices ?? [])
  const LANG_NAMES: Record<string, string> = { es: 'Spanish', 'en-us': 'English (US)', 'en-gb': 'English (UK)' }

  async function begin() {
    problem = null
    try {
      status = await api.ttsStatus()
      if (!status.espeak) {
        problem = 'Reading aloud needs espeak-ng. Install it with your system’s package manager (it is called “espeak-ng”), then try again.'
        return
      }
      if (!status.installed) {
        const ok = await confirm({
          title: 'Download the voice?',
          message: `Biblion reads aloud with Kokoro, a natural voice that runs on this computer. It needs a one-time download of ${formatBytes(status.download_bytes)} from GitHub. Nothing you read is ever sent anywhere.`,
          confirmLabel: 'Download',
        })
        if (!ok) return onclose()
        progress = { done: 0, total: status.download_bytes }
        await api.ttsInstall((p) => (progress = p))
        progress = null
        status = await api.ttsStatus()
      }
      const voice = pickVoice(status.voices, voiceKey, readerSettings.value.voices)
      await readAloud.start(source(), voice, readerSettings.value.voiceSpeed, status.sample_rate)
    } catch (e) {
      progress = null
      problem = errorMessage(e)
    }
  }

  function setVoice(id: string) {
    readerSettings.update({ voices: { ...readerSettings.value.voices, [voiceKey]: id } })
    void readAloud.change(id, readAloud.speed)
  }

  function setSpeed(dir: 1 | -1) {
    const next = Math.round(Math.min(VOICE_SPEED.max, Math.max(VOICE_SPEED.min, readAloud.speed + dir * VOICE_SPEED.step)) * 100) / 100
    readerSettings.update({ voiceSpeed: next })
    void readAloud.change(readAloud.voice, next)
  }

  function close() {
    readAloud.stop()
    onclose()
  }

  onMount(() => {
    void begin()
    return () => readAloud.stop()
  })

  const busy = $derived(readAloud.status === 'starting')
  const error = $derived(problem ?? readAloud.error)
</script>

<div class="read-aloud" role="region" aria-label="Read aloud" in:rise={{ y: 14, duration: 240 }}>
  {#if progress}
    <div class="download">
      <Icon name="headphones" size={18} />
      <div class="grow">
        <span>Downloading the voice… {formatBytes(progress.done)} of {formatBytes(progress.total)}</span>
        <div class="bar"><div style:width="{(progress.done / Math.max(1, progress.total)) * 100}%"></div></div>
      </div>
    </div>
  {:else if error}
    <div class="download">
      <Icon name="headphones" size={18} />
      <p class="grow problem">{error}</p>
      <button class="text" onclick={begin}>Try again</button>
      <button class="icon" onclick={close} aria-label="Close read aloud"><Icon name="close" size={16} /></button>
    </div>
  {:else}
    <div class="controls">
      <button class="icon" onclick={() => readAloud.skip(-1)} disabled={!readAloud.active || busy} aria-label="Previous sentence" title="Previous sentence"
        ><Icon name="skip-back" size={16} /></button
      >
      <button
        class="play"
        class:busy
        onclick={() => readAloud.toggle()}
        disabled={busy}
        aria-label={readAloud.status === 'playing' ? 'Pause reading' : 'Continue reading'}
      >
        {#if busy}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name={readAloud.status === 'playing' ? 'pause' : 'play'} size={18} filled />{/if}
      </button>
      <button class="icon" onclick={() => readAloud.skip(1)} disabled={!readAloud.active || busy} aria-label="Next sentence" title="Next sentence"
        ><Icon name="skip-forward" size={16} /></button
      >
    </div>
    <p class="sentence" aria-live="off">{busy ? 'Getting the voice ready…' : readAloud.sentence}</p>
    <div class="options">
      <label class="voice">
        <span class="sr-only">Voice</span>
        <select value={readAloud.voice} onchange={(e) => setVoice(e.currentTarget.value)} aria-label="Voice">
          {#each Object.entries(LANG_NAMES) as [key, label] (key)}
            <optgroup {label}>
              {#each voices.filter((v) => v.lang === key) as v (v.id)}<option value={v.id}>{v.name}</option>{/each}
            </optgroup>
          {/each}
        </select>
      </label>
      <div class="speed" role="group" aria-label="Reading pace">
        <button class="icon small" onclick={() => setSpeed(-1)} disabled={readAloud.speed <= VOICE_SPEED.min} aria-label="Slower">−</button>
        <span>{readAloud.speed.toFixed(2).replace(/0$/, '')}×</span>
        <button class="icon small" onclick={() => setSpeed(1)} disabled={readAloud.speed >= VOICE_SPEED.max} aria-label="Faster">+</button>
      </div>
      <button class="icon" onclick={close} aria-label="Stop reading aloud" title="Stop (L)"><Icon name="close" size={16} /></button>
    </div>
  {/if}
</div>

<style>
  .read-aloud {
    position: absolute;
    left: 50%;
    bottom: 64px;
    z-index: 6;
    translate: -50% 0;
    display: flex;
    align-items: center;
    gap: 14px;
    width: min(760px, calc(100% - 32px));
    padding: 10px 12px 10px 10px;
    border-radius: 18px;
    border: 1px solid var(--r-border);
    background: color-mix(in srgb, var(--r-chrome) 92%, transparent);
    color: var(--r-fg);
    box-shadow: 0 14px 40px rgb(0 0 0 / 0.22);
    backdrop-filter: blur(14px);
  }

  .controls,
  .options,
  .speed,
  .download {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .download {
    flex: 1;
    gap: 12px;
    padding: 2px 4px;
    color: var(--r-muted);
  }

  .grow {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    font-size: 13px;
    color: var(--r-fg);
  }

  .problem {
    line-height: 1.4;
  }

  .bar {
    height: 5px;
    border-radius: 3px;
    background: var(--r-border);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--r-link);
    transition: width 200ms linear;
  }

  button.icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: none;
    border-radius: 10px;
    background: transparent;
    color: var(--r-fg);
  }

  button.icon:hover:not(:disabled) {
    background: var(--r-hover);
  }

  button.icon.small {
    width: 26px;
    height: 26px;
    font-size: 16px;
  }

  .play {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: var(--r-link);
    color: var(--r-bg);
    box-shadow: 0 4px 14px color-mix(in srgb, var(--r-link) 40%, transparent);
    transition: transform 200ms var(--ease);
  }

  .play:hover:not(:disabled) {
    background: var(--r-link);
    transform: scale(1.06);
  }

  .spinner {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-right-color: transparent;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(1turn);
    }
  }

  .sentence {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-family: var(--serif);
    font-size: 13.5px;
    font-style: italic;
    color: var(--r-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .voice select {
    max-width: 150px;
    padding: 5px 8px;
    border-radius: 9px;
    border: 1px solid var(--r-border);
    background: transparent;
    color: var(--r-fg);
    font-size: 12.5px;
  }

  .speed span {
    min-width: 42px;
    text-align: center;
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }

  .text {
    padding: 4px 8px;
    border: none;
    background: none;
    color: var(--r-link);
    font-size: 12.5px;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }

  @media (max-width: 640px) {
    .sentence {
      display: none;
    }
  }
</style>
