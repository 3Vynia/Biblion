// Reader preferences shared by every reader, loaded once and saved (debounced)
// to settings.json through the backend.

import { api } from '../lib/api'
import { DEFAULT_SETTINGS, normalizeSettings, type ReaderSettings } from './readerSettings'

const SAVE_DELAY_MS = 400

class SettingsStore {
  value = $state<ReaderSettings>({ ...DEFAULT_SETTINGS })
  #loaded: Promise<void> | null = null
  #timer: ReturnType<typeof setTimeout> | undefined

  /** Reads the settings again (after restoring a backup). */
  reload(): Promise<void> {
    this.#loaded = null
    return this.load()
  }

  load(): Promise<void> {
    this.#loaded ??= api
      .getReaderSettings()
      .then((raw) => {
        this.value = normalizeSettings(raw)
      })
      .catch((e) => console.error('could not load reader settings', e))
    return this.#loaded
  }

  update(patch: Partial<ReaderSettings>) {
    this.value = normalizeSettings({ ...this.value, ...patch })
    clearTimeout(this.#timer)
    const snapshot = $state.snapshot(this.value)
    this.#timer = setTimeout(() => {
      api.setReaderSettings(snapshot).catch((e) => console.error('could not save reader settings', e))
    }, SAVE_DELAY_MS)
  }

  reset() {
    this.update({ ...DEFAULT_SETTINGS })
  }
}

export const readerSettings = new SettingsStore()
