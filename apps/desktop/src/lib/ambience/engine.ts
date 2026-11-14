// Web Audio engine for the ambience: a generative soundscape plus optional
// textures, all synthesised live (no recordings), into one limited output.

import { chordNotes, melodyNote, midiToHz, nextChord, scapeParams, type ScapeParams } from './composer'
import { noiseSamples, rng, type NoiseColor } from './noise'
import type { AmbienceSettings, LayerId, LayerState, SoundscapeState } from './settings'

const FADE = 1.2 // seconds for volume changes
const random = rng(Date.now() % 2 ** 31)
const between = (a: number, b: number) => a + random() * (b - a)

/** A stereo noise buffer that loops seamlessly enough (different seeds per side). */
function noiseBuffer(ctx: BaseAudioContext, color: NoiseColor, seconds = 6): AudioBuffer {
  const length = Math.floor(ctx.sampleRate * seconds)
  const buffer = ctx.createBuffer(2, length, ctx.sampleRate)
  for (let ch = 0; ch < 2; ch++) {
    const data = noiseSamples(color, length, Math.floor(random() * 1e9))
    // Short crossfade at the loop point to avoid a click.
    const fade = Math.floor(ctx.sampleRate * 0.05)
    for (let i = 0; i < fade; i++) {
      const t = i / fade
      data[i] = data[i] * t + data[length - fade + i] * (1 - t)
    }
    buffer.copyToChannel(data, ch)
  }
  return buffer
}

function loop(ctx: BaseAudioContext, buffer: AudioBuffer): AudioBufferSourceNode {
  const src = ctx.createBufferSource()
  src.buffer = buffer
  src.loop = true
  src.loopEnd = buffer.duration - 0.05
  src.start(0, random() * buffer.duration)
  return src
}

/** A soft, wide room: stereo noise with an exponential tail. */
function impulse(ctx: BaseAudioContext, seconds = 4.5): AudioBuffer {
  const length = Math.floor(ctx.sampleRate * seconds)
  const buffer = ctx.createBuffer(2, length, ctx.sampleRate)
  for (let ch = 0; ch < 2; ch++) {
    const data = noiseSamples('pink', length, ch + 11)
    for (let i = 0; i < length; i++) data[i] *= Math.pow(1 - i / length, 3.2) * 0.6
    buffer.copyToChannel(data, ch)
  }
  return buffer
}

/** Repeats `fn` at random intervals until stopped. */
class Timers {
  #ids = new Set<ReturnType<typeof setTimeout>>()
  every(min: number, max: number, fn: () => void) {
    const tick = () => {
      fn()
      this.after(between(min, max), tick)
    }
    this.after(between(min, max), tick)
  }
  after(seconds: number, fn: () => void) {
    const id = setTimeout(() => {
      this.#ids.delete(id)
      fn()
    }, seconds * 1000)
    this.#ids.add(id)
  }
  clear() {
    this.#ids.forEach(clearTimeout)
    this.#ids.clear()
  }
}

interface Context {
  ctx: AudioContext
  out: AudioNode
  reverb: AudioNode
  noise: Record<NoiseColor, AudioBuffer>
}

/** Plays one short burst of a noise buffer through a filter, with an envelope. */
function burst(
  c: Context,
  dest: AudioNode,
  opts: { color?: NoiseColor; type: BiquadFilterType; freq: number; q?: number; gain: number; attack: number; decay: number; pan?: number; sweepTo?: number },
) {
  const { ctx } = c
  const t = ctx.currentTime + 0.01
  const src = ctx.createBufferSource()
  src.buffer = c.noise[opts.color ?? 'white']
  const filter = ctx.createBiquadFilter()
  filter.type = opts.type
  filter.frequency.setValueAtTime(opts.freq, t)
  if (opts.sweepTo) filter.frequency.exponentialRampToValueAtTime(opts.sweepTo, t + opts.attack + opts.decay)
  filter.Q.value = opts.q ?? 1
  const env = ctx.createGain()
  env.gain.setValueAtTime(0.0001, t)
  env.gain.exponentialRampToValueAtTime(opts.gain, t + opts.attack)
  env.gain.exponentialRampToValueAtTime(0.0001, t + opts.attack + opts.decay)
  const pan = ctx.createStereoPanner()
  pan.pan.value = opts.pan ?? 0
  src.connect(filter).connect(env).connect(pan).connect(dest)
  src.start(t, random() * 4)
  src.stop(t + opts.attack + opts.decay + 0.05)
}

// ───────────────────────────── Soundscape ─────────────────────────────

class Soundscape {
  #c: Context
  #bus: GainNode
  #padFilter: BiquadFilterNode
  #melodyBus: GainNode
  #reverbSend: GainNode
  #air: AudioBufferSourceNode
  #airGain: GainNode
  #airFilter: BiquadFilterNode
  #timers = new Timers()
  #voices: { stop: (at: number) => void }[] = []
  #chord = 0
  #params: ScapeParams
  #state: SoundscapeState
  #lastActivity = Date.now()

  constructor(c: Context, state: SoundscapeState) {
    this.#c = c
    this.#state = state
    this.#params = scapeParams(state, new Date().getHours())
    const { ctx } = c
    this.#bus = ctx.createGain()
    this.#bus.gain.value = 0.0001
    this.#bus.connect(c.out)
    this.#padFilter = ctx.createBiquadFilter()
    this.#padFilter.type = 'lowpass'
    this.#padFilter.Q.value = 0.4
    this.#padFilter.connect(this.#bus)
    this.#melodyBus = ctx.createGain()
    this.#melodyBus.gain.value = 0.55
    this.#melodyBus.connect(this.#bus)
    this.#reverbSend = ctx.createGain()
    this.#melodyBus.connect(this.#reverbSend)
    this.#padFilter.connect(this.#reverbSend)
    this.#reverbSend.connect(c.reverb)
    // Air: a faint breathing texture under everything.
    this.#airFilter = ctx.createBiquadFilter()
    this.#airFilter.type = 'bandpass'
    this.#airFilter.Q.value = 0.7
    this.#airGain = ctx.createGain()
    this.#airGain.gain.value = 0.02
    this.#air = loop(ctx, c.noise.pink)
    this.#air.connect(this.#airFilter).connect(this.#airGain).connect(this.#bus)
    this.#applyParams(true)
    this.#playChord()
    this.#scheduleMelody()
    this.#timers.every(5, 9, () => {
      const t = ctx.currentTime
      this.#airFilter.frequency.setTargetAtTime(between(500, 1800), t, 3)
      this.#airGain.gain.setTargetAtTime(between(0.01, 0.035), t, 3)
    })
    if (this.#params.pulse) this.#schedulePulse()
    // The hour changes slowly: re-read it every few minutes.
    this.#timers.every(240, 300, () => this.update(this.#state))
  }

  get running() {
    return true
  }

  #applyParams(immediate = false) {
    const t = this.#c.ctx.currentTime
    this.#padFilter.frequency.setTargetAtTime(this.#params.padCutoff, t, immediate ? 0.01 : 4)
    this.#reverbSend.gain.setTargetAtTime(this.#params.reverb, t, immediate ? 0.01 : 2)
    this.#bus.gain.setTargetAtTime(Math.max(0.0001, this.#state.volume * 0.5), t, immediate ? 1.5 : FADE)
  }

  update(state: SoundscapeState) {
    const modeChanged = state.mode !== this.#state.mode
    this.#state = state
    this.#params = scapeParams(state, new Date().getHours())
    this.#applyParams()
    if (modeChanged) this.#playChord()
  }

  /** A page turned: answer with one soft note in the harmony. */
  react() {
    this.#lastActivity = Date.now()
    if (this.#state.reactToReading) this.#bell(0.45)
  }

  #playChord() {
    const { ctx } = this.#c
    const now = ctx.currentTime
    this.#voices.forEach((v) => v.stop(now))
    this.#chord = nextChord(this.#params.mode, this.#chord, random)
    const notes = chordNotes(this.#params.mode, this.#chord)
    this.#voices = notes.map((m, i) => this.#padVoice(midiToHz(m), i === 0 ? 0.22 : 0.13, (i - 1.5) * 0.35))
    this.#timers.after(this.#params.chordSeconds * between(0.85, 1.15), () => this.#playChord())
  }

  #padVoice(freq: number, level: number, pan: number) {
    const { ctx } = this.#c
    const t = ctx.currentTime
    const env = ctx.createGain()
    env.gain.setValueAtTime(0.0001, t)
    env.gain.linearRampToValueAtTime(level, t + between(5, 8))
    const panner = ctx.createStereoPanner()
    panner.pan.value = pan
    env.connect(panner).connect(this.#padFilter)
    const oscs = (['sine', 'triangle'] as OscillatorType[]).map((type, k) => {
      const o = ctx.createOscillator()
      o.type = type
      o.frequency.value = freq * (k ? 1.0035 : 1)
      const g = ctx.createGain()
      g.gain.value = k ? 0.35 : 1
      // Slow vibrato-free drift keeps the pad alive.
      const lfo = ctx.createOscillator()
      lfo.frequency.value = between(0.03, 0.08)
      const depth = ctx.createGain()
      depth.gain.value = freq * 0.0015
      lfo.connect(depth).connect(o.frequency)
      lfo.start(t)
      o.connect(g).connect(env)
      o.start(t)
      return [o, lfo]
    })
    return {
      stop: (at: number) => {
        env.gain.cancelScheduledValues(at)
        env.gain.setValueAtTime(env.gain.value, at)
        env.gain.linearRampToValueAtTime(0.0001, at + 8)
        oscs.flat().forEach((o) => o.stop(at + 8.2))
      },
    }
  }

  #scheduleMelody() {
    const step = () => {
      // Quiet down after a few minutes without page turns.
      const idle = (Date.now() - this.#lastActivity) / 60000
      const damp = this.#state.reactToReading && idle > 3 ? 0.4 : 1
      const perSecond = (this.#params.notesPerMinute / 60) * damp
      if (random() < perSecond * 0.5) this.#bell(between(0.5, 0.9))
      this.#timers.after(0.5, step)
    }
    this.#timers.after(between(1, 3), step)
  }

  #bell(strength: number) {
    const { ctx } = this.#c
    const t = ctx.currentTime + 0.02
    const freq = midiToHz(melodyNote(this.#params.mode, this.#chord, random, this.#state.warmth))
    const decay = between(3, 6) * (1.3 - this.#state.energy * 0.5)
    const env = ctx.createGain()
    env.gain.setValueAtTime(0.0001, t)
    env.gain.exponentialRampToValueAtTime(0.09 * strength, t + 0.015)
    env.gain.exponentialRampToValueAtTime(0.0001, t + decay)
    const tone = ctx.createBiquadFilter()
    tone.type = 'lowpass'
    tone.frequency.value = 900 + this.#state.warmth * 3500
    const pan = ctx.createStereoPanner()
    pan.pan.value = between(-0.6, 0.6)
    env.connect(tone).connect(pan).connect(this.#melodyBus)
    for (const [ratio, level] of [[1, 1], [2, 0.28], [3.01, 0.08]] as const) {
      const o = ctx.createOscillator()
      o.type = 'sine'
      o.frequency.value = freq * ratio
      const g = ctx.createGain()
      g.gain.value = level
      o.connect(g).connect(env)
      o.start(t)
      o.stop(t + decay + 0.1)
    }
  }

  #schedulePulse() {
    const beat = () => {
      if (!this.#params.pulse) return
      const { ctx } = this.#c
      const t = ctx.currentTime + 0.02
      const o = ctx.createOscillator()
      o.frequency.value = midiToHz(this.#params.mode.root - 12)
      const env = ctx.createGain()
      env.gain.setValueAtTime(0.0001, t)
      env.gain.exponentialRampToValueAtTime(0.07 * (0.5 + this.#state.energy), t + 0.04)
      env.gain.exponentialRampToValueAtTime(0.0001, t + 0.6)
      o.connect(env).connect(this.#bus)
      o.start(t)
      o.stop(t + 0.7)
      this.#timers.after(60 / this.#params.pulse, beat)
    }
    this.#timers.after(2, beat)
  }

  stop() {
    const { ctx } = this.#c
    const t = ctx.currentTime
    this.#timers.clear()
    this.#bus.gain.cancelScheduledValues(t)
    this.#bus.gain.setTargetAtTime(0.0001, t, 0.8)
    this.#voices.forEach((v) => v.stop(t))
    setTimeout(() => {
      this.#air.stop()
      this.#bus.disconnect()
      this.#reverbSend.disconnect()
    }, 9000)
  }
}

// ───────────────────────────── Textures ─────────────────────────────

/** One optional sound layer; `tone` (0 – 1) sets its character. */
class Texture {
  #c: Context
  #gain: GainNode
  #timers = new Timers()
  #sources: AudioScheduledSourceNode[] = []
  #tone: number
  #retone: ((tone: number) => void) | null = null

  constructor(c: Context, id: LayerId, state: LayerState) {
    this.#c = c
    this.#tone = state.tone
    this.#gain = c.ctx.createGain()
    this.#gain.gain.value = 0.0001
    this.#gain.connect(c.out)
    this.#build(id)
    this.update(state)
  }

  update(state: LayerState) {
    this.#tone = state.tone
    this.#retone?.(state.tone)
    this.#gain.gain.setTargetAtTime(Math.max(0.0001, state.volume), this.#c.ctx.currentTime, FADE)
  }

  #noise(color: NoiseColor, filters: [BiquadFilterType, number, number?][], level: number): { gain: GainNode; filters: BiquadFilterNode[] } {
    const { ctx } = this.#c
    const src = loop(ctx, this.#c.noise[color])
    this.#sources.push(src)
    let node: AudioNode = src
    const nodes = filters.map(([type, freq, q]) => {
      const f = ctx.createBiquadFilter()
      f.type = type
      f.frequency.value = freq
      f.Q.value = q ?? 0.7
      node.connect(f)
      node = f
      return f
    })
    const gain = ctx.createGain()
    gain.gain.value = level
    node.connect(gain).connect(this.#gain)
    return { gain, filters: nodes }
  }

  #build(id: LayerId) {
    const c = this.#c
    const t = () => this.#tone
    switch (id) {
      case 'brown':
      case 'pink':
      case 'white':
        this.#noise(id, [['lowpass', id === 'white' ? 9000 : 6000]], id === 'white' ? 0.18 : 0.45)
        break
      case 'rain': {
        const bed = this.#noise('pink', [['highpass', 400], ['lowpass', 4000]], 0.35)
        this.#retone = (tone) => {
          bed.filters[1].frequency.setTargetAtTime(2500 + tone * 5500, c.ctx.currentTime, 1)
          bed.gain.gain.setTargetAtTime(0.18 + tone * 0.35, c.ctx.currentTime, 1)
        }
        // Drops on the window.
        this.#timers.every(0.03, 0.25, () => {
          if (random() > 0.25 + t() * 0.7) return
          burst(c, this.#gain, { type: 'bandpass', freq: between(2500, 7000), q: 4, gain: between(0.05, 0.16), attack: 0.002, decay: between(0.02, 0.06), pan: between(-0.9, 0.9) })
        })
        break
      }
      case 'thunder':
        this.#timers.every(25, 90, () => {
          if (random() > 0.35 + t() * 0.6) return
          burst(c, this.#gain, { color: 'brown', type: 'lowpass', freq: between(120, 320), sweepTo: 60, q: 0.5, gain: between(0.5, 0.95), attack: between(0.3, 1.2), decay: between(4, 9), pan: between(-0.5, 0.5) })
        })
        break
      case 'wind': {
        const air = this.#noise('pink', [['bandpass', 500, 0.9]], 0.35)
        const move = () => {
          const now = c.ctx.currentTime
          air.filters[0].frequency.setTargetAtTime(between(220, 500) + t() * 700, now, 3)
          air.gain.gain.setTargetAtTime(between(0.12, 0.3) + t() * 0.35, now, 2.5)
        }
        move()
        this.#timers.every(3, 7, move)
        break
      }
      case 'waves': {
        const sea = this.#noise('pink', [['lowpass', 600]], 0.2)
        const swell = () => {
          const now = c.ctx.currentTime
          const period = between(7, 11) - t() * 2
          sea.filters[0].frequency.setTargetAtTime(900 + t() * 1500, now, period * 0.25)
          sea.gain.gain.setTargetAtTime(0.35 + t() * 0.35, now, period * 0.2)
          setTimeout(() => {
            const later = c.ctx.currentTime
            sea.filters[0].frequency.setTargetAtTime(350, later, period * 0.3)
            sea.gain.gain.setTargetAtTime(0.08, later, period * 0.3)
          }, period * 450)
          this.#timers.after(period, swell)
        }
        swell()
        break
      }
      case 'night': {
        const crickets = [0, 1, 2, 3, 4, 5].map(() => ({ freq: between(4200, 5400), pan: between(-0.9, 0.9) }))
        this.#timers.every(0.35, 1.1, () => {
          const active = 1 + Math.round(t() * 5)
          const cricket = crickets[Math.floor(random() * active)]
          const { ctx } = c
          const start = ctx.currentTime + 0.02
          const o = ctx.createOscillator()
          o.frequency.value = cricket.freq
          const env = ctx.createGain()
          env.gain.value = 0
          const pan = ctx.createStereoPanner()
          pan.pan.value = cricket.pan
          o.connect(env).connect(pan).connect(this.#gain)
          const pulses = 2 + Math.floor(random() * 3)
          for (let i = 0; i < pulses; i++) {
            const at = start + i * 0.065
            env.gain.setValueAtTime(0, at)
            env.gain.linearRampToValueAtTime(0.035, at + 0.01)
            env.gain.linearRampToValueAtTime(0, at + 0.04)
          }
          o.start(start)
          o.stop(start + pulses * 0.065 + 0.05)
        })
        break
      }
      case 'fire': {
        const roar = this.#noise('brown', [['lowpass', 500]], 0.3)
        this.#retone = (tone) => {
          roar.filters[0].frequency.setTargetAtTime(350 + tone * 600, c.ctx.currentTime, 1)
          roar.gain.gain.setTargetAtTime(0.18 + tone * 0.3, c.ctx.currentTime, 1)
        }
        this.#timers.every(0.04, 0.35, () => {
          if (random() > 0.3 + t() * 0.6) return
          const pop = random() < 0.08
          burst(c, this.#gain, {
            type: pop ? 'bandpass' : 'highpass',
            freq: pop ? between(500, 900) : between(1500, 3500),
            q: pop ? 2 : 0.7,
            gain: pop ? between(0.15, 0.3) : between(0.04, 0.14),
            attack: 0.001,
            decay: pop ? between(0.04, 0.09) : between(0.004, 0.025),
            pan: between(-0.5, 0.5),
          })
        })
        break
      }
      case 'clock': {
        let tock = false
        this.#timers.every(1, 1, () => {
          tock = !tock
          burst(c, this.#gain, { type: 'bandpass', freq: tock ? 1500 : 2100, q: 6, gain: 0.25, attack: 0.001, decay: 0.03, pan: tock ? -0.15 : 0.15 })
        })
        break
      }
    }
  }

  stop() {
    const t = this.#c.ctx.currentTime
    this.#timers.clear()
    this.#gain.gain.cancelScheduledValues(t)
    this.#gain.gain.setTargetAtTime(0.0001, t, 0.6)
    setTimeout(() => {
      this.#sources.forEach((s) => s.stop())
      this.#gain.disconnect()
    }, 3000)
  }
}

// ───────────────────────────── Engine ─────────────────────────────

/**
 * The user's music: an <audio> element fed into the mix, so the master
 * volume, the warmth filter, the limiter and the sleep timer apply to it.
 */
class Music {
  el = new Audio()
  #gain: GainNode
  #c: Context

  constructor(c: Context, onEnded: () => void, onError: () => void) {
    this.#c = c
    this.el.crossOrigin = 'anonymous' // custom scheme: needed to hear it through Web Audio
    this.el.preload = 'auto'
    this.el.addEventListener('ended', onEnded)
    this.el.addEventListener('error', onError)
    this.#gain = c.ctx.createGain()
    this.#gain.gain.value = 0.0001
    c.ctx.createMediaElementSource(this.el).connect(this.#gain).connect(c.out)
  }

  play(url: string, volume: number) {
    const now = this.#c.ctx.currentTime
    if (this.el.src !== url) {
      this.el.src = url
      this.#gain.gain.cancelScheduledValues(now)
      this.#gain.gain.setValueAtTime(0.0001, now) // each track fades in
    }
    this.#gain.gain.setTargetAtTime(Math.max(0.0001, volume), now, 0.6)
    if (this.el.paused) this.el.play().catch(() => {})
  }

  pause() {
    this.#gain.gain.setTargetAtTime(0.0001, this.#c.ctx.currentTime, 0.3)
    setTimeout(() => this.el.pause(), 900)
  }

  get position(): number {
    return this.el.duration ? this.el.currentTime / this.el.duration : 0
  }
}

export class AmbienceEngine {
  #music: Music | null = null
  /** Called when a track ends, or cannot be played (to move on). */
  onTrackEnd: ((failed: boolean) => void) | null = null

  #c: Context | null = null
  #master!: GainNode
  #analyser!: AnalyserNode
  #soundscape: Soundscape | null = null
  #textures = new Map<LayerId, Texture>()
  #settings: AmbienceSettings | null = null
  #levels = new Float32Array(1024)

  get running() {
    return !!this.#c && this.#c.ctx.state === 'running'
  }

  #ensure(): Context {
    if (this.#c) return this.#c
    const ctx = new AudioContext({ latencyHint: 'playback' })
    this.#master = ctx.createGain()
    this.#master.gain.value = 0.0001
    // Warmth: roll off harsh highs, then a limiter so nothing ever clips.
    const warm = ctx.createBiquadFilter()
    warm.type = 'highshelf'
    warm.frequency.value = 6000
    warm.gain.value = -4
    const limiter = ctx.createDynamicsCompressor()
    limiter.threshold.value = -10
    limiter.knee.value = 8
    limiter.ratio.value = 12
    limiter.attack.value = 0.005
    limiter.release.value = 0.3
    this.#analyser = ctx.createAnalyser()
    this.#analyser.fftSize = 2048
    this.#master.connect(warm).connect(limiter).connect(this.#analyser).connect(ctx.destination)
    const reverb = ctx.createConvolver()
    reverb.buffer = impulse(ctx)
    const reverbOut = ctx.createGain()
    reverbOut.gain.value = 0.7
    reverb.connect(reverbOut).connect(this.#master)
    this.#c = {
      ctx,
      out: this.#master,
      reverb,
      noise: { white: noiseBuffer(ctx, 'white'), pink: noiseBuffer(ctx, 'pink'), brown: noiseBuffer(ctx, 'brown') },
    }
    return this.#c
  }

  /** Brings the sound in line with the settings (start, stop, levels). */
  async apply(s: AmbienceSettings, playing: boolean) {
    this.#settings = s
    if (!playing) {
      if (this.#c) {
        this.#master.gain.setTargetAtTime(0.0001, this.#c.ctx.currentTime, 0.8)
        this.#soundscape?.stop()
        this.#soundscape = null
        this.#textures.forEach((t) => t.stop())
        this.#textures.clear()
      }
      return
    }
    const c = this.#ensure()
    if (c.ctx.state !== 'running') await c.ctx.resume().catch(() => {})
    this.#master.gain.setTargetAtTime(Math.max(0.0001, s.master), c.ctx.currentTime, FADE)

    if (s.soundscape.on) {
      if (this.#soundscape) this.#soundscape.update(s.soundscape)
      else this.#soundscape = new Soundscape(c, s.soundscape)
    } else if (this.#soundscape) {
      this.#soundscape.stop()
      this.#soundscape = null
    }

    for (const [id, layer] of Object.entries(s.layers) as [LayerId, LayerState][]) {
      const current = this.#textures.get(id)
      if (layer.on && current) current.update(layer)
      else if (layer.on) this.#textures.set(id, new Texture(c, id, layer))
      else if (current) {
        current.stop()
        this.#textures.delete(id)
      }
    }
  }

  /** Plays a track of the user's music (null: no music). */
  async music(url: string | null, volume: number, playing: boolean) {
    if (!url || !playing) {
      this.#music?.pause()
      return
    }
    const c = this.#ensure()
    if (c.ctx.state !== 'running') await c.ctx.resume().catch(() => {})
    this.#music ??= new Music(
      c,
      () => this.onTrackEnd?.(false),
      () => this.onTrackEnd?.(true),
    )
    this.#music.play(url, volume)
  }

  /** How far into the current track, 0 – 1. */
  musicPosition(): number {
    return this.#music?.position ?? 0
  }

  /** Fades everything out over `seconds` (the sleep timer). */
  fadeOut(seconds: number) {
    if (this.#c) this.#master.gain.setTargetAtTime(0.0001, this.#c.ctx.currentTime, seconds / 4)
  }

  /** A page turned: a paper sound (if wanted) and the soundscape answers. */
  pageTurn() {
    const s = this.#settings
    if (!s) return
    if (s.pageTurn) {
      const c = this.#ensure()
      if (c.ctx.state !== 'running') c.ctx.resume().catch(() => {})
      const v = s.pageTurnVolume * 0.6
      burst(c, c.ctx.destination, { color: 'pink', type: 'bandpass', freq: 900, sweepTo: 3200, q: 0.8, gain: 0.25 * v + 0.0002, attack: 0.08, decay: 0.28, pan: 0.2 })
    }
    this.#soundscape?.react()
  }

  /** Current output loudness, 0 – 1, for visuals and tests. */
  level(): number {
    if (!this.#c) return 0
    this.#analyser.getFloatTimeDomainData(this.#levels)
    let sum = 0
    for (const v of this.#levels) sum += v * v
    return Math.min(1, Math.sqrt(sum / this.#levels.length) * 4)
  }

  /** Peak sample of the output, to check nothing clips. */
  peak(): number {
    if (!this.#c) return 0
    this.#analyser.getFloatTimeDomainData(this.#levels)
    let max = 0
    for (const v of this.#levels) max = Math.max(max, Math.abs(v))
    return max
  }
}
