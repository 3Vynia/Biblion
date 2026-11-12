import { describe, expect, it } from 'vitest'
import { BUILT_IN_SCENES, DEFAULT_AMBIENCE, activeScene, anyLayerOn, applyScene, normalizeAmbience, sceneFromMix } from './settings'

describe('ambience settings', () => {
  it('normalises missing and broken values', () => {
    expect(normalizeAmbience(null)).toEqual(DEFAULT_AMBIENCE)
    const s = normalizeAmbience({ master: 3, layers: { rain: { on: true, volume: -1, tone: 'x' }, bogus: { on: true } }, timerMinutes: 7 })
    expect(s.master).toBe(1)
    expect(s.layers.rain).toEqual({ on: true, volume: 0, tone: DEFAULT_AMBIENCE.layers.rain.tone })
    expect('bogus' in s.layers).toBe(false)
    expect(s.timerMinutes).toBe(0)
  })

  it('normalises the soundscape', () => {
    const s = normalizeAmbience({ soundscape: { on: true, mode: 'party', energy: 2, adaptToTime: false } })
    expect(s.soundscape).toMatchObject({ on: true, mode: 'read', energy: 1, adaptToTime: false, reactToReading: true })
  })

  it('keeps valid custom scenes only', () => {
    const s = normalizeAmbience({
      scenes: [
        { id: 'a', name: '  Mine ', layers: { fire: { volume: 0.4, tone: 0.9 }, nope: { volume: 1 } } },
        { id: 'b', name: '   ' },
        'junk',
      ],
    })
    expect(s.scenes).toEqual([{ id: 'a', name: 'Mine', layers: { fire: { volume: 0.4, tone: 0.9 } } }])
  })

  it('applies a scene, recognises it, and saves the mix back', () => {
    const cabin = BUILT_IN_SCENES.find((s) => s.id === 'cabin')!
    const mixed = applyScene(DEFAULT_AMBIENCE, cabin)
    expect(mixed.layers.fire).toEqual({ on: true, volume: 0.65, tone: 0.5 })
    expect(mixed.layers.rain.on).toBe(false)
    expect(mixed.soundscape).toMatchObject({ on: true, mode: 'relax', volume: 0.45 })
    expect(activeScene(mixed, BUILT_IN_SCENES)?.id).toBe('cabin')
    expect(anyLayerOn(mixed)).toBe(true)

    const tweaked = { ...mixed, layers: { ...mixed.layers, fire: { ...mixed.layers.fire, volume: 0.2 } } }
    expect(activeScene(tweaked, BUILT_IN_SCENES)).toBeUndefined()
    const saved = sceneFromMix(tweaked, ' My cabin ', 'x')
    expect(saved.name).toBe('My cabin')
    expect(Object.keys(saved.layers).sort()).toEqual(['fire', 'wind'])
    expect(saved.soundscape?.mode).toBe('relax')
    expect(activeScene(tweaked, [saved])?.id).toBe('x')
    const moodChanged = { ...tweaked, soundscape: { ...tweaked.soundscape, mode: 'sleep' as const } }
    expect(activeScene(moodChanged, [saved])).toBeUndefined()
  })

  it('starts silent', () => {
    expect(anyLayerOn(DEFAULT_AMBIENCE)).toBe(false)
  })
})
