<!-- Barre de titre dessinée par l'app (fenêtre sans cadre), boutons de Windows 11 à droite. -->
<script lang="ts">
  import Mark from './lib/Mark.svelte'
  import Icon from './lib/Icon.svelte'
  import { currentWindow } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let { title = 'KyberNow', onback, onapp, badge = false }: { title?: string; onback?: () => void; onapp?: () => void; badge?: boolean } = $props()
</script>

<header data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    {#if onback}
      <button class="back" aria-label={t('titlebar.back')} onclick={onback}><Icon name="back" size={16} /></button>
    {/if}
    <Mark size={16} state="now" />
    <span data-tauri-drag-region>{title}</span>
  </div>
  <div class="caption">
    {#if onapp}
      <button aria-label={t('titlebar.app_settings')} class="gear" onclick={onapp}><Icon name="gear" size={16} stroke={1.6} />{#if badge}<span class="badge"></span>{/if}</button>
    {/if}
    <button aria-label={t('titlebar.minimize')} onclick={() => currentWindow().minimize()}><Icon name="minimize" size={12} stroke={1.2} /></button>
    <button aria-label={t('titlebar.maximize')} onclick={() => currentWindow().toggleMaximize()}><Icon name="maximize" size={11} stroke={1.2} /></button>
    <button aria-label={t('common.close')} class="close" onclick={() => currentWindow().close()}><Icon name="x" size={12} stroke={1.2} /></button>
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

  .gear {
    position: relative;
    color: var(--muted) !important;
  }

  /* Une mise à jour attend. */
  .badge {
    position: absolute;
    top: 10px;
    right: 13px;
    width: 7px;
    height: 7px;
    border-radius: 4px;
    background: var(--signal);
    box-shadow: 0 0 8px var(--signal);
  }
</style>
