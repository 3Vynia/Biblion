// The ambience as the interface sees it: settings (saved), whether it is
// playing, scenes and the sleep timer. The engine does the sound.

import { api } from '../api'
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

  get scenes(): Scene[] {
    return [...BUILT_IN_SCENES, ...this.value.scenes]
  }

  get active(): Scene | undefined {
    return activeScene(this.value, this.scenes)
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
    clearTimeout(this.#saveTimer)
    const snapshot = $state.snapshot(this.value)
    this.#saveTimer = setTimeout(
      () => api.setAmbienceSettings(snapshot).catch((e) => console.error('could not save ambience', e)),
      SAVE_DELAY_MS,
    )
  }

  async play() {
    await this.load()
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
  }

  toggle() {
    if (this.playing) this.pause()
    else this.play()
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
