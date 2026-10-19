<script lang="ts">
  let { checked = $bindable(), label, disabled = false }: { checked: boolean; label: string; disabled?: boolean } = $props()
</script>

<button role="switch" aria-checked={checked} aria-label={label} class:on={checked} {disabled} onclick={() => (checked = !checked)}>
  <span></span>
</button>

<style>
  button {
    width: 48px;
    height: 28px;
    flex-shrink: 0;
    border-radius: 14px;
    border: none;
    padding: 3px;
    background: var(--line-2);
    display: flex;
    transition: background-color 0.25s;
  }

  button.on {
    background: var(--accent);
  }

  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  span {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #f4f7fa;
    transition: transform 0.25s cubic-bezier(0.2, 0.7, 0.2, 1);
  }

  button.on span {
    transform: translateX(20px);
  }
</style>
