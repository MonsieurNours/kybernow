<!--
  Avis au premier lancement après une mise à jour de l'app (docs/link-ui.md « Mise à jour des Stations ») : une
  fenêtre par-dessus l'accueil, jamais en session (App attend le retour à l'accueil). Une Station : « {nom} a aussi une
  mise à jour », « Mettre à jour la Station » et « Plus tard ». Plusieurs : une ligne par ordinateur (nom,
  « Version {v} », « Mettre à jour »), « Tout mettre à jour » (l'une après l'autre ; un refus laisse sa ligne avec
  « Mettre à jour quand même » et passe à la suivante) et « Plus tard ». Chaque mise à jour suit le flux des réglages
  (`station_update`, puis `station-outcome`). « Plus tard » vaut « Plus tard » pour chaque Station de la liste ;
  quand toutes sont à jour, il devient « Fermer ».
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import type { StationNotice, StationOutcome } from './lib/api'
  import { invoke, listen, stationRefusal, stationRefusalText } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let { notice, onclose }: { notice: StationNotice; onclose: () => void } = $props()

  type Row =
    | { state: 'idle' }
    | { state: 'calling' }
    /** Lancée : l'app suit la Station jusqu'à son retour. */
    | { state: 'pending' }
    | { state: 'done' }
    /** Un appareil est en session : « {appareil} est connecté », « Mettre à jour quand même ». */
    | { state: 'refused'; device: string | null }
    | { state: 'failed'; text: string }

  /** L'état de chaque ligne (absente : pas encore lancée). */
  let rows = $state<Record<string, Row>>({})
  const row = (id: string): Row => rows[id] ?? { state: 'idle' }
  const one = $derived(notice.stations.length === 1 ? notice.stations[0] : null)
  const allDone = $derived(notice.stations.every((s) => row(s.id).state === 'done'))
  const busy = $derived(notice.stations.some((s) => ['calling', 'pending'].includes(row(s.id).state)))
  /** Il en reste à lancer (« Tout mettre à jour » ne force pas une Station refusée : sa ligne le propose). */
  const remaining = $derived(notice.stations.some((s) => ['idle', 'failed'].includes(row(s.id).state)))
  let running = $state(false)

  onMount(() => {
    const off = listen<StationOutcome>('station-outcome', (o) => {
      if (!notice.stations.some((s) => s.id === o.id)) return
      if (o.outcome === 'updated') rows[o.id] = { state: 'done' }
      else if (o.outcome === 'update_failed') rows[o.id] = { state: 'failed', text: t('station.update_failed') }
      else if (o.outcome === 'slow_back') rows[o.id] = { state: 'failed', text: t('station.slow_back') }
    })
    return () => off.then((f) => f())
  })

  async function update(id: string, force = false) {
    const s = notice.stations.find((x) => x.id === id)
    if (!s) return
    rows[id] = { state: 'calling' }
    try {
      await invoke('station_update', { id, force })
      if (row(id).state === 'calling') rows[id] = { state: 'pending' }
    } catch (e) {
      const r = stationRefusal(e)
      rows[id] = r.kind === 'in_session' ? { state: 'refused', device: r.device } : { state: 'failed', text: stationRefusalText(r, s.version) ?? r.message }
    }
  }

  /** « Tout mettre à jour » : l'une après l'autre, sans redemander celles déjà lancées ou refusées. */
  async function updateAll() {
    running = true
    for (const s of notice.stations) {
      if (row(s.id).state === 'idle' || row(s.id).state === 'failed') await update(s.id)
    }
    running = false
  }

  /** « Plus tard » : pour chaque Station pas encore lancée ; « Fermer » quand tout est à jour. */
  function later() {
    for (const s of notice.stations) {
      const r = row(s.id).state
      if (r === 'idle' || r === 'refused' || r === 'failed') invoke('station_later', { id: s.id, version: s.version }).catch(() => {})
    }
    onclose()
  }

  function focus(node: HTMLElement) {
    node.focus()
  }
</script>

{#snippet status(id: string)}
  {@const r = row(id)}
  {#if r.state === 'calling' || r.state === 'pending'}
    <span class="progress" role="status"><span class="dot"></span>{t('station_offer.row_updating')}</span>
  {:else if r.state === 'done'}
    <span class="ok" role="status"><Icon name="check" size={16} stroke={2} />{t('station_offer.row_done')}</span>
  {:else if r.state === 'refused'}
    <span class="warn">{r.device ? t('station.in_session_title', { device: r.device }) : t('station.in_session_anonymous')}</span>
  {:else if r.state === 'failed'}
    <span class="warn">{r.text}</span>
  {/if}
{/snippet}

<div class="veil" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="notice-title" aria-describedby="notice-text">
    {#if one}
      <h2 id="notice-title">{t('station_offer.notice_title_one', { name: one.name })}</h2>
      <p id="notice-text">{t('station_offer.notice_text_one', { app: notice.app })}</p>
      {@render status(one.id)}
      <div class="buttons">
        <button class="ghost" onclick={later}>{allDone ? t('common.close') : t('station_offer.later')}</button>
        {#if row(one.id).state === 'refused'}
          <button class="primary small" use:focus onclick={() => update(one.id, true)}>{t('station.update_anyway')}</button>
        {:else if row(one.id).state === 'idle' || row(one.id).state === 'failed'}
          <button class="primary small" use:focus onclick={() => update(one.id)}><Icon name="upgrade" size={16} />{t('station.update')}</button>
        {/if}
      </div>
    {:else}
      <h2 id="notice-title">{t('station_offer.notice_title_many')}</h2>
      <p id="notice-text">{t('station_offer.notice_text_many', { app: notice.app })}</p>
      <ul>
        {#each notice.stations as s (s.id)}
          {@const r = row(s.id)}
          <li>
            <div class="who">
              <span class="name">{s.name}</span>
              <span class="version">{t('station_offer.version', { version: s.version })}</span>
              {@render status(s.id)}
            </div>
            {#if r.state === 'refused'}
              <button class="ghost" onclick={() => update(s.id, true)}>{t('station.update_anyway')}</button>
            {:else if r.state === 'idle' || r.state === 'failed'}
              <button class="ghost" disabled={running} onclick={() => update(s.id)}>{t('station_offer.update')}</button>
            {/if}
          </li>
        {/each}
      </ul>
      <div class="buttons">
        <button class="ghost" onclick={later}>{allDone ? t('common.close') : t('station_offer.later')}</button>
        {#if remaining}
          <button class="primary small" use:focus disabled={running || busy} onclick={updateAll}><Icon name="upgrade" size={16} />{t('station_offer.update_all')}</button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 56px 24px 24px;
    background: rgb(5 7 10 / 0.7);
  }

  .dialog {
    width: min(520px, 100%);
    max-height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 28px 30px 24px;
    border: 1px solid var(--line-2);
    border-radius: 18px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
  }

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }

  p {
    margin: 0;
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--ink-3);
  }

  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 12px 0;
    border-bottom: 1px solid var(--ink-3);
  }

  .who {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    font-size: 13px;
  }

  .name {
    font-size: 15px;
    font-weight: 600;
  }

  .version {
    color: var(--muted);
  }

  .progress,
  .ok,
  .warn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .ok {
    color: var(--ok);
  }

  .warn {
    color: var(--warn);
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

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 8px;
  }

  .ghost {
    flex: none;
    white-space: nowrap;
  }

  .primary.small {
    height: 40px;
    padding: 0 18px;
    font-size: 14px;
    gap: 8px;
    white-space: nowrap;
    clip-path: polygon(0 0, 100% 0, 100% calc(100% - 9px), calc(100% - 9px) 100%, 0 100%);
  }
</style>
