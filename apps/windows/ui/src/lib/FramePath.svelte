<!--
  Trajet d'une image (docs/link-ui.md « Statistiques »), sur le modèle de l'app
  Mac (SessionStats.swift, Waterfall) : les six étapes en cascade, chacune
  commence où la précédente finit, sur une échelle commune de la capture à
  l'écran ; groupées par lieu (« Sur l'ordinateur », « Réseau », « Sur ce PC »),
  les valeurs à droite, l'aide au survol. Le pointillé marque la durée d'une
  image au débit du moment. « — » pour une étape sans mesure.
-->
<script lang="ts">
  import { ms } from './api'
  import { t } from './i18n/index.svelte'

  /** Médianes de la seconde (ms), dans l'ordre des étapes ; `null` sans mesure. */
  let { chain, fps }: { chain: (number | null)[]; fps: number } = $props()

  type Place = 'computer' | 'network' | 'device'
  const steps: { key: string; place: Place }[] = [
    { key: 'capture', place: 'computer' },
    { key: 'encode', place: 'computer' },
    { key: 'send', place: 'computer' },
    { key: 'network', place: 'network' },
    { key: 'decode', place: 'device' },
    { key: 'display', place: 'device' },
  ]
  const placeName: Record<Place, string> = { computer: 'stats.on_computer', network: 'stats.network', device: 'stats.on_device' }

  /** Durée d'une image au débit du moment (comme sur Mac : 30 i/s au moins). */
  const budget = $derived(1000 / Math.max(30, fps || 60))
  const stages = $derived(steps.map((s, i) => ({ ...s, ms: chain[i] ?? null })))
  const scale = $derived(Math.max(stages.reduce((a, s) => a + (s.ms ?? 0), 0), budget) * 1.08)
  const starts = $derived(stages.map((_, i) => stages.slice(0, i).reduce((a, s) => a + (s.ms ?? 0), 0)))
  const frac = (v: number) => Math.min(1, v / scale)
</script>

<section class="path" aria-label={t('stats.frame_path')}>
  <div class="head">
    <span class="eyebrow">{t('stats.frame_path')}</span>
    <span class="mono budget"><span class="dash" aria-hidden="true"></span>{ms(budget)} ms</span>
  </div>
  <div class="rows" style:--budget={frac(budget)}>
    {#each stages as s, i (s.key)}
      {#if i === 0 || stages[i - 1].place !== s.place}
        <div class="mono place" class:first={i === 0}>{t(placeName[s.place])}</div>
      {/if}
      <div class="row" title={t(`stats.${s.key}_help`)}>
        <span class="name" class:none={s.ms == null}>{t(`stats.${s.key}`)}</span>
        <span class="track">
          {#if s.ms != null}
            <span class="step" style:left="{frac(starts[i]) * 100}%" style:width="max(2px, {frac(s.ms) * 100}%)"></span>
          {/if}
        </span>
        <span class="mono value" class:none={s.ms == null}>{s.ms != null ? `${ms(s.ms)} ms` : '—'}</span>
      </div>
    {/each}
    <!-- Repère : une image au débit du moment, en travers des barres. -->
    <span class="guide" aria-hidden="true"></span>
  </div>
</section>

<style>
  .path {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .budget {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    color: var(--faint);
  }

  .dash {
    width: 0;
    height: 11px;
    border-left: 1px dashed rgb(255 255 255 / 0.4);
  }

  .rows {
    --name: 78px;
    --value: 64px;
    --gap: 10px;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .place {
    padding-top: 5px;
    font-size: 9px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ghost);
  }

  .place.first {
    padding-top: 0;
  }

  .row {
    display: grid;
    grid-template-columns: var(--name) 1fr var(--value);
    align-items: center;
    gap: var(--gap);
    height: 16px;
  }

  .name {
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track {
    position: relative;
    height: 100%;
  }

  .track::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 1px;
    background: rgb(255 255 255 / 0.05);
  }

  .step {
    position: absolute;
    top: 4px;
    height: 8px;
    border-radius: 2px;
    background: var(--signal);
  }

  .value {
    justify-self: end;
    font-size: 11.5px;
    color: var(--text);
  }

  .name.none {
    color: var(--faint);
  }

  .value.none {
    color: var(--ghost);
  }

  .guide {
    position: absolute;
    top: 0;
    bottom: 0;
    left: calc(var(--name) + var(--gap) + (100% - var(--name) - var(--value) - 2 * var(--gap)) * var(--budget));
    border-left: 1px dashed rgb(255 255 255 / 0.4);
    pointer-events: none;
  }
</style>
