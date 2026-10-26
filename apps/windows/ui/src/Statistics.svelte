<!--
  Rubrique Statistiques des réglages d'un ordinateur : la dernière session
  (médiane en grand, règle de trame, 95e centile, durée, ce qui a été
  transmis) et cet ordinateur (système, carte graphique, adresse d'Admin,
  empreinte).
-->
<script lang="ts">
  import FrameRuler from './lib/FrameRuler.svelte'
  import type { Computer } from './lib/api'
  import { duration, fingerprintGroups, localLabel, moment, ms } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let { computer }: { computer: Computer } = $props()

  const last = $derived(computer.last?.median_ms != null ? computer.last : null)

  /**
   * « 2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s » ; avec plusieurs écrans, leur nombre d'abord, puis l'écran
   * principal et le débit total (« 3 écrans · 3840 × 2160 · 160 Hz · HEVC · 120 Mbit/s ») ; une session de la
   * 0.4.x n'a que son profil et son format.
   */
  const sent = $derived.by(() => {
    if (!last) return null
    if (last.width && last.height) {
      return [
        last.screens && last.screens > 1 ? t('stats.screens', { count: last.screens }) : null,
        `${last.width} × ${last.height}`,
        last.refresh ? `${last.refresh} Hz` : null,
        last.codec,
        last.mbps ? `${last.mbps} Mbit/s` : null,
      ]
        .filter(Boolean)
        .join(' · ')
    }
    return last.label ? localLabel(last.label) : null
  })

  const address = $derived(`${computer.host.includes(':') ? `[${computer.host}]` : computer.host}:${computer.port}`)
  const fingerprint = $derived(fingerprintGroups(computer.fingerprint).slice(0, 4).join(' '))
</script>

<section class="card">
  <h3 class="eyebrow">{t('stats.last_session')}</h3>
  {#if last}
    <div class="big">
      <span class="mono value">{ms(last.median_ms)}&nbsp;ms</span>
      <span class="mono unit">{t('stats.median')}</span>
    </div>
    <FrameRuler median={last.median_ms ?? 0} p95={last.p95_ms} />
    {#if last.p95_ms != null}<p class="p95">{t('stats.p95', { ms: ms(last.p95_ms) })}</p>{/if}
    <p class="when">{duration(last.seconds)} · {moment(last.at)}</p>
    {#if sent}<p class="mono sent">{sent}</p>{/if}
    <p class="help">{t('stats.help')}</p>
  {:else}
    <p class="empty">{t('stats.none')}</p>
  {/if}
</section>

<section class="card">
  <h3 class="eyebrow">{t('stats.this_computer')}</h3>
  <dl>
    <div><dt>{t('stats.system')}</dt><dd>{computer.info?.os || computer.os || '—'}</dd></div>
    <div><dt>{t('stats.gpu')}</dt><dd>{computer.info?.gpu ?? computer.gpu ?? '—'}</dd></div>
    <div><dt>{t('stats.address')}</dt><dd class="mono">{address}</dd></div>
    <div><dt>{t('stats.fingerprint')}</dt><dd class="mono">{fingerprint || '—'}</dd></div>
  </dl>
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px 22px 20px;
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--ink-1);
  }

  h3 {
    margin: 0 0 4px;
    font-weight: 500;
  }

  .big {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  .value {
    font-size: 36px;
    font-weight: 500;
  }

  .unit {
    font-size: 13px;
    color: var(--muted);
  }

  p {
    margin: 0;
  }

  .p95 {
    margin-top: 6px;
    font-size: 13px;
    color: var(--muted);
  }

  .when {
    font-size: 13px;
    color: var(--text);
  }

  .sent {
    font-size: 11px;
    line-height: 1.6;
    color: var(--muted);
  }

  .help {
    margin-top: 4px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--faint);
  }

  .empty {
    font-size: 14px;
    color: var(--muted);
  }

  dl {
    margin: 0;
  }

  dl div {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 11px 0;
    border-bottom: 1px solid var(--ink-3);
    font-size: 14px;
  }

  dl div:last-child {
    border-bottom: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    text-align: right;
    overflow-wrap: anywhere;
  }

  dd.mono {
    font-size: 12px;
  }
</style>
