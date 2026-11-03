<!--
  Fenêtre de mise à jour, ouverte depuis l'icône de la barre des menus
  (« Mettre à jour (x.y.z)… ») : nouveautés, progression, session coupée.
  Admin télécharge, vérifie la signature et installe ; cette fenêtre suit.
-->
<script lang="ts">
  import Mark from '../../lib/Mark.svelte'
  import { t } from '../../lib/i18n/index.svelte'
  import { send } from '../bridge'
  import Lights from '../Lights.svelte'
  import { app } from '../state.svelte'

  const u = $derived(app.update)
  const info = $derived(u.info)
  const upToDate = $derived(u.phase === 'ready' && info && !info.version)
  const working = $derived(u.phase === 'working')

  const label = $derived(
    info?.version && (u.phase === 'ready' || working)
      ? t('mac.update.label', { from: info.current, to: info.version })
      : t('mac.update.labelPlain'),
  )
  const title = $derived.by(() => {
    if (u.phase === 'checking') return t('mac.update.checking')
    if (u.phase === 'failed') return t('mac.update.failedTitle')
    if (u.phase === 'done') return t('mac.update.doneTitle', { version: info?.version ?? '' })
    if (upToDate) return t('mac.update.upToDate', { version: info!.current })
    return t('mac.update.title', { version: info!.version! })
  })
  const lead = $derived.by(() => {
    if (u.phase === 'failed') return u.error
    if (u.phase === 'done') return t('mac.update.doneLead')
    if (upToDate || u.phase === 'checking') return t('mac.update.upToDateLead')
    if (info?.refusal) return info.refusal
    return t('mac.update.lead')
  })
</script>

<div class="solo">
  <svg class="solo-mark" viewBox="0 0 380 380" aria-hidden="true">
    <path fill="#0E141A" d="M55 0 L333 0 A9 9 0 0 1 339 15 L15 339 A9 9 0 0 1 0 333 L0 55 A55 55 0 0 1 55 0 Z" />
    <path fill="#121922" d="M380 45 L380 325 A55 55 0 0 1 325 380 L45 380 A9 9 0 0 1 39 365 L365 39 A9 9 0 0 1 380 45 Z" />
    <line x1="44" y1="336" x2="336" y2="44" stroke="#9A8CFF" stroke-width="8" stroke-linecap="round" opacity=".55" />
    <line x1="44" y1="336" x2="336" y2="44" stroke="#F4F1FF" stroke-width="2.5" stroke-linecap="round" />
  </svg>
  <div class="top"><Lights closeOnly disabled={working} /></div>
  <main>
    <div class="solo-head">
      <Mark size={30} state="server" />
      <span class="lbl-m">{label}</span>
    </div>
    <h1 class="t small">{title}</h1>
    <p class="lead-m" class:bad={u.phase === 'failed'}>{lead}</p>

    {#if info?.version && info.notes.length && u.phase !== 'failed'}
      <div class="card-m notes">
        <span class="lbl-s">{t('mac.update.notes')}</span>
        {#each info.notes.slice(0, 4) as note, i (i)}<span class="note"><span class="chev">›</span>{note}</span>{/each}
        {#if info.page}<button type="button" class="link" onclick={() => send({ cmd: 'openUrl', url: info.page! })}>{t('mac.update.readAll')}</button>{/if}
      </div>
    {/if}

    {#if working || u.phase === 'done'}
      <div class="progress">
        <div class="row"><span>{t(`mac.update.phase.${u.phase === 'done' ? 'restart' : u.step}`)}</span><span class="mono-m">{Math.round(u.percent)} %</span></div>
        <div class="bar" aria-hidden="true"><div style:width="{u.percent}%"></div></div>
      </div>
    {/if}

    <div class="foot-m">
      {#if info?.session && (u.phase === 'ready' || working)}
        <p class="session"><span aria-hidden="true">●</span>{t('mac.update.session', { name: info.session })}</p>
      {:else}
        <span></span>
      {/if}
      <div class="buttons">
        {#if u.phase === 'ready' && info?.version && !info.refusal}
          <button type="button" class="btn" onclick={() => send({ cmd: 'close' })}>{t('mac.update.later')}</button>
          <button type="button" class="btn primary-m" onclick={() => send({ cmd: 'updateStart' })}>{t('mac.update.start')}</button>
        {:else if working}
          <button type="button" class="btn primary-m" disabled>{t('mac.update.working')}</button>
        {:else}
          <button type="button" class="btn primary-m" disabled={u.phase === 'checking'} onclick={() => send({ cmd: 'close' })}>{t('mac.update.close')}</button>
        {/if}
      </div>
    </div>
    <p class="fine">{t('mac.update.fine')}</p>
  </main>
</div>

<style>
  .top {
    position: relative;
    padding: 16px 20px 0;
  }

  .lead-m.bad {
    color: var(--bad-text);
  }

  .notes {
    margin-top: 18px;
    padding: 14px 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13.5px;
    line-height: 1.45;
  }

  .note {
    display: flex;
    gap: 10px;
  }

  .chev {
    color: var(--signal);
  }

  .notes .link {
    align-self: flex-start;
    font-size: 12.5px;
  }

  .progress {
    margin-top: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .row {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    color: var(--faint);
  }

  .session {
    margin: 0;
    max-width: 330px;
    display: flex;
    gap: 8px;
    font-size: 12.5px;
    color: var(--warn);
  }

  .fine {
    margin: 14px 0 0;
    font-size: 12px;
    color: #65737e;
  }
</style>
