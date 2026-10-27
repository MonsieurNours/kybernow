<!--
  Partager un écran du PC, sur l'accueil d'Admin : « Partager un écran… », le
  choix de l'écran (l'écran principal d'abord), puis le panneau du partage.
  Un partage ouvert depuis la session d'un appareil s'y montre aussi.
-->
<script lang="ts">
  import { api, ApiError, type LocalDisplay } from './api'
  import { app, errorText, refreshStatus, setShare } from './store.svelte'
  import { screenLine } from './format'
  import Icon from './Icon.svelte'
  import SharePanel from './SharePanel.svelte'
  import { t } from './i18n/index.svelte'

  /** Écrans du PC, une fois « Partager un écran… » choisi (`null` : en lecture). */
  let displays = $state<LocalDisplay[] | null>(null)
  let choosing = $state(false)
  let opening = $state(false)
  /** Refus d'Admin (partage coupé, appareil en session…), dans la langue d'Admin. */
  let refusal = $state<string | null>(null)

  const status = $derived(app.status)
  const share = $derived(status?.share ?? null)
  const live = $derived(status?.activity?.live ?? null)
  const enabled = $derived(status?.share_enabled !== false)
  // Pendant la session d'un appareil, c'est lui qui partage (« Partager » dans sa barre).
  const reason = $derived(!enabled ? t('share.off') : live ? t('share.inSession', { name: live.name }) : null)

  async function choose() {
    choosing = true
    refusal = null
    displays = null
    try {
      displays = await api.displays()
    } catch (e) {
      refusal = errorText(e)
      choosing = false
    }
  }

  async function open(d: LocalDisplay) {
    opening = true
    refusal = null
    try {
      setShare(await api.shareOpen(d.id))
      choosing = false
    } catch (e) {
      refusal = errorText(e)
      if (e instanceof ApiError && ['disabled', 'session', 'busy', 'other_share'].includes(e.code ?? '')) {
        choosing = false
        refreshStatus()
      }
    } finally {
      opening = false
    }
  }

  // Un partage s'ouvre ailleurs (icône, appareil) : le choix en cours n'a plus lieu d'être.
  $effect(() => {
    if (share) {
      choosing = false
      refusal = null
    }
  })
</script>

{#if share}
  <section class="card share-open">
    <SharePanel
      {share}
      computer={status?.hostname ?? t('overview.thisComputer')}
      host={share.origin === 'host' && live ? { name: live.name, kind: live.kind, since: live.started_at } : null}
    />
  </section>
{:else}
  <section class="card start" aria-labelledby="t-share">
    <span class="tile-icon" aria-hidden="true"><Icon name="share" size={20} stroke={1.7} /></span>
    {#if choosing}
      <div class="text">
        <h2 id="t-share" class="sr-only">{t('share.start')}</h2>
        <p>{displays ? t('share.choose') : t('share.loading')}</p>
        {#if displays}
          <div class="screens">
            {#each displays as d (d.id)}
              <button type="button" class="screen" disabled={opening} onclick={() => open(d)}>
                <Icon name="display" size={18} />
                {screenLine(d)}
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <button type="button" class="ghost" disabled={opening} onclick={() => (choosing = false)}>{t('share.cancel')}</button>
    {:else}
      <div class="text">
        <h2 id="t-share" class="sr-only">{t('share.start')}</h2>
        {#if !enabled}
          <p><a href="#/appareils">{reason}</a></p>
        {:else}
          <p>{reason ?? t('share.startHelp')}</p>
        {/if}
      </div>
      <button type="button" class="ghost" disabled={!!reason} onclick={choose}><Icon name="share" size={15} />{t('share.start')}</button>
    {/if}
    {#if refusal}
      <div class="notice bad refusal" role="alert"><Icon name="warning" size={16} /><span>{refusal}</span></div>
    {/if}
  </section>
{/if}

<style>
  .share-open {
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
    display: flex;
    flex-direction: column;
    gap: 12px;
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

  .screens {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .screen {
    height: 44px;
    padding: 0 16px 0 12px;
    border: 0;
    border-radius: 12px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    transition: box-shadow 0.2s, background-color 0.2s;
  }

  .screen :global(svg) {
    color: var(--signal-light);
  }

  .screen:hover:not(:disabled) {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .refusal {
    grid-column: 2 / -1;
  }

  @media (max-width: 760px) {
    .share-open {
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

    .screen {
      width: 100%;
    }
  }
</style>
