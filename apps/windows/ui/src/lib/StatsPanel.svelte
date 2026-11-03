<!--
  Panneau « Statistiques » de la barre de session (docs/link-ui.md, « Barre de
  session »), sur le modèle de l'app Mac (SessionStats.swift) : la latence en
  grand, le trajet d'une image, la latence et le débit des 30 dernières
  secondes, les pertes, puis le chemin, le réseau, l'image, les mesures et le
  son. Sur Windows, il pend sous les mesures de la barre (une fenêtre à part).
-->
<script lang="ts">
  import Icon from './Icon.svelte'
  import FramePath from './FramePath.svelte'
  import type { Stats } from './api'
  import { ms } from './api'
  import { t } from './i18n/index.svelte'

  let {
    stats,
    seconds,
    losses,
    route,
    onclose,
  }: {
    stats: Stats | null
    /** Les 30 dernières secondes, la plus récente à la fin. */
    seconds: Stats[]
    /** Pertes depuis le début de la session. */
    losses: { frames: number; packets: number }
    /** « Direct », « Par le relais », « Par le relais (TCP) ». */
    route: string
    onclose: () => void
  } = $props()

  /** Durée d'une image au débit du moment (repère des courbes). */
  const budget = $derived(1000 / Math.max(30, stats?.fps || 60))
  const sum = (f: (s: Stats) => number) => seconds.reduce((a, s) => a + f(s), 0)
  const lost = $derived(sum((s) => s.lost_frames ?? 0))
  const packets = $derived(sum((s) => s.lost_packets ?? 0))
  const keyframes = $derived(sum((s) => s.keyframes ?? 0))

  const mbps = (v: number) => t('stats.mbps', { value: v >= 100 ? String(Math.round(v)) : ms(v, 1) })

  /** Une courbe de 30 s : `top` au moins, le repère `guide` en pointillé. */
  function curve(values: (number | null)[], floor: number) {
    const top = Math.max(floor, ...values.map((v) => v ?? 0))
    const n = values.length
    const pts = values
      .map((v, i) => (v == null ? null : `${n > 1 ? (i / (n - 1)) * 352 : 0},${38 - (v / top) * 36}`))
      .filter(Boolean)
      .join(' ')
    return { pts, y: (v: number) => 38 - (v / top) * 36 }
  }
  const latency = $derived(curve(seconds.map((s) => s.latency_ms), budget * 1.6))
  const bitrate = $derived(curve(seconds.map((s) => s.mbps ?? 0), 10))

  const codec = $derived(stats?.decoder ?? '')
  const imageLine = $derived(
    stats
      ? (stats.screens > 1 ? `${t('stats.screens', { count: stats.screens })} · ` : '') +
          t(stats.hdr ? 'stats.image_line_hdr' : 'stats.image_line', { width: stats.width, height: stats.height, format: codec, fps: stats.fps })
      : '',
  )
  const measures = $derived(
    stats?.measured_pct != null
      ? t('stats.measured', { pct: stats.measured_pct }) + ((stats.rejected ?? 0) > 0 ? ' · ' + t('stats.rejected', { count: stats.rejected ?? 0 }) : '')
      : '',
  )
  const sound = $derived(
    stats?.audio_buffer_ms != null && stats.audio_output_ms != null
      ? t('stats.sound_line', { buffer: `${ms(stats.audio_buffer_ms)} ms`, output: `${ms(stats.audio_output_ms)} ms` })
      : '',
  )
</script>

<section class="panel" aria-label={t('stats.title')}>
  <div class="head">
    <div class="lead">
      <span class="eyebrow">{t('stats.title')}</span>
      <div class="big">
        <span class="mono lit" class:none={stats?.latency_ms == null}>{stats?.latency_ms != null ? `${ms(stats.latency_ms)} ms` : '—'}</span>
        <span class="from">{t('stats.from_capture')}</span>
      </div>
    </div>
    <span class="mono keycap">{t('stats.shortcut')}</span>
    <button class="close" aria-label={t('bar.close_statistics')} title={t('bar.close_statistics') + t('bar.statistics_shortcut')} onclick={onclose}>
      <Icon name="x" size={14} stroke={2} />
    </button>
  </div>

  {#if !stats}
    <p class="wait">{t('stats.waiting')}</p>
  {:else}
    <FramePath chain={stats.chain} fps={stats.fps} />
    <hr />
    <div class="curves">
      <div class="curve">
        <div class="ctitle"><span>{t('stats.latency')}</span><span class="mono">{stats.latency_ms != null ? `${ms(stats.latency_ms)} ms` : '—'}</span></div>
        <svg viewBox="0 0 352 40" preserveAspectRatio="none" aria-hidden="true">
          <line x1="0" x2="352" y1={latency.y(budget)} y2={latency.y(budget)} class="guide" />
          <polyline points={latency.pts} class="line lit" />
        </svg>
        <span class="glabel mono">{t('stats.one_frame')} · {ms(budget)} ms</span>
      </div>
      <div class="curve">
        <div class="ctitle"><span>{t('stats.bitrate')}</span><span class="mono">{mbps(stats.mbps ?? 0)}</span></div>
        <svg viewBox="0 0 352 40" preserveAspectRatio="none" aria-hidden="true">
          <polyline points={bitrate.pts} class="line" />
        </svg>
      </div>
    </div>
    <hr />
    <div class="losses">
      <div class="lhead">
        <span class="eyebrow">{t('stats.losses')}</span>
        <span class="mono session">{t('stats.session_losses', { frames: losses.frames, packets: losses.packets })}</span>
      </div>
      <div class="tiles">
        {#each [{ v: lost, k: 'lost_frames' }, { v: packets, k: 'lost_packets' }, { v: keyframes, k: 'keyframes' }] as tile (tile.k)}
          <div class="tile" title={t(`stats.${tile.k}_help`)}>
            <span class="mono tv" class:alert={tile.v > 0} class:zero={tile.v === 0}>
              {#if tile.v > 0}<Icon name="warning" size={12} stroke={2} />{/if}{tile.v}
            </span>
            <span class="tl">{t(`stats.${tile.k}`)}</span>
          </div>
        {/each}
      </div>
    </div>
    <hr />
    <dl class="details">
      <dt>{t('stats.path')}</dt><dd>{route}</dd>
      <dt>{t('stats.network')}</dt><dd>{t('stats.round_trip', { ms: stats.rtt_ms != null ? `${ms(stats.rtt_ms)} ms` : '—' })}</dd>
      <dt>{t('stats.image')}</dt><dd>{imageLine}</dd>
      {#if measures}<dt>{t('stats.measures')}</dt><dd>{measures}</dd>{/if}
      {#if sound}<dt>{t('stats.sound')}</dt><dd>{sound}</dd>{/if}
    </dl>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
    white-space: normal;
  }

  hr {
    width: 100%;
    margin: 0;
    border: 0;
    border-top: 1px solid rgb(255 255 255 / 0.08);
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }

  .lead {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .big {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  .big .mono {
    font-size: 26px;
    font-weight: 500;
    white-space: nowrap;
  }

  /* Mesure en direct : chiffres allumés, comme sur Mac. */
  .lit {
    color: #c9c1ff;
    text-shadow:
      0 0 5px rgb(154 140 255 / 0.9),
      0 0 12px rgb(109 91 255 / 0.7);
  }

  .big .none {
    color: var(--faint);
    text-shadow: none;
  }

  .from {
    font-size: 11.5px;
    line-height: 1.3;
    color: var(--muted);
    white-space: pre;
  }

  .keycap {
    flex: none;
    margin-top: 5px;
    padding: 2px 7px;
    border-radius: 5px;
    background: rgb(255 255 255 / 0.08);
    font-size: 10.5px;
    color: var(--text);
    opacity: 0.8;
  }

  .close {
    flex: none;
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 50%;
    background: rgb(255 255 255 / 0.07);
    color: var(--muted);
  }

  .close:hover {
    color: var(--text);
    background: rgb(255 255 255 / 0.12);
  }

  .wait {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
  }

  .curves {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .curve {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .ctitle {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--muted);
  }

  .ctitle .mono {
    font-size: 11.5px;
    color: var(--text);
  }

  svg {
    width: 100%;
    height: 40px;
  }

  .line {
    fill: none;
    stroke: var(--faint);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
    stroke-linejoin: round;
  }

  .line.lit {
    stroke: var(--signal);
  }

  .guide {
    stroke: rgb(255 255 255 / 0.25);
    stroke-dasharray: 2 3;
    vector-effect: non-scaling-stroke;
  }

  .glabel {
    font-size: 10px;
    color: var(--faint);
  }

  .lhead {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .session {
    font-size: 10.5px;
    color: var(--faint);
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
  }

  .tile {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tv {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 17px;
    font-weight: 500;
    color: var(--text);
  }

  .tv.zero {
    color: var(--muted);
  }

  .tv.alert {
    color: var(--warn);
  }

  .tl {
    font-size: 11px;
    color: var(--muted);
  }

  .details {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 7px 14px;
    margin: 0;
    font-size: 12px;
  }

  .details dt {
    color: var(--muted);
  }

  .details dd {
    margin: 0;
    color: var(--text);
  }
</style>
