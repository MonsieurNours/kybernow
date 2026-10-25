<!--
  Règle de trame, comme celle de l'app Mac (Theme.swift, FrameRuler) : la
  latence lue contre la durée d'une image à 160, 120 et 60 Hz. Échelle de 0 à
  20 ms, plus si la mesure dépasse ; bande de la médiane au 95e centile,
  repère de la médiane.
-->
<script lang="ts">
  import { ms } from './api'
  import { t } from './i18n/index.svelte'

  let { median, p95 = null }: { median: number; p95?: number | null } = $props()

  let width = $state(0)

  const scale = $derived(Math.max(20, Math.ceil((Math.max(median, p95 ?? 0) * 1.25) / 10) * 10))
  const pct = (v: number) => (Math.min(v, scale) / scale) * 100

  // Repères d'une image ; 160 Hz au-dessus, 120 et 60 Hz dessous. Une étiquette
  // trop près de la suivante de sa rangée (moins de 38 px) se tait.
  const ticks = $derived.by(() => {
    const all = [
      { at: 1000 / 160, label: '160 Hz', up: true },
      { at: 1000 / 120, label: '120 Hz', up: false },
      { at: 1000 / 60, label: '60 Hz', up: false },
    ]
    return all.map((tick) => ({
      ...tick,
      crowded: all.some((o) => o.up === tick.up && o.at > tick.at && ((pct(o.at) - pct(tick.at)) / 100) * width < 38),
    }))
  })
</script>

<div class="ruler" bind:clientWidth={width} role="img" aria-label={t('stats.ruler_label', { median: ms(median), p95: ms(p95) })}>
  <div class="track"></div>
  {#if p95 != null && p95 > median}
    <div class="band" style:left="{pct(median)}%" style:width="max(4px, {pct(p95) - pct(median)}%)"></div>
  {/if}
  {#each ticks as tick (tick.label)}
    <div class="tick" style:left="{pct(tick.at)}%"></div>
    {#if !tick.crowded}<span class="mono label" class:up={tick.up} style:left="{pct(tick.at)}%">{tick.label}</span>{/if}
  {/each}
  <div class="median" style:left="{pct(median)}%"></div>
</div>
<div class="scale mono" aria-hidden="true"><span>0</span><span>{scale} ms</span></div>

<style>
  .ruler {
    position: relative;
    height: 40px;
    margin-top: 12px;
  }

  .track {
    position: absolute;
    left: 0;
    right: 0;
    top: 16px;
    height: 2px;
    background: rgb(255 255 255 / 0.16);
  }

  .band {
    position: absolute;
    top: 12px;
    height: 10px;
    border: 1px solid rgb(154 140 255 / 0.5);
    border-radius: 3px;
    background: rgb(154 140 255 / 0.2);
  }

  .tick {
    position: absolute;
    top: 8px;
    width: 1px;
    height: 18px;
    background: rgb(138 153 163 / 0.7);
  }

  .label {
    position: absolute;
    top: 28px;
    transform: translateX(-50%);
    font-size: 9px;
    color: var(--faint);
    white-space: nowrap;
  }

  .label.up {
    top: -10px;
  }

  .median {
    position: absolute;
    top: 5px;
    width: 3px;
    height: 24px;
    margin-left: -1.5px;
    border-radius: 2px;
    background: var(--signal);
  }

  .scale {
    display: flex;
    justify-content: space-between;
    margin-top: 2px;
    font-size: 10px;
    color: var(--ghost);
  }
</style>
