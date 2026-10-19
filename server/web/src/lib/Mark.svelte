<!-- Symbole KyberNow (brand/kybernow-mark.svg), en ligne pour pouvoir l'animer. -->
<script lang="ts">
  let { size = 32, animated = false, id = 'kn' }: { size?: number; animated?: boolean; id?: string } = $props()
</script>

<svg width={size} height={size} viewBox="64 64 384 384" aria-hidden="true" class:animated>
  <defs>
    <linearGradient id="{id}-arm-shade" gradientUnits="userSpaceOnUse" x1="236" y1="268" x2="300" y2="340">
      <stop offset="0" stop-color="#9AA6B4" />
      <stop offset="1" stop-color="#DCE3EA" />
    </linearGradient>
  </defs>
  <g fill="none" stroke="#6FE3C1" stroke-width="7" stroke-linecap="round" stroke-linejoin="round">
    <path class="wave w1" pathLength="1000" opacity="0.5" d="M70 248 H252 C272 248 280 294 300 294 C322 294 326 150.5 346 150.5 C366 150.5 368 248 388 248 H470" />
    <path class="wave w2" pathLength="1000" d="M70 262 H252 C272 262 280 312 300 312 C322 312 326 156 346 156 C366 156 368 262 388 262 H470" />
    <path class="wave w3" pathLength="1000" opacity="0.5" d="M70 276 H252 C272 276 280 322 300 322 C322 322 326 178.5 346 178.5 C366 178.5 368 276 388 276 H470" />
  </g>
  <path class="piece p4" fill="url(#{id}-arm-shade)" d="M238 264.5 L404 416 H330 L206 306 Z" />
  <path class="piece p2" fill="#F4F7FA" d="M206 224 L321 96 H392 L206 302 Z" />
  <path class="piece p1" fill="#F4F7FA" d="M206 96 V416 H188 C167 416 150 399 150 378 V134 C150 113 167 96 188 96 Z" />
  <path class="piece p3" fill="#B6C1CD" d="M150 158 C150 121 172 96 206 96 V112 C178 114 158 130 150 158 Z" />
  <path class="piece p3" fill="#B6C1CD" d="M150 354 C150 391 172 416 206 416 V400 C178 398 158 382 150 354 Z" />
  {#if animated}
    <path class="pulse" pathLength="1000" fill="none" stroke="#DFFBF2" stroke-width="7" stroke-linecap="round" d="M70 262 H252 C272 262 280 312 300 312 C322 312 326 156 346 156 C366 156 368 262 388 262 H470" />
  {/if}
</svg>

<style>
  svg {
    flex-shrink: 0;
    overflow: visible;
  }

  .animated .wave {
    stroke-dasharray: 1000;
    stroke-dashoffset: 1000;
    animation: draw 1.6s cubic-bezier(0.6, 0.1, 0.2, 1) forwards;
  }
  .animated .w1 { animation-delay: 0.35s; }
  .animated .w2 { animation-delay: 0.2s; }
  .animated .w3 { animation-delay: 0.5s; }

  .animated .piece {
    transform-box: fill-box;
    transform-origin: center;
    opacity: 0;
    animation: rise 0.7s cubic-bezier(0.2, 0.7, 0.2, 1) forwards;
  }
  .animated .p2 { animation-delay: 0.1s; }
  .animated .p3 { animation-delay: 0.15s; }
  .animated .p4 { animation-delay: 0.25s; }

  .pulse {
    stroke-dasharray: 60 940;
    opacity: 0;
    animation: travel 2.4s linear 1.8s infinite;
  }

  @keyframes draw {
    to { stroke-dashoffset: 0; }
  }
  @keyframes rise {
    from { opacity: 0; transform: translateY(18px) scale(0.96); }
    to { opacity: 1; transform: none; }
  }
  @keyframes travel {
    from { stroke-dashoffset: 1000; opacity: 1; }
    to { stroke-dashoffset: 0; opacity: 1; }
  }
</style>
