import { describe, expect, it } from 'vitest'
import { MODE_DEFS, chordNotes, melodyNote, midiToHz, nextChord, scapeParams, timeOfDay } from './composer'
import { rng } from './noise'

const base = { mode: 'read' as const, energy: 0.5, warmth: 0.5, adaptToTime: false }

describe('composer', () => {
  it('converts MIDI to Hz', () => {
    expect(midiToHz(69)).toBe(440)
    expect(midiToHz(57)).toBeCloseTo(220)
  })

  it('makes nights darker and slower than mornings', () => {
    expect(timeOfDay(8).brightness).toBeGreaterThan(timeOfDay(23).brightness)
    expect(timeOfDay(8).pace).toBeGreaterThan(timeOfDay(2).pace)
    const morning = scapeParams({ ...base, adaptToTime: true }, 9)
    const night = scapeParams({ ...base, adaptToTime: true }, 23)
    expect(morning.padCutoff).toBeGreaterThan(night.padCutoff)
    expect(morning.notesPerMinute).toBeGreaterThan(night.notesPerMinute)
  })

  it('energy adds notes and warmth opens the filter', () => {
    expect(scapeParams({ ...base, energy: 0.9 }, 12).notesPerMinute).toBeGreaterThan(scapeParams({ ...base, energy: 0.1 }, 12).notesPerMinute)
    expect(scapeParams({ ...base, warmth: 0.9 }, 12).padCutoff).toBeGreaterThan(scapeParams({ ...base, warmth: 0.1 }, 12).padCutoff)
  })

  it('sleep is the calmest mode and only focus pulses', () => {
    const p = (mode: 'read' | 'focus' | 'relax' | 'sleep') => scapeParams({ ...base, mode }, 12)
    expect(p('sleep').notesPerMinute).toBeLessThan(p('read').notesPerMinute)
    expect(p('sleep').chordSeconds).toBeGreaterThan(p('focus').chordSeconds)
    expect(p('focus').pulse).not.toBeNull()
    expect(p('read').pulse).toBeNull()
  })

  it('moves to a different chord every time', () => {
    const random = rng(9)
    for (const mode of Object.values(MODE_DEFS)) {
      let c = 0
      for (let i = 0; i < 50; i++) {
        const n = nextChord(mode, c, random)
        expect(n).not.toBe(c)
        expect(n).toBeGreaterThanOrEqual(0)
        expect(n).toBeLessThan(mode.chords.length)
        c = n
      }
      expect(chordNotes(mode, 0).every((m) => m > 30 && m < 80)).toBe(true)
    }
  })

  it('keeps melody notes in key and in a gentle register', () => {
    const random = rng(4)
    for (const mode of Object.values(MODE_DEFS)) {
      const pcs = new Set([...mode.scale, ...mode.chords.flat()].map((d) => (((mode.root + d) % 12) + 12) % 12))
      for (let i = 0; i < 200; i++) {
        const note = melodyNote(mode, i % mode.chords.length, random, 0.5)
        expect(pcs.has(note % 12)).toBe(true)
        expect(note).toBeGreaterThanOrEqual(55)
        expect(note).toBeLessThanOrEqual(88)
      }
    }
  })
})
