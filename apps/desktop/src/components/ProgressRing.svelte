<script lang="ts">
  // Circular progress that fills up when it appears.
  import { onMount } from 'svelte'
  import { reducedMotion } from '../lib/motion'

  interface Props {
    value: number
    size?: number
    stroke?: number
    label?: boolean
  }

  let { value, size = 44, stroke = 4, label = true }: Props = $props()

  const r = $derived((size - stroke) / 2)
  const circumference = $derived(2 * Math.PI * r)
  // Starts empty and fills to `value` on the next frame, so it animates in.
  let mounted = $state(false)
  const shown = $derived(mounted ? value : 0)

  onMount(() => {
    if (reducedMotion()) {
      mounted = true
      return
    }
    const id = requestAnimationFrame(() => (mounted = true))
    return () => cancelAnimationFrame(id)
  })
</script>

<div class="ring" style:width="{size}px" style:height="{size}px">
  <svg width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
    <circle class="track" cx={size / 2} cy={size / 2} {r} stroke-width={stroke} />
    <circle
      class="bar"
      cx={size / 2}
      cy={size / 2}
      {r}
      stroke-width={stroke}
      stroke-dasharray={circumference}
      stroke-dashoffset={circumference * (1 - Math.min(1, Math.max(0, shown)))}
    />
  </svg>
  {#if label}<span>{Math.round(value * 100)}%</span>{/if}
</div>

<style>
  .ring {
    position: relative;
    display: grid;
    place-items: center;
  }

  svg {
    position: absolute;
    inset: 0;
    transform: rotate(-90deg);
  }

  circle {
    fill: none;
  }

  .track {
    stroke: color-mix(in srgb, var(--text) 12%, transparent);
  }

  .bar {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dashoffset 900ms var(--ease);
  }

  span {
    font-size: 11px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
</style>
