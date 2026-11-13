<script lang="ts">
  // The keyboard shortcuts, grouped; used by the help sheet and Settings.
  import { SHORTCUTS } from '../lib/shortcuts.svelte'

  const mac = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform)
  const label = (key: string) => (mac && key === 'Ctrl' ? '⌘' : key)
</script>

<div class="groups">
  {#each SHORTCUTS as group (group.title)}
    <section>
      <h3>{group.title}</h3>
      <dl>
        {#each group.items as s (s.label)}
          <div class="row">
            <dt>{s.label}</dt>
            <dd>
              {#each s.keys as chord, i (i)}
                {#if i > 0}<span class="or">or</span>{/if}
                <span class="chord">
                  {#each chord as key, j (j)}{#if j > 0}<span class="plus">+</span>{/if}<kbd>{label(key)}</kbd>{/each}
                </span>
              {/each}
            </dd>
          </div>
        {/each}
      </dl>
    </section>
  {/each}
</div>

<style>
  .groups {
    columns: 2 300px;
    column-gap: 32px;
  }

  section {
    break-inside: avoid;
    margin-bottom: 20px;
  }

  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
  }

  dl {
    margin: 0;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 5px 0;
    border-bottom: 1px solid var(--border);
  }

  dt {
    font-size: 13px;
  }

  dd {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 4px;
    margin: 0;
  }

  .chord {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }

  kbd {
    min-width: 22px;
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 6px;
    background: var(--surface-2);
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    text-align: center;
  }

  .or,
  .plus {
    font-size: 11px;
    color: var(--muted);
  }
</style>
