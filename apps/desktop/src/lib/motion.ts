// Animation helpers that honour the system "reduce motion" setting.

import { cubicOut } from 'svelte/easing'
import type { TransitionConfig } from 'svelte/transition'

export const reducedMotion = (): boolean =>
  typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches

/** Duration in ms, or 0 when the user asked for less motion. */
export const ms = (duration: number) => (reducedMotion() ? 0 : duration)

/** Fade + rise, used for views, cards and panels. */
export function rise(
  _node: Element,
  { delay = 0, duration = 260, y = 10, scale = 1 }: { delay?: number; duration?: number; y?: number; scale?: number } = {},
): TransitionConfig {
  if (reducedMotion()) return { duration: 0 }
  return {
    delay,
    duration,
    easing: cubicOut,
    css: (t, u) => `opacity: ${t}; transform: translateY(${u * y}px) scale(${scale + (1 - scale) * t});`,
  }
}

/** Slide in from the right, used for side panels. */
export function slideIn(_node: Element, { duration = 240, x = 24 }: { duration?: number; x?: number } = {}): TransitionConfig {
  if (reducedMotion()) return { duration: 0 }
  return {
    duration,
    easing: cubicOut,
    css: (t, u) => `opacity: ${t}; transform: translateX(${u * x}px);`,
  }
}
