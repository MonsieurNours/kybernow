<!--
  Infobulle d'explication : s'ouvre au survol, au focus clavier ou au clic
  (le clic la garde ouverte), se ferme avec Échap ou en quittant la zone.
-->
<script lang="ts">
  import type { Snippet } from 'svelte'

  let { label, children }: { label: string; children: Snippet } = $props()

  const id = `tip-${Math.random().toString(36).slice(2, 9)}`
  let hover = $state(false)
  let pinned = $state(false)
  const open = $derived(hover || pinned)

  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && open) {
      hover = false
      pinned = false
      event.stopPropagation()
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
  class="wrap"
  onmouseenter={() => (hover = true)}
  onmouseleave={() => (hover = false)}
  onfocusin={() => (hover = true)}
  onfocusout={() => ((hover = false), (pinned = false))}
  onkeydown={keydown}
>
  <button
    type="button"
    class="dot"
    class:active={open}
    aria-label="En savoir plus : {label}"
    aria-expanded={open}
    aria-describedby={open ? id : undefined}
    onclick={() => (pinned = !pinned)}
  >
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><circle cx="7" cy="7" r="5.8" fill="none" stroke="currentColor" stroke-width="1.3" /><path d="M7 6.2v3.6M7 4.2v.2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
  </button>
  {#if open}
    <span class="tip" role="tooltip" {id}>
      {@render children()}
    </span>
  {/if}
</span>

<style>
  .wrap {
    position: relative;
    display: inline-flex;
    vertical-align: middle;
  }

  .dot {
    min-height: 0;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--faint);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .dot:hover,
  .dot.active {
    color: var(--accent);
    background: var(--accent-soft);
  }

  .tip {
    position: absolute;
    top: calc(100% + 8px);
    left: -12px;
    z-index: 30;
    width: 380px;
    max-width: calc(100vw - 40px);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border-radius: 12px;
    background: #141b24;
    border: 1px solid var(--border-2);
    box-shadow: 0 14px 40px rgb(0 0 0 / 0.45);
    font-size: 0.86rem;
    font-weight: 400;
    line-height: 1.55;
    color: var(--text-2);
    white-space: normal;
    animation: tip-in 0.16s ease-out both;
  }

  .tip::before {
    content: '';
    position: absolute;
    top: -6px;
    left: 18px;
    width: 10px;
    height: 10px;
    background: #141b24;
    border-left: 1px solid var(--border-2);
    border-top: 1px solid var(--border-2);
    transform: rotate(45deg);
  }

  @keyframes tip-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
