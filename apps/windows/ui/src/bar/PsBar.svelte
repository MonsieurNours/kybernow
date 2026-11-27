<!--
  La barre d'une partie sur une console PlayStation (docs/playstation.md) : son nom, les mesures, « Par le relais », le
  son, la souris, ses réglages, le plein écran, la veille, la fin, et le bandeau quand la manette pilote la barre. Ce que
  fait chaque bouton reste dans la barre (Bar.svelte, `act`) ; les règles de style communes avec la barre d'un
  ordinateur (`.icon`, `.sep`, `.stats`, `.quit`, `.menu`) y sont reprises pour les éléments de ce composant.
-->
<script module lang="ts">
  export type PsItem = 'stats' | 'sound' | 'mouse' | 'settings' | 'fullscreen' | 'rest' | 'disconnect'
</script>

<script lang="ts">
  import Icon from '../lib/Icon.svelte'
  import type { PsBar, PsStats, RouteView } from '../lib/api'
  import { t } from '../lib/i18n/index.svelte'

  let {
    b,
    stats,
    lines,
    menu,
    route,
    routeTitle,
    ringed,
    act,
  }: {
    b: PsBar
    stats: PsStats | null
    lines: string[]
    menu: string | null
    route: RouteView | null
    routeTitle: string
    ringed: (item: PsItem) => boolean
    act: (item: PsItem) => void
  } = $props()
</script>

<!-- Partie sur une console : son nom, les mesures, le son, la souris, ses réglages, le plein écran, la veille, la fin. -->
<span class="psname">{b.name}</span>
<span class="sep"></span>
<div class="slot">
  <button
    class="stats"
    class:open={menu === 'stats'}
    class:ring={ringed('stats')}
    aria-haspopup="dialog"
    aria-expanded={menu === 'stats'}
    aria-label={t('bar.statistics')}
    title={t('bar.statistics')}
    onclick={() => act('stats')}
  >
    {#if stats && stats.width > 0}
      <span class="mono lat">{t('ps.fps', { n: Math.round(stats.fps) })}</span>
      <span class="sep"></span>
      <span class="mono info">{stats.width} × {stats.height} · {stats.hevc ? 'H.265' : 'H.264'}{stats.hdr ? ' · HDR' : ''}</span>
    {:else}
      <span class="lat"><Icon name="chart" size={18} stroke={1.6} /></span>
    {/if}
  </button>
  {#if menu === 'stats'}
    <div class="menu path psstats" role="dialog" aria-label={t('bar.statistics')}>
      <div class="eyebrow">{t('bar.statistics')}</div>
      {#if lines.length}
        {#each lines as line, i (i)}<p class="mono">{line}</p>{/each}
      {:else}
        <p class="help">{t('ps.stats_waiting')}</p>
      {/if}
    </div>
  {/if}
</div>
{#if route?.via_relay}
  <span class="relay" class:tcp={route.transport === 'relay_tcp'} title={routeTitle}><Icon name="globe" size={14} stroke={1.6} />{t('relay.via')}</span>
{/if}
<span class="sep"></span>
<button class="icon" class:on={b.muted} class:ring={ringed('sound')} aria-pressed={b.muted} aria-label={b.muted ? t('ps.unmute') : t('ps.mute')} title={b.muted ? t('ps.unmute') : t('ps.mute')} onclick={() => act('sound')}>
  <Icon name={b.muted ? 'speakeroff' : 'speaker'} size={20} stroke={1.6} />
</button>
{#if !b.pad}
  <!-- Une manette branchée l'emporte sur le clavier et la souris : pas de souris. -->
  <button class="icon" class:on={b.captured} class:ring={ringed('mouse')} aria-pressed={b.captured} aria-label={(b.captured ? t('ps.give_mouse') : t('ps.take_mouse')) + t('bar.mouse_shortcut')} title={(b.captured ? t('ps.give_mouse') : t('ps.take_mouse')) + t('bar.mouse_shortcut')} onclick={() => act('mouse')}>
    <Icon name="mouse" size={20} stroke={1.6} />
  </button>
{/if}
<button class="icon" class:ring={ringed('settings')} aria-label={t('home.settings_for', { name: b.name })} title={t('home.settings_for', { name: b.name })} onclick={() => act('settings')}>
  <Icon name="gear" size={20} stroke={1.6} />
</button>
<button class="icon" class:ring={ringed('fullscreen')} aria-label={(b.fullscreen ? t('ps.exit_fullscreen') : t('bar.fullscreen')) + t('bar.fullscreen_shortcut')} title={b.fullscreen ? t('ps.exit_fullscreen') : t('bar.fullscreen')} onclick={() => act('fullscreen')}>
  <Icon name="fullscreen" size={20} stroke={1.6} />
</button>
<button class="icon" class:ring={ringed('rest')} aria-label={t('ps.rest')} title={t('ps.rest')} onclick={() => act('rest')}>
  <Icon name="power" size={20} stroke={1.6} />
</button>
<span class="sep"></span>
<button class="quit" class:ring={ringed('disconnect')} title={t('bar.disconnect') + t('bar.disconnect_shortcut')} onclick={() => act('disconnect')}>{t('bar.disconnect')}</button>
{#if b.pad_mode}
  <!-- La manette pilote la barre : la console reçoit une manette au repos. -->
  <div class="padbanner" role="note">
    <span class="drives"><Icon name="gamepad" size={16} stroke={1.8} />{t('ps.pad_drives')}</span>
    <span class="away">{t('ps.pad_away')}</span>
    <span class="sep"></span>
    <span class="k"><span class="mono">◀ ▶</span>{t('ps.pad_navigate')}</span>
    <span class="k"><span class="mono">✕</span>{t('ps.pad_select')}</span>
    <span class="k"><span class="mono">○</span>{t('ps.pad_close')}</span>
    <span class="k"><span class="mono">L1 R1 OPTIONS CREATE</span>{t('ps.pad_back')}</span>
  </div>
{/if}

<style>
  .psname {
    padding: 0 6px 0 4px;
    font-size: 14px;
    font-weight: 600;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .menu.psstats {
    gap: 8px;
    width: 440px;
  }

  .menu.psstats p {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
    white-space: normal;
  }

  .menu.psstats .help {
    font-family: var(--sans);
    font-size: 13px;
  }

  .ring {
    box-shadow: 0 0 0 2px var(--signal), 0 0 14px rgb(109 91 255 / 0.55) !important;
  }

  .padbanner {
    position: absolute;
    top: calc(100% + 10px);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 16px;
    height: 40px;
    padding: 0 16px;
    border: 1px solid rgb(154 140 255 / 0.4);
    border-radius: 12px;
    background: rgb(9 13 18 / 0.92);
    box-shadow: 0 16px 40px rgb(0 0 0 / 0.5);
    font-size: 12px;
    color: var(--muted);
    animation: drop 0.2s var(--ease-out);
  }

  .padbanner .drives {
    display: flex;
    align-items: center;
    gap: 7px;
    font-weight: 600;
    color: var(--signal);
  }

  .padbanner .away {
    color: var(--faint);
  }

  .padbanner .k {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .padbanner .k .mono {
    font-size: 11px;
    color: var(--text);
  }

  :global(.bar.padmode) .menu {
    top: calc(100% + 62px);
  }

  .stats {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 10px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: none;
    color: inherit;
    text-align: left;
  }

  .stats:hover {
    background: rgb(255 255 255 / 0.06);
  }

  .stats.open {
    background: var(--signal-soft);
    border-color: var(--signal);
  }

  .lat {
    display: flex;
    flex: none;
    align-items: center;
    gap: 10px;
    padding: 0 4px;
    font-size: 15px;
  }

  .sep {
    width: 1px;
    height: 24px;
    background: var(--line-2);
  }

  .info {
    font-size: 11px;
    color: var(--faint);
  }

  .icon {
    width: 38px;
    height: 34px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon:hover {
    background: rgb(255 255 255 / 0.06);
    color: var(--text);
  }

  .menu {
    position: absolute;
    top: calc(100% + 12px);
    left: 50%;
    transform: translateX(-50%);
    min-width: 250px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    border: 1px solid rgb(43 55 66 / 0.9);
    border-radius: 12px;
    background: rgb(9 13 18 / 0.96);
    box-shadow: 0 16px 40px rgb(0 0 0 / 0.5);
    animation: drop 0.18s var(--ease-out);
  }

  .slot {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    align-items: center;
    min-width: 0;
  }

  .slot > * {
    grid-area: 1 / 1;
  }

  .quit {
    height: 36px;
    padding: 0 14px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    border: 1px solid rgb(255 107 107 / 0.4);
    border-radius: 9px;
    background: rgb(255 107 107 / 0.14);
    color: #ff8a8a;
    font-size: 13px;
    font-weight: 600;
  }

  .quit:hover {
    background: rgb(255 107 107 / 0.22);
  }
</style>
