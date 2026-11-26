<!--
  Bandeau de la mise à jour de la Station, sur l'accueil de l'ordinateur choisi, sous la rangée « Se connecter » et
  profils (docs/link-ui.md « Mise à jour des Stations ») : ce que le cœur calcule (`computer.offer.banner`), puis le flux
  de « Mettre à jour la Station » des réglages (`station_update` : la confirmation quand un appareil est en session,
  le suivi par l'app jusqu'au retour de la Station, `station-outcome` à la fin). « KyberNow Station {v} est en place. »
  reste quelques secondes, puis le bandeau s'en va ; un échec reste avec « Réessayer » et « Plus tard ». « Plus tard »
  (`station_later`) le cache pour cette version et cet ordinateur.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import type { Computer, StationOutcome } from './lib/api'
  import { invoke, listen, stationRefusal, stationRefusalText } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let { computer }: { computer: Computer } = $props()

  const banner = $derived(computer.offer?.banner ?? { kind: 'none' as const })
  /** La version proposée, gardée pour « Réessayer » et « Plus tard » après un échec. */
  let version = $state('')
  $effect(() => {
    if ('version' in banner) version = banner.version
  })
  /** L'appel en cours (Admin télécharge le paquet). */
  let calling = $state(false)
  /** Un appareil est en session : la confirmation des réglages, dans le bandeau. */
  let confirm = $state<{ device: string | null } | null>(null)
  let failure = $state<string | null>(null)
  /** « KyberNow Station {v} est en place. », quelques secondes. */
  let done = $state<string | null>(null)
  const installing = $derived(calling || computer.station_pending === 'update' || banner.kind === 'installing')

  onMount(() => {
    let timer: ReturnType<typeof setTimeout> | undefined
    const off = listen<StationOutcome>('station-outcome', (o) => {
      if (o.id !== computer.id) return
      if (o.outcome === 'updated') {
        failure = null
        done = o.version ?? version
        clearTimeout(timer)
        timer = setTimeout(() => (done = null), 6000)
      } else if (o.outcome === 'update_failed') failure = t('station.update_failed')
      else if (o.outcome === 'slow_back') failure = t('station.slow_back')
    })
    return () => {
      clearTimeout(timer)
      off.then((f) => f())
    }
  })

  async function update(force = false) {
    confirm = null
    failure = null
    calling = true
    try {
      await invoke('station_update', { id: computer.id, force })
    } catch (e) {
      const r = stationRefusal(e)
      if (r.kind === 'in_session') confirm = { device: r.device }
      else failure = stationRefusalText(r, version)
    } finally {
      calling = false
    }
  }

  function later() {
    confirm = null
    failure = null
    if (version) invoke('station_later', { id: computer.id, version }).catch(() => {})
  }
</script>

{#if done}
  <div class="banner ok" role="status"><Icon name="check" size={18} stroke={2} /><p>{t('station.updated', { version: done })}</p></div>
{:else if installing}
  <div class="banner" role="status"><span class="dot"></span><p>{t('station.updating')}</p></div>
{:else if confirm}
  <div class="banner warn" role="alertdialog" aria-labelledby="offer-confirm">
    <Icon name="warning" size={18} />
    <p><strong id="offer-confirm">{confirm.device ? t('station.in_session_title', { device: confirm.device }) : t('station.in_session_anonymous')}</strong> {t('station.in_session_update')}</p>
    <span class="buttons">
      <button class="primary small" onclick={() => update(true)}>{t('station.update_anyway')}</button>
      <button class="ghost" onclick={() => (confirm = null)}>{t('common.cancel')}</button>
    </span>
  </div>
{:else if failure && version}
  <div class="banner warn" role="alert">
    <Icon name="warning" size={18} />
    <p>{failure}</p>
    <span class="buttons">
      <button class="primary small" onclick={() => update()}>{t('station_offer.retry')}</button>
      <button class="ghost" onclick={later}>{t('station_offer.later')}</button>
    </span>
  </div>
{:else if banner.kind === 'update'}
  <div class="banner">
    <Icon name="upgrade" size={18} />
    <p>{t('station_offer.banner_update', { version: banner.version })}</p>
    <span class="buttons">
      <button class="primary small" onclick={() => update()}>{t('station_offer.update')}</button>
      <button class="ghost" onclick={later}>{t('station_offer.later')}</button>
    </span>
  </div>
{:else if banner.kind === 'setup' || banner.kind === 'away'}
  <div class="banner">
    <Icon name="upgrade" size={18} />
    <p>{t(banner.kind === 'setup' ? 'station_offer.banner_setup' : 'station_offer.banner_away', { version: banner.version })}</p>
    <span class="buttons"><button class="ghost" onclick={later}>{t('station_offer.later')}</button></span>
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px 14px;
    max-width: 760px;
    padding: 12px 14px 12px 16px;
    border: 1px solid var(--line-2);
    border-left: 3px solid var(--signal);
    border-radius: 10px;
    background: rgb(9 13 18 / 0.82);
    color: var(--signal);
  }

  .banner.ok {
    border-left-color: var(--ok);
    color: var(--ok);
  }

  .banner.warn {
    border-left-color: var(--warn);
    color: var(--warn);
  }

  p {
    flex: 1 1 280px;
    margin: 0;
    font-size: 14px;
    line-height: 1.45;
    color: var(--text);
  }

  strong {
    font-weight: 600;
    color: var(--warn);
  }

  .buttons {
    display: flex;
    gap: 10px;
    flex: none;
  }

  .primary.small {
    height: 40px;
    padding: 0 18px;
    font-size: 14px;
    white-space: nowrap;
    clip-path: polygon(0 0, 100% 0, 100% calc(100% - 9px), calc(100% - 9px) 100%, 0 100%);
  }

  .ghost {
    white-space: nowrap;
  }

  .dot {
    width: 8px;
    height: 8px;
    margin: 0 5px;
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
</style>
