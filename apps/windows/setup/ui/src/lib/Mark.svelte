<!--
  Symbole KyberNow v2, en trois états (brand/v2/mark) : « console » (la
  couture seule, ici par défaut), « server » (la moitié haute en signal),
  « now » (la moitié basse). `lit` allume la couture en néon.
-->
<script lang="ts">
  let {
    size = 26,
    state = 'console',
    lit = false,
    breathe = false,
    seam = '#9A8CFF',
  }: { size?: number; state?: 'console' | 'server' | 'now'; lit?: boolean; breathe?: boolean; seam?: string } = $props()

  const top = $derived(state === 'server' ? '#9A8CFF' : state === 'now' ? '#E3EDF0' : '#33404B')
  const bottom = $derived(state === 'server' ? '#E3EDF0' : state === 'now' ? '#9A8CFF' : '#33404B')
  const showSeam = $derived(state === 'console' || lit)
</script>

<svg width={size} height={size} viewBox="0 0 48 48" aria-hidden="true" class:lit class:breathe>
  <path fill={top} d="M7 0 L42.275 0 A1.2 1.2 0 0 1 43.123 2.049 L2.049 43.123 A1.2 1.2 0 0 1 0 42.275 L0 7 A7 7 0 0 1 7 0 Z" />
  <path fill={bottom} d="M48 5.725 L48 41 A7 7 0 0 1 41 48 L5.725 48 A1.2 1.2 0 0 1 4.877 45.951 L45.951 4.877 A1.2 1.2 0 0 1 48 5.725 Z" />
  {#if showSeam}<path
    fill={lit ? '#F4F1FF' : seam}
    d="M6.237 43.035 L43.035 6.237 A0.85 0.85 0 0 0 43.035 5.035 L42.965 4.965 A0.85 0.85 0 0 0 41.763 4.965 L4.965 41.763 A0.85 0.85 0 0 0 4.965 42.965 L5.035 43.035 A0.85 0.85 0 0 0 6.237 43.035 Z"
  />{/if}
</svg>

<style>
  svg {
    flex: none;
    overflow: visible;
  }

  .lit {
    filter: drop-shadow(0 0 1px #f4f1ff) drop-shadow(0 0 4px #9a8cff) drop-shadow(0 0 10px rgb(109 91 255 / 0.8));
  }

  .breathe {
    animation: respire 3.2s ease-in-out infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    .breathe {
      animation: none;
    }
  }
</style>
