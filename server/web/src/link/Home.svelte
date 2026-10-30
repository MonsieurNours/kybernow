<!--
  Accueil (docs/link-ui.md) : sur l'aperçu de l'écran de l'ordinateur, son
  état, son nom, son système, l'écran que la session demandera,
  « Se connecter », le profil et l'accès aux réglages. Rien d'autre : les
  chiffres et ce que sait ce navigateur sont dans les réglages. Ordinateur
  prêté à un ami (docs/pret.md) : « Prêté à Léa » et « Reprendre
  l'ordinateur » à la place de « Se connecter ».
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { backend, type Info, type Live, type Preview } from './backend'
  import { supportedCodecs, type CodecSupport } from './engine'
  import { choose, preferredProtocol } from './plan'
  import { canFullscreen, deviceKind, isIsolated, measureRefresh, screenMode } from './device'
  import { isCustom, pickProfile, PROFILES, type Prefs, type Profile } from './prefs'
  import Mark from '../lib/Mark.svelte'
  import Icon from '../lib/Icon.svelte'
  import Settings from './Settings.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { tName } from './names'
  import { adminLend, type LendView } from '../lib/api'
  import { errorText } from '../lib/store.svelte'
  import { countdown, isLent, lentLine, takingBack } from '../lib/lend'

  let {
    hostname,
    prefs = $bindable(),
    notice = '',
    onConnect,
    onSignedOut,
    onJoin,
  }: { hostname: string; prefs: Prefs; notice?: string; onConnect: () => void; onSignedOut: () => void; onJoin: () => void } = $props()

  let preview = $state<Preview | null>(null)
  let live = $state<Live | null>(null)
  let info = $state<Info | null>(null)
  let codecs = $state<CodecSupport[] | null>(null)
  let codecError = $state(false)
  let refresh = $state(60)
  let sheet = $state(false)
  /** Le prêt de l'ordinateur (`GET /api/lend`), quand la Station le connaît. */
  let lend = $state<LendView | null>(null)
  let lendBusy = $state(false)
  let lendError = $state('')
  let now = $state(Date.now() / 1000)
  const lent = $derived(isLent(lend) ? lend : null)
  const noticeAt = $derived(takingBack(lend))

  const kind = deviceKind()
  const mode = $derived(screenMode(refresh))
  const plan = $derived(codecs ? choose(prefs, mode, codecs) : null)
  // Lu une fois le moteur chargé (avec les formats) : corrigé, il change le transport en tactile.
  const protocol = $derived(codecs ? preferredProtocol() : null)
  const blocked = $derived(!isIsolated ? 'isolation' : codecError ? 'engine' : null)
  const custom = $derived(isCustom(prefs))
  /** En ligne : Admin répond (la page est là) et le moteur de Kyber se charge. */
  const status = $derived(codecError ? 'offline' : codecs || !isIsolated ? 'online' : 'searching')

  /** L'écran que la session demandera : celui de cet appareil, si l'ordinateur sait en créer un. */
  const displayText = $derived(
    prefs.virtualDisplay && info?.virtual_display.available !== false
      ? t('web.home.displaySized', { width: mode.width, height: mode.height, refresh: mode.refresh })
      : t('web.home.displaysAsIs'),
  )

  /** Le nom peut passer à la ligne après un point (« pc-salon.home »), pas au milieu d'un mot. */
  const displayName = $derived(hostname.replace(/([._])/g, '$1\u200b'))

  const profileNote = $derived(custom ? t('web.profile.customNote') : t(`web.profile.${prefs.profile}Note`))

  async function loadPreview() {
    const width = Math.min(1920, Math.round(window.innerWidth * (window.devicePixelRatio || 1)))
    const next = await backend.preview(width).catch(() => null)
    if (next) {
      if (preview) URL.revokeObjectURL(preview.url)
      preview = next
    }
  }

  async function loadState() {
    const [l, i] = await Promise.all([backend.live().catch(() => null), backend.info().catch(() => null)])
    live = l
    if (i) info = i
    if (i?.lend?.available) lend = await adminLend.view().catch(() => lend)
  }

  /** « Reprendre l'ordinateur » (préavis de 2 min pour l'ami), ou « Reprendre tout de suite ». */
  async function takeBack(right: boolean) {
    lendBusy = true
    lendError = ''
    try {
      lend = await adminLend.takeBack(right)
    } catch (e) {
      lendError = errorText(e)
    } finally {
      lendBusy = false
    }
  }

  // Le préavis compte à la seconde, et l'état est relu plus souvent pendant un prêt.
  $effect(() => {
    if (!lent) return
    const tick = setInterval(() => (now = Date.now() / 1000), 1000)
    const reread = setInterval(loadState, 3000)
    return () => {
      clearInterval(tick)
      clearInterval(reread)
    }
  })

  function loadCodecs() {
    codecError = false
    supportedCodecs()
      .then((c) => (codecs = c))
      .catch(() => (codecError = true))
  }

  onMount(() => {
    loadPreview()
    loadState()
    measureRefresh().then((hz) => (refresh = hz))
    loadCodecs()
    const timers = [setInterval(loadState, 8000), setInterval(loadPreview, 30000)]
    return () => {
      timers.forEach(clearInterval)
      if (preview) URL.revokeObjectURL(preview.url)
    }
  })

  function connect() {
    if (blocked === 'isolation') {
      location.reload()
      return
    }
    if (blocked === 'engine') {
      // Le moteur se recharge : Kyber a pu repartir entre-temps.
      loadCodecs()
      return
    }
    // Le plein écran se demande dans le clic lui-même.
    if (prefs.fullscreen && canFullscreen && !document.fullscreenElement) {
      document.documentElement.requestFullscreen({ navigationUI: 'hide' }).catch(() => {})
    }
    onConnect()
  }

  function choice(p: Profile) {
    pickProfile(prefs, p)
  }
</script>

<div class="home" class:phone={kind === 'phone'}>
  <div class="bg" aria-hidden="true">
    {#if preview}<img src={preview.url} alt="" />{:else}<div class="signature"></div>{/if}
  </div>
  <div class="veil" aria-hidden="true"></div>

  <header>
    <div class="brand">
      <Mark product="link" size={26} />
    </div>
    <nav>
      <a class="navlink" href="/">
        <Icon name="overview" size={16} />
        <span>{t('web.home.admin')}</span>
      </a>
    </nav>
  </header>

  <main>
    {#if kind === 'phone'}
      <div class="thumb" aria-hidden="true">
        {#if preview}<img src={preview.url} alt="" />{/if}
      </div>
    {/if}
    {#if notice}<p class="notice" role="status"><Icon name="check" size={15} stroke={2.4} />{notice}</p>{/if}
    {#if lent}
      <span class="pill busy"><span class="dot"></span>{lentLine(lent)}</span>
    {:else if live}
      <span class="pill busy"><span class="dot"></span>{t('web.home.live', { name: live.name })}</span>
    {:else}
      <span class="pill {status}"><span class="dot"></span>{status === 'offline' ? t('web.home.offline') : status === 'searching' ? t('web.home.searching') : t('web.home.online')}</span>
    {/if}
    <h1 class="disp" class:long={hostname.length > 14}>{displayName}</h1>
    <div class="facts">
      {#if info?.os}<span>{info.os}</span>{/if}
      <span>{displayText}</span>
    </div>

    {#if blocked}
      <p class="blocked" role="alert">
        {blocked === 'isolation' ? t('web.home.blockedIsolation') : t('web.home.blockedEngine')}
      </p>
    {/if}

    {#if lendError}<p class="blocked" role="alert">{lendError}</p>{/if}

    <div class="actions">
      {#if lent}
        <!-- Ordinateur prêté : le reprendre (préavis de 2 min), ou tout de suite pendant le préavis. -->
        {#if noticeAt !== null}
          <span class="taking mono" role="timer">{t('lend.host.takingBack', { time: countdown(noticeAt - now) })}</span>
          <button type="button" class="primary cta" disabled={lendBusy || lent.state === 'ending'} onclick={() => takeBack(true)}>{t('lend.host.takeBackNow')}</button>
        {:else}
          <button type="button" class="primary cta" disabled={lendBusy || lent.state === 'ending'} onclick={() => takeBack(false)}>{t('lend.host.takeBack')}</button>
        {/if}
      {:else}
      <button type="button" class="primary cta" onclick={connect}>
        {#if blocked}
          <Icon name="restart" size={18} stroke={2.2} />
          {t('web.home.retry')}
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
          {t('web.home.connect')}
        {/if}
      </button>
      {/if}
      <div class="profiles">
        <div class="seg" class:four={custom} role="radiogroup" aria-label={t('web.settings.profile')}>
          {#each PROFILES as p (p)}
            <button type="button" role="radio" aria-checked={!custom && prefs.profile === p} onclick={() => choice(p)}>
              {#if p === 'bureau'}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></svg>
              {:else if p === 'jeu'}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 8h10a4 4 0 0 1 4 4v1a3 3 0 0 1-5.4 1.8L14.5 13h-5l-1.1 1.8A3 3 0 0 1 3 13v-1a4 4 0 0 1 4-4z" /><path d="M8 10.5v3M6.5 12h3" /></svg>
              {:else}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3l2.4 5.2 5.6.6-4.2 3.8 1.2 5.6L12 15.4l-5 2.8 1.2-5.6L4 8.8l5.6-.6z" /></svg>
              {/if}
              {t(`web.profile.${p}`)}
            </button>
          {/each}
          {#if custom}
            <button type="button" role="radio" aria-checked="true" onclick={() => (sheet = true)}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2" /><circle cx="10" cy="17" r="2" /></svg>
              {t('web.profile.custom')}
            </button>
          {/if}
        </div>
        <p class="note">{profileNote}</p>
      </div>
      <button type="button" class="glassbtn" aria-label={tName('web.home.settingsFor', 'name', hostname)} onclick={() => (sheet = true)}>
        <Icon name="gear" size={18} stroke={1.7} />
        {t('web.home.settings')}
      </button>
      <button type="button" class="glassbtn" onclick={onJoin}>
        <Icon name="share" size={18} stroke={1.7} />
        {t('share.join.button')}
      </button>
    </div>
  </main>

  {#if sheet}
    <Settings bind:prefs {hostname} {mode} {plan} {codecs} {protocol} blocked={!!blocked} {info} onClose={() => (sheet = false)} {onSignedOut} />
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

  .notice {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-radius: 12px;
    font-size: 13.5px;
    color: var(--ok-text);
    background: rgb(9 12 17 / 0.7);
    backdrop-filter: blur(22px);
    box-shadow: inset 0 0 0 1px var(--ok-line);
  }

  .notice :global(svg) {
    color: var(--ok);
  }

  h1 {
    font-size: clamp(56px, 8vw, 104px);
    line-height: 0.9;
    font-weight: 680;
    letter-spacing: -0.045em;
    overflow-wrap: anywhere;
  }

  h1.long {
    font-size: clamp(40px, 5vw, 72px);
    line-height: 0.95;
  }

  .facts {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 15px;
    color: var(--text-2);
    margin-top: 2px;
  }

  .facts span:first-child:not(:last-child) {
    color: var(--muted);
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

  .pill.offline .dot {
    background: var(--ghost);
  }

  .pill.searching .dot {
    background: var(--faint);
    animation: pulse 1.2s ease-in-out infinite alternate;
  }

  @keyframes pulse {
    to {
      opacity: 0.3;
    }
  }

  .taking {
    font-size: 15px;
    color: var(--warn-text);
  }

  .actions {
    display: flex;
    align-items: flex-start;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 40px;
  }

  /* Les profils et leur phrase, juste dessous. */
  .profiles {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 3px;
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
    margin-top: 3px;
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
    max-width: 420px;
    padding-left: 4px;
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

  .phone h1 {
    font-size: 40px;
    margin-top: 10px;
  }

  .phone h1.long {
    font-size: 30px;
  }

  .phone .pill,
  .phone .notice {
    background: var(--ink-2);
  }

  .phone .facts {
    font-size: 14px;
  }

  .phone .actions {
    flex-direction: column;
    align-items: stretch;
    margin-top: 10px;
    gap: 10px;
  }

  .phone .profiles,
  .phone .glassbtn {
    margin-top: 0;
  }

  .phone .seg {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    background: var(--ink-2);
  }

  .phone .seg.four {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .phone .seg button {
    justify-content: center;
    padding: 0 8px;
  }

  .phone .glassbtn {
    background: var(--ink-2);
    justify-content: flex-start;
  }

  @media (prefers-reduced-motion: reduce) {
    .bg img,
    main > *,
    .pill.searching .dot {
      animation: none;
    }
  }
</style>
