<!-- Étape 4 · Installation : les tâches défilent, la dernière ligne du journal en bas. -->
<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte'
  import { send } from '../bridge'
  import Glyph from '../Glyph.svelte'
  import { app, go, TASK_ORDER } from '../state.svelte'

  let showLog = $state(false)
  let logBox: HTMLElement | undefined = $state()

  const failed = $derived(!!app.failure)
  const last = $derived(app.log.at(-1) ?? '')
  // Une estimation grossière : l'installation prend une minute environ.
  const remaining = $derived(Math.max(5, Math.round(((100 - app.percent) / 100) * 60)))

  $effect(() => {
    if (showLog && app.log.length && logBox) logBox.scrollTop = logBox.scrollHeight
  })

  function statusText(id: string): string {
    const s = app.tasks[id]
    if (s === 'pending') return id === 'admin' ? t('mac.install.port', { port: app.ports.console }) : t('mac.install.coming')
    return t(`mac.install.status.${s}`)
  }
</script>

<div class="install">
  <span class="lbl-m kn-enter" class:bad={failed}>
    {failed ? t('mac.install.labelFailed') : t('mac.install.label', { percent: Math.round(app.percent) })}
  </span>
  <h1 class="t kn-enter kn-d1">{failed ? t('mac.install.titleFailed') : t('mac.install.title')}</h1>
  <p class="lead-m kn-enter kn-d2">{failed ? app.failure!.message : t('mac.install.lead')}</p>

  <div class="bar" aria-hidden="true"><div style:width="{app.percent}%" class:bad={failed}></div></div>

  {#if showLog}
    <pre class="log mono-m" bind:this={logBox}>{app.log.join('\n')}</pre>
  {:else}
    <div class="tasks">
      {#each TASK_ORDER as id (id)}
        {@const s = app.tasks[id]}
        <Glyph status={s} />
        <span class:later={s === 'pending' || s === 'skipped'}>
          <b>{t(`mac.install.tasks.${id}`)}</b>
          <span class="detail">{app.taskDetail[id] ?? t(`mac.install.tasks.${id}Detail`)}</span>
        </span>
        <span class="state {s}" class:mono-m={s === 'pending' && id === 'admin'}>{statusText(id)}</span>
      {/each}
    </div>
  {/if}
</div>

<div class="foot-m">
  <span class="cmd mono-m">{last ? `› ${last}` : ''}</span>
  {#if failed}
    <div class="buttons">
      <button type="button" class="btn" onclick={() => (showLog = !showLog)}>{showLog ? t('mac.install.hideDetail') : t('mac.install.detail')}</button>
      <button type="button" class="btn" onclick={() => go('options')}>{t('common.back')}</button>
      <button type="button" class="btn primary-m" onclick={() => send({ cmd: 'close' })}>{t('mac.install.quit')}</button>
    </div>
  {:else}
    <span class="eta">
      {app.percent >= 100 ? t('mac.install.almost') : t('mac.install.remaining', { seconds: remaining })} ·
      <button type="button" class="link" onclick={() => (showLog = !showLog)}>{showLog ? t('mac.install.hideDetail') : t('mac.install.detail')}</button>
    </span>
  {/if}
</div>

<style>
  .install {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }

  .lbl-m.bad {
    color: var(--bad);
  }

  .bar {
    margin-top: 18px;
    flex: none;
  }

  .bar > div.bad {
    background: var(--bad);
    box-shadow: none;
  }

  .tasks {
    margin-top: 16px;
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    column-gap: 12px;
    row-gap: 12px;
    font-size: 13.5px;
    align-items: start;
  }

  b {
    font-weight: 600;
  }

  .detail {
    display: block;
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--faint);
  }

  .later {
    color: var(--faint);
  }

  .later .detail {
    color: #65737e;
  }

  .state {
    font-size: 12px;
    color: #65737e;
    white-space: nowrap;
  }

  .state.done {
    color: var(--ok);
  }

  .state.running {
    color: var(--signal);
  }

  .state.warn {
    color: var(--warn);
  }

  .state.failed {
    color: var(--bad);
  }

  .log {
    margin: 16px 0 0;
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px 14px;
    border-radius: 10px;
    background: #090d12;
    border: 1px solid #1e2832;
    font-size: 11px;
    line-height: 1.6;
    color: var(--muted);
    white-space: pre-wrap;
    user-select: text;
  }

  .foot-m {
    padding-top: 16px;
  }

  .cmd {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11.5px;
    color: #65737e;
  }

  .eta {
    flex: none;
    font-size: 12.5px;
    color: var(--faint);
  }
</style>
