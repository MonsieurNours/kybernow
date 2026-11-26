<!--
  Après un appairage, sans compte (docs/link-ui.md « Accueil et compte KyberNow ») : sous les boutons de l'ordinateur
  ajouté, « Retrouvez-le aussi hors de chez vous et sur vos autres appareils : connectez un compte KyberNow. »,
  « Connecter un compte » et « Plus tard ». Une fois par ordinateur (`welcome_paired_hint`, côté app).
-->
<script lang="ts">
  import Icon from './lib/Icon.svelte'
  import { t } from './lib/i18n/index.svelte'

  let { signingIn = false, onsignin, onlater }: { signingIn?: boolean; onsignin: () => void; onlater: () => void } = $props()
</script>

<div class="banner" role="status">
  <Icon name="globe" size={22} stroke={1.8} />
  <p>{t('welcome.paired_text')}</p>
  <span class="buttons">
    <button class="primary small" disabled={signingIn} onclick={onsignin}>{signingIn ? t('account.signing_in') : t('account.sign_in')}</button>
    <button class="ghost" onclick={onlater}>{t('welcome.later')}</button>
  </span>
</div>

<style>
  /* La lueur de « Sur le réseau » et de la carte du compte ; mêmes boutons que le bandeau de la Station. */
  .banner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px 14px;
    max-width: 760px;
    padding: 12px 14px 12px 16px;
    border: 1px solid var(--signal-strong);
    border-radius: 12px;
    background: linear-gradient(160deg, #1b1640 0%, #0e1020 100%);
    box-shadow: 0 0 30px rgb(109 91 255 / 0.3);
    color: #d4ceff;
    animation: light 1.2s var(--ease-out);
  }

  @keyframes light {
    from {
      box-shadow: 0 0 0 1px var(--signal), 0 0 40px rgb(109 91 255 / 0.6);
    }
  }

  p {
    flex: 1 1 280px;
    margin: 0;
    font-size: 14px;
    line-height: 1.45;
    color: #dad6f2;
  }

  .buttons {
    display: flex;
    gap: 10px;
    flex: none;
  }

  .primary.small {
    height: 40px;
    padding: 0 18px;
    font-size: 14px;
    white-space: nowrap;
    clip-path: polygon(0 0, 100% 0, 100% calc(100% - 9px), calc(100% - 9px) 100%, 0 100%);
  }

  .ghost {
    border-color: transparent;
    color: #c9c5e2;
    white-space: nowrap;
  }
</style>
