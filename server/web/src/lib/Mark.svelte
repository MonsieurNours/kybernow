<!--
  Symbole KyberNow v2, en trois états (brand/v2/mark) : « console » (la
  couture seule, ici par défaut), « server » (la moitié haute en signal),
  « now » (la moitié basse). `lit` allume la couture en néon.

  Avec `product` (« admin », « link ») : le logo signé du produit
  (brand/v2/logo/kybernow-{admin,link}-*.svg) : le symbole dans l'état du
  produit, le nom « KyberNow », et dessous l'étiquette ADMIN ou LINK. `size`
  reste la taille du symbole ; sous ~32 px de haut, l'étiquette est omise.
  Couleurs du nom et de l'étiquette : --logo-word, --logo-tag, --logo-tag-text
  (app.css).
-->
<script lang="ts">
  import { SIGNED, WORDMARK, type Product } from './signed'

  let {
    size = 26,
    state,
    lit = false,
    breathe = false,
    seam = '#9A8CFF',
    product,
  }: { size?: number; state?: 'console' | 'server' | 'now'; lit?: boolean; breathe?: boolean; seam?: string; product?: Product } = $props()

  const logo = $derived(product ? SIGNED[product] : null)
  const shown = $derived(state ?? logo?.state ?? 'console')
  const top = $derived(shown === 'server' ? '#9A8CFF' : shown === 'now' ? '#E3EDF0' : '#33404B')
  const bottom = $derived(shown === 'server' ? '#E3EDF0' : shown === 'now' ? '#9A8CFF' : '#33404B')
  const showSeam = $derived(shown === 'console' || lit)
  /** Pixels par unité du logo : le symbole garde `size` pixels. */
  const unit = $derived(logo ? size / logo.mark : 1)
  const signature = $derived(!!logo && logo.height * unit >= 32)
</script>

{#snippet symbol()}
  <svg width={size} height={size} viewBox="0 0 48 48" aria-hidden="true" class:lit class:breathe>
    <path fill={top} d="M7 0 L42.275 0 A1.2 1.2 0 0 1 43.123 2.049 L2.049 43.123 A1.2 1.2 0 0 1 0 42.275 L0 7 A7 7 0 0 1 7 0 Z" />
    <path fill={bottom} d="M48 5.725 L48 41 A7 7 0 0 1 41 48 L5.725 48 A1.2 1.2 0 0 1 4.877 45.951 L45.951 4.877 A1.2 1.2 0 0 1 48 5.725 Z" />
    {#if showSeam}<path
      fill={lit ? '#F4F1FF' : seam}
      d="M6.237 43.035 L43.035 6.237 A0.85 0.85 0 0 0 43.035 5.035 L42.965 4.965 A0.85 0.85 0 0 0 41.763 4.965 L4.965 41.763 A0.85 0.85 0 0 0 4.965 42.965 L5.035 43.035 A0.85 0.85 0 0 0 6.237 43.035 Z"
    />{/if}
  </svg>
{/snippet}

{#if logo}
  <!-- Le symbole (avec son néon) et, à sa droite, le nom et la signature, à la même échelle. -->
  {@const x = logo.mark}
  {@const width = logo.width - x}
  {@const height = signature ? logo.height : logo.baseline + 20}
  <span class="signed" role="img" aria-label={logo.name}>
    {@render symbol()}
    <svg class="rest" width={width * unit} height={height * unit} viewBox="{x} 0 {width} {height}" aria-hidden="true">
      <g class="s-word" transform="translate({logo.wordX} {logo.baseline})">
        <path d={WORDMARK.kyber} />
        <path transform="translate({WORDMARK.nowOffset} 0)" d={WORDMARK.now} />
      </g>
      {#if signature}
        <path class="s-tag" d={logo.tag} />
        <path class="s-txt" transform="translate({logo.sigX} {logo.sigY}) scale({logo.sigScale})" d={logo.sig} />
      {/if}
    </svg>
  </span>
{:else}
  {@render symbol()}
{/if}

<style>
  svg {
    flex: none;
    overflow: visible;
  }

  .signed {
    display: inline-flex;
    align-items: flex-start;
    flex: none;
    line-height: 0;
  }

  .s-word {
    fill: var(--logo-word, #e3edf0);
  }

  .s-tag {
    fill: var(--logo-tag, #1e2832);
  }

  .s-txt {
    fill: var(--logo-tag-text, #c9d5da);
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
