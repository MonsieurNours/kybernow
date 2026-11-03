<!--
  Fenêtre de désinstallation, depuis l'icône (« Désinstaller KyberNow
  Station… ») ou l'app « Désinstaller » du .dmg : ce qui part, et le choix
  de garder réglages et appareils pour une réinstallation.
-->
<script lang="ts">
  import Mark from '../../lib/Mark.svelte'
  import { t } from '../../lib/i18n/index.svelte'
  import { send } from '../bridge'
  import Lights from '../Lights.svelte'
  import { app } from '../state.svelte'

  const u = $derived(app.uninstall)
  const items = ['app', 'startup', 'audio', 'permissions']

  function start() {
    app.uninstall.phase = 'working'
    send({ cmd: 'uninstall', keepSettings: app.uninstall.keepSettings })
  }
</script>

<div class="solo">
  <div class="top"><Lights closeOnly disabled={u.phase === 'working'} /></div>
  <main>
    <div class="solo-head">
      <span class="dim"><Mark size={30} state="server" /></span>
      <span class="lbl-m quiet">{t('mac.uninstall.label')}</span>
    </div>

    {#if u.phase === 'done'}
      <h1 class="t small">{t('mac.uninstall.doneTitle')}</h1>
      <p class="lead-m">{u.keepSettings ? t('mac.uninstall.doneKept') : t('mac.uninstall.doneAll')}</p>
    {:else}
      <h1 class="t small">{t('mac.uninstall.title')}</h1>
      <p class="lead-m wide" class:bad={u.phase === 'failed'}>{u.phase === 'failed' ? u.error : t('mac.uninstall.lead')}</p>
      <div class="items">
        {#each items as id (id)}
          <span class="dash">–</span>
          <span>{t(`mac.uninstall.items.${id}`)}{#if id === 'audio' || id === 'permissions'}<span class="aside">&nbsp;· {t(`mac.uninstall.items.${id}Aside`)}</span>{/if}</span>
        {/each}
      </div>
      <div class="card-m keep">
        <span>
          <span class="name">{t('mac.uninstall.keep')}</span>
          <span class="detail">{t('mac.uninstall.keepDetail')}</span>
        </span>
        <button
          type="button"
          role="switch"
          class="switch"
          class:on={u.keepSettings}
          aria-checked={u.keepSettings}
          aria-label={t('mac.uninstall.keep')}
          disabled={u.phase === 'working'}
          onclick={() => (app.uninstall.keepSettings = !app.uninstall.keepSettings)}
        ><span></span></button>
      </div>
    {/if}

    <div class="foot-m">
      <p class="foot-note">{u.phase === 'done' ? '' : t('mac.uninstall.password')}</p>
      <div class="buttons">
        {#if u.phase === 'done'}
          <button type="button" class="btn primary-m" onclick={() => send({ cmd: 'close' })}>{t('mac.uninstall.close')}</button>
        {:else}
          <button type="button" class="btn" disabled={u.phase === 'working'} onclick={() => send({ cmd: 'close' })}>{t('mac.uninstall.cancel')}</button>
          <button type="button" class="btn danger" disabled={u.phase === 'working'} onclick={start}>{u.phase === 'working' ? t('mac.uninstall.working') : t('mac.uninstall.confirm')}</button>
        {/if}
      </div>
    </div>
    {#if u.phase !== 'done'}<p class="fine">{t('mac.uninstall.fine')}</p>{/if}
  </main>
</div>

<style>
  .top {
    position: relative;
    padding: 16px 20px 0;
  }

  .dim {
    display: inline-flex;
    opacity: 0.55;
    filter: grayscale(1);
  }

  .wide {
    max-width: 600px;
  }

  .lead-m.bad {
    color: var(--bad-text);
  }

  .items {
    margin-top: 18px;
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    column-gap: 12px;
    row-gap: 10px;
    font-size: 13.5px;
    line-height: 1.45;
  }

  .dash,
  .aside {
    color: var(--faint);
  }

  .keep {
    margin-top: 18px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 44px;
    gap: 20px;
    align-items: center;
    padding: 14px 18px;
  }

  .name {
    display: block;
    font-size: 14.5px;
    font-weight: 600;
  }

  .detail {
    display: block;
    margin-top: 4px;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--faint);
  }

  .foot-note {
    max-width: 330px;
  }

  .fine {
    margin: 14px 0 0;
    font-size: 12px;
    color: #65737e;
  }
</style>
