// Ambience preferences: a generative soundscape (the heart of it, in the
// spirit of adaptive "functional music"), optional nature/room textures
// mixed under it, scenes (named mixes), the sleep timer and options.

export type Mode = 'read' | 'focus' | 'relax' | 'sleep'

export interface SoundscapeState {
  on: boolean
  mode: Mode
  volume: number
  /** 0 – 1: how many notes and how much movement. */
  energy: number
  /** 0 – 1: dark and soft → bright and open. */
  warmth: number
  /** Follow the time of day (brighter mornings, darker nights). */
  adaptToTime: boolean
  /** Answer page turns with a soft note; calm down when reading slows. */
  reactToReading: boolean
}

export const MODES: { id: Mode; label: string; hint: string }[] = [
  { id: 'read', label: 'Read', hint: 'Warm and unobtrusive' },
  { id: 'focus', label: 'Focus', hint: 'Steady, with a gentle pulse' },
  { id: 'relax', label: 'Relax', hint: 'Slow and open' },
  { id: 'sleep', label: 'Sleep', hint: 'Low, dark and very slow' },
]

export type LayerId = 'rain' | 'thunder' | 'fire' | 'waves' | 'wind' | 'night' | 'clock' | 'brown' | 'pink' | 'white'

export interface LayerState {
  on: boolean
  /** 0 – 1 */
  volume: number
  /** 0 – 1, meaning depends on the layer (see LAYERS). */
  tone: number
}

export interface Scene {
  id: string
  name: string
  /** The soundscape's mode and levels, or absent when the scene has none. */
  soundscape?: Pick<SoundscapeState, 'mode' | 'volume' | 'energy' | 'warmth'>
  layers: Partial<Record<LayerId, Pick<LayerState, 'volume' | 'tone'>>>
}

export interface AmbienceSettings {
  master: number
  soundscape: SoundscapeState
  layers: Record<LayerId, LayerState>
  /** Start the ambience when a book opens. */
  autoStart: boolean
  /** A soft paper sound on every page turn. */
  pageTurn: boolean
  pageTurnVolume: number
  /** Last chosen sleep timer, in minutes (0 = off). */
  timerMinutes: number
  /** Mixes saved by the user. */
  scenes: Scene[]
}

export interface LayerInfo {
  id: LayerId
  label: string
  group: 'Nature' | 'Indoors' | 'Noise'
  /** Labels for the two ends of the tone slider, if the layer has one. */
  tone?: [string, string]
}

export const LAYERS: LayerInfo[] = [
  { id: 'rain', label: 'Rain', group: 'Nature', tone: ['Drizzle', 'Downpour'] },
  { id: 'thunder', label: 'Distant thunder', group: 'Nature', tone: ['Rare', 'Frequent'] },
  { id: 'wind', label: 'Wind', group: 'Nature', tone: ['Breeze', 'Gale'] },
  { id: 'waves', label: 'Sea waves', group: 'Nature', tone: ['Calm', 'Rough'] },
  { id: 'night', label: 'Night crickets', group: 'Nature', tone: ['Few', 'Many'] },
  { id: 'fire', label: 'Fireplace', group: 'Indoors', tone: ['Embers', 'Roaring'] },
  { id: 'clock', label: 'Old clock', group: 'Indoors' },
  { id: 'brown', label: 'Brown noise', group: 'Noise' },
  { id: 'pink', label: 'Pink noise', group: 'Noise' },
  { id: 'white', label: 'White noise', group: 'Noise' },
]

const off = (volume = 0.5, tone = 0.5): LayerState => ({ on: false, volume, tone })

export const DEFAULT_AMBIENCE: AmbienceSettings = {
  master: 0.6,
  soundscape: { on: false, mode: 'read', volume: 0.7, energy: 0.45, warmth: 0.5, adaptToTime: true, reactToReading: true },
  layers: {
    rain: off(0.55, 0.4),
    thunder: off(0.35, 0.3),
    wind: off(0.35, 0.3),
    waves: off(0.5, 0.4),
    night: off(0.35, 0.4),
    fire: off(0.6, 0.5),
    clock: off(0.25),
    brown: off(0.45),
    pink: off(0.35),
    white: off(0.25),
  },
  autoStart: false,
  pageTurn: false,
  pageTurnVolume: 0.5,
  timerMinutes: 0,
  scenes: [],
}

const scape = (mode: Mode, volume: number, energy: number, warmth: number) => ({ mode, volume, energy, warmth })

export const BUILT_IN_SCENES: Scene[] = [
  { id: 'reading-room', name: 'Reading room', soundscape: scape('read', 0.7, 0.45, 0.5), layers: {} },
  { id: 'rainy-library', name: 'Rainy library', soundscape: scape('read', 0.55, 0.35, 0.4), layers: { rain: { volume: 0.55, tone: 0.4 }, thunder: { volume: 0.25, tone: 0.2 } } },
  { id: 'cabin', name: 'Cabin by the fire', soundscape: scape('relax', 0.45, 0.3, 0.35), layers: { fire: { volume: 0.65, tone: 0.5 }, wind: { volume: 0.25, tone: 0.3 } } },
  { id: 'seaside', name: 'Seaside', soundscape: scape('relax', 0.5, 0.4, 0.65), layers: { waves: { volume: 0.6, tone: 0.4 } } },
  { id: 'deep-focus', name: 'Deep focus', soundscape: scape('focus', 0.6, 0.5, 0.5), layers: { brown: { volume: 0.3, tone: 0.5 } } },
  { id: 'night-reading', name: 'Night reading', soundscape: scape('sleep', 0.6, 0.25, 0.25), layers: { night: { volume: 0.3, tone: 0.4 } } },
]

const modes = MODES.map((m) => m.id)
const clamp01 = (n: unknown, d: number) => (typeof n === 'number' && Number.isFinite(n) ? Math.min(1, Math.max(0, n)) : d)
const ids = LAYERS.map((l) => l.id)

/** Stored values may be missing, old or hand-edited. */
export function normalizeAmbience(raw: unknown): AmbienceSettings {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>
  const d = DEFAULT_AMBIENCE
  const rawLayers = (r.layers && typeof r.layers === 'object' ? r.layers : {}) as Record<string, Record<string, unknown>>
  const layers = Object.fromEntries(
    ids.map((id) => {
      const l = rawLayers[id] ?? {}
      const def = d.layers[id]
      return [id, { on: typeof l.on === 'boolean' ? l.on : def.on, volume: clamp01(l.volume, def.volume), tone: clamp01(l.tone, def.tone) }]
    }),
  ) as Record<LayerId, LayerState>
  const scenes = (Array.isArray(r.scenes) ? r.scenes : [])
    .filter((s): s is Record<string, unknown> => !!s && typeof s === 'object')
    .filter((s) => typeof s.id === 'string' && typeof s.name === 'string' && s.name.trim())
    .slice(0, 50)
    .map((s) => ({
      id: s.id as string,
      name: (s.name as string).trim().slice(0, 60),
      ...(s.soundscape && typeof s.soundscape === 'object'
        ? (() => {
            const sc = s.soundscape as Record<string, unknown>
            return {
              soundscape: {
                mode: modes.includes(sc.mode as Mode) ? (sc.mode as Mode) : 'read',
                volume: clamp01(sc.volume, 0.6),
                energy: clamp01(sc.energy, 0.45),
                warmth: clamp01(sc.warmth, 0.5),
              },
            }
          })()
        : {}),
      layers: Object.fromEntries(
        Object.entries((s.layers ?? {}) as Record<string, Record<string, unknown>>)
          .filter(([id]) => ids.includes(id as LayerId))
          .map(([id, l]) => [id, { volume: clamp01(l?.volume, 0.5), tone: clamp01(l?.tone, 0.5) }]),
      ),
    }))
  const rs = (r.soundscape && typeof r.soundscape === 'object' ? r.soundscape : {}) as Record<string, unknown>
  const ds = d.soundscape
  const soundscape: SoundscapeState = {
    on: typeof rs.on === 'boolean' ? rs.on : ds.on,
    mode: modes.includes(rs.mode as Mode) ? (rs.mode as Mode) : ds.mode,
    volume: clamp01(rs.volume, ds.volume),
    energy: clamp01(rs.energy, ds.energy),
    warmth: clamp01(rs.warmth, ds.warmth),
    adaptToTime: typeof rs.adaptToTime === 'boolean' ? rs.adaptToTime : ds.adaptToTime,
    reactToReading: typeof rs.reactToReading === 'boolean' ? rs.reactToReading : ds.reactToReading,
  }
  return {
    master: clamp01(r.master, d.master),
    soundscape,
    layers,
    autoStart: typeof r.autoStart === 'boolean' ? r.autoStart : d.autoStart,
    pageTurn: typeof r.pageTurn === 'boolean' ? r.pageTurn : d.pageTurn,
    pageTurnVolume: clamp01(r.pageTurnVolume, d.pageTurnVolume),
    timerMinutes: typeof r.timerMinutes === 'number' && [0, 15, 30, 45, 60, 90, 120].includes(r.timerMinutes) ? r.timerMinutes : 0,
    scenes,
  }
}

/** Turns a scene on: its sounds play at its levels, everything else stops. */
export function applyScene(s: AmbienceSettings, scene: Scene): AmbienceSettings {
  const layers = { ...s.layers }
  for (const id of ids) {
    const l = scene.layers[id]
    layers[id] = l ? { on: true, volume: l.volume, tone: l.tone } : { ...layers[id], on: false }
  }
  const soundscape = scene.soundscape ? { ...s.soundscape, ...scene.soundscape, on: true } : { ...s.soundscape, on: false }
  return { ...s, soundscape, layers }
}

/** The current mix as a scene, for "Save mix". */
export function sceneFromMix(s: AmbienceSettings, name: string, id: string): Scene {
  const layers: Scene['layers'] = {}
  for (const l of LAYERS) if (s.layers[l.id].on) layers[l.id] = { volume: s.layers[l.id].volume, tone: s.layers[l.id].tone }
  const { on, mode, volume, energy, warmth } = s.soundscape
  return { id, name: name.trim(), ...(on ? { soundscape: { mode, volume, energy, warmth } } : {}), layers }
}

/** The scene matching the current mix, if any (to highlight it). */
export function activeScene(s: AmbienceSettings, scenes: Scene[]): Scene | undefined {
  const close = (a: number, b: number) => Math.abs(a - b) < 0.02
  const sameScape = (scene: Scene) => {
    const want = scene.soundscape
    const have = s.soundscape
    if (!want) return !have.on
    return have.on && have.mode === want.mode && close(have.volume, want.volume) && close(have.energy, want.energy) && close(have.warmth, want.warmth)
  }
  return scenes.find(
    (scene) =>
      (Object.keys(scene.layers).length > 0 || !!scene.soundscape) &&
      sameScape(scene) &&
      ids.every((id) => {
        const want = scene.layers[id]
        const have = s.layers[id]
        return want ? have.on && close(have.volume, want.volume) && close(have.tone, want.tone) : !have.on
      }),
  )
}

/** Something would make a sound. */
export const anyLayerOn = (s: AmbienceSettings) => s.soundscape.on || ids.some((id) => s.layers[id].on)
