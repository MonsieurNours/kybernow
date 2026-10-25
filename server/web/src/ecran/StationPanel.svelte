<!--
  Rubrique Station : sa version, Admin (un lien vers / dans ce navigateur),
  « Redémarrer la Station », « Mettre à jour la Station » quand une version
  s'installe depuis un appareil, le compte, et la fin de la session d'Admin
  dans ce navigateur (ce qui tient lieu de « Retirer cet ordinateur » : il
  n'y a qu'un ordinateur, celui qui sert la page).
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from '../lib/Icon.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { backend, type Info } from './backend'
  import { cancelConfirm, run, station } from './station.svelte'

  let { info, onSignedOut }: { info: Info | null; onSignedOut: () => void } = $props()

  let account = $state<string | null>(null)

  onMount(() => {
    backend
      .ticket()
      .then((ticket) => (account = ticket.user))
      .catch(() => {})
  })

  const st = $derived(info?.station ?? null)
  /** Station d'avant la 0.5.0 : ni version ni commandes pour les appareils. */
  const tooOld = $derived(!!info && !info.station)
  const restarting = $derived(station.running === 'restart' || !!st?.restarting)
  const updating = $derived(station.running === 'update' || !!st?.update.installing)
  const busy = $derived(restarting || updating)
  const canUpdate = $derived(!!st?.update.available && st.update.installable && !st.update.requires_setup)

  async function signOut() {
    await fetch('/api/logout', { method: 'POST', headers: { 'x-kyber-admin': '1' }, credentials: 'same-origin' }).catch(() => {})
    onSignedOut()
  }
</script>

{#if st}
  <div class="version">
    <span class="name">{t('web.station.version', { version: st.version })}</span>
    {#if st.update.available && st.update.version}
      <span class="badge new">{t('web.station.available', { version: st.update.version })}</span>
    {:else}
      <span class="badge"><Icon name="check" size={13} stroke={2.4} />{t('web.station.upToDate')}</span>
    {/if}
  </div>
{/if}

{#if tooOld}
  <p class="note">{t('web.station.tooOld')}</p>
{/if}

{#if busy}
  <p class="progress" role="status"><span class="dot" aria-hidden="true"></span>{restarting ? t('web.station.restarting') : t('web.station.updating')}</p>
{/if}

{#if station.confirm}
  {@const c = station.confirm}
  <div class="confirm" role="alertdialog" aria-labelledby="station-confirm" aria-describedby="station-confirm-text">
    <strong id="station-confirm">{t('web.station.connected', { device: c.device })}</strong>
    <p id="station-confirm-text">{c.action === 'restart' ? t('web.station.cutRestart') : t('web.station.cutUpdate')}</p>
    <div class="buttons">
      <button type="button" class="danger" onclick={() => run(c.action, true)}>{c.action === 'restart' ? t('web.station.restartAnyway') : t('web.station.updateAnyway')}</button>
      <button type="button" class="ghost" onclick={cancelConfirm}>{t('web.station.cancel')}</button>
    </div>
  </div>
{/if}

{#if station.message}
  <p class="note" role="alert">{station.message}</p>
{/if}

<div class="item">
  <a class="ghost" href="/"><Icon name="overview" size={16} />{t('web.station.openAdmin')}</a>
  <p class="sub">{t('web.station.adminHelp')}</p>
</div>

{#if st}
  <div class="item">
    <button type="button" class="ghost" disabled={busy || !!station.confirm} onclick={() => run('restart')}><Icon name="restart" size={16} />{t('web.station.restart')}</button>
    <p class="sub">{t('web.station.restartHelp')}</p>
  </div>

  {#if canUpdate && st.update.version}
    <div class="item">
      <button type="button" class="ghost accent" disabled={busy || !!station.confirm} onclick={() => run('update')}><Icon name="download" size={16} />{t('web.station.update')}</button>
      <p class="sub">{t('web.station.updateHelp', { version: st.update.version })}</p>
    </div>
  {:else if st.update.available && st.update.requires_setup && st.update.version}
    <p class="note">{t('web.station.requiresSetup', { version: st.update.version })}</p>
  {/if}
{/if}

<dl>
  {#if account}<div><dt>{t('web.station.account')}</dt><dd class="mono">{account}</dd></div>{/if}
</dl>

<div class="item">
  <button type="button" class="ghost" onclick={signOut}><Icon name="logout" size={16} />{t('web.station.signOut')}</button>
  <p class="sub">{t('web.station.signOutHelp')}</p>
</div>

<style>
  .version {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 12px;
    min-height: 32px;
  }

  .name {
    font-size: 15px;
    font-weight: 600;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 26px;
    padding: 2px 10px;
    border-radius: 13px;
    font-size: 12.5px;
    color: var(--ok-text);
    background: var(--ok-bg);
    box-shadow: inset 0 0 0 1px var(--ok-line);
  }

  .badge.new {
    color: var(--signal-light);
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .note {
    font-size: 13px;
    color: var(--warn-text);
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--warn-bg);
    box-shadow: inset 0 0 0 1px var(--warn-line);
  }

  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 14px;
    color: var(--text);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--signal);
    animation: pulse 1.2s ease-in-out infinite alternate;
  }

  @keyframes pulse {
    from {
      opacity: 1;
    }
    to {
      opacity: 0.3;
    }
  }

  .confirm {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px;
    border-radius: 12px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }

  .confirm strong {
    font-size: 14px;
  }

  .confirm p {
    font-size: 13px;
    color: var(--muted);
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }

  .item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }

  .item .ghost {
    height: 40px;
  }

  .ghost.accent {
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--signal-light);
  }

  .sub {
    font-size: 12px;
    color: var(--faint);
  }

  dl {
    margin: 0;
  }

  dl > div {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 12px;
    padding: 9px 0;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    font-size: 13.5px;
  }

  dt {
    color: var(--faint);
  }

  dd {
    margin: 0;
    text-align: right;
    font-size: 12px;
    overflow-wrap: anywhere;
  }

  @media (prefers-reduced-motion: reduce) {
    .dot {
      animation: none;
    }
  }
</style>
