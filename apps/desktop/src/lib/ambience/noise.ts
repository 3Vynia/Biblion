// Noise colours as plain sample arrays, so they can be tested without audio.

export type NoiseColor = 'white' | 'pink' | 'brown'

/** Small deterministic PRNG (mulberry32), so tests and buffers are repeatable. */
export function rng(seed = 1): () => number {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/** `length` samples of noise in [-1, 1]. Pink uses Paul Kellet's filter; brown integrates white noise. */
export function noiseSamples(color: NoiseColor, length: number, seed = 1): Float32Array<ArrayBuffer> {
  const random = rng(seed)
  const out = new Float32Array(length)
  let b0 = 0, b1 = 0, b2 = 0, b3 = 0, b4 = 0, b5 = 0, b6 = 0
  let last = 0
  for (let i = 0; i < length; i++) {
    const white = random() * 2 - 1
    if (color === 'white') {
      out[i] = white
    } else if (color === 'pink') {
      b0 = 0.99886 * b0 + white * 0.0555179
      b1 = 0.99332 * b1 + white * 0.0750759
      b2 = 0.969 * b2 + white * 0.153852
      b3 = 0.8665 * b3 + white * 0.3104856
      b4 = 0.55 * b4 + white * 0.5329522
      b5 = -0.7616 * b5 - white * 0.016898
      out[i] = (b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362) * 0.11
      b6 = white * 0.115926
    } else {
      last = (last + 0.02 * white) / 1.02
      out[i] = last * 3.5
    }
  }
  return normalize(out, 0.9)
}

/** Scales samples so the loudest one has absolute value `peak`. */
export function normalize(samples: Float32Array<ArrayBuffer>, peak: number): Float32Array<ArrayBuffer> {
  let max = 0
  for (const s of samples) max = Math.max(max, Math.abs(s))
  if (max > 0) for (let i = 0; i < samples.length; i++) samples[i] = (samples[i] / max) * peak
  return samples
}

/** Root-mean-square level of a block of samples. */
export function rms(samples: Float32Array): number {
  let sum = 0
  for (const s of samples) sum += s * s
  return Math.sqrt(sum / Math.max(1, samples.length))
}

/**
 * Share of energy below `cutoff` (as a fraction of the sample rate), via a
 * simple one-pole low-pass. Brown noise is darker than pink, pink than white.
 */
export function lowShare(samples: Float32Array, cutoff = 0.01): number {
  const k = 1 - Math.exp(-2 * Math.PI * cutoff)
  let y = 0
  const low = new Float32Array(samples.length)
  for (let i = 0; i < samples.length; i++) {
    y += k * (samples[i] - y)
    low[i] = y
  }
  return rms(low) / Math.max(1e-9, rms(samples))
}
