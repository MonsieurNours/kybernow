<!--
  Rubrique Station des réglages d'un ordinateur : version de KyberNow Station,
  Admin dans le navigateur, redémarrage et mise à jour de la Station (avec
  confirmation quand un appareil est en session), compte, retrait.

  Après un redémarrage ou une mise à jour, l'app (côté Rust, `follow_loop`
  de ui.rs, le même suivi que pour le menu de l'icône) interroge la Station
  toutes les 2 s jusqu'à son retour (et sa nouvelle version pour une mise à
  jour) : `station_pending` pendant l'attente, `station-outcome` à la fin.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import type { Computer, StationOutcome, StationRefusal } from './lib/api'
  import { invoke, listen } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let { computer, onremoved }: { computer: Computer; onremoved: () => void } = $props()

  type Kind = 'restart' | 'update'

  /** Appel en cours (une mise à jour attend qu'Admin ait téléchargé le paquet). */
  let calling = $state<Kind | null>(null)
  /** Attente du retour de la Station après un appel accepté (suivie par l'app). */
  const pending = $derived(computer.station_pending ?? null)
  let outcome = $state<{ text: string; tone: 'ok' | 'warn' } | null>(null)
  /** Un appareil est en session : confirmer avant de couper. */
  let confirm = $state<{ kind: Kind; device: string | null } | null>(null)
  let confirmRemove = $state(false)

  const station = $derived(computer.info?.station ?? null)
  const offline = $derived(computer.online === false)
  /**
   * Joignable seulement par le relais : l'interface d'Admin ne passe jamais par lui (« Ouvrir Admin » n'ouvrirait rien),
   * et la Station ne se redémarre ni ne se met à jour que depuis le réseau de l'ordinateur.
   */
  const away = $derived(computer.away === true)
  const update = $derived(station?.update.available && station.update.version ? station.update : null)
  const restarting = $derived(calling === 'restart' || pending === 'restart' || !!station?.restarting)
  const installing = $derived(calling === 'update' || pending === 'update' || !!station?.update.installing)
  const busy = $derived(restarting || installing)
  /** Station connue mais trop ancienne : ni redémarrage ni mise à jour depuis l'app. */
  const tooOld = $derived(computer.online === true && computer.info != null && station == null)
  const version = $derived(station?.version || computer.info?.admin_version || null)

  // Fin du suivi (lancé d'ici ou du menu de l'icône) : les textes de la rubrique.
  // Revenue sans être partie (`silent`) : le redémarrage n'a pas eu lieu, on ne prétend rien.
  onMount(() => {
    const off = listen<StationOutcome>('station-outcome', (o) => {
      if (o.id !== computer.id) return
      if (o.outcome === 'restarted') outcome = { text: t('station.restarted'), tone: 'ok' }
      else if (o.outcome === 'updated') outcome = { text: t('station.updated', { version: o.version ?? '' }), tone: 'ok' }
      else if (o.outcome === 'update_failed') outcome = { text: t('station.update_failed'), tone: 'warn' }
      else if (o.outcome === 'slow_back') outcome = { text: t('station.slow_back'), tone: 'warn' }
    })
    return () => off.then((f) => f())
  })

  function refusal(e: unknown, kind: Kind): string | null {
    const r = (typeof e === 'object' && e && 'kind' in e ? e : { kind: 'other', message: String(e), device: null, seconds: null }) as StationRefusal
    switch (r.kind) {
      case 'in_session':
        confirm = { kind, device: r.device }
        return null
      case 'too_soon':
        return t('station.too_soon')
      case 'unsupported':
        return t('station.too_old')
      case 'requires_setup':
        return t('station.requires_setup', { version: update?.version ?? '' })
      default:
        return r.message
    }
  }

  async function run(kind: Kind, force = false) {
    confirm = null
    outcome = null
    calling = kind
    try {
      await invoke(kind === 'restart' ? 'station_restart' : 'station_update', { id: computer.id, force })
    } catch (e) {
      const text = refusal(e, kind)
      if (text) outcome = { text, tone: 'warn' }
    } finally {
      calling = null
    }
  }

  async function openAdmin() {
    try {
      await invoke('open_admin', { id: computer.id })
    } catch (e) {
      outcome = { text: String(e), tone: 'warn' }
    }
  }

  async function remove() {
    await invoke('remove_computer', { id: computer.id })
    onremoved()
  }
</script>

<section class="card">
  <div class="version">
    <div>
      <div class="name">{version ? t('station.name_version', { version }) : t('station.name')}</div>
      {#if station}
        <div class="state" class:avail={!!update}>
          {update ? t('station.available', { version: update.version ?? '' }) : t('station.up_to_date')}
        </div>
      {/if}
    </div>
    {#if busy}
      <div class="progress" role="status"><span class="dot"></span>{restarting ? t('station.restarting') : t('station.updating')}</div>
    {/if}
  </div>
  {#if outcome}<p class="outcome" class:warn={outcome.tone === 'warn'} role="status">{outcome.text}</p>{/if}
  {#if offline && !busy}<p class="note">{t('station.offline')}</p>{/if}
  {#if tooOld}<p class="note">{t('station.too_old')}</p>{/if}

  <div class="line">
    <p class="text">{t('station.admin_help')}</p>
    <button class="ghost" disabled={away} onclick={openAdmin}><Icon name="globe" size={16} /> {t('station.open_admin')}</button>
  </div>

  {#if station}
    <div class="line">
      <p class="text">{t('station.restart_help')}</p>
      <button class="ghost" disabled={busy || offline || away} onclick={() => run('restart')}><Icon name="restart" size={16} /> {t('station.restart')}</button>
    </div>
    {#if update?.installable}
      <div class="line">
        <p class="text">{t('station.update_help', { version: update.version ?? '' })}</p>
        <button class="primary small" disabled={busy || offline || away} onclick={() => run('update')}><Icon name="download" size={16} /> {t('station.update')}</button>
      </div>
    {:else if update?.requires_setup}
      <p class="note">{t('station.requires_setup', { version: update.version ?? '' })}</p>
    {/if}
  {/if}
</section>

<section class="card">
  <div class="line">
    <span>{t('station.account')}</span>
    <span class="account">
      {#if computer.by_code || computer.info?.account?.own_device}{t('station.own_access')}<span class="mono user">{computer.user}</span>{:else}<span class="mono">{computer.user}</span>{/if}
    </span>
  </div>
  <div class="line remove">
    {#if confirmRemove}
      <span>{t('station.remove_confirm', { name: computer.name })}</span>
      <span class="buttons">
        <button class="danger" onclick={remove}>{t('station.remove')}</button>
        <button class="ghost" onclick={() => (confirmRemove = false)}>{t('common.cancel')}</button>
      </span>
    {:else}
      <button class="ghost" onclick={() => (confirmRemove = true)}><Icon name="trash" size={16} /> {t('station.remove_computer')}</button>
    {/if}
  </div>
</section>

{#if confirm}
  {@const c = confirm}
  <div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && (confirm = null)}>
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby="confirm-text">
      <h2 id="confirm-title">{c.device ? t('station.in_session_title', { device: c.device }) : t('station.in_session_anonymous')}</h2>
      <p id="confirm-text">{c.kind === 'restart' ? t('station.in_session_restart') : t('station.in_session_update')}</p>
      <div class="buttons">
        <button class="ghost" onclick={() => (confirm = null)}>{t('common.cancel')}</button>
        <button class="danger" onclick={() => run(c.kind, true)}>{c.kind === 'restart' ? t('station.restart_anyway') : t('station.update_anyway')}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    padding: 18px 22px 8px;
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--ink-1);
  }

  .version {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 10px 20px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--ink-3);
  }

  .name {
    font-size: 18px;
    font-weight: 600;
  }

  .state {
    margin-top: 4px;
    font-size: 13px;
    color: var(--ok);
  }

  .state.avail {
    color: var(--signal);
  }

  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--text);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 4px;
    background: var(--signal);
    box-shadow: 0 0 10px var(--signal);
    animation: blink 1.2s ease-in-out infinite;
  }

  @keyframes blink {
    50% {
      opacity: 0.3;
    }
  }

  p {
    margin: 12px 0 0;
    font-size: 13px;
    line-height: 1.5;
  }

  .outcome {
    color: var(--ok);
  }

  .outcome.warn,
  .note {
    color: var(--warn);
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 10px 20px;
    padding: 14px 0;
    border-bottom: 1px solid var(--ink-3);
    font-size: 14px;
  }

  .line:last-child {
    border-bottom: 0;
  }

  .line .text {
    flex: 1 1 280px;
    margin: 0;
    font-size: 13px;
    color: var(--muted);
  }

  .line .ghost,
  .line .primary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }

  .primary.small {
    height: 40px;
    padding: 0 18px;
    font-size: 14px;
    clip-path: polygon(0 0, 100% 0, 100% calc(100% - 9px), calc(100% - 9px) 100%, 0 100%);
  }

  .account {
    display: flex;
    align-items: baseline;
    gap: 10px;
    text-align: right;
  }

  .account .mono {
    font-size: 12px;
  }

  .user {
    color: var(--faint);
  }

  .remove {
    color: var(--muted);
  }

  .buttons {
    display: flex;
    gap: 10px;
  }

  .danger {
    height: 40px;
    padding: 0 16px;
    border: 0;
    border-radius: 8px;
    background: #c42b1c;
    color: #fff;
    font-weight: 600;
  }

  .veil {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 56px 24px 24px;
    background: rgb(5 7 10 / 0.7);
  }

  .dialog {
    width: min(480px, 100%);
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 28px 30px 24px;
    border: 1px solid var(--line-2);
    border-radius: 18px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
  }

  .dialog h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }

  .dialog p {
    margin: 0;
    font-size: 14px;
    color: var(--muted);
  }

  .dialog .buttons {
    justify-content: flex-end;
    margin-top: 8px;
  }
</style>
