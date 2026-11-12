<script lang="ts">
  // A soft, slowly moving light that breathes with the sound.
  import { onMount } from 'svelte'
  import { ambience } from '../lib/ambience/store.svelte'
  import { palette } from '../lib/ambience/palette'
  import { reducedMotion } from '../lib/motion'

  interface Props {
    size?: number
  }

  let { size = 160 }: Props = $props()

  let canvas: HTMLCanvasElement
  const colors = $derived(palette(ambience.value.soundscape.mode, new Date().getHours()))

  onMount(() => {
    const ctx = canvas.getContext('2d')!
    const dpr = window.devicePixelRatio || 1
    canvas.width = size * dpr
    canvas.height = size * dpr
    ctx.scale(dpr, dpr)
    let frame = 0
    let smooth = 0
    let raf = 0
    const still = reducedMotion()
    const draw = () => {
      const level = ambience.playing ? ambience.level() : 0
      smooth += (level - smooth) * 0.08
      const t = still ? 0 : frame++ / 60
      const r = size / 2
      ctx.clearRect(0, 0, size, size)
      ctx.globalCompositeOperation = 'lighter'
      const [a, b, c] = colors
      const blobs: [string, number, number, number][] = [
        [a, Math.sin(t * 0.31) * 0.18, Math.cos(t * 0.27) * 0.15, 0.62],
        [b, Math.cos(t * 0.23 + 1) * 0.2, Math.sin(t * 0.19 + 2) * 0.18, 0.55],
        [c, Math.sin(t * 0.17 + 3) * 0.22, Math.cos(t * 0.21 + 1) * 0.2, 0.6],
      ]
      for (const [color, dx, dy, k] of blobs) {
        const radius = r * (k + smooth * 0.35 + (ambience.playing ? 0.04 * Math.sin(t * 0.8) : 0))
        const x = r + dx * r
        const y = r + dy * r
        const g = ctx.createRadialGradient(x, y, 0, x, y, radius)
        g.addColorStop(0, color + 'ee')
        g.addColorStop(0.55, color + '55')
        g.addColorStop(1, color + '00')
        ctx.fillStyle = g
        ctx.beginPath()
        ctx.arc(x, y, radius, 0, Math.PI * 2)
        ctx.fill()
      }
      ctx.globalCompositeOperation = 'source-over'
      raf = requestAnimationFrame(draw)
    }
    draw()
    return () => cancelAnimationFrame(raf)
  })
</script>

<canvas
  bind:this={canvas}
  class:playing={ambience.playing}
  style:width="{size}px"
  style:height="{size}px"
  aria-hidden="true"
></canvas>

<style>
  canvas {
    display: block;
    border-radius: 50%;
    filter: blur(2px) saturate(1.1);
    opacity: 0.55;
    transition:
      opacity 800ms ease,
      transform 800ms var(--ease);
    transform: scale(0.92);
  }

  canvas.playing {
    opacity: 1;
    transform: none;
  }
</style>
