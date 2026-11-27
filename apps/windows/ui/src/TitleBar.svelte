<!--
  Barre de titre dessinée par l'app (fenêtre sans cadre), boutons de Windows 11 à droite. Sans compte KyberNow, sur
  l'accueil : « Connecter un compte » (icône d'une personne) à côté de « Réglages de l'app » (docs/link-ui.md « Accueil
  et compte KyberNow ») ; « Connexion au compte… » pendant la connexion.
-->
<script lang="ts">
  import Mark from './lib/Mark.svelte'
  import Icon from './lib/Icon.svelte'
  import { currentWindow } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    title = 'KyberNow Link',
    onback,
    onapp,
    badge = false,
    onaccount,
    signingIn = false,
    crowded = false,
  }: {
    title?: string
    onback?: () => void
    onapp?: () => void
    badge?: boolean
    onaccount?: () => void
    signingIn?: boolean
    /** L'avis de la mise à jour occupe le milieu de la barre : dans une fenêtre étroite, l'icône seule. */
    crowded?: boolean
  } = $props()
</script>

<header data-tauri-drag-region data-stays-active>
  <div class="left" data-tauri-drag-region>
    {#if onback}
      <button class="back" aria-label={t('titlebar.back')} onclick={onback}><Icon name="back" size={16} /></button>
    {/if}
    <Mark size={16} state="now" />
    <span data-tauri-drag-region>{title}</span>
  </div>
  <div class="caption">
    {#if onaccount}
      <button class="account" class:crowded disabled={signingIn} aria-label={signingIn ? t('account.signing_in') : t('account.sign_in')} onclick={onaccount}
        ><Icon name="user" size={15} stroke={2} /><span class="label">{signingIn ? t('account.signing_in') : t('account.sign_in')}</span></button
      >
    {/if}
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

  /* « Connecter un compte » : sur une ligne, à côté de l'engrenage des réglages de l'app. */
  .caption .account {
    width: auto;
    align-self: center;
    height: 28px;
    margin-right: 6px;
    padding: 0 12px;
    gap: 8px;
    border-radius: 7px;
    background: var(--signal-strong);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }

  /* Fenêtre étroite et avis au milieu : l'icône seule (libellé pour l'accessibilité), rien ne se chevauche. */
  @media (max-width: 1239px) {
    .caption .account.crowded {
      width: 32px;
      padding: 0;
      justify-content: center;
    }

    .caption .account.crowded .label {
      display: none;
    }
  }

  .caption .account:hover:not(:disabled) {
    background: var(--signal-strong);
    filter: brightness(1.12);
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
