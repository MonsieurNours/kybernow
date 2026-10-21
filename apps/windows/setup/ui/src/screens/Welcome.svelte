<!--
  Étape 1 · Bienvenue. L'analyse du PC tourne déjà à côté : rien ne change
  avant le clic sur Installer.
-->
<script lang="ts">
  import StatusIcon from '../lib/StatusIcon.svelte'
  import Toggle from '../lib/Toggle.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { app, rerunChecks, startInstall } from '../lib/state.svelte'

  const running = $derived(!app.checksDone)
  const update = $derived(!!app.existing)
</script>

<div class="welcome">
  <div class="intro">
    {#if update}
      <span class="lbl signal kn-enter">{t('welcome.updateLabel')}</span>
      <h1 class="title kn-enter kn-d1">{t('welcome.updateTitle')}</h1>
      <p class="lead kn-enter kn-d2">{t('welcome.updateLead', { from: app.existing ?? '', to: __APP_VERSION__ })}</p>
    {:else}
      <span class="lbl signal kn-enter">{t('welcome.label')}</span>
      <h1 class="title kn-enter kn-d1">{t('welcome.title')}</h1>
      <p class="lead kn-enter kn-d2">{t('welcome.lead')}</p>
    {/if}
    <p class="note kn-enter kn-d3">{t('welcome.note', { action: t(update ? 'welcome.update' : 'welcome.install') })}</p>

    <div class="option kn-enter kn-d3">
      <div>
        <span class="name">{t('welcome.desktop')}</span>
        <span class="note">{t('welcome.desktopNote')}</span>
      </div>
      <Toggle bind:checked={app.desktop} label={t('welcome.desktop')} />
    </div>
    {#if app.dir}<p class="mono where kn-enter kn-d3">{app.dir}</p>{/if}
  </div>

  <div class="panel checks kn-enter kn-d2">
    <div class="checks-head">
      <span class="lbl">{running ? t('welcome.looking') : t('welcome.seen')}</span>
      {#if !running}<button type="button" class="text-button again" onclick={rerunChecks}>{t('welcome.rerun')}</button>{/if}
    </div>
    <ul>
      {#each app.checks as check (check.id)}
        <li class:pending={check.status === 'pending'}>
          <StatusIcon status={check.status} />
          <div class="text">
            <span class="name">{check.title}</span>
            {#if check.status !== 'pending' && check.detail}
              <span class="detail" class:warn={check.status === 'warn'} class:bad={check.status === 'error'}>{check.detail}</span>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  </div>
</div>

<div class="footer">
  {#if app.running}<span class="note warn-note">{t('welcome.willClose')}</span>{/if}
  <div class="spacer"></div>
  <button type="button" class="primary" disabled={running} onclick={startInstall}>
    {running ? t('welcome.analysing') : update ? t('welcome.update') : t('welcome.install')}
  </button>
</div>

<style>
  .welcome {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 36px;
    align-items: start;
    padding-top: 16px;
  }

  .intro {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 16px;
  }

  .intro h1 {
    font-size: 36px;
  }

  .option {
    margin-top: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 16px;
    border-radius: 12px;
    background: var(--ink-1);
    box-shadow: inset 0 0 0 1px var(--line);
  }

  .option > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .option .note {
    font-size: 12px;
  }

  .where {
    margin: -6px 0 0;
    font-size: 11px;
    color: var(--ghost);
    overflow-wrap: anywhere;
  }

  .checks {
    padding: 16px 18px 6px;
    max-height: 100%;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .checks-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 6px;
  }

  .again {
    font-size: 12px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    gap: 12px;
    align-items: start;
    padding: 10px 0;
    border-top: 1px solid var(--line);
    transition: opacity 0.3s;
  }

  li.pending {
    opacity: 0.45;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .name {
    font-size: 14px;
  }

  .detail {
    font-size: 12px;
    line-height: 1.45;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  .detail.warn,
  .warn-note {
    color: var(--warn-text);
  }

  .detail.bad {
    color: var(--bad-text);
  }
</style>
