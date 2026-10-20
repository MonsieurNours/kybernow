<!--
  Accueil : l'ordinateur en grand sur l'aperçu de son écran, le profil, et ce
  que ce navigateur sait faire. Sur téléphone, une carte et deux conseils.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { backend, type Live, type Preview } from './backend'
  import { supportedCodecs, type CodecSupport } from './engine'
  import { choose } from './plan'
  import {
    canFullscreen,
    deviceKind,
    hasKeyboardLock,
    hasWebTransport,
    isIsolated,
    isStandalone,
    isTouch,
    measureRefresh,
    screenMode,
  } from './device'
  import type { Prefs, Profile } from './prefs'
  import { ago } from '../lib/format'
  import Mark from '../lib/Mark.svelte'
  import Icon from '../lib/Icon.svelte'
  import Settings from './Settings.svelte'

  let {
    hostname,
    prefs = $bindable(),
    onConnect,
    onSignedOut,
  }: { hostname: string; prefs: Prefs; onConnect: () => void; onSignedOut: () => void } = $props()

  let preview = $state<Preview | null>(null)
  let live = $state<Live | null>(null)
  let codecs = $state<CodecSupport[] | null>(null)
  let codecError = $state(false)
  let refresh = $state(60)
  let sheet = $state(false)
  let now = $state(Date.now())

  const kind = deviceKind()
  const mode = $derived(screenMode(refresh))
  const plan = $derived(codecs ? choose(prefs, mode, codecs) : null)
  const hardware = $derived(!!codecs?.find((c) => c.codec === (plan?.codec ?? 'h264'))?.hardware)
  const blocked = $derived(!hasWebTransport ? 'transport' : !isIsolated ? 'isolation' : codecError ? 'engine' : null)

  const profiles: { value: Profile; label: string; icon: string }[] = [
    { value: 'bureau', label: 'Bureau', icon: 'desk' },
    { value: 'jeu', label: 'Jeu', icon: 'pad' },
  ]

  const profileNote = $derived(
    prefs.profile === 'jeu'
      ? 'Jeu : la latence la plus basse, et les manettes branchées ici passent sur l’ordinateur.'
      : plan?.yuv444
        ? 'Bureau : un texte parfaitement net (4:4:4), aucune image perdue, le presse-papiers partagé.'
        : 'Bureau : aucune image perdue, plus de débit pour le texte, le presse-papiers partagé.',
  )

  async function loadPreview() {
    const width = Math.min(1920, Math.round(window.innerWidth * (window.devicePixelRatio || 1)))
    const next = await backend.preview(width).catch(() => null)
    if (next) {
      if (preview) URL.revokeObjectURL(preview.url)
      preview = next
    }
  }

  async function loadLive() {
    live = await backend.live().catch(() => null)
  }

  onMount(() => {
    loadPreview()
    loadLive()
    measureRefresh().then((hz) => (refresh = hz))
    supportedCodecs()
      .then((c) => (codecs = c))
      .catch(() => (codecError = true))
    const timers = [setInterval(loadLive, 8000), setInterval(loadPreview, 30000), setInterval(() => (now = Date.now()), 5000)]
    return () => {
      timers.forEach(clearInterval)
      if (preview) URL.revokeObjectURL(preview.url)
    }
  })

  function connect() {
    if (blocked) return
    // Le plein écran se demande dans le clic lui-même.
    if (prefs.fullscreen && canFullscreen && !document.fullscreenElement) {
      document.documentElement.requestFullscreen({ navigationUI: 'hide' }).catch(() => {})
    }
    onConnect()
  }

  async function signOut() {
    await fetch('/api/logout', { method: 'POST', headers: { 'x-kyber-admin': '1' }, credentials: 'same-origin' })
    onSignedOut()
  }

  function previewAge(): string {
    if (!preview) return ''
    const s = Math.max(0, Math.round((now - preview.capturedAt) / 1000))
    return s < 60 ? `il y a ${Math.max(s, 1)} s` : ago(preview.capturedAt / 1000)
  }
</script>

<div class="home" class:phone={kind === 'phone'}>
  <div class="bg" aria-hidden="true">
    {#if preview}<img src={preview.url} alt="" />{:else}<div class="signature"></div>{/if}
  </div>
  <div class="veil" aria-hidden="true"></div>

  <header>
    <div class="brand">
      <Mark size={26} state="now" />
      <span class="disp wordmark"><span>Kyber</span><b>Now</b></span>
    </div>
    <nav>
      <a class="navlink" href="/">
        <Icon name="overview" size={16} />
        <span>Console</span>
      </a>
      <button type="button" class="navlink" onclick={signOut}>
        <Icon name="logout" size={16} />
        <span>Se déconnecter</span>
      </button>
    </nav>
  </header>

  <main>
    {#if kind === 'phone'}
      <div class="thumb" aria-hidden="true">
        {#if preview}<img src={preview.url} alt="" />{/if}
      </div>
    {/if}
    {#if preview}<span class="lbl age">Aperçu · {previewAge()}</span>{/if}
    <h1 class="disp">{hostname}</h1>

    {#if live}
      <span class="pill busy"><span class="dot"></span>En session sur {live.name} · vous connecter y mettra fin</span>
    {:else}
      <span class="pill"><span class="dot"></span>Libre · prêt pour vous</span>
    {/if}

    <div class="actions">
      <button type="button" class="primary cta" onclick={connect} disabled={!!blocked}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
        {live ? 'Prendre la main' : 'Se connecter'}
      </button>
      <div class="seg" role="radiogroup" aria-label="Profil">
        {#each profiles as p (p.value)}
          <button type="button" role="radio" aria-checked={prefs.profile === p.value} onclick={() => (prefs.profile = p.value)}>
            {#if p.icon === 'desk'}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></svg>
            {:else}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 8h10a4 4 0 0 1 4 4v1a3 3 0 0 1-5.4 1.8L14.5 13h-5l-1.1 1.8A3 3 0 0 1 3 13v-1a4 4 0 0 1 4-4z" /><path d="M8 10.5v3M6.5 12h3" /></svg>
            {/if}
            {p.label}
          </button>
        {/each}
      </div>
      <button type="button" class="glassbtn" onclick={() => (sheet = true)}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2" /><circle cx="10" cy="17" r="2" /></svg>
        Réglages
      </button>
    </div>
    <p class="note">{profileNote}</p>

    {#if blocked}
      <p class="blocked" role="alert">
        {#if blocked === 'transport'}
          Ce navigateur ne sait pas recevoir l'image (il lui manque WebTransport). Ouvrez cette page dans Chrome, Edge ou Firefox.
        {:else if blocked === 'isolation'}
          La page n'a pas pu s'isoler, ce dont l'image a besoin. Rechargez-la ; si cela persiste, mettez à jour KyberNow Server.
        {:else}
          Le moteur d'image de l'ordinateur n'a pas pu se charger. Le serveur Kyber est-il en marche ?
        {/if}
      </p>
    {/if}

    {#if isTouch}
      <div class="tips">
        {#if kind === 'phone'}
          <p class="tip"><Icon name="phone" size={18} /><span><b>Tournez le téléphone.</b> L'ordinateur crée un écran à sa taille, en largeur.</span></p>
        {/if}
        {#if !isStandalone}
          <p class="tip"><Icon name="plus" size={18} /><span><b>Ajoutez cette page à l'écran d'accueil.</b> Elle s'ouvre alors en plein écran, sans la barre du navigateur.</span></p>
        {/if}
      </div>
    {/if}
  </main>

  {#if kind !== 'phone'}
    <section class="caps" aria-label="Ce navigateur">
      <div class="caps-title">
        <span class="lbl">Ce navigateur</span>
        <span class="strong">{blocked ? 'Il manque quelque chose' : 'Prêt, sans rien installer'}</span>
      </div>
      <span class="sep" aria-hidden="true"></span>
      <ul>
        {#if codecs}
          <li class:off={!hardware}><Icon name={hardware ? 'check' : 'info'} size={14} stroke={2.4} />{hardware ? 'Image décodée par la carte graphique' : 'Image décodée par le processeur'}</li>
        {/if}
        <li class:off={!prefs.virtualDisplay}><Icon name={prefs.virtualDisplay ? 'check' : 'info'} size={14} stroke={2.4} />{prefs.virtualDisplay ? "Écran de l'ordinateur à la taille du vôtre" : "Écrans de l'ordinateur tels quels"}</li>
        <li class:off={!hasKeyboardLock}><Icon name={hasKeyboardLock ? 'check' : 'info'} size={14} stroke={2.4} />{hasKeyboardLock ? 'Toutes les touches en plein écran' : 'Raccourcis système par le menu Touches'}</li>
        {#if prefs.profile === 'bureau' && plan}
          <li class:off={!plan.yuv444}><Icon name={plan.yuv444 ? 'check' : 'info'} size={14} stroke={2.4} />{plan.yuv444 ? 'Texte net (4:4:4)' : 'Texte net (4:4:4) : pas dans ce navigateur'}</li>
        {/if}
      </ul>
      <span class="mono mode">{mode.width} × {mode.height} · {mode.refresh} Hz</span>
    </section>
  {/if}

  {#if sheet}
    <Settings bind:prefs {hostname} {mode} {plan} onClose={() => (sheet = false)} />
  {/if}
</div>

<style>
  .home {
    position: relative;
    min-height: 100vh;
    min-height: 100dvh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: var(--ink-0);
  }

  .bg,
  .veil {
    position: absolute;
    inset: 0;
  }

  .bg img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: drift 40s ease-in-out infinite alternate;
    transform-origin: 65% 60%;
  }

  .signature {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(60% 70% at 78% 30%, rgb(109 91 255 / 0.28), transparent 70%),
      radial-gradient(50% 60% at 15% 90%, rgb(18 58 74 / 0.6), transparent 70%),
      var(--ink-0);
  }

  @keyframes drift {
    from {
      transform: scale(1);
    }
    to {
      transform: scale(1.05) translate(-10px, -6px);
    }
  }

  .veil {
    background:
      linear-gradient(90deg, rgb(5 7 10 / 0.9) 0%, rgb(5 7 10 / 0.6) 40%, rgb(5 7 10 / 0) 70%),
      linear-gradient(0deg, #05070a 0%, rgb(5 7 10 / 0.8) 22%, rgb(5 7 10 / 0) 50%);
  }

  header {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 18px 28px 0 40px;
    min-height: 72px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .wordmark {
    font-size: 19px;
    letter-spacing: -0.02em;
  }

  .wordmark span {
    font-weight: 420;
  }

  .wordmark b {
    font-weight: 720;
  }

  nav {
    display: flex;
    gap: 4px;
  }

  .navlink {
    height: 40px;
    padding: 0 12px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    font-size: 13.5px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    text-decoration: none;
  }

  .navlink:hover {
    background: rgb(255 255 255 / 0.06);
    color: var(--text);
  }

  main {
    position: relative;
    flex: 1;
    width: min(860px, calc(100% - 80px));
    margin-left: 72px;
    padding: 9vh 0 40px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  main > * {
    animation: rise 0.8s var(--ease-out) both;
  }

  main > :nth-child(2) {
    animation-delay: 0.1s;
  }

  main > :nth-child(3) {
    animation-delay: 0.2s;
  }

  main > :nth-child(n + 4) {
    animation-delay: 0.32s;
  }

  @keyframes rise {
    from {
      opacity: 0;
      translate: 0 18px;
    }
    to {
      opacity: 1;
      translate: 0 0;
    }
  }

  .age {
    color: var(--muted);
    font-size: 11px;
  }

  h1 {
    font-size: clamp(56px, 8vw, 104px);
    line-height: 0.9;
    font-weight: 680;
    letter-spacing: -0.045em;
    overflow-wrap: anywhere;
  }

  .pill {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 9px;
    min-height: 30px;
    padding: 4px 14px 4px 10px;
    border-radius: 15px;
    font-size: 13px;
    background: rgb(9 12 17 / 0.6);
    backdrop-filter: blur(22px);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.09);
    margin-top: 4px;
  }

  .pill .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
    flex: none;
  }

  .pill.busy {
    box-shadow: inset 0 0 0 1px rgb(245 184 65 / 0.4);
  }

  .pill.busy .dot {
    background: var(--warn);
  }

  .actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 40px;
  }

  .cta {
    --r-control: 9px;
    height: 56px;
    padding: 0 28px 0 22px;
    font-size: 17px;
    gap: 12px;
    background: linear-gradient(315deg, transparent 8px, var(--edge) 8px, var(--edge) 9.8px, var(--signal-strong) 9.8px);
  }

  .seg {
    display: flex;
    padding: 3px;
    border-radius: 11px;
    background: rgb(9 12 17 / 0.6);
    backdrop-filter: blur(22px);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.09);
  }

  .seg button {
    height: 44px;
    padding: 0 16px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    font-size: 14px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .seg button[aria-checked='true'] {
    background: rgb(154 140 255 / 0.18);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.6);
    color: #fff;
  }

  .glassbtn {
    height: 50px;
    padding: 0 16px;
    border: 0;
    border-radius: 11px;
    background: rgb(9 12 17 / 0.6);
    backdrop-filter: blur(22px);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.09);
    color: var(--text);
    font-size: 14px;
    display: inline-flex;
    align-items: center;
    gap: 9px;
  }

  .glassbtn:hover {
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.2);
  }

  .note {
    max-width: 560px;
    font-size: 14px;
    color: var(--muted);
  }

  .blocked {
    max-width: 560px;
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--warn-bg);
    box-shadow: inset 0 0 0 1px var(--warn-line);
    color: var(--warn-text);
    font-size: 13.5px;
  }

  .tips {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 8px;
  }

  .tip {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    max-width: 520px;
    padding: 14px;
    border-radius: 14px;
    background: rgb(14 20 26 / 0.85);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: 13px;
    color: var(--muted);
  }

  .tip :global(svg) {
    color: var(--signal);
    margin-top: 1px;
  }

  .tip b {
    color: var(--text);
    font-weight: 600;
  }

  .caps {
    position: relative;
    margin: 0 72px 36px;
    border-radius: 16px;
    padding: 16px 22px;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 14px 28px;
    background: rgb(9 12 17 / 0.6);
    backdrop-filter: blur(22px) saturate(1.3);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.09), inset 0 1px 0 rgb(255 255 255 / 0.06);
    animation: rise 0.8s 0.6s var(--ease-out) both;
  }

  .caps-title {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 150px;
  }

  .strong {
    font-size: 14px;
    font-weight: 600;
  }

  .sep {
    width: 1px;
    align-self: stretch;
    background: rgb(255 255 255 / 0.1);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 10px 26px;
    font-size: 13px;
    color: var(--text-2);
  }

  li {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  li :global(svg) {
    color: var(--ok);
  }

  li.off {
    color: var(--faint);
  }

  li.off :global(svg) {
    color: var(--faint);
  }

  .mode {
    margin-left: auto;
    font-size: 11px;
    color: var(--faint);
  }

  .thumb {
    display: none;
  }

  /* Tablette et petits écrans : tout passe en colonne. */
  @media (max-width: 900px) {
    main {
      margin: 0 auto;
      width: calc(100% - 48px);
      padding-top: 6vh;
    }

    header {
      padding: 14px 16px 0 24px;
    }

    .caps {
      margin: 0 24px 24px;
    }

    .mode {
      margin-left: 0;
    }
  }

  /* Téléphone : l'aperçu en carte, le bouton sur toute la largeur. */
  .phone .bg,
  .phone .veil {
    display: none;
  }

  .phone header {
    padding: 14px 12px 0 20px;
  }

  .phone .navlink span {
    display: none;
  }

  .phone main {
    width: calc(100% - 32px);
    padding: 8px 0 24px;
    gap: 12px;
  }

  .phone .thumb {
    display: block;
    position: relative;
    height: 196px;
    border-radius: 18px;
    overflow: hidden;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--line);
  }

  .phone .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .phone .age {
    margin-top: -36px;
    margin-left: 14px;
    position: relative;
    color: var(--text);
  }

  .phone h1 {
    font-size: 40px;
    margin-top: 10px;
  }

  .phone .pill {
    background: var(--ink-2);
  }

  .phone .actions {
    flex-direction: column;
    align-items: stretch;
    margin-top: 10px;
    gap: 10px;
  }

  .phone .seg {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    background: var(--ink-2);
  }

  .phone .seg button {
    justify-content: center;
  }

  .phone .glassbtn {
    background: var(--ink-2);
    justify-content: flex-start;
  }

  .phone .tips {
    margin-top: auto;
    padding-top: 12px;
  }

  @media (prefers-reduced-motion: reduce) {
    .bg img,
    main > *,
    .caps {
      animation: none;
    }
  }
</style>
