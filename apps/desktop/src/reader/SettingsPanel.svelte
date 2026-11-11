<script lang="ts">
  import { FONTS, FONT_SIZE, LINE_HEIGHT, THEMES, type FontId, type ThemeId } from './readerSettings'
  import { readerSettings } from './settingsStore.svelte'

  const s = $derived(readerSettings.value)
  const set = readerSettings.update.bind(readerSettings)
</script>

<div class="group">
  <span class="label">Theme</span>
  <div class="themes" role="radiogroup" aria-label="Theme">
    {#each Object.entries(THEMES) as [id, theme] (id)}
      <button
        class="theme"
        class:active={s.theme === id}
        style:background={theme.bg}
        style:color={theme.fg}
        role="radio"
        aria-checked={s.theme === id}
        onclick={() => set({ theme: id as ThemeId })}
      >
        Aa<span>{theme.label}</span>
      </button>
    {/each}
  </div>
</div>

<div class="group">
  <span class="label">Font</span>
  <div class="segmented">
    {#each Object.entries(FONTS) as [id, font] (id)}
      <button
        class:active={s.font === id}
        style:font-family={font.stack ?? 'inherit'}
        onclick={() => set({ font: id as FontId })}>{font.label}</button
      >
    {/each}
  </div>
</div>

<div class="group">
  <span class="label">Text size</span>
  <div class="stepper">
    <button
      aria-label="Smaller text"
      disabled={s.fontSize <= FONT_SIZE.min}
      onclick={() => set({ fontSize: s.fontSize - FONT_SIZE.step })}>A−</button
    >
    <span class="value">{s.fontSize}%</span>
    <button
      aria-label="Larger text"
      disabled={s.fontSize >= FONT_SIZE.max}
      onclick={() => set({ fontSize: s.fontSize + FONT_SIZE.step })}>A+</button
    >
  </div>
</div>

<div class="group">
  <span class="label">Line spacing</span>
  <div class="stepper">
    <button
      aria-label="Less line spacing"
      disabled={s.lineHeight <= LINE_HEIGHT.min}
      onclick={() => set({ lineHeight: s.lineHeight - LINE_HEIGHT.step })}>−</button
    >
    <span class="value">{s.lineHeight.toFixed(1)}</span>
    <button
      aria-label="More line spacing"
      disabled={s.lineHeight >= LINE_HEIGHT.max}
      onclick={() => set({ lineHeight: s.lineHeight + LINE_HEIGHT.step })}>+</button
    >
  </div>
</div>

<div class="group">
  <span class="label">Margins</span>
  <div class="segmented">
    <button class:active={s.margin === 'narrow'} onclick={() => set({ margin: 'narrow' })}>Narrow</button>
    <button class:active={s.margin === 'normal'} onclick={() => set({ margin: 'normal' })}>Normal</button>
    <button class:active={s.margin === 'wide'} onclick={() => set({ margin: 'wide' })}>Wide</button>
  </div>
</div>

<div class="group">
  <span class="label">Layout</span>
  <div class="segmented">
    <button class:active={s.flow === 'paginated'} onclick={() => set({ flow: 'paginated' })}>Pages</button>
    <button class:active={s.flow === 'scrolled'} onclick={() => set({ flow: 'scrolled' })}>Scroll</button>
  </div>
  <div class="segmented">
    <button
      class:active={s.spread === 'auto'}
      disabled={s.flow === 'scrolled'}
      onclick={() => set({ spread: 'auto' })}
      title="Two pages side by side when the window is wide">Two pages</button
    >
    <button class:active={s.spread === 'single'} disabled={s.flow === 'scrolled'} onclick={() => set({ spread: 'single' })}
      >One page</button
    >
  </div>
</div>

<div class="group toggles">
  <label><input type="checkbox" checked={s.justify} onchange={(e) => set({ justify: e.currentTarget.checked })} /> Justify text</label>
  <label
    ><input type="checkbox" checked={s.hyphenate} onchange={(e) => set({ hyphenate: e.currentTarget.checked })} /> Hyphenation</label
  >
</div>

<button class="reset" onclick={() => readerSettings.reset()}>Reset to defaults</button>

<style>
  .group {
    margin-bottom: 18px;
  }

  .label {
    display: block;
    font-size: 12px;
    color: var(--r-muted);
    margin-bottom: 6px;
  }

  button {
    background: var(--r-bg);
    color: var(--r-fg);
    border-color: var(--r-border);
  }

  button:hover:not(:disabled) {
    background: var(--r-hover);
  }

  .themes {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
  }

  .theme {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 8px 4px;
    font-family: var(--serif);
    font-size: 18px;
  }

  .theme:hover:not(:disabled) {
    filter: brightness(0.96);
  }

  .theme span {
    font-family: var(--font);
    font-size: 11px;
  }

  .theme.active,
  .segmented button.active {
    outline: 2px solid var(--r-link);
    outline-offset: -2px;
  }

  .segmented {
    display: flex;
    gap: 4px;
    margin-bottom: 6px;
  }

  .segmented button {
    flex: 1;
    padding: 6px 4px;
  }

  .stepper {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .stepper button {
    width: 48px;
  }

  .value {
    flex: 1;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .toggles label {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
  }

  .reset {
    width: 100%;
  }
</style>
