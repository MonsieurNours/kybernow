<!--
  L'écran de connexion et de fin (docs/link-ui.md, « Connexion ») : les trois étapes, rien d'autre
  (« La Station redémarre… » au dernier recours) ; un ami qui attend l'accord de l'hôte ; puis, à la fin,
  la raison, les « Détails : » du moteur à recopier, et de quoi repartir : « Se reconnecter » ou
  « Revenir à l'accueil » (hôte), « Rejoindre » de nouveau (invité), « Revenir » ou « Fermer » (ami).
-->
<script lang="ts">
  import Mark from '../lib/Mark.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { tName } from './names'
  import type { LiveSession } from './livesession.svelte'

  let { s }: { s: LiveSession } = $props()

  const borrowing = $derived(s.borrowing)
  /** Titre de l'écran de connexion d'un ami : « Ordinateur de {hôte} ». */
  const title = $derived(borrowing ? borrowing.startTitle : s.title)
  const current = $derived(s.phase === 'live' ? 2 : s.phase === 'preparing' ? 0 : s.connectStep === 'kyber' ? 1 : 2)
  const steps = $derived([borrowing ? t('lend.steps.preparing') : t('session.start.preparing'), t('session.start.connecting'), t('session.start.firstImages')])
</script>

<div class="overlay" role="status">
  {#if s.phase === 'ended'}
    <div class="card">
      <Mark size={34} state="now" />
      <h2 class="disp">{s.wasLive || borrowing?.over ? t('session.start.ended') : t('session.start.failed')}</h2>
      <p>{s.error}</p>
      {#if s.errorDetail}<p class="detail">{t('session.error.details')}{' '}<code>{s.errorDetail}</code></p>{/if}
      {#if borrowing && !borrowing.over}<p class="comeback">{t('lend.friend.comeBackHint')}</p>{/if}
      <div class="row">
        {#if s.guest}
          <!-- Un invité sorti du partage ne le reprend pas : il rejoint de nouveau avec un code. -->
          <button type="button" class="primary" onclick={() => s.quit()}>{t('share.join.button')}</button>
        {:else if borrowing}
          <!-- Un ami : « Revenir » au même prêt tant qu'il n'est pas fini (15 min), ou « Fermer ». -->
          {#if !borrowing.over}<button type="button" class="primary" onclick={() => borrowing.comeBack()}>{t('lend.friend.comeBack')}</button>{/if}
          <button type="button" class={borrowing.over ? 'primary' : 'ghost'} onclick={() => s.quit()}>{t('lend.close')}</button>
        {:else}
          <button type="button" class="primary" onclick={() => s.reconnect()}>{t('session.start.reconnect')}</button>
          <button type="button" class="ghost" onclick={() => s.quit()}>{t('session.start.home')}</button>
        {/if}
      </div>
    </div>
  {:else if s.phase === 'waiting' && borrowing}
    <!-- Un ami attend l'accord de l'hôte (ordinateur utilisé). -->
    <div class="card">
      <Mark size={34} state="now" lit breathe />
      <h2 class="disp">{title}</h2>
      <p class="waiting"><span class="wdot" aria-hidden="true"></span>{borrowing.host ? tName('lend.friend.waiting', 'host', borrowing.host) : t('share.join.waiting')}</p>
      <button type="button" class="ghost" onclick={() => borrowing.cancel()}>{t('share.cancel')}</button>
    </div>
  {:else}
    <div class="card">
      <Mark size={34} state="now" lit breathe />
      <h2 class="disp">{title}</h2>
      <ol class="steps">
        {#if s.phase === 'restarting'}
          <li class="current" aria-current="step"><span class="mark" aria-hidden="true"></span>{t('web.station.restarting')}</li>
        {:else}
          {#each steps as label, i (i)}
            <li class:done={i < current} class:current={i === current} aria-current={i === current ? 'step' : undefined}><span class="mark" aria-hidden="true"></span>{label}</li>
          {/each}
        {/if}
      </ol>
      <button type="button" class="ghost" onclick={() => (borrowing ? borrowing.cancel() : s.quit())}>{t('session.start.cancel')}</button>
    </div>
  {/if}
</div>

<style>
  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 24px;
    background: radial-gradient(60% 50% at 50% 40%, rgb(109 91 255 / 0.14), transparent 70%), var(--ink-0);
  }

  .card {
    width: min(480px, 100%);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 16px;
  }

  h2 {
    font-size: clamp(26px, 4vw, 36px);
    line-height: 1.05;
    font-weight: 650;
    letter-spacing: -0.03em;
  }

  p {
    color: var(--muted);
  }

  .comeback {
    color: var(--signal-light);
  }

  /* « Détails » : le message du moteur, à recopier tel quel. */
  .detail {
    font-size: 12px;
    opacity: 0.75;
    overflow-wrap: anywhere;
  }

  .detail code {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    user-select: all;
    -webkit-user-select: all;
  }

  .waiting {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .wdot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    background: var(--signal);
    box-shadow: 0 0 10px var(--signal);
    animation: step 0.8s ease-in-out infinite alternate;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 6px;
  }

  /* Les trois étapes de la connexion : faite, en cours (point qui respire), à venir. */
  .steps {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 14px;
    color: var(--ghost);
  }

  .steps li {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .steps li.done,
  .steps li.current {
    color: var(--text);
  }

  .steps .mark {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    box-shadow: inset 0 0 0 1px var(--ghost);
  }

  .steps li.done .mark {
    background: var(--signal-light);
    box-shadow: 0 0 6px rgb(154 140 255 / 0.8);
  }

  .steps li.current .mark {
    width: 10px;
    height: 10px;
    margin: 0 -1px;
    box-shadow: inset 0 0 0 2px var(--text);
    animation: step 1.2s ease-in-out infinite alternate;
  }

  @keyframes step {
    to {
      opacity: 0.35;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .steps li.current .mark {
      animation: none;
    }
  }
</style>
