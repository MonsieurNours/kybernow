<!--
  « Statistiques » de la session (docs/link-ui.md, « Barre de session » et « Statistiques »), sur le
  modèle du panneau de l'app Windows : la latence en grand « de la capture à votre écran », le trajet
  d'une image, la latence et le débit des 30 dernières secondes (repère « 1 image », « Débit visé »),
  « Pertes · 30 s », puis le chemin, le réseau, l'image, les mesures et le son ; PyroWave ajoute son
  décodage et ses images perdues. Ce que le moteur web ne mesure pas (débit reçu, paquets perdus, images
  clés, aller-retour, son) : « — ».
-->
<script lang="ts">
  import Icon from '../lib/Icon.svelte'
  import { ms } from '../lib/format'
  import { i18n, t } from '../lib/i18n/index.svelte'
  import FramePath from './FramePath.svelte'
  import type { LiveSession } from './livesession.svelte'
  import { formatLabel } from './share'

  let { s, onclose }: { s: LiveSession; onclose: () => void } = $props()

  const m = $derived(s.metrics)
  const seconds = $derived(m.seconds)
  const last = $derived(seconds.at(-1) ?? null)
  /** « 8,3 » en français, « 8.3 » en anglais. */
  const decimal = (v: number) => (i18n.lang === 'fr' ? v.toFixed(1).replace('.', ',') : v.toFixed(1))

  /** Durée d'une image au débit du moment (repère des courbes). */
  const budget = $derived(1000 / Math.max(30, m.fps || 60))
  /** Une courbe de 30 s, à l'échelle de `floor` au moins. */
  function curve(values: (number | null)[], floor: number) {
    const top = Math.max(floor, ...values.map((v) => v ?? 0))
    const n = values.length
    const pts = values
      .map((v, i) => (v == null ? null : `${n > 1 ? (i / (n - 1)) * 352 : 0},${38 - (v / top) * 36}`))
      .filter(Boolean)
      .join(' ')
    return { pts, y: (v: number) => 38 - (v / top) * 36 }
  }
  const latencyCurve = $derived(curve(seconds.map((x) => x.latency), budget * 1.6))
  /** Le débit reçu n'est pas mesuré par le moteur web : seul le trait du débit visé est tracé. */
  const targetCurve = $derived(curve(seconds.map((x) => x.target), 10))

  const target = $derived(s.controls.target())
  const targetText = $derived(target ? t(target.hidden ? 'session.stats.targetHidden' : 'session.stats.targetFixed', { mbps: target.mbps }) : '—')
  const skipped = $derived(seconds.reduce((a, x) => a + x.skipped, 0))

  const plan = $derived(s.plan)
  /** Le format tel que cet appareil le décode : celui que la Station dit envoyer à un invité, sinon le sien. */
  const codec = $derived(s.guest?.image?.codec ?? plan?.codec ?? '')
  const decodedBy = $derived(
    codec === 'pyrowave' ? t('session.stats.graphics') : s.codecList.find((c) => c.codec === codec)?.hardware ? t('session.stats.hardware') : t('session.stats.software'),
  )
  const imageLine = $derived.by(() => {
    const w = s.canvasSize.width || s.mode?.width
    const h = s.canvasSize.height || s.mode?.height
    if (!w || !h || !plan) return '—'
    return t('session.stats.imageLine', { width: w, height: h, format: `${formatLabel(codec, plan.yuv444)} · ${decodedBy}`, fps: m.fps })
  })
  const pyro = $derived(plan?.pyrowave ?? null)
</script>

<section class="sessionstats frost" aria-label={t('session.stats.title')}>
  <div class="head">
    <div class="lead">
      <span class="lbl">{t('session.stats.title')}</span>
      <div class="big">
        <span class="mono lit" class:none={m.latency == null}>{m.latency != null ? ms(m.latency) : '—'}</span>
        <span class="from">{t('session.stats.fromCapture')}</span>
      </div>
    </div>
    <span class="mono keycap">{s.shortcut('S')}</span>
    <button type="button" class="close" aria-label={t('session.stats.close')} title={t('session.stats.close')} onclick={onclose}>
      <Icon name="x" size={14} stroke={2} />
    </button>
  </div>

  {#if !last}
    <p class="wait">{t('session.stats.waiting')}</p>
  {:else}
    <FramePath chain={last.chain} fps={m.fps} />
    <hr />
    <div class="curves">
      <div class="curve">
        <div class="ctitle"><span>{t('session.stats.latency')}</span><span class="mono">{m.latency != null ? ms(m.latency) : '—'}</span></div>
        <svg viewBox="0 0 352 40" preserveAspectRatio="none" aria-hidden="true">
          <line x1="0" x2="352" y1={latencyCurve.y(budget)} y2={latencyCurve.y(budget)} class="guide" />
          <polyline points={latencyCurve.pts} class="line lit" />
        </svg>
        <span class="glabel mono">{t('session.stats.oneFrame')} · {ms(budget)}</span>
      </div>
      <div class="curve">
        <div class="ctitle"><span>{t('session.stats.bitrate')}</span><span class="mono">—</span></div>
        <svg viewBox="0 0 352 40" preserveAspectRatio="none" aria-hidden="true">
          <polyline points={targetCurve.pts} class="target" />
        </svg>
        <div class="tline"><span>{t('session.stats.target')}</span><span class="mono">{targetText}</span></div>
      </div>
    </div>
    <hr />
    <div class="losses">
      <div class="lhead">
        <span class="lbl">{t('session.stats.losses')}</span>
        <span class="mono session">{t('session.stats.sessionLosses', { frames: m.skippedTotal, packets: '—' })}</span>
      </div>
      <div class="tiles">
        <div class="tile">
          <span class="mono tv" class:alert={skipped > 0} class:zero={skipped === 0}>{#if skipped > 0}<Icon name="warning" size={12} stroke={2} />{/if}{skipped}</span>
          <span class="tl">{t('session.stats.lostFrames')}</span>
        </div>
        <div class="tile"><span class="mono tv zero">—</span><span class="tl">{t('session.stats.lostPackets')}</span></div>
        <div class="tile"><span class="mono tv zero">—</span><span class="tl">{t('session.stats.keyframes')}</span></div>
      </div>
    </div>
    <hr />
    <dl class="details">
      <dt>{t('session.stats.path')}</dt><dd>{s.relayed ? t('session.bar.relay') : t('session.stats.direct')}</dd>
      <dt>{t('session.stats.network')}</dt><dd>{t('session.stats.roundTrip', { ms: '—' })}</dd>
      <dt>{t('session.stats.image')}</dt><dd>{imageLine}</dd>
      <dt>{t('session.stats.measures')}</dt><dd>{last.measuredPct != null ? t('session.stats.measured', { pct: last.measuredPct }) : '—'}</dd>
      <dt>{t('session.stats.sound')}</dt><dd>—</dd>
      {#if pyro}
        <dt>{t('session.pyrowave.decode')}</dt><dd class="mono">{m.pyro?.decodeMs != null ? ms(m.pyro.decodeMs) : '—'}</dd>
        <dt>{t('session.pyrowave.lost')}</dt><dd class="mono">{m.pyro?.lostPercent != null ? `${decimal(m.pyro.lostPercent)} %` : '—'}</dd>
        {#if pyro.lowered}<dt>{t('session.stats.image')}</dt><dd>{t('session.pyrowave.lowered', { width: pyro.lowered.width, height: pyro.lowered.height, mbps: pyro.lowered.mbps })}</dd>{/if}
      {/if}
    </dl>
  {/if}
</section>

<style>
  .sessionstats {
    position: absolute;
    left: 50%;
    top: 80px;
    translate: -50% 0;
    width: min(400px, calc(100% - 32px));
    max-height: calc(100% - 96px);
    overflow-y: auto;
    border-radius: 16px;
    padding: 16px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    white-space: normal;
    z-index: 5;
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

  /* Mesure en direct : chiffres allumés, comme sur Mac et Windows. */
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
    padding: 0;
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

  /* Débit visé : un trait clair, en tirets. */
  .target {
    fill: none;
    stroke: rgb(201 193 255 / 0.55);
    stroke-width: 1;
    stroke-dasharray: 4 3;
    vector-effect: non-scaling-stroke;
  }

  .tline {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 11px;
    color: var(--muted);
  }

  .tline .mono {
    font-size: 10.5px;
    color: var(--text);
    text-align: right;
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
