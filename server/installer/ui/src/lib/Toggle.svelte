<!-- Interrupteur, allumé au néon quand il est actif. -->
<script lang="ts">
  let { checked = $bindable(), label, disabled = false }: { checked: boolean; label: string; disabled?: boolean } = $props()
</script>

<button type="button" role="switch" aria-checked={checked} aria-label={label} class:on={checked} {disabled} onclick={() => (checked = !checked)}>
  <span></span>
</button>

<style>
  button {
    width: 42px;
    height: 24px;
    flex: none;
    border: 0;
    border-radius: 999px;
    padding: 3px;
    background: #1e2832;
    display: inline-flex;
    align-items: center;
    transition: background-color 0.2s, box-shadow 0.2s;
  }

  span {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--faint);
    transition: transform 0.25s var(--ease-out), background-color 0.2s;
  }

  .on {
    background: var(--signal-strong);
    box-shadow: 0 0 0 1px rgb(154 140 255 / 0.6), 0 0 12px rgb(109 91 255 / 0.55);
  }

  .on span {
    background: #fff;
    transform: translateX(18px);
  }
</style>
