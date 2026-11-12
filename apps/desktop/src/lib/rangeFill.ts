// Keeps --fill on a range input in sync with its value, so the track can be
// painted up to the thumb (WebKitGTK has no ::-moz-range-progress).
export function rangeFill(el: HTMLInputElement, _deps?: unknown) {
  const sync = () => {
    const min = Number(el.min || 0)
    const max = Number(el.max || 100)
    const pct = max > min ? ((Number(el.value) - min) / (max - min)) * 100 : 0
    el.style.setProperty('--fill', `${Math.min(100, Math.max(0, pct))}%`)
  }
  sync()
  el.addEventListener('input', sync)
  return {
    update: () => queueMicrotask(sync),
    destroy: () => el.removeEventListener('input', sync),
  }
}
