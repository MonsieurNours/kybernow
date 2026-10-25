<!--
  Règle de trame : la latence lue contre la durée d'une image à 160, 120 et
  60 Hz, comme la carte de l'app Mac (`FrameRuler`, Theme.swift). Échelle de
  0 à 20 ms, plus si la mesure dépasse ; bande de la médiane au 95e centile.
-->
<script lang="ts">
  import { t } from '../lib/i18n/index.svelte'

  let { median, p95, decimal }: { median: number; p95: number | null; decimal: (v: number) => string } = $props()

  let width = $state(0)

  const scale = $derived(Math.max(20, Math.ceil((Math.max(median, p95 ?? 0) * 1.25) / 10) * 10))
  const x = (ms: number) => (Math.min(ms, scale) / scale) * width
  /** Repères d'une image ; 160 Hz au-dessus, les autres dessous. */
  const ticks = [
    { ms: 1000 / 160, label: '160 Hz', up: true },
    { ms: 1000 / 120, label: '120 Hz', up: false },
    { ms: 1000 / 60, label: '60 Hz', up: false },
  ]
  /** Une étiquette seulement si elle a la place (38 px jusqu'à la suivante du même côté). */
  const crowded = (i: number) => ticks.some((o, j) => j !== i && o.up === ticks[i].up && o.ms > ticks[i].ms && x(o.ms) - x(ticks[i].ms) < 38)
</script>

<div class="ruler" bind:clientWidth={width} role="img" aria-label={t('web.stats.rulerAria', { ms: decimal(median) })}>
  {#if width}
    <span class="track"></span>
    {#if p95 !== null}
      <span class="band" style="left: {x(median)}px; width: {Math.max(4, x(p95) - x(median))}px"></span>
    {/if}
    {#each ticks as tick, i (tick.label)}
      <span class="tick" style="left: {x(tick.ms)}px"></span>
      {#if !crowded(i)}
        <span class="mono label" class:up={tick.up} style="left: {x(tick.ms)}px">{tick.label}</span>
      {/if}
    {/each}
    <span class="median" style="left: {x(median)}px"></span>
  {/if}
</div>

<style>
  .ruler {
    position: relative;
    height: 44px;
    margin: 8px 0 2px;
  }

  .ruler > span {
    position: absolute;
  }

  .track {
    left: 0;
    right: 0;
    top: 16px;
    height: 2px;
    background: rgb(255 255 255 / 0.16);
  }

  .band {
    top: 12px;
    height: 10px;
    border-radius: 3px;
    background: rgb(154 140 255 / 0.2);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
  }

  .tick {
    top: 8px;
    width: 1px;
    height: 18px;
    background: rgb(138 153 163 / 0.7);
  }

  .label {
    top: 32px;
    translate: -50% 0;
    font-size: 9px;
    line-height: 1;
    white-space: nowrap;
    color: var(--faint);
  }

  .label.up {
    top: -7px;
  }

  .median {
    top: 5px;
    width: 3px;
    height: 24px;
    translate: -1.5px 0;
    border-radius: 2px;
    background: var(--signal);
  }
</style>
