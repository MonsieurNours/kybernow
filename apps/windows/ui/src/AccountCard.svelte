<!--
  Carte du compte, sans compte, en dernier dans la rangée des ordinateurs de l'accueil (docs/link-ui.md « Accueil et
  compte KyberNow ») : « COMPTE KYBERNOW », « Retrouvez vos ordinateurs partout », « Sur tous vos appareils, même hors
  de chez vous. », « Connecter un compte », « Créer un compte », la croix « Fermer » (revenue après 30 jours : « Ne plus
  afficher »). Avec la lueur de « Sur le réseau ».
-->
<script lang="ts">
  import Icon from './lib/Icon.svelte'
  import { t } from './lib/i18n/index.svelte'

  let {
    final = false,
    signingIn = false,
    onsignin,
    onsignup,
    onclose,
  }: { final?: boolean; signingIn?: boolean; onsignin: () => void; onsignup: () => void; onclose: () => void } = $props()
</script>

<div class="account" role="group" aria-labelledby="account-card-title">
  <button class="x" aria-label={final ? t('welcome.dont_show') : t('common.close')} title={final ? t('welcome.dont_show') : t('common.close')} onclick={onclose}><Icon name="x" size={13} stroke={2.2} /></button>
  <span class="eyebrow"><Icon name="globe" size={14} stroke={2} />{t('welcome.card_eyebrow')}</span>
  <strong id="account-card-title">{t('welcome.card_title')}</strong>
  <span class="text">{t('welcome.card_text')}</span>
  <span class="buttons">
    <button class="go" disabled={signingIn} onclick={onsignin}>{signingIn ? t('account.signing_in') : t('account.sign_in')}</button>
    <button class="other" onclick={onsignup}>{t('welcome.create_account')}</button>
  </span>
</div>

<style>
  .account {
    position: relative;
    grid-column: span 2;
    min-width: 380px;
    min-height: clamp(104px, 15vh, 132px);
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 16px;
    border: 1px solid var(--signal-strong);
    border-radius: 16px;
    background: linear-gradient(160deg, #1b1640 0%, #0e1020 100%);
    box-shadow: 0 0 26px rgb(109 91 255 / 0.3);
    animation: light 1.2s var(--ease-out);
  }

  @keyframes light {
    from {
      box-shadow: 0 0 0 1px var(--signal), 0 0 30px rgb(109 91 255 / 0.6);
    }
  }

  .eyebrow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-right: 28px;
    color: #d4ceff;
    letter-spacing: 0.14em;
  }

  strong {
    font-size: 16px;
    font-weight: 700;
    font-stretch: 112%;
    line-height: 1.2;
  }

  .text {
    font-size: 13px;
    line-height: 1.35;
    color: #c9c5e2;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: auto;
    padding-top: 4px;
  }

  /* Un bouton tient sur une ligne. */
  .buttons button {
    height: 30px;
    padding: 0 14px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }

  .go {
    border: 0;
    background: var(--signal-strong);
    color: #fff;
  }

  .other {
    border: 1px solid #4a4380;
    background: #241e4a;
    color: var(--text);
  }

  .go:hover:not(:disabled),
  .other:hover {
    filter: brightness(1.15);
  }

  .x {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: #b4aed6;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .x:hover {
    background: rgb(255 255 255 / 0.08);
    color: var(--text);
  }
</style>
