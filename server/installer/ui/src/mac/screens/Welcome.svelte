<!--
  Étape 1 · Bienvenue. L'analyse du Mac tourne pendant la lecture ; rien ne
  change sur le Mac avant « Installer ».
-->
<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte'
  import Glyph from '../Glyph.svelte'
  import { app, go, rerunChecks, validatePorts } from '../state.svelte'

  let editing = $state(false)
  let stream = $state('')
  let consolePort = $state('')

  const running = $derived(!app.checksDone)
  const problems = $derived(app.checks.filter((c) => c.status === 'error').length)

  function edit() {
    stream = String(app.ports.stream)
    consolePort = String(app.ports.console)
    app.portErrors = { stream: null, console: null }
    editing = true
  }

  async function confirmPorts() {
    if (await validatePorts(Number(stream), Number(consolePort))) editing = false
  }

  const digits = (v: string) => v.replace(/[^0-9]/g, '').slice(0, 5)
</script>

<div class="welcome">
  <span class="lbl-m kn-enter">{t('mac.welcome.label')}</span>
  <h1 class="t kn-enter kn-d1">{t('mac.welcome.title')}</h1>
  <p class="lead-m kn-enter kn-d2">{t('mac.welcome.lead')}</p>

  <div class="card-m checks kn-enter kn-d2">
    <div class="head">
      <span class="lbl-s">{running ? t('mac.welcome.checking') : t('mac.welcome.checked')}</span>
      {#if !running}<button type="button" class="again" onclick={rerunChecks}>{t('mac.welcome.rerun')}</button>{/if}
    </div>
    <div class="grid">
      {#each app.checks as check (check.id)}
        <Glyph status={check.status} />
        <span class="name" class:pending={check.status === 'pending'}>{check.title}</span>
        <span class="detail" class:bad={check.status === 'error'}>
          {#if check.id === 'ports' && app.checksDone && check.status === 'ok'}
            {#if editing}
              <span class="ports">
                <label>{t('mac.welcome.stream')} <input class="mono-m" bind:value={stream} oninput={() => (stream = digits(stream))} inputmode="numeric" aria-invalid={!!app.portErrors.stream} /></label>
                <label>{t('mac.welcome.console')} <input class="mono-m" bind:value={consolePort} oninput={() => (consolePort = digits(consolePort))} inputmode="numeric" aria-invalid={!!app.portErrors.console} /></label>
                <button type="button" class="link" disabled={app.portsPending} onclick={confirmPorts}>{t('mac.welcome.validate')}</button>
              </span>
              {#if app.portErrors.stream || app.portErrors.console}<span class="err">{app.portErrors.stream ?? app.portErrors.console}</span>{/if}
            {:else}
              <span class="mono-m pair">{t('mac.checks.portsDetail', { stream: app.ports.stream, console: app.ports.console })}</span> ·
              <button type="button" class="link" onclick={edit}>{t('mac.welcome.edit')}</button>
            {/if}
          {:else}
            {check.detail}
          {/if}
        </span>
      {/each}
    </div>
  </div>
</div>

<div class="foot-m">
  <p class="foot-note" class:bad={problems > 0}>
    {problems === 0 ? t('mac.welcome.note') : problems === 1 ? t('mac.welcome.problemOne') : t('mac.welcome.problemMany', { count: problems })}
  </p>
  <button type="button" class="btn primary-m" disabled={running || editing || problems > 0} onclick={() => go(app.existing ? 'reinstall' : 'account')}>
    {running ? t('mac.welcome.analysing') : t('common.continue')}
  </button>
</div>

<style>
  .welcome {
    padding-top: 8px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  h1 {
    max-width: 520px;
  }

  .lead-m {
    margin-top: 14px;
    max-width: 540px;
  }

  .checks {
    margin-top: 22px;
    padding: 6px 18px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 0 8px;
    border-bottom: 1px solid #1e2832;
  }

  .again {
    border: 0;
    background: none;
    color: var(--signal);
    font-size: 13px;
    padding: 0;
  }

  .grid {
    display: grid;
    grid-template-columns: 22px 150px minmax(0, 1fr);
    column-gap: 12px;
    row-gap: 9px;
    padding: 12px 0 10px;
    font-size: 13px;
    align-items: start;
  }

  .name {
    color: var(--text);
    transition: opacity 0.3s;
  }

  .name.pending {
    opacity: 0.45;
  }

  .detail {
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  .detail.bad,
  .err,
  .foot-note.bad {
    color: var(--bad-text);
  }

  .pair {
    font-size: 12.5px;
  }

  .ports {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  .ports label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--faint);
  }

  .ports input {
    width: 74px;
    height: 28px;
    padding: 0 8px;
    border-radius: 6px;
    border: 1px solid #2b3742;
    background: #05070a;
    color: var(--text);
    font-size: 12.5px;
  }

  .ports input[aria-invalid='true'] {
    border-color: var(--bad);
  }

  .err {
    display: block;
    margin-top: 4px;
    font-size: 12.5px;
  }
</style>
