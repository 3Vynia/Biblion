// Scan state shared by the scan panel and the header. Lives outside any
// component so a running scan survives closing the panel.

import { api, errorMessage, isCancelled } from './api'
import type { ScanReport } from './types'

export type ScanPhase = 'idle' | 'walking' | 'processing' | 'finished' | 'cancelled' | 'failed'

class ScanState {
  phase = $state<ScanPhase>('idle')
  found = $state(0)
  done = $state(0)
  total = $state(0)
  report = $state<ScanReport | null>(null)
  error = $state<string | null>(null)

  get running() {
    return this.phase === 'walking' || this.phase === 'processing'
  }

  /** 0..1, or null while the total is unknown. */
  get fraction(): number | null {
    if (this.phase !== 'processing') return null
    return this.total > 0 ? this.done / this.total : 1
  }

  /** Called while processing so the grid can show books as they arrive. */
  onProgress: (() => void) | null = null
  /** Called once a scan ends (successfully or not). */
  onFinished: (() => void) | null = null

  async start(force = false) {
    if (this.running) return
    this.phase = 'walking'
    this.found = this.done = this.total = 0
    this.report = null
    this.error = null
    try {
      const report = await api.scan(force, (e) => {
        switch (e.kind) {
          case 'walking':
            this.found = e.found
            break
          case 'processing':
            this.phase = 'processing'
            this.done = e.done
            this.total = e.total
            this.onProgress?.()
            break
          case 'finished':
            this.report = e.report
            break
        }
      })
      this.report = report
      this.phase = 'finished'
    } catch (e) {
      if (isCancelled(e)) {
        this.phase = 'cancelled'
      } else {
        this.phase = 'failed'
        this.error = errorMessage(e)
      }
    } finally {
      this.onFinished?.()
    }
  }

  async cancel() {
    if (this.running) await api.cancelScan()
  }
}

export const scan = new ScanState()
