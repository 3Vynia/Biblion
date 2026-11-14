// Whether looking books up on Open Library is turned on (off by default).

import { api } from './api'

class OnlineMetadata {
  enabled = $state(false)
  #loaded: Promise<void> | null = null

  load(): Promise<void> {
    this.#loaded ??= api
      .getOnlineMetadata()
      .then((on) => void (this.enabled = on))
      .catch(() => {})
    return this.#loaded
  }

  async set(on: boolean) {
    const before = this.enabled
    this.enabled = on
    try {
      await api.setOnlineMetadata(on)
    } catch (e) {
      this.enabled = before
      throw e
    }
  }
}

export const onlineMetadata = new OnlineMetadata()

