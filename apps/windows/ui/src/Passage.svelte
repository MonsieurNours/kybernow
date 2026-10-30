<!--
  Passage vers la session : l'accueil coupé en deux le long de la couture à
  45°, un tube néon s'y allume pendant la préparation (ses trois étapes, ou
  « La Station redémarre… » quand le cœur la redémarre en dernier recours,
  avant de reprendre), puis les deux moitiés s'écartent quand la fenêtre de
  la session se montre : l'image seule, sans chiffre ni capsule par-dessus.
  Au retour, elles se referment. Comme l'app Mac (Passage.swift).
-->
<script lang="ts">
  import Icon from './lib/Icon.svelte'
  import type { Computer, Phase } from './lib/api'
  import { hue } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    computer,
    phase,
    opening,
    closing,
    restarting = false,
    many = false,
    guest = false,
    borrow = false,
    cancellable = true,
    oncancel,
  }: {
    /** L'ordinateur ; pour un invité (KyberNow Share), « Invité sur {nom} » et la teinte de l'ordinateur qui partage. */
    computer: Pick<Computer, 'name' | 'preview' | 'fingerprint'>
    phase: Phase['phase']
    opening: boolean
    closing: boolean
    /** Dernier recours : la Station redémarre, les étapes reprennent ensuite. */
    restarting?: boolean
    /** Plusieurs écrans : « Préparation des écrans ». */
    many?: boolean
    /** Session d'invité : pas de conseil sur le journal d'Admin (l'invité n'y a pas accès). */
    guest?: boolean
    /** Session d'emprunt (prêt de l'ordinateur) : « Préparation de l'ordinateur » en première étape (le mode console). */
    borrow?: boolean
    /** Faux pendant l'emprunt lui-même (la Station prépare l'ordinateur) : rien à annuler encore. */
    cancellable?: boolean
    oncancel: () => void
  } = $props()

  // Pas d'image au bout de 12 s : on le dit, plutôt qu'un passage figé.
  let slow = $state(false)
  $effect(() => {
    slow = false
    if (phase !== 'first_images') return
    const t = setTimeout(() => (slow = true), 12000)
    return () => clearTimeout(t)
  })

  const order: Phase['phase'][] = ['preparing', 'connecting', 'first_images', 'streaming']
  const at = $derived(order.indexOf(phase))
  const steps = $derived([
    { label: borrow ? t('lend.preparing') : many ? t('passage.preparing_many') : t('passage.preparing'), from: 0 },
    { label: t('passage.connecting'), from: 1 },
    { label: t('passage.first_images'), from: 2 },
  ])

  const background = $derived(
    computer.preview
      ? `center / cover no-repeat url(${computer.preview})`
      : `radial-gradient(900px 600px at 78% 22%, hsl(${hue(computer.fingerprint)} 90% 62% / 0.38), transparent 70%), #070a0e`,
  )
</script>

<div class="passage" class:opening class:closing>
  <div class="half top" style:background={background}><div class="dim"></div><h1>{computer.name}</h1></div>
  <div class="half bottom" style:background={background}><div class="dim"></div></div>
  <div class="seam"></div>

  {#if !opening && !closing}
    <div class="capsule" role="status">
      {#if restarting}
        <span class="step now"><span class="dot"></span>{t('station.restarting')}</span>
      {:else}
        {#each steps as s (s.from)}
          <span class="step" class:done={at > s.from} class:now={at === s.from}>
            {#if at > s.from}<Icon name="check" size={14} stroke={2.2} />{:else}<span class="dot"></span>{/if}
            {s.label}
          </span>
        {/each}
      {/if}
      {#if cancellable}<button onclick={oncancel}>{t('common.cancel')}</button>{/if}
    </div>
    {#if slow && !guest && !borrow}
      <p class="slow" role="status">
        {t('passage.slow', { name: computer.name })}
      </p>
    {/if}
  {/if}
</div>

<style>
  .passage {
    position: absolute;
    inset: 0;
    z-index: 40;
    background: #000;
    overflow: hidden;
  }

  .half {
    position: absolute;
    inset: 0;
    transition: transform 0.8s var(--ease-out);
  }

  .dim {
    position: absolute;
    inset: 0;
    background: rgb(5 7 10 / 0.45);
  }

  .top {
    clip-path: polygon(0 0, calc(50% + 50vh) 0, calc(50% - 50vh) 100%, 0 100%);
    transform: translate(-16px, -16px);
  }

  .bottom {
    clip-path: polygon(calc(50% + 50vh) 0, 100% 0, 100% 100%, calc(50% - 50vh) 100%);
    transform: translate(16px, 16px);
  }

  h1 {
    position: absolute;
    left: 72px;
    top: 152px;
    margin: 0;
    font-size: clamp(48px, 6vw, 84px);
    font-weight: 640;
    font-stretch: 125%;
    line-height: 0.95;
    letter-spacing: -0.02em;
    opacity: 0.85;
  }

  .seam {
    position: absolute;
    left: 50%;
    top: 50%;
    width: calc(141.5vh + 40px);
    height: 3px;
    border-radius: 2px;
    background: var(--neon-core);
    transform: translate(-50%, -50%) rotate(-45deg);
    box-shadow:
      0 0 2px var(--neon-core),
      0 0 10px var(--signal),
      0 0 34px var(--signal-strong),
      0 0 80px rgb(109 91 255 / 0.6);
    animation:
      ignite 0.9s var(--ease-out),
      hum 2.4s ease-in-out 0.9s infinite;
  }

  @keyframes ignite {
    0% {
      opacity: 0;
      width: 0;
    }
    30% {
      opacity: 1;
    }
  }

  @keyframes hum {
    50% {
      box-shadow:
        0 0 2px var(--neon-core),
        0 0 8px var(--signal),
        0 0 22px var(--signal-strong),
        0 0 50px rgb(109 91 255 / 0.4);
    }
  }

  /* Ouvert pendant la session : l'accueil (montré depuis l'icône) reste utilisable et visible
     dessous (plusieurs ordinateurs : « Afficher la session », « Se connecter » sur un autre). */
  .passage.opening {
    pointer-events: none;
    background: transparent;
    transition: background 0.8s;
  }

  .opening .top {
    transform: translate(-70vw, -70vh);
  }

  .opening .bottom {
    transform: translate(70vw, 70vh);
  }

  .opening .seam {
    opacity: 0;
    transition: opacity 0.4s;
  }

  .closing .top,
  .closing .bottom {
    animation: shut 0.7s var(--ease-out) both;
  }

  .closing .top {
    --from: translate(-70vw, -70vh);
  }

  .closing .bottom {
    --from: translate(70vw, 70vh);
  }

  @keyframes shut {
    from {
      transform: var(--from);
    }
  }

  .slow {
    position: absolute;
    left: 50%;
    bottom: 28px;
    transform: translateX(-50%);
    width: min(640px, calc(100% - 48px));
    margin: 0;
    text-align: center;
    font-size: 13px;
    line-height: 1.5;
    color: var(--warn-text, #f5b841);
    animation: up 0.4s var(--ease-out) both;
  }

  .capsule {
    position: absolute;
    left: 50%;
    bottom: 72px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 12px 14px 12px 24px;
    border: 1px solid var(--line-2);
    border-radius: 999px;
    background: rgb(9 13 18 / 0.92);
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.5);
    white-space: nowrap;
    animation: up 0.4s var(--ease-out) 0.3s both;
  }

  @keyframes up {
    from {
      opacity: 0;
      transform: translate(-50%, 12px);
    }
  }

  .step {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    color: var(--ghost);
  }

  .step .dot {
    width: 8px;
    height: 8px;
    border-radius: 4px;
    border: 1px solid var(--ghost);
  }

  .step.done {
    color: var(--faint);
  }

  .step.done :global(svg) {
    color: var(--ok);
  }

  .step.now {
    color: var(--text);
    font-weight: 600;
  }

  .step.now .dot {
    border: 0;
    background: var(--signal);
    box-shadow: 0 0 10px var(--signal);
  }

  .capsule button {
    height: 40px;
    padding: 0 18px;
    border: 1px solid var(--line-2);
    border-radius: 999px;
    background: none;
    color: var(--muted);
    font-size: 14px;
  }

  .capsule button:hover {
    color: var(--text);
  }
</style>
