<!-- Interrupteur, allumé au néon quand il est actif. -->
<script lang="ts">
  let { checked, label, disabled = false, onchange }: { checked: boolean; label: string; disabled?: boolean; onchange: (v: boolean) => void } =
    $props()
</script>

<button type="button" class="sw" class:on={checked} role="switch" aria-checked={checked} aria-label={label} {disabled} onclick={() => onchange(!checked)}>
  <span></span>
</button>

<style>
  .sw {
    width: 42px;
    height: 24px;
    border: 0;
    border-radius: 999px;
    padding: 3px;
    background: #1e2832;
    display: inline-flex;
    align-items: center;
    justify-self: start;
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
