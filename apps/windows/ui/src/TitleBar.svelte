<!-- Barre de titre dessinée par l'app (fenêtre sans cadre), boutons de Windows 11 à droite. -->
<script lang="ts">
  import Mark from './lib/Mark.svelte'
  import Icon from './lib/Icon.svelte'
  import { currentWindow } from './lib/api'

  let { title = 'KyberNow', onback }: { title?: string; onback?: () => void } = $props()
</script>

<header data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    {#if onback}
      <button class="back" aria-label="Retour à l'accueil" onclick={onback}><Icon name="back" size={16} /></button>
    {/if}
    <Mark size={16} state="now" />
    <span data-tauri-drag-region>{title}</span>
  </div>
  <div class="caption">
    <button aria-label="Réduire" onclick={() => currentWindow().minimize()}><Icon name="minimize" size={12} stroke={1.2} /></button>
    <button aria-label="Agrandir" onclick={() => currentWindow().toggleMaximize()}><Icon name="maximize" size={11} stroke={1.2} /></button>
    <button aria-label="Fermer" class="close" onclick={() => currentWindow().close()}><Icon name="x" size={12} stroke={1.2} /></button>
  </div>
</header>

<style>
  header {
    position: absolute;
    inset: 0 0 auto 0;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 14px;
    z-index: 20;
  }

  .left {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--muted);
  }

  .back {
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .back:hover {
    background: rgb(255 255 255 / 0.06);
  }

  .caption {
    display: flex;
    height: 40px;
  }

  .caption button {
    width: 46px;
    border: 0;
    background: none;
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .caption button:hover {
    background: rgb(255 255 255 / 0.08);
  }

  .caption .close:hover {
    background: #c42b1c;
  }
</style>
