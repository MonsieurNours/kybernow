<!--
  Session dans le navigateur : la console prépare l'écran, le moteur de Kyber
  dessine l'image dans une toile, la barre de session reste à portée (en haut
  au survol, ou sous la languette en tactile).
-->
<script lang="ts">
  import { onMount, tick, untrack } from 'svelte'
  import { backend } from './backend'
  import { Engine, supportedCodecs, type Latency } from './engine'
  import { choose, type Plan } from './plan'
  import { deviceId, type MouseMode, type Prefs, type TouchMode } from './prefs'
  import { deviceKind, deviceName, hasKeyboardLock, isTouch, measureRefresh, preferredProtocol, screenMode, type Mode } from './device'
  import { installAbsoluteMouse, installPointerGate, isRemoteCursorHidden, releasePointer, removeAbsoluteMouse, removePointerGate, setPositionMapper, setRemoteCursorHidden } from './pointer'
  import { forgetInputSocket, injected, learnDisplayId, watchInputSocket } from './inputs'
  import { installInputGuard, removeInputGuard } from './guard'
  import { SPECIAL_KEYS, TouchInput, sendKey, sendText } from './touch'
  import { errorText } from '../lib/store.svelte'
  import Mark from '../lib/Mark.svelte'
  import { diag, diagEnabled, installDiag } from './diag'

  let { hostname, prefs = $bindable(), onEnd }: { hostname: string; prefs: Prefs; onEnd: () => void } = $props()

  type Phase = 'preparing' | 'connecting' | 'live' | 'ended'
  let phase = $state<Phase>('preparing')
  let error = $state('')
  let reconnecting = $state('')
  let plan = $state<Plan | null>(null)
  let mode = $state<Mode | null>(null)
  let latency = $state<Latency | null>(null)
  let fps = $state(0)
  let spark = $state<number[]>([])
  let muted = $state(false)
  let fullscreen = $state(!!document.fullscreenElement)
  let keyboardLocked = $state(false)
  let barVisible = $state(true)
  let metricsOpen = $state(false)
  let keysOpen = $state(false)
  let hintOn = $state(true)
  let touchMode = $state<TouchMode>(untrack(() => prefs.touch))
  let touchBar = $state(false)
  let keyboardOpen = $state(false)
  let guide = $state(isTouch && !untrack(() => prefs.touchGuideSeen))
  let cursor = $state<{ x: number; y: number } | null>(null)
  let mods = $state(new Set<string>())
  let mouseMode = $state<MouseMode>(untrack(() => prefs.mouse))
  let diagText = $state('')

  let stage: HTMLElement
  let canvasBox: HTMLElement
  let touchLayer: HTMLElement
  let textInput: HTMLTextAreaElement
  let canvasSize = { width: 0, height: 0 }
  let engine: Engine | null = null
  let touch: TouchInput | null = null
  let barTimer: ReturnType<typeof setTimeout> | null = null
  let ended = false
  let ticketToken = ''
  let codecList: import('./engine').CodecSupport[] = []
  /** Sans première image à temps, on essaie l'autre transport. */
  let imageWatch: ReturnType<typeof setTimeout> | null = null
  let gameInForeground = false
  /** La console a préparé l'écran : il faudra le lui rendre. */
  let prepared = false

  const profileLabel = $derived(prefs.profile === 'jeu' ? 'Jeu' : 'Bureau')
  const latencyText = $derived(latency ? `${latency.total.toFixed(latency.total < 10 ? 1 : 0).replace('.', ',')} ms` : '— ms')
  const formatText = $derived(
    mode && plan ? `${mode.width} × ${mode.height} · ${mode.refresh} Hz · ${plan.codec === 'hevc' ? 'HEVC' : 'H.264'}${plan.yuv444 ? ' 4:4:4' : ''}` : '',
  )
  const sparkPath = $derived(
    spark.length < 2
      ? ''
      : (() => {
          const max = Math.max(20, ...spark)
          return spark.map((v, i) => `${i ? 'L' : 'M'}${((i / (spark.length - 1)) * 64).toFixed(1)} ${(20 - (v / max) * 18).toFixed(1)}`).join(' ')
        })(),
  )
  const parts = $derived(
    latency
      ? [
          { label: 'Ordinateur', sub: 'Capture et encodage', v: latency.server, cls: 'a' },
          { label: 'Réseau', sub: 'Jusqu’à cet appareil', v: latency.network, cls: 'b' },
          { label: 'Ce navigateur', sub: 'Décodage et affichage', v: latency.client, cls: 'c' },
        ]
      : [],
  )

  function ms(v: number): string {
    return `${v.toFixed(1).replace('.', ',')} ms`
  }

  function fit() {
    const canvas = document.getElementById('kn-canvas') as HTMLCanvasElement | null
    if (!canvas || !canvasSize.width || !stage) return
    const r = stage.getBoundingClientRect()
    const scale = Math.min(r.width / canvasSize.width, r.height / canvasSize.height)
    canvas.style.width = `${Math.floor(canvasSize.width * scale)}px`
    canvas.style.height = `${Math.floor(canvasSize.height * scale)}px`
  }

  let cursorWatch: MutationObserver | null = null

  /** Le moteur dessine le pointeur de l'ordinateur en curseur de la toile, et
   * le masque (`cursor: none`) quand l'ordinateur le dit caché, ce que Windows
   * fait sans cesse sans souris branchée. Hors capture, on garde une flèche. */
  function keepPointerVisible(canvas: HTMLCanvasElement) {
    cursorWatch?.disconnect()
    setRemoteCursorHidden(false)
    const fix = () => {
      const c = canvas.style.cursor
      // Le moteur pose `none` (masqué) ou l'image du pointeur ; `default` vient de nous.
      if (c === 'none') setRemoteCursorHidden(true)
      else if (c && c !== 'default') setRemoteCursorHidden(false)
      if (c === 'none' && !document.pointerLockElement) canvas.style.cursor = 'default'
    }
    cursorWatch = new MutationObserver(fix)
    cursorWatch.observe(canvas, { attributes: true, attributeFilter: ['style'] })
    // Souris rendue après une capture : la flèche revient aussi.
    document.removeEventListener('pointerlockchange', cursorFix)
    cursorFix = fix
    document.addEventListener('pointerlockchange', cursorFix)
    fix()
  }

  let cursorFix = () => {}

  function freshCanvas() {
    canvasBox.querySelector('canvas')?.remove()
    const canvas = document.createElement('canvas')
    canvas.id = 'kn-canvas'
    canvas.width = 800
    canvas.height = 600
    canvas.tabIndex = 0
    canvasBox.appendChild(canvas)
    keepPointerVisible(canvas)
    return canvas
  }

  function streamHost(): string {
    return location.hostname.replace(/^\[|\]$/g, '')
  }

  async function start() {
    error = ''
    phase = 'preparing'
    try {
      const [refresh, codecs, ticket] = await Promise.all([measureRefresh(), supportedCodecs(), backend.ticket()])
      mode = screenMode(refresh)
      codecList = codecs
      ticketToken = ticket.token
      const device = deviceName()
      await backend.prepare({
        mode,
        audio: prefs.audio,
        virtualDisplay: prefs.virtualDisplay,
        profile: profileLabel,
        device: { id: deviceId(), name: device.name, kind: deviceKind(), os: device.os, app: device.app },
      })
      prepared = true
      await connect(preferredProtocol)
    } catch (e) {
      fail(errorText(e))
    }
  }

  async function connect(protocol: 'kymux' | 'websocket') {
    if (!mode) return
    phase = 'connecting'
    plan = choose(prefs, mode, codecList, protocol)
    await tick()
    freshCanvas()
    const e = new Engine('kn-canvas', onEngine)
    engine = e
    if (imageWatch) clearTimeout(imageWatch)
    imageWatch = setTimeout(() => noImage(e, protocol), protocol === 'kymux' ? 12000 : 20000)
    await e.connect(streamHost(), location.port || '443', ticketToken, plan)
  }

  /** L'image n'est pas venue : WebTransport bloqué (réseau, navigateur), on passe par WebSocket. */
  async function noImage(e: Engine, protocol: 'kymux' | 'websocket') {
    if (engine !== e || phase === 'live' || ended) return
    if (protocol === 'kymux') {
      console.warn('KyberNow : pas d’image en WebTransport, essai par WebSocket')
      reconnecting = 'Autre chemin pour l’image (WebSocket)…'
      engine = null
      await e.stop()
      if (!ended) connect('websocket')
    } else {
      fail(`L'image de ${hostname} n'arrive pas dans ce navigateur. Essayez Chrome ou Edge, ou le format H.264 dans Réglages › Avancé.`)
    }
  }

  function onEngine(e: import('./engine').EngineEvent) {
    switch (e.type) {
      case 'reconnecting':
        reconnecting = `Connexion perdue, nouvelle tentative (${e.attempt} sur ${e.max})…`
        break
      case 'streaming':
        reconnecting = ''
        break
      case 'resized':
        canvasSize = { width: e.width, height: e.height }
        fit()
        break
      case 'first-image':
        if (imageWatch) clearTimeout(imageWatch)
        imageWatch = null
        phase = 'live'
        reconnecting = ''
        learnDisplayId()
        document.getElementById('kn-canvas')?.focus()
        scheduleBarHide()
        setTimeout(() => (hintOn = false), 7000)
        break
      case 'error':
        console.warn('KyberNow : connexion impossible', e.message)
        fail(`Le serveur Kyber de ${hostname} ne répond pas. Vérifiez qu'il est en marche dans la console.`)
        break
      case 'stopped':
        if (ended) return
        if (e.reason === 'restart') {
          // Le moteur a perdu sa toile en se reconnectant : on recommence proprement.
          restart()
        } else if (e.reason === 'decode_error') {
          fail("Ce navigateur n'arrive pas à décoder l'image. Essayez le format H.264 dans Réglages › Avancé.")
        } else if (e.reason === 'requested' || e.reason === 'streaming_stopped') {
          fail(`${hostname} a mis fin à la session.`)
        } else {
          fail(`La connexion à ${hostname} s'est interrompue.`)
        }
        break
    }
  }

  async function restart() {
    await engine?.stop()
    engine = null
    start()
  }

  function fail(message: string) {
    if (imageWatch) clearTimeout(imageWatch)
    imageWatch = null
    error = message
    phase = 'ended'
    barVisible = true
    // L'écran préparé pour cette session est rendu à l'ordinateur.
    const e = engine
    engine = null
    e?.stop()
    if (prepared) {
      prepared = false
      backend.end(null).catch(() => {})
    }
  }

  async function quit() {
    if (ended) return
    ended = true
    if (imageWatch) clearTimeout(imageWatch)
    const median = engine?.sessionMedian() ?? null
    const e = engine
    engine = null
    // On rend la main tout de suite ; le moteur et la console finissent derrière.
    const wasPrepared = prepared
    prepared = false
    releasePointer()
    if (document.fullscreenElement) document.exitFullscreen().catch(() => {})
    onEnd()
    await e?.stop()
    if (wasPrepared) backend.end(median).catch(() => {})
  }

  function setMouseMode(m: MouseMode) {
    mouseMode = m
    prefs.mouse = m
    if (!pointerAllowed()) releasePointer()
  }

  function pointerAllowed(): boolean {
    return mouseMode === 'jeu' || (mouseMode === 'auto' && gameInForeground)
  }

  function cycleMouse() {
    setMouseMode(mouseMode === 'auto' ? 'bureau' : mouseMode === 'bureau' ? 'jeu' : 'auto')
  }

  const mouseLabel = $derived(mouseMode === 'auto' ? 'Souris auto' : mouseMode === 'jeu' ? 'Souris jeu' : 'Souris bureau')

  function showBar() {
    barVisible = true
    scheduleBarHide()
  }

  function scheduleBarHide() {
    if (barTimer) clearTimeout(barTimer)
    if (isTouch || metricsOpen || keysOpen || phase !== 'live') return
    barTimer = setTimeout(() => (barVisible = false), 2500)
  }

  function onPointerMove(e: PointerEvent) {
    if (isTouch || e.pointerType === 'touch') return
    if (e.clientY < 24 && !document.pointerLockElement) showBar()
  }

  async function toggleFullscreen() {
    if (document.fullscreenElement) await document.exitFullscreen().catch(() => {})
    else await document.documentElement.requestFullscreen({ navigationUI: 'hide' }).catch(() => {})
  }

  async function onFullscreenChange() {
    fullscreen = !!document.fullscreenElement
    keyboardLocked = false
    if (fullscreen && hasKeyboardLock) {
      try {
        await (navigator as any).keyboard.lock()
        keyboardLocked = true
      } catch {
        keyboardLocked = false
      }
    }
    setTimeout(fit, 50)
  }

  function toggleMute() {
    muted = !muted
    engine?.setMute(muted)
  }

  function canvas(): HTMLElement | null {
    return document.getElementById('kn-canvas')
  }

  function pressSpecial(k: (typeof SPECIAL_KEYS)[number]) {
    if (k.sticky) {
      const next = new Set(mods)
      if (next.has(k.key)) next.delete(k.key)
      else next.add(k.key)
      mods = next
      return
    }
    sendKey(canvas(), k.key, k.code, mods)
    mods = new Set()
  }

  function sendCombo(keys: { key: string; code: string }[], last: { key: string; code: string }) {
    sendKey(canvas(), last.key, last.code, new Set(keys.map((k) => k.key)))
  }

  function onTextInput(e: Event) {
    const ev = e as InputEvent
    if (ev.inputType === 'deleteContentBackward') sendKey(canvas(), 'Backspace', 'Backspace', mods)
    else if (ev.inputType === 'insertLineBreak') sendKey(canvas(), 'Enter', 'Enter', mods)
    else if (ev.data) sendText(canvas(), ev.data, mods)
    textInput.value = ''
    if (mods.size) mods = new Set()
  }

  async function openKeyboard() {
    keyboardOpen = true
    touchBar = false
    await tick()
    textInput.focus()
  }

  function closeKeyboard() {
    keyboardOpen = false
    textInput?.blur()
  }

  function setTouchMode(m: TouchMode) {
    touchMode = m
    prefs.touch = m
    if (touch) touch.mode = m
    cursor = m === 'trackpad' ? (touch?.cursor ?? null) : null
  }

  function closeGuide() {
    guide = false
    prefs.touchGuideSeen = true
  }

  onMount(() => {
    installDiag()
    // Avant le moteur : seuls les clics sur l'image partent à l'ordinateur.
    installInputGuard((t) => t === canvas() || (!!touchLayer && t === touchLayer))
    installPointerGate(pointerAllowed)
    installAbsoluteMouse()
    watchInputSocket()
    // La toile a les proportions de l'image : un point de la toile donne une
    // position sur l'écran de l'ordinateur (taille de l'image reçue).
    setPositionMapper((cx, cy) => {
      const c = document.getElementById('kn-canvas')
      if (!c || !canvasSize.width) return null
      const r = c.getBoundingClientRect()
      if (cx < r.left || cy < r.top || cx >= r.right || cy >= r.bottom) return null
      return {
        x: Math.min(canvasSize.width - 1, ((cx - r.left) * canvasSize.width) / r.width),
        y: Math.min(canvasSize.height - 1, ((cy - r.top) * canvasSize.height) / r.height),
      }
    })
    start()
    // Ctrl + Alt + Maj : la barre, et la souris rendue.
    const shortcut = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.altKey && e.shiftKey && ['Control', 'Alt', 'Shift'].includes(e.key)) {
        releasePointer()
        if (barVisible) barVisible = false
        else showBar()
      }
    }
    window.addEventListener('keydown', shortcut, true)
    let prev = { ...diag }
    // Quinze secondes d'historique : on reproduit, puis on capture après coup.
    const hist: { souris: number; relayee: number; entrees: number; flux: number; tailles: string }[] = []
    const diagTimer = diagEnabled
      ? setInterval(() => {
          const c = document.getElementById('kn-canvas')
          const r = c?.getBoundingClientRect()
          hist.push({
            souris: diag.trusted - prev.trusted,
            relayee: diag.relayed - prev.relayed,
            entrees: diag.wsInputs - prev.wsInputs,
            flux: diag.streamWrites - prev.streamWrites,
            tailles: [...diag.sizes].sort((a, b) => a - b).join('/') || '-',
          })
          diag.sizes.clear()
          if (hist.length > 15) hist.shift()
          const row = (k: 'souris' | 'relayee' | 'entrees' | 'flux') => hist.map((h) => String(h[k]).padStart(3)).join('')
          diagText = [
            `souris  ${row('souris')}`,
            `relayée ${row('relayee')}`,
            `entrées ${row('entrees')}`,
            `flux    ${row('flux')}`,
            `octets  ${hist.slice(-8).map((h) => h.tailles).join(" ")} (8 dernières s)`,
            `positions page ${injected.count} · sorties ${diag.outside} · dernière ${diag.lastX},${diag.lastY} · toile ${r ? `${Math.round(r.left)},${Math.round(r.top)} ${Math.round(r.width)}×${Math.round(r.height)}` : '—'}`,
            `capture ${document.pointerLockElement ? 'oui' : 'non'} · pointeur de l'ordinateur ${isRemoteCursorHidden() ? 'masqué' : 'visible'} · transport ${plan?.protocol ?? '—'}`,
          ].join('\n')
          prev = { ...diag }
        }, 1000)
      : null
    const foreground = setInterval(async () => {
      if (phase !== 'live' || mouseMode !== 'auto') return
      const f = await backend.foreground().catch(() => null)
      const game = !!f?.game
      if (game !== gameInForeground) {
        gameInForeground = game
        if (!game) releasePointer()
      }
    }, 1000)
    const resize = () => fit()
    const unlockAudio = () => engine?.resumeAudio()
    const leave = () => {
      if (ended || !prepared) return
      fetch('/api/stream/end', { method: 'POST', keepalive: true, credentials: 'same-origin', headers: { 'x-kyber-admin': '1', 'content-type': 'application/json' }, body: '{}' })
    }
    window.addEventListener('resize', resize)
    document.addEventListener('fullscreenchange', onFullscreenChange)
    window.addEventListener('pointerdown', unlockAudio)
    window.addEventListener('keydown', unlockAudio)
    window.addEventListener('pagehide', leave)
    if (document.fullscreenElement) onFullscreenChange()

    // Mesures et signal de vie.
    const metrics = setInterval(() => {
      if (!engine || phase !== 'live') return
      latency = engine.latency.median()
      fps = engine.latency.fps()
      if (latency) spark = [...spark.slice(-31), latency.total]
    }, 500)
    const heartbeat = setInterval(async () => {
      if (!engine || phase !== 'live') return
      const l = engine.sample()
      const r = await backend.heartbeat(l?.total ?? null, profileLabel).catch(() => null)
      if (r?.end) fail('La console de l’ordinateur a mis fin à la session.')
    }, 5000)

    if (isTouch) {
      touch = new TouchInput(touchLayer, canvas, touchMode, (p) => (cursor = touchMode === 'trackpad' ? p : null))
      if (touchMode === 'trackpad') cursor = touch.cursor
    }

    return () => {
      clearInterval(metrics)
      clearInterval(heartbeat)
      clearInterval(foreground)
      if (diagTimer) clearInterval(diagTimer)
      cursorWatch?.disconnect()
      document.removeEventListener('pointerlockchange', cursorFix)
      if (imageWatch) clearTimeout(imageWatch)
      window.removeEventListener('keydown', shortcut, true)
      releasePointer()
      removePointerGate()
      removeAbsoluteMouse()
      removeInputGuard()
      setPositionMapper(() => null)
      forgetInputSocket()
      if (barTimer) clearTimeout(barTimer)
      touch?.destroy()
      window.removeEventListener('resize', resize)
      document.removeEventListener('fullscreenchange', onFullscreenChange)
      window.removeEventListener('pointerdown', unlockAudio)
      window.removeEventListener('keydown', unlockAudio)
      window.removeEventListener('pagehide', leave)
      if (!ended) {
        ended = true
        engine?.stop()
        if (prepared) backend.end(null).catch(() => {})
      }
    }
  })
</script>

<div class="session" class:touch={isTouch} onpointermove={onPointerMove} role="application" aria-label="Session sur {hostname}">
  <div class="stage" class:kb={keyboardOpen} bind:this={stage} oncontextmenu={(e) => e.preventDefault()} role="presentation">
    <div class="canvas-box" bind:this={canvasBox}></div>
    <div class="touch-layer" class:active={isTouch && phase === 'live'} bind:this={touchLayer}></div>
    {#if cursor && phase === 'live'}
      <div class="tcursor" style="left: {cursor.x}px; top: {cursor.y}px" aria-hidden="true"></div>
    {/if}
  </div>

  {#if phase !== 'live'}
    <div class="overlay" role="status">
      {#if phase === 'ended'}
        <div class="card">
          <Mark size={34} state="now" />
          <h2 class="disp">Session terminée</h2>
          <p>{error}</p>
          <div class="row">
            <button type="button" class="primary" onclick={() => { ended = false; restart() }}>Se reconnecter</button>
            <button type="button" class="ghost" onclick={quit}>Revenir à l'accueil</button>
          </div>
        </div>
      {:else}
        <div class="card">
          <Mark size={34} state="now" lit breathe />
          <h2 class="disp">Vous entrez dans {hostname}</h2>
          <ul class="steps mono">
            <li class:done={phase === 'connecting'}>{phase === 'preparing' ? 'Préparation de l’écran…' : `Écran ${mode?.width ?? ''} × ${mode?.height ?? ''} à ${mode?.refresh ?? ''} Hz`}</li>
            {#if phase === 'connecting'}<li>{reconnecting || 'Connexion et premières images…'}</li>{/if}
          </ul>
          <button type="button" class="ghost" onclick={quit}>Annuler</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if phase === 'live' && reconnecting}
    <div class="toast glass" role="status">{reconnecting}</div>
  {/if}

  <!-- Barre de session (souris et clavier) -->
  {#if !isTouch && phase === 'live'}
    {#if !barVisible}
      <button type="button" class="edge" aria-label="Afficher la barre de session" onclick={showBar}><span></span></button>
    {/if}
    <div class="bar glass" class:hidden={!barVisible} role="toolbar" tabindex="-1" aria-label="Session" onpointerenter={() => barTimer && clearTimeout(barTimer)} onpointerleave={scheduleBarHide}>
      <Mark size={22} state="now" lit breathe />
      <span class="host">{hostname}</span>
      <span class="sep" aria-hidden="true"></span>
      <button type="button" class="lat" aria-expanded={metricsOpen} aria-label="Mesures détaillées" onclick={() => { metricsOpen = !metricsOpen; keysOpen = false }}>
        <span class="mono value">{latencyText}</span>
        <svg width="64" height="22" viewBox="0 0 64 22" aria-hidden="true"><path d={sparkPath} fill="none" stroke="#9A8CFF" stroke-width="1.5" stroke-linejoin="round" /></svg>
        <span class="mono fmt">{formatText}</span>
      </button>
      <span class="sep" aria-hidden="true"></span>
      <span class="chip static">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9A8CFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></svg>
        <span class="label">{profileLabel}</span>
      </span>
      <button type="button" class="chip" aria-label={mouseLabel} title="Auto : capturée seulement quand un jeu est en plein écran sur l'ordinateur" onclick={cycleMouse}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round" aria-hidden="true"><path d="M5 3l14 7-6 2-2 6z" /></svg>
        <span class="label">{mouseLabel}</span>
      </button>
      {#if prefs.audio}
        <button type="button" class="chip" aria-pressed={muted} aria-label={muted ? 'Remettre le son' : 'Couper le son'} onclick={toggleMute}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" />{#if muted}<path d="M17 9l4 6M21 9l-4 6" />{:else}<path d="M17 9a4 4 0 0 1 0 6" />{/if}</svg>
          <span class="label">{muted ? 'Muet' : 'Son'}</span>
        </button>
      {/if}
      <button type="button" class="chip" aria-label="Touches" aria-expanded={keysOpen} onclick={() => { keysOpen = !keysOpen; metricsOpen = false }}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
        <span class="label">Touches</span>
      </button>
      <span class="sep" aria-hidden="true"></span>
      <button type="button" class="chip icon" aria-label={fullscreen ? 'Quitter le plein écran' : 'Plein écran'} onclick={toggleFullscreen}>
        {#if fullscreen}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" /></svg>
        {:else}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></svg>
        {/if}
      </button>
      <button type="button" class="quit" aria-label="Quitter" onclick={quit}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 3v9" /><path d="M6.3 6.3a8 8 0 1 0 11.4 0" /></svg>
        <span class="label">Quitter</span>
      </button>

      {#if keysOpen}
        <div class="menu glass" role="menu" aria-label="Touches">
          <button type="button" role="menuitem" onclick={() => { engine?.ctrlAltDel(); keysOpen = false }}>Ctrl + Alt + Suppr<span class="mono">verrouiller, changer</span></button>
          <button type="button" role="menuitem" onclick={() => { sendKey(canvas(), 'Meta', 'MetaLeft', new Set()); keysOpen = false }}>Touche ⊞ seule<span class="mono">menu Démarrer</span></button>
          <button type="button" role="menuitem" onclick={() => { sendCombo([{ key: 'Alt', code: 'AltLeft' }], { key: 'Tab', code: 'Tab' }); keysOpen = false }}>Alt + Tab<span class="mono">changer d'app</span></button>
          <button type="button" role="menuitem" onclick={() => { sendKey(canvas(), 'PrintScreen', 'PrintScreen', new Set()); keysOpen = false }}>Impr. écran<span class="mono">capture</span></button>
        </div>
      {/if}
    </div>

    {#if metricsOpen}
      <section class="metrics glass" aria-label="Mesures détaillées">
        <div class="mhead">
          <span>De la capture à votre écran</span>
          <span class="mono muted">médiane sur 3 s</span>
        </div>
        {#if latency}
          <div class="bars">
            {#each parts as p (p.cls)}<div class={p.cls} style="flex: {Math.max(p.v, 0.3)}">{p.label}</div>{/each}
          </div>
          <div class="nums">
            {#each parts as p (p.cls)}<div><span class="lit-number">{ms(p.v)}</span><span class="muted">{p.sub}</span></div>{/each}
          </div>
        {:else}
          <p class="muted">Les mesures arrivent avec les premières secondes d'image.</p>
        {/if}
        <div class="mono foot">
          <span>{fps} images/s</span><span>{plan?.mbps ?? '—'} Mbit/s</span><span>{plan?.reliable ? 'Fiable' : 'Rapide'}</span>
        </div>
        <p class="muted small">Dans un navigateur, l'affichage attend le rafraîchissement de l'écran : comptez un peu plus que dans l'app.</p>
      </section>
    {/if}

    {#if hintOn}
      <div class="hint glass" role="status">
        {#if keyboardLocked}
          Toutes les touches vont à {hostname}. La barre : en haut de l'écran, ou <kbd class="mono">Ctrl</kbd> + <kbd class="mono">Alt</kbd> + <kbd class="mono">Maj</kbd>
        {:else}
          La barre : en haut de l'écran, ou <kbd class="mono">Ctrl</kbd> + <kbd class="mono">Alt</kbd> + <kbd class="mono">Maj</kbd>. Les raccourcis système sont dans le menu Touches.
        {/if}
      </div>
    {/if}
  {/if}

  <!-- Tactile : languette, barre compacte, clavier -->
  {#if isTouch && phase === 'live'}
    {#if !touchBar}
      <button type="button" class="tab glass" aria-label="Afficher la barre de session" onpointerdown={(e) => e.preventDefault()} onclick={() => { touchBar = true; guide = false }}>
        <span class="mono">{latencyText}</span>
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6" /></svg>
      </button>
    {:else}
      <div class="tbar glass" role="toolbar" tabindex="-1" aria-label="Session" onpointerdown={(e) => e.preventDefault()}>
        <Mark size={20} state="now" lit />
        <span class="mono tlat">{latencyText}</span>
        <span class="sep" aria-hidden="true"></span>
        <div class="tmodes" role="radiogroup" aria-label="Toucher">
          <button type="button" role="radio" aria-checked={touchMode === 'trackpad'} onclick={() => setTouchMode('trackpad')}>Pavé tactile</button>
          <button type="button" role="radio" aria-checked={touchMode === 'direct'} onclick={() => setTouchMode('direct')}>Toucher direct</button>
        </div>
        <button type="button" class="tb" onclick={openKeyboard} aria-label="Clavier">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
        </button>
        {#if prefs.audio}
          <button type="button" class="tb" aria-pressed={muted} onclick={toggleMute} aria-label={muted ? 'Remettre le son' : 'Couper le son'}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" />{#if muted}<path d="M17 9l4 6M21 9l-4 6" />{:else}<path d="M17 9a4 4 0 0 1 0 6" />{/if}</svg>
          </button>
        {/if}
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="tb" onclick={() => (touchBar = false)} aria-label="Masquer la barre">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 15l6-6 6 6" /></svg>
        </button>
        <button type="button" class="tb danger-tb" onclick={quit} aria-label="Quitter">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 3v9" /><path d="M6.3 6.3a8 8 0 1 0 11.4 0" /></svg>
        </button>
      </div>
    {/if}

    {#if !keyboardOpen}
      <button type="button" class="fab glass" onpointerdown={(e) => e.preventDefault()} onclick={openKeyboard} aria-label="Ouvrir le clavier">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
      </button>
    {:else}
      <div class="keys" role="toolbar" aria-label="Touches spéciales">
        {#each SPECIAL_KEYS as k (k.code)}
          <button type="button" class="key" aria-pressed={k.sticky ? mods.has(k.key) : undefined} aria-label={k.aria} onpointerdown={(e) => e.preventDefault()} onclick={() => pressSpecial(k)}>{k.label}</button>
        {/each}
        <button type="button" class="key" onpointerdown={(e) => e.preventDefault()} onclick={() => engine?.ctrlAltDel()}>Ctrl+Alt+Suppr</button>
        <button type="button" class="key close" aria-label="Fermer le clavier" onclick={closeKeyboard}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6" /></svg>
        </button>
      </div>
    {/if}

    {#if guide}
      <section class="guide glass" aria-label="Les gestes">
        <strong>{touchMode === 'trackpad' ? 'Pavé tactile' : 'Toucher direct'}</strong>
        {#if touchMode === 'trackpad'}
          <p>Glissez pour déplacer le pointeur, touchez pour cliquer, à deux doigts pour le clic droit. Glissez à deux doigts pour défiler.</p>
        {:else}
          <p>Touchez pour cliquer là où est le doigt, longuement pour le clic droit. Glissez à deux doigts pour défiler.</p>
        {/if}
        <button type="button" onclick={closeGuide}>Compris</button>
      </section>
    {/if}
  {/if}

  {#if diagEnabled && diagText}
    <pre class="diag mono" aria-hidden="true">{diagText}</pre>
  {/if}

  <textarea
    class="ime"
    bind:this={textInput}
    oninput={onTextInput}
    autocapitalize="off"
    autocomplete="off"
    spellcheck="false"
    aria-label="Saisie vers l'ordinateur"
  ></textarea>
</div>

<style>
  .session {
    position: fixed;
    inset: 0;
    background: #000;
    overflow: hidden;
    color: var(--text);
    user-select: none;
    -webkit-user-select: none;
  }

  .stage {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }

  .stage.kb {
    bottom: 50%;
  }

  .canvas-box {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
  }

  .canvas-box :global(canvas) {
    display: block;
    outline: none;
    touch-action: none;
  }

  .touch-layer {
    position: absolute;
    inset: 0;
    display: none;
    touch-action: none;
  }

  .touch-layer.active {
    display: block;
  }

  .tcursor {
    position: fixed;
    width: 0;
    height: 0;
    pointer-events: none;
  }

  .tcursor::before {
    content: '';
    position: absolute;
    left: -7px;
    top: -7px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid #fff;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.6);
  }

  .glass {
    background: rgb(9 12 17 / 0.7);
    backdrop-filter: blur(26px) saturate(1.4);
    -webkit-backdrop-filter: blur(26px) saturate(1.4);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1), inset 0 1px 0 rgb(255 255 255 / 0.07), 0 18px 50px -12px rgb(0 0 0 / 0.7);
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 24px;
    background: radial-gradient(60% 50% at 50% 40%, rgb(109 91 255 / 0.14), transparent 70%), var(--ink-0);
  }

  .overlay .card {
    width: min(480px, 100%);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 16px;
  }

  .overlay h2 {
    font-size: clamp(26px, 4vw, 36px);
    line-height: 1.05;
    font-weight: 650;
    letter-spacing: -0.03em;
  }

  .overlay p {
    color: var(--muted);
  }

  .overlay .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 6px;
  }

  .steps {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }

  .steps li.done::before {
    content: '✓ ';
    color: var(--ok);
  }

  /* Languette du haut : la barre au clic, sans viser le bord de l'écran. */
  .edge {
    position: absolute;
    left: 50%;
    top: 0;
    width: 220px;
    height: 18px;
    translate: -50% 0;
    padding: 0;
    border: 0;
    background: none;
    display: flex;
    justify-content: center;
    z-index: 4;
  }

  .edge span {
    width: 100%;
    height: 4px;
    border-radius: 0 0 4px 4px;
    background: rgb(255 255 255 / 0.3);
    transition: height 0.2s, background-color 0.2s;
  }

  .edge:hover span {
    height: 8px;
    background: rgb(154 140 255 / 0.7);
  }

  .bar {
    position: absolute;
    left: 50%;
    top: 14px;
    translate: -50% 0;
    height: 54px;
    padding: 0 8px 0 14px;
    border-radius: 14px;
    display: flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
    transition:
      translate 0.35s var(--ease-out),
      opacity 0.35s;
    z-index: 5;
  }

  .bar.hidden {
    translate: -50% -90px;
    opacity: 0;
  }

  .host {
    font-size: 14px;
    font-weight: 600;
    margin: 0 8px 0 6px;
  }

  .sep {
    width: 1px;
    height: 26px;
    background: rgb(255 255 255 / 0.12);
    margin: 0 2px;
  }

  .lat {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 12px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: inherit;
  }

  .lat[aria-expanded='true'] {
    background: rgb(154 140 255 / 0.16);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
  }

  .lat .value {
    font-size: 15px;
    font-weight: 500;
    color: var(--signal-light);
    min-width: 64px;
    text-align: right;
  }

  .fmt {
    font-size: 11px;
    color: var(--muted);
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 36px;
    padding: 0 11px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text-2);
    font-size: 13px;
  }

  button.chip:hover,
  .chip[aria-expanded='true'] {
    background: rgb(255 255 255 / 0.07);
    color: #fff;
  }

  .chip.static {
    cursor: default;
  }

  .chip.icon {
    width: 40px;
    padding: 0;
    justify-content: center;
  }

  .quit {
    height: 38px;
    padding: 0 14px;
    margin-left: 4px;
    border: 0;
    border-radius: 9px;
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
    font-size: 13px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .menu {
    position: absolute;
    top: 62px;
    right: 120px;
    width: 300px;
    border-radius: 12px;
    padding: 6px;
    display: flex;
    flex-direction: column;
  }

  .menu button {
    height: 42px;
    padding: 0 11px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text-2);
    font-size: 13px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .menu button:hover {
    background: rgb(255 255 255 / 0.07);
    color: #fff;
  }

  .menu .mono {
    font-size: 10px;
    color: var(--faint);
  }

  .metrics {
    position: absolute;
    left: 50%;
    top: 80px;
    translate: -50% 0;
    width: min(620px, calc(100% - 32px));
    border-radius: 16px;
    padding: 18px 20px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    z-index: 5;
  }

  .mhead {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    font-size: 15px;
    font-weight: 600;
  }

  .mhead .mono {
    font-size: 11px;
    font-weight: 400;
  }

  .bars {
    display: flex;
    height: 34px;
    border-radius: 8px;
    overflow: hidden;
    gap: 2px;
    font-size: 12px;
  }

  .bars div {
    display: flex;
    align-items: center;
    padding-left: 10px;
    white-space: nowrap;
    overflow: hidden;
  }

  .bars .a {
    background: var(--signal-strong);
  }

  .bars .b {
    background: var(--signal);
    color: #05070a;
  }

  .bars .c {
    background: var(--signal-light);
    color: #05070a;
  }

  .nums {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }

  .nums div {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
  }

  .nums .lit-number {
    font-size: 20px;
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    gap: 22px;
    padding-top: 14px;
    border-top: 1px solid rgb(255 255 255 / 0.1);
    font-size: 11.5px;
    color: var(--muted);
  }

  .small {
    font-size: 12px;
  }

  .hint,
  .toast {
    position: absolute;
    left: 50%;
    bottom: 80px;
    translate: -50% 0;
    max-width: calc(100% - 32px);
    padding: 10px 16px;
    border-radius: 12px;
    font-size: 13px;
    color: var(--text-2);
    display: flex;
    align-items: center;
    gap: 10px;
    animation: hint 7s ease-out both;
    pointer-events: none;
  }

  .toast {
    animation: none;
    top: 80px;
    bottom: auto;
  }

  kbd {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 5px;
    background: rgb(255 255 255 / 0.1);
    color: var(--text);
  }

  @keyframes hint {
    0% {
      opacity: 0;
      translate: -50% 8px;
    }
    8%,
    84% {
      opacity: 1;
      translate: -50% 0;
    }
    100% {
      opacity: 0;
    }
  }

  /* Tactile */
  .tab {
    position: absolute;
    left: 50%;
    top: 0;
    translate: -50% 0;
    width: 120px;
    height: 40px;
    border: 0;
    border-radius: 0 0 14px 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--signal-light);
    font-size: 12.5px;
    z-index: 5;
  }

  .tbar {
    position: absolute;
    left: 50%;
    top: max(8px, env(safe-area-inset-top));
    translate: -50% 0;
    max-width: calc(100% - 16px);
    padding: 4px 6px 4px 12px;
    border-radius: 16px;
    display: flex;
    align-items: center;
    gap: 2px;
    white-space: nowrap;
    z-index: 5;
    animation: down 0.35s var(--ease-out) both;
  }

  @keyframes down {
    from {
      opacity: 0;
      translate: -50% -12px;
    }
    to {
      opacity: 1;
      translate: -50% 0;
    }
  }

  .tlat {
    font-size: 12px;
    color: var(--signal-light);
    margin: 0 8px;
    min-width: 46px;
  }

  .tmodes {
    display: flex;
    padding: 2px;
    margin: 0 4px;
    border-radius: 11px;
    background: rgb(255 255 255 / 0.06);
  }

  .tmodes button {
    height: 40px;
    padding: 0 12px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: var(--muted);
    font-size: 12.5px;
  }

  .tmodes button[aria-checked='true'] {
    background: rgb(154 140 255 / 0.2);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.6);
    color: #fff;
  }

  .tb {
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: 11px;
    background: none;
    color: var(--text-2);
    display: grid;
    place-items: center;
  }

  .tb[aria-pressed='true'] {
    color: var(--warn);
  }

  .danger-tb {
    color: var(--bad-text);
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
  }

  .fab {
    position: absolute;
    right: max(20px, env(safe-area-inset-right));
    bottom: max(20px, env(safe-area-inset-bottom));
    width: 54px;
    height: 54px;
    border: 0;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--text);
    z-index: 5;
  }

  .keys {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    min-height: 50px;
    padding: 6px max(12px, env(safe-area-inset-left));
    background: var(--ink-2);
    border-top: 1px solid #1e2832;
    display: flex;
    align-items: center;
    gap: 6px;
    overflow-x: auto;
    z-index: 6;
  }

  .key {
    flex: none;
    height: 38px;
    min-width: 44px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px #1e2832;
    color: var(--text);
    font-size: 13px;
  }

  .key[aria-pressed='true'] {
    background: rgb(154 140 255 / 0.18);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.6);
  }

  .key.close {
    margin-left: auto;
    display: grid;
    place-items: center;
  }

  .guide {
    position: absolute;
    right: max(16px, env(safe-area-inset-right));
    bottom: 90px;
    width: min(340px, calc(100% - 32px));
    border-radius: 16px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
    z-index: 6;
  }

  .guide strong {
    font-size: 15px;
    color: var(--text);
  }

  .guide button {
    align-self: flex-end;
    height: 44px;
    padding: 0 18px;
    border: 0;
    border-radius: 10px;
    background: rgb(154 140 255 / 0.18);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.55);
    color: #fff;
    font-size: 14px;
    font-weight: 600;
  }

  .diag {
    position: absolute;
    left: 12px;
    bottom: 12px;
    margin: 0;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgb(0 0 0 / 0.75);
    color: #c9f2da;
    font-size: 11px;
    line-height: 1.6;
    pointer-events: none;
    z-index: 9;
  }

  /* Champ caché qui fait apparaître le clavier de l'appareil. */
  .ime {
    position: fixed;
    left: 0;
    bottom: 0;
    width: 1px;
    height: 1px;
    opacity: 0;
    padding: 0;
    border: 0;
    resize: none;
    font-size: 16px;
  }

  /* Fenêtre étroite : la barre se resserre. */
  @media (max-width: 1180px) {
    .fmt {
      display: none;
    }
  }

  @media (max-width: 880px) {
    .host,
    .label {
      display: none;
    }

    .bar {
      padding: 0 8px;
    }

    .chip {
      width: 40px;
      padding: 0;
      justify-content: center;
    }

    .quit {
      width: 40px;
      padding: 0;
      justify-content: center;
    }

    .menu {
      right: 8px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bar,
    .tbar,
    .hint {
      animation: none;
      transition: none;
    }
  }
</style>
