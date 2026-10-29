<!--
  Un ami veut emprunter l'ordinateur pendant la session de l'hôte (docs/pret.md,
  « Quand votre ami arrive ») : « Léa veut emprunter l'ordinateur. », le compte
  Steam, « Accepter » (la session va se fermer) et « Refuser ». Visible même
  barre fermée. Ou le créneau d'un ami qui commence : « Créneau de Léa :
  l'ordinateur lui est prêté dans 2 min. ».
-->
<script lang="ts">
  import { errorText } from '../lib/store.svelte'
  import type { LendOps, LendSteam, LendView } from '../lib/api'
  import { askingRequests, lendAppName, slotStarting, steamName } from '../lib/lend'
  import Icon from '../lib/Icon.svelte'
  import Segmented from '../lib/Segmented.svelte'
  import { t, tn } from '../lib/i18n/index.svelte'

  let {
    view,
    ops,
    onlend,
    onaccepted,
  }: {
    view: LendView
    ops: LendOps
    onlend: (view: LendView) => void
    /** Demande acceptée : la session va se fermer. */
    onaccepted: (name: string) => void
  } = $props()

  let steam = $state<LendSteam>('friend')
  let busy = $state(false)
  let error = $state('')
  /** Avis du créneau refermé (jusqu'au créneau suivant). */
  let dismissed = $state('')

  const requests = $derived(askingRequests(view))
  const request = $derived(requests[0] ?? null)
  const slot = $derived(slotStarting(view))
  const slotKey = $derived(slot !== null ? `${slot} ${view.notice?.at}` : '')

  async function answer(accept: boolean) {
    if (!request || busy) return
    busy = true
    error = ''
    const name = request.name
    try {
      onlend(await ops.answer(request.id, accept, steam))
      if (accept) onaccepted(name)
    } catch (e) {
      error = errorText(e)
    } finally {
      busy = false
    }
  }
</script>

{#if request}
  <div class="lendask glass" role="alertdialog" aria-labelledby="lendask-title">
    <div class="head">
      <span class="icon"><Icon name="lend" size={18} stroke={1.7} /></span>
      <div class="who">
        <strong id="lendask-title">{t('lend.requests.asks', { name: request.name })}</strong>
        <span class="meta">{request.via === 'relay' ? t('lend.requests.away', { app: lendAppName(request.app) }) : lendAppName(request.app)}{requests.length > 1 ? ` · ${tn('share.requestCount', requests.length)}` : ''}</span>
      </div>
    </div>
    <Segmented label={t('lend.steam.label')} options={(['friend', 'host'] as LendSteam[]).map((s) => ({ value: s, label: steamName(s) }))} value={steam} disabled={busy} onchange={(s: LendSteam) => (steam = s)} />
    <p class="closes">{t('lend.requests.sessionCloses')}</p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="row">
      <button type="button" class="ghost" disabled={busy} onclick={() => answer(false)}>{t('lend.requests.refuse')}</button>
      <button type="button" class="primary" disabled={busy} onclick={() => answer(true)}><Icon name="check" size={15} stroke={2} />{t('lend.requests.accept')}</button>
    </div>
  </div>
{:else if slot !== null && dismissed !== slotKey}
  <section class="lendask glass slot" role="status">
    <div class="head">
      <span class="icon"><Icon name="clock" size={18} stroke={1.7} /></span>
      <strong class="who">{t('lend.requests.slot', { name: slot })}</strong>
      <button type="button" class="x" aria-label={t('web.settings.close')} onclick={() => (dismissed = slotKey)}><Icon name="x" size={15} stroke={2} /></button>
    </div>
  </section>
{/if}

<style>
  .glass {
    background: rgb(9 12 17 / 0.82);
    backdrop-filter: blur(26px) saturate(1.4);
    -webkit-backdrop-filter: blur(26px) saturate(1.4);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.45), inset 0 1px 0 rgb(255 255 255 / 0.07), 0 18px 50px -12px rgb(0 0 0 / 0.7);
  }

  .lendask {
    position: absolute;
    right: 16px;
    bottom: max(16px, env(safe-area-inset-bottom));
    width: min(380px, calc(100% - 32px));
    border-radius: 16px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    z-index: 6;
    user-select: none;
    animation: rise 0.35s var(--ease-out) both;
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }

  .icon {
    width: 34px;
    height: 34px;
    border-radius: 9px;
    flex: none;
    display: grid;
    place-items: center;
    background: rgb(109 91 255 / 0.16);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
    color: var(--signal-light);
  }

  .who {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 14px;
    line-height: 1.4;
  }

  .meta {
    font-size: 12.5px;
    color: var(--faint);
  }

  .closes {
    font-size: 12.5px;
    color: var(--warn-text);
  }

  .error {
    font-size: 12.5px;
    color: var(--bad-text);
  }

  .row {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .x {
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: grid;
    place-items: center;
    flex: none;
  }

  .x:hover {
    background: rgb(255 255 255 / 0.07);
    color: #fff;
  }

  @media (max-width: 760px) {
    .lendask {
      left: 16px;
      right: 16px;
      width: auto;
    }
  }
</style>
