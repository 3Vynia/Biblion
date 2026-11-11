<script lang="ts">
  interface Props {
    checked: boolean
    label: string
    description?: string
    onchange: (checked: boolean) => void
  }

  let { checked, label, description, onchange }: Props = $props()
</script>

<label class="row">
  <span class="text">
    <span class="label">{label}</span>
    {#if description}<span class="description">{description}</span>{/if}
  </span>
  <input type="checkbox" role="switch" {checked} onchange={(e) => onchange(e.currentTarget.checked)} />
  <span class="track" aria-hidden="true"><span class="knob"></span></span>
</label>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 0;
    cursor: pointer;
  }

  .text {
    flex: 1;
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
  }

  input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .track {
    position: relative;
    flex-shrink: 0;
    width: 40px;
    height: 23px;
    border-radius: 12px;
    background: color-mix(in srgb, var(--text) 18%, transparent);
    transition: background 220ms ease;
  }

  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.3);
    transition: transform 260ms var(--ease);
  }

  input:checked + .track {
    background: var(--accent);
  }

  input:checked + .track .knob {
    transform: translateX(17px);
  }

  input:focus-visible + .track {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .row:active .knob {
    width: 21px;
  }
</style>
