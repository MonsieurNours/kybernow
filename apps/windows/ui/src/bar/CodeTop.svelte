<!--
  Le code et son lien, partagés par « Partager » et « Prêter l'ordinateur… » : « 123 456 », « Copier le lien », le QR code
  (un indicateur d'attente tant que le lien du relais se prépare) et, copie impossible, l'adresse à copier à la main.
  Styles : bar/panels.css.
-->
<script lang="ts">
  import Icon from '../lib/Icon.svelte'
  import type { ShareQr } from '../lib/api'
  import { t } from '../lib/i18n/index.svelte'
  import type { RelayPhase } from '../lib/relay.svelte'

  let {
    code,
    qr,
    address,
    phase,
    done,
    copy,
  }: { code: string; qr: ShareQr | null; address: string; phase: RelayPhase; done: 'ok' | 'fail' | null; copy: () => void } = $props()

  /** Le lien se sélectionne en entier au premier clic (copie à la main). */
  const selectAll = (e: Event) => (e.currentTarget as HTMLInputElement).select()
</script>

<div class="top">
  <div class="codebox">
    <span class="eyebrow">{t('share.code')}</span>
    <span class="digits mono" aria-label={t('share.code') + ' ' + code}>{code}</span>
    <button class="ghost" onclick={copy} disabled={phase === 'waiting'}><Icon name="copy" size={16} stroke={1.6} />{t('share.copy_link')}</button>
    {#if done === 'ok'}<span class="copied" role="status">{t('share.link_copied')}</span>{/if}
  </div>
  {#if phase === 'waiting'}
    <div class="qr qr-wait" role="img" aria-label={t('share.relay_preparing')}><span class="spin big"></span></div>
  {:else if qr}
    <svg class="qr" role="img" aria-label={t('share.qr')} viewBox="-4 -4 {qr.size + 8} {qr.size + 8}" shape-rendering="crispEdges">
      <rect x="-4" y="-4" width={qr.size + 8} height={qr.size + 8} fill="#fff" />
      <path d={qr.path} fill="#000" />
    </svg>
  {/if}
</div>
{#if done === 'fail'}
  <!-- Copie impossible : l'adresse reste là, sélectionnée d'un clic. -->
  <p class="note refusal" role="alert">{t('share.copy_failed')}</p>
  <input class="field link mono" readonly value={address} onfocus={selectAll} onclick={selectAll} aria-label={t('share.copy_link')} />
{/if}

<style>
  /* Le style de base des boutons d'un menu de la barre (Bar.svelte, `.menu button`), pour les boutons de ce composant. */
  :global(.menu) button {
    display: grid;
    grid-template-columns: 18px auto 1fr;
    align-items: center;
    gap: 8px;
    padding: 9px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text);
    text-align: left;
    font-size: 13px;
  }

  :global(.menu) button:hover {
    background: rgb(255 255 255 / 0.07);
  }
</style>
