<!--
  Prêter l'ordinateur, sur l'accueil d'Admin (docs/pret.md) : « Prêter
  l'ordinateur… », puis le panneau du code (durée, compte Steam, code, lien,
  QR code). Les demandes des amis et un code en cours s'y montrent d'emblée.
  Pendant un prêt, c'est le haut de l'accueil qui le dit (« Prêté à Léa »).
-->
<script lang="ts">
  import { app } from './store.svelte'
  import { isLent } from './lend'
  import Icon from './Icon.svelte'
  import LendPanel from './LendPanel.svelte'
  import { t } from './i18n/index.svelte'

  let open = $state(false)

  const lend = $derived(app.status?.lend ?? null)
  const computer = $derived(app.status?.hostname ?? t('overview.thisComputer'))
  /** Un code en cours ou des demandes : le panneau, sans attendre un clic. */
  const shown = $derived(!!lend && (open || !!lend.code || lend.requests.length > 0))
</script>

{#if lend && !isLent(lend)}
  {#if shown}
    <section class="card lend-open" aria-label={t('lend.open')}>
      <LendPanel view={lend} {computer} oncancel={open && !lend.code ? () => (open = false) : undefined} />
    </section>
  {:else}
    <section class="card start" aria-labelledby="t-lend">
      <span class="tile-icon" aria-hidden="true"><Icon name="lend" size={20} stroke={1.7} /></span>
      <div class="text">
        <h2 id="t-lend" class="sr-only">{t('lend.open')}</h2>
        {#if !lend.enabled}
          <p><a href="#/pret">{t('lend.off')}</a></p>
        {:else}
          <p>{t('lend.startHelp')}</p>
        {/if}
      </div>
      <button type="button" class="ghost" disabled={!lend.enabled} onclick={() => (open = true)}><Icon name="lend" size={15} />{t('lend.open')}</button>
    </section>
  {/if}
{/if}

<style>
  .lend-open {
    position: relative;
    padding: 26px 28px 22px;
    background: linear-gradient(135deg, rgb(109 91 255 / 0.1), rgb(109 91 255 / 0) 55%), var(--card);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .start {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    gap: 14px 18px;
    align-items: center;
    padding: 18px 22px;
  }

  .tile-icon {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--signal-light);
    align-self: start;
  }

  .text {
    min-width: 0;
  }

  .text p {
    font-size: 13.5px;
    line-height: 1.5;
    color: var(--text-2);
    max-width: 64ch;
  }

  .text a {
    color: var(--text-2);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .text a:hover {
    color: var(--text);
  }

  @media (max-width: 760px) {
    .lend-open {
      padding: 20px 18px 16px;
    }

    .start {
      grid-template-columns: 40px minmax(0, 1fr);
      padding: 16px;
    }

    .start > button {
      grid-column: 2;
      justify-self: start;
    }
  }
</style>
