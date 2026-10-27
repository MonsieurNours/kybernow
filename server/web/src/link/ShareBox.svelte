<!--
  « Partager » dans la barre de session (KyberNow Share, docs/link-ui.md) : le
  panneau de l'hôte. Il ouvre le partage de cette session au premier affichage
  (l'écran principal, et le son de l'ordinateur), puis montre le code, le lien,
  son QR code, les invités et leurs droits : le même panneau que celui d'Admin
  (`SharePanel.svelte`), sur les routes de l'hôte.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { backend } from './backend'
  import type { ShareView } from '../lib/api'
  import { errorText } from '../lib/store.svelte'
  import SharePanel from '../lib/SharePanel.svelte'
  import Icon from '../lib/Icon.svelte'
  import { t } from '../lib/i18n/index.svelte'

  let {
    share,
    support,
    computer,
    onshare,
    onclose,
  }: {
    /** Le partage ouvert (signe de vie de l'hôte), ou `null`. */
    share: ShareView | null
    /** `ok`, `old` (Station d'avant la 0.8.0) ou `off` (partage coupé sur l'ordinateur). */
    support: 'ok' | 'old' | 'off'
    /** Nom de l'ordinateur : titre du panneau. */
    computer: string
    onshare: (view: ShareView | null) => void
    onclose: () => void
  } = $props()

  /** Refus ou échec de l'ouverture, dans la langue d'Admin. */
  let refusal = $state('')
  /** Résultat de la dernière action (copie du lien, partage arrêté…). */
  let message = $state<{ kind: 'ok' | 'error'; text: string } | null>(null)
  let timer: ReturnType<typeof setTimeout> | null = null

  async function open() {
    refusal = ''
    try {
      onshare(await backend.hostShare.open())
    } catch (e) {
      refusal = errorText(e)
    }
  }

  onMount(() => {
    if (support === 'ok' && !share) open()
    return () => {
      if (timer) clearTimeout(timer)
    }
  })

  function say(kind: 'ok' | 'error', text: string) {
    message = { kind, text }
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => (message = null), 4000)
  }

  /** Partage relu après une erreur (invité parti, partage fermé entre-temps). */
  async function reread() {
    try {
      const view = await backend.hostShare.get()
      onshare('open' in view && view.open ? (view as ShareView) : null)
    } catch {
      // Le prochain signe de vie le dira.
    }
  }

  function changed(view: ShareView | null) {
    onshare(view)
    // Partage arrêté : le panneau n'a plus rien à montrer.
    if (!view) onclose()
  }
</script>

<section class="sharebox glass" aria-label={t('share.bar.share')}>
  <button type="button" class="close" aria-label={t('web.settings.close')} onclick={onclose}><Icon name="x" size={16} stroke={2} /></button>
  {#if support === 'old'}
    <p class="reason">{t('share.bar.tooOld')}</p>
  {:else if support === 'off'}
    <p class="reason">{t('share.off')}</p>
  {:else if share}
    <SharePanel {share} {computer} own ops={backend.hostShare} onchange={changed} notify={say} refresh={reread} />
  {:else if refusal}
    <p class="reason bad" role="alert">{refusal}</p>
    <button type="button" class="ghost" onclick={open}>{t('web.home.retry')}</button>
  {:else}
    <p class="reason" aria-busy="true">{t('web.gate.wait')}</p>
  {/if}
  {#if message}<p class="message" class:bad={message.kind === 'error'} role="status">{message.text}</p>{/if}
</section>

<style>
  .glass {
    background: rgb(9 12 17 / 0.78);
    backdrop-filter: blur(26px) saturate(1.4);
    -webkit-backdrop-filter: blur(26px) saturate(1.4);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1), inset 0 1px 0 rgb(255 255 255 / 0.07), 0 18px 50px -12px rgb(0 0 0 / 0.7);
  }

  .sharebox {
    position: absolute;
    left: 50%;
    top: 80px;
    translate: -50% 0;
    width: min(720px, calc(100% - 32px));
    max-height: calc(100% - 100px);
    overflow-y: auto;
    border-radius: 16px;
    padding: 22px 24px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    z-index: 6;
    user-select: text;
    -webkit-user-select: text;
  }

  .close {
    position: absolute;
    right: 12px;
    top: 12px;
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: var(--muted);
    display: grid;
    place-items: center;
    z-index: 1;
  }

  .close:hover {
    background: rgb(255 255 255 / 0.07);
    color: #fff;
  }

  .reason {
    font-size: 14px;
    line-height: 1.5;
    color: var(--text-2);
    padding-right: 36px;
  }

  .reason.bad,
  .message.bad {
    color: var(--bad-text);
  }

  .ghost {
    align-self: flex-start;
  }

  .message {
    font-size: 13px;
    color: var(--ok-text);
  }

  /* Tactile : le panneau prend la largeur, sous la barre compacte. */
  @media (max-width: 760px) {
    .sharebox {
      top: 64px;
      padding: 18px 16px 16px;
      max-height: calc(100% - 76px);
    }
  }
</style>
