<script lang="ts">
  import { send } from './backend'
  import { app } from './state.svelte'
  import Mark from './Mark.svelte'

  // Glisser la barre déplace la fenêtre (sauf sur les boutons).
  function drag(event: MouseEvent) {
    if (event.button === 0 && !(event.target as HTMLElement).closest('button')) send({ cmd: 'drag' })
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<header onmousedown={drag}>
  <Mark size={18} id="kn-title" />
  <span>Installation de KyberNow</span>
  <div class="spacer"></div>
  <button aria-label="Réduire" onclick={() => send({ cmd: 'minimize' })}>
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M2 6h8" stroke="currentColor" stroke-width="1.4" /></svg>
  </button>
  <button
    aria-label="Fermer"
    class="close"
    disabled={app.installing}
    title={app.installing ? "Patientez jusqu'à la fin de l'installation" : undefined}
    onclick={() => send({ cmd: 'close' })}
  >
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.4" /></svg>
  </button>
</header>

<style>
  header {
    height: 40px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 0 0 14px;
    border-bottom: 1px solid var(--line);
    background: var(--bar);
    font-size: 13px;
    color: var(--muted);
  }

  button {
    width: 46px;
    height: 40px;
    border: none;
    background: transparent;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  button:hover:not(:disabled) {
    background: #1a212b;
    color: var(--text);
  }

  .close:hover:not(:disabled) {
    background: #c42b1c;
    color: #fff;
  }

  button:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
