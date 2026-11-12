// Musical decisions for the generative soundscape, separate from audio so
// they can be tested: scales per mode, chord movement, note choice, and how
// energy, warmth and the time of day shape the sound.

import type { Mode, SoundscapeState } from './settings'

export const midiToHz = (m: number) => 440 * 2 ** ((m - 69) / 12)

interface ModeDef {
  /** MIDI note of the tonal centre. */
  root: number
  /** Scale degrees (semitones from the root) used for melody. */
  scale: number[]
  /** Chords as scale-degree triads/sevenths relative to the root. */
  chords: number[][]
  /** Seconds a chord lasts at medium pace. */
  chordSeconds: number
  /** Melody notes per minute at energy 0.5. */
  notesPerMinute: number
  /** Soft heartbeat-like pulse (beats per minute), or null. */
  pulse: number | null
  /** Shift of the melody register in semitones (whole octaves, to stay in key). */
  register: number
}

// Pentatonic-leaning scales and open chords: no tension, nothing to follow.
const MODES: Record<Mode, ModeDef> = {
  read: {
    root: 48, // C3
    scale: [0, 2, 4, 7, 9, 11, 14, 16],
    chords: [[0, 7, 16, 23], [9, 16, 19, 26], [5, 12, 16, 21], [2, 9, 14, 21], [7, 14, 19, 26]],
    chordSeconds: 28,
    notesPerMinute: 9,
    pulse: null,
    register: 24,
  },
  focus: {
    root: 50, // D3, dorian colour
    scale: [0, 2, 3, 7, 9, 10, 14],
    chords: [[0, 7, 14, 15], [-2, 5, 14, 17], [3, 10, 14, 19], [5, 12, 15, 21]],
    chordSeconds: 24,
    notesPerMinute: 12,
    pulse: 62,
    register: 24,
  },
  relax: {
    root: 53, // F3
    scale: [0, 2, 4, 7, 9, 12, 14],
    chords: [[0, 7, 14, 16], [-3, 4, 12, 19], [5, 12, 16, 21], [-5, 7, 14, 19]],
    chordSeconds: 36,
    notesPerMinute: 6,
    pulse: null,
    register: 12,
  },
  sleep: {
    root: 45, // A2
    scale: [0, 3, 5, 7, 10, 12],
    chords: [[0, 7, 12, 15], [-4, 3, 8, 12], [-2, 5, 10, 14], [5, 12, 15, 19]],
    chordSeconds: 44,
    notesPerMinute: 3,
    pulse: null,
    register: 12,
  },
}

export interface TimeShape {
  /** 0 – 1: how bright the sound is at this hour. */
  brightness: number
  /** 0 – 1: how lively. */
  pace: number
}

/** Brighter, livelier mornings; darker, slower nights. */
export function timeOfDay(hour: number): TimeShape {
  const h = ((hour % 24) + 24) % 24
  if (h >= 6 && h < 11) return { brightness: 0.75, pace: 0.7 }
  if (h >= 11 && h < 17) return { brightness: 0.65, pace: 0.6 }
  if (h >= 17 && h < 21) return { brightness: 0.45, pace: 0.45 }
  return { brightness: 0.25, pace: 0.3 }
}

export interface ScapeParams {
  mode: ModeDef
  /** Low-pass cutoff for pads, Hz. */
  padCutoff: number
  /** Melody notes per minute. */
  notesPerMinute: number
  chordSeconds: number
  /** Reverb send, 0 – 1. */
  reverb: number
  pulse: number | null
}

export function scapeParams(s: Pick<SoundscapeState, 'mode' | 'energy' | 'warmth' | 'adaptToTime'>, hour: number): ScapeParams {
  const mode = MODES[s.mode]
  const t = s.adaptToTime ? timeOfDay(hour) : { brightness: 0.55, pace: 0.55 }
  const brightness = (s.warmth * 0.7 + t.brightness * 0.3)
  const pace = s.energy * 0.7 + t.pace * 0.3
  return {
    mode,
    padCutoff: Math.round(350 + brightness ** 1.6 * 3200),
    notesPerMinute: mode.notesPerMinute * (0.25 + pace * 1.5),
    chordSeconds: mode.chordSeconds * (1.35 - pace * 0.6),
    reverb: 0.35 + (1 - pace) * 0.35,
    pulse: mode.pulse,
  }
}

/** Next chord: never the same one, preferring small moves (shared tones). */
export function nextChord(mode: ModeDef, current: number, random: () => number): number {
  const n = mode.chords.length
  if (n < 2) return 0
  const candidates = [...Array(n).keys()].filter((i) => i !== current)
  const shared = (i: number) => mode.chords[i].filter((x) => mode.chords[current]?.some((y) => (x - y) % 12 === 0)).length
  const weights = candidates.map((i) => 1 + shared(i) * 1.5)
  let pick = random() * weights.reduce((a, b) => a + b, 0)
  for (let k = 0; k < candidates.length; k++) {
    pick -= weights[k]
    if (pick <= 0) return candidates[k]
  }
  return candidates[candidates.length - 1]
}

/** MIDI notes of a chord, voiced in the pad register. */
export const chordNotes = (mode: ModeDef, index: number) => mode.chords[index].map((d) => mode.root + d)

/** A melody note: usually a chord tone, sometimes another scale tone, in the upper register. */
export function melodyNote(mode: ModeDef, chord: number, random: () => number, warmth: number): number {
  const fromChord = random() < 0.7
  const pool = fromChord ? mode.chords[chord] : mode.scale
  const degree = pool[Math.floor(random() * pool.length)]
  const octave = random() < 0.25 + warmth * 0.3 ? 12 : 0
  let note = mode.root + mode.register + (((degree % 12) + 12) % 12) + octave
  while (note > 88) note -= 12
  return note
}

export { MODES as MODE_DEFS }
