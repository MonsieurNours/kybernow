<!-- Choix exclusif en pastilles. -->
<script lang="ts" generics="T extends string">
  let {
    options,
    value,
    label,
    disabled = false,
    onchange,
  }: { options: { value: T; label: string }[]; value: T; label: string; disabled?: boolean; onchange: (v: T) => void } = $props()
</script>

<div class="seg" role="group" aria-label={label}>
  {#each options as o (o.value)}
    <button type="button" aria-pressed={o.value === value} {disabled} onclick={() => o.value !== value && onchange(o.value)}>{o.label}</button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    flex-wrap: wrap;
    padding: 3px;
    border-radius: 10px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
    justify-self: start;
    /* Ne se comprime pas dans une rangée ; ne passe à la ligne que sur un écran étroit. */
    flex: none;
    max-width: 100%;
  }

  button {
    height: 32px;
    padding: 0 13px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--muted);
    font-size: 13px;
    transition: background-color 0.15s, color 0.15s;
  }

  button:hover:not([aria-pressed='true']):not(:disabled) {
    color: var(--text);
  }

  button[aria-pressed='true'] {
    background: rgb(109 91 255 / 0.2);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.55);
    color: #fff;
  }
</style>
