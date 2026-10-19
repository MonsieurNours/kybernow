<!-- Fond « flux de signal » : des impulsions parcourent des lignes fines. -->
<script lang="ts">
  let { speed = 1 }: { speed?: number } = $props()

  const lines = [
    { d: 'M-20 40 C 300 10, 600 90, 860 30', dur: 12, delay: -2, dash: 34 },
    { d: 'M-20 150 C 200 90, 500 250, 860 170', dur: 9, delay: -6, dash: 26 },
    { d: 'M-20 300 C 260 360, 520 220, 860 320', dur: 14, delay: -9, dash: 40 },
    { d: 'M-20 460 C 240 400, 560 560, 860 470', dur: 10, delay: -4, dash: 30 },
    { d: 'M-20 600 C 300 640, 520 520, 860 620', dur: 13, delay: -11, dash: 36 },
  ]
</script>

<svg aria-hidden="true" viewBox="0 0 836 660" preserveAspectRatio="none">
  <g fill="none" stroke="#161F29" stroke-width="1">
    {#each lines as line (line.d)}
      <path d={line.d} />
    {/each}
  </g>
  <g fill="none" stroke="var(--accent)" stroke-width="1.6" stroke-linecap="round" stroke-opacity="0.7">
    {#each lines as line (line.d)}
      <path
        d={line.d}
        pathLength="1000"
        stroke-dasharray="{line.dash} {1000 - line.dash}"
        style="animation: kn-pulse {line.dur / speed}s linear infinite; animation-delay: {line.delay}s"
      />
    {/each}
  </g>
</svg>

<style>
  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
</style>
