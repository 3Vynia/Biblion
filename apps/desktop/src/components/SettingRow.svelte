<script lang="ts">
  // One setting: its name and a line of help on the left, the control on the
  // right. Wide rows put the control below, for pickers that need the room.
  import type { Snippet } from 'svelte'

  interface Props {
    label: string
    description?: string
    wide?: boolean
    children?: Snippet
  }

  let { label, description, wide = false, children }: Props = $props()
</script>

<div class="setting" class:wide>
  <div class="text">
    <span class="label">{label}</span>
    {#if description}<span class="description">{description}</span>{/if}
  </div>
  {#if children}<div class="control">{@render children()}</div>{/if}
</div>

<style>
  .setting {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 14px 0;
  }

  .setting.wide {
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .label {
    font-weight: 500;
  }

  .description {
    font-size: 12px;
    color: var(--muted);
    line-height: 1.45;
  }

  .control {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
