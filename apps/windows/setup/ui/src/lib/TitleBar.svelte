<!-- Barre de la fenêtre sans bordure : on la glisse pour déplacer la fenêtre. -->
<script lang="ts">
  import { send } from './backend'
  import { app } from './state.svelte'

  // Glisser la barre déplace la fenêtre (sauf sur les boutons).
  function drag(event: MouseEvent) {
    if (event.button === 0 && !(event.target as HTMLElement).closest('button')) send({ cmd: 'drag' })
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<header onmousedown={drag}>
  <button type="button" aria-label="Réduire" onclick={() => send({ cmd: 'minimize' })}>
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M2 6h8" stroke="currentColor" stroke-width="1.4" /></svg>
  </button>
  <button
    type="button"
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
    flex: none;
    display: flex;
    justify-content: flex-end;
  }

  button {
    width: 46px;
    height: 40px;
    border: 0;
    background: none;
    color: var(--text-2);
    display: grid;
    place-items: center;
  }

  button:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.07);
  }

  .close:hover:not(:disabled) {
    background: #c42b1c;
    color: #fff;
  }
</style>
