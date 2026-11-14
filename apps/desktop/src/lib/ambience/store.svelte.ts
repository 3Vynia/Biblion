// The ambience as the interface sees it: settings (saved), whether it is
// playing, scenes and the sleep timer. The engine does the sound.

import { convertFileSrc } from '@tauri-apps/api/core'
import { api } from '../api'
import type { MusicLibrary, Track } from '../types'
import { Playlist } from './playlist'
import { confirm } from '../dialog.svelte'
import { AmbienceEngine } from './engine'
import {
  BUILT_IN_SCENES,
  DEFAULT_AMBIENCE,
  activeScene,
  anyLayerOn,
  applyScene,
  normalizeAmbience,
  sceneFromMix,
  type AmbienceSettings,
  type LayerId,
  type LayerState,
  type MusicState,
  type Scene,
  type SoundscapeState,
} from './settings'

const SAVE_DELAY_MS = 400
const SLEEP_FADE_SECONDS = 40

class AmbienceStore {
  value = $state<AmbienceSettings>(structuredClone(DEFAULT_AMBIENCE))
  playing = $state(false)
  /** When the sleep timer ends (ms since epoch), or null. */
  timerEnds = $state<number | null>(null)
  #engine = new AmbienceEngine()
  #saveTimer: ReturnType<typeof setTimeout> | undefined
  #sleepTimer: ReturnType<typeof setTimeout> | undefined
  #loaded: Promise<void> | null = null
  /** The music folder and its tracks (null until asked for). */
  musicLibrary = $state<MusicLibrary | null>(null)
  track = $state<Track | null>(null)
  #playlist: Playlist | null = null
  #failures = 0

  constructor() {
    this.#engine.onTrackEnd = (failed) => {
      // A track that cannot be played is skipped, unless none can.
      this.#failures = failed ? this.#failures + 1 : 0
      if (failed && this.#failures >= (this.musicLibrary?.tracks.length ?? 0)) {
        console.warn('none of the music tracks can be played')
        this.track = null
        this.#syncMusic()
        return
      }
      this.nextTrack()
    }
  }

  get scenes(): Scene[] {
    return [...BUILT_IN_SCENES, ...this.value.scenes]
  }

  get active(): Scene | undefined {
    return activeScene(this.value, this.scenes)
  }

  /** Reads the settings again (after restoring a backup). */
  reload(): Promise<void> {
    this.#loaded = null
    return this.load()
  }

  load(): Promise<void> {
    this.#loaded ??= api
      .getAmbienceSettings()
      .then((raw) => {
        this.value = normalizeAmbience(raw)
      })
      .catch((e) => console.error('could not load ambience settings', e))
    return this.#loaded
  }

  #commit(next: AmbienceSettings) {
    this.value = next
    this.#engine.apply($state.snapshot(this.value), this.playing)
    this.#syncMusic()
    clearTimeout(this.#saveTimer)
    const snapshot = $state.snapshot(this.value)
    this.#saveTimer = setTimeout(
      () => api.setAmbienceSettings(snapshot).catch((e) => console.error('could not save ambience', e)),
      SAVE_DELAY_MS,
    )
  }

  async play() {
    await this.load()
    if (this.value.music.on) await this.loadMusic()
    // Nothing chosen yet: start with the soundscape alone.
    if (!anyLayerOn(this.value)) this.value = { ...this.value, soundscape: { ...this.value.soundscape, on: true } }
    this.playing = true
    this.#commit(this.value)
    if (this.value.timerMinutes) this.setTimer(this.value.timerMinutes)
  }

  pause() {
    this.playing = false
    this.#clearTimer()
    this.#engine.apply($state.snapshot(this.value), false)
    this.#syncMusic()
  }

  toggle() {
    if (this.playing) this.pause()
    else this.play()
  }

  // ── The user's music ──

  /** Lists the music folder (once, or again with `force`). */
  async loadMusic(force = false) {
    if (this.musicLibrary && !force) return
    try {
      this.#setLibrary(await api.getMusic())
    } catch (e) {
      console.error('could not list the music folder', e)
    }
  }

  #setLibrary(lib: MusicLibrary) {
    this.musicLibrary = lib
    this.#failures = 0
    this.#playlist = new Playlist(lib.tracks.length, this.value.music.shuffle)
    this.track = this.#current()
  }

  #current(): Track | null {
    const i = this.#playlist?.current
    return i === null || i === undefined ? null : (this.musicLibrary?.tracks[i] ?? null)
  }

  async chooseMusicFolder(path: string) {
    this.#setLibrary(await api.setMusicFolder(path))
    if (this.musicLibrary?.tracks.length) this.setMusic({ on: true })
  }

  async forgetMusicFolder(): Promise<boolean> {
    const ok = await confirm({
      title: 'Stop using this music folder?',
      message: 'Your music files stay where they are; Biblion just stops playing them.',
      confirmLabel: 'Stop using it',
      danger: true,
    })
    if (!ok) return false
    this.#commit({ ...this.value, music: { ...this.value.music, on: false } })
    this.#setLibrary(await api.setMusicFolder(null))
    return true
  }

  setMusic(patch: Partial<MusicState>) {
    const shuffleChanged = patch.shuffle !== undefined && patch.shuffle !== this.value.music.shuffle
    this.#commit({ ...this.value, music: { ...this.value.music, ...patch } })
    if (shuffleChanged && this.musicLibrary) {
      const keep = this.track
      this.#playlist = new Playlist(this.musicLibrary.tracks.length, this.value.music.shuffle)
      if (keep) this.#playlist.jumpTo(keep.index)
    }
    if (patch.on && !this.playing) this.play()
  }

  nextTrack() {
    this.#playlist?.next()
    this.track = this.#current()
    this.#syncMusic()
  }

  prevTrack() {
    this.#playlist?.prev()
    this.track = this.#current()
    this.#syncMusic()
  }

  #syncMusic() {
    const on = this.value.music.on && this.playing && !!this.track
    const url = on && this.track ? convertFileSrc(String(this.track.index), 'music') : null
    this.#engine.music(url, this.value.music.volume, on)
  }

  musicPosition(): number {
    return this.#engine.musicPosition()
  }

  setMaster(master: number) {
    this.#commit({ ...this.value, master })
  }

  setSoundscape(patch: Partial<SoundscapeState>) {
    this.#commit({ ...this.value, soundscape: { ...this.value.soundscape, ...patch } })
    if (patch.on && !this.playing) this.play()
  }

  setLayer(id: LayerId, patch: Partial<LayerState>) {
    this.#commit({ ...this.value, layers: { ...this.value.layers, [id]: { ...this.value.layers[id], ...patch } } })
    if (patch.on && !this.playing) this.play()
  }

  setOptions(patch: Partial<Pick<AmbienceSettings, 'autoStart' | 'pageTurn' | 'pageTurnVolume'>>) {
    this.#commit({ ...this.value, ...patch })
  }

  useScene(scene: Scene) {
    this.#commit(applyScene(this.value, scene))
    if (!this.playing) this.play()
  }

  saveScene(name: string): Scene {
    const scene = sceneFromMix(this.value, name, `user-${Date.now().toString(36)}`)
    this.#commit({ ...this.value, scenes: [...this.value.scenes, scene] })
    return scene
  }

  async deleteScene(scene: Scene): Promise<boolean> {
    const ok = await confirm({
      title: `Delete the scene “${scene.name}”?`,
      message: "Only the saved mix is removed; the sounds stay available. This can't be undone.",
      confirmLabel: 'Delete scene',
      danger: true,
    })
    if (!ok) return false
    this.#commit({ ...this.value, scenes: this.value.scenes.filter((s) => s.id !== scene.id) })
    return true
  }

  /** Sleep timer: fade out over the last 40 s, then stop. 0 turns it off. */
  setTimer(minutes: number) {
    this.#clearTimer()
    this.#commit({ ...this.value, timerMinutes: minutes })
    if (!minutes || !this.playing) return
    const ms = minutes * 60_000
    this.timerEnds = Date.now() + ms
    this.#sleepTimer = setTimeout(() => {
      this.#engine.fadeOut(SLEEP_FADE_SECONDS)
      this.#sleepTimer = setTimeout(() => this.pause(), SLEEP_FADE_SECONDS * 1000)
    }, Math.max(0, ms - SLEEP_FADE_SECONDS * 1000))
  }

  #clearTimer() {
    clearTimeout(this.#sleepTimer)
    this.timerEnds = null
  }

  /** A book was opened: start if the user asked for that. */
  async bookOpened() {
    await this.load()
    if (this.value.autoStart && !this.playing) this.play()
  }

  pageTurned() {
    this.#engine.pageTurn()
  }

  level(): number {
    return this.#engine.level()
  }

  peak(): number {
    return this.#engine.peak()
  }
}

export const ambience = new AmbienceStore()
