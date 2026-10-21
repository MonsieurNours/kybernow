<!--
  Session dans le navigateur : la console prépare l'écran, le moteur de Kyber
  dessine l'image dans une toile, la barre de session reste à portée (en haut
  au survol, ou sous la languette en tactile).
-->
<script lang="ts">
  import { onMount, tick, untrack } from 'svelte'
  import { backend, endBody, heartbeatBody } from './backend'
  import { Heartbeat } from './heartbeat'
  import { ApiError } from '../lib/api'
  import { Engine, allStopped, isFixedEngine, retire, supportedCodecs, type DisplayInfo, type Latency } from './engine'
  import { choose, preferredProtocol, type Plan } from './plan'
  import { deviceId, type MouseMode, type Prefs, type TouchMode } from './prefs'
  import { deviceKind, deviceName, hasKeyboardLock, hasTouchScreen, isTouch, measureRefresh, screenMode, type Mode } from './device'
  import { installAbsoluteMouse, installPointerGate, isRemoteCursorHidden, releasePointer, removeAbsoluteMouse, removePointerGate, setPositionMapper, setRemoteCursorHidden } from './pointer'
  import { currentDisplayId, engineInputs, forgetInputSocket, injected, learnDisplayId, watchInputSocket } from './inputs'
  import { installInputGuard, removeInputGuard } from './guard'
  import { SPECIAL_KEYS, TEXT_ATTRIBUTE, TouchInput, TypedText, sendKey } from './touch'
  import { errorText } from '../lib/store.svelte'
  import Mark from '../lib/Mark.svelte'
  import { diag, diagEnabled, installDiag } from './diag'
  import { i18n, t, tn } from '../lib/i18n/index.svelte'

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
  /** Le navigateur attend un geste pour jouer le son. */
  let soundBlocked = $state(false)
  /** Information passagère (écran changé, image relancée). */
  let notice = $state('')
  let noticeTimer: ReturnType<typeof setTimeout> | null = null
  /** Manettes vues par le moteur (et transmises à l'ordinateur). */
  let gamepads = $state<{ index: number; name: string }[]>([])
  const gamepadText = $derived(tn('session.gamepads.count', gamepads.length))
  const gamepadAria = $derived(t('session.gamepads.aria', { count: gamepadText, names: gamepads.map((g) => g.name).join(', ') }))
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
  /** Kyber a démarré l'image, mais elle n'arrive pas : on essaie l'autre transport. */
  let imageWatch: ReturnType<typeof setTimeout> | null = null
  /** Le plan de contrôle ne se connecte pas : Kyber ne répond pas. */
  let connectWatch: ReturnType<typeof setTimeout> | null = null
  let connectStep = $state<'kyber' | 'image'>('kyber')
  /** D'où part le délai de l'image : la connexion (le démarrage de l'image
   * peut ne jamais venir) ou le démarrage lui-même. */
  let imageWatchFrom: 'connected' | 'streaming' | null = null
  /** Numéro de la tentative en cours. `start`, `fail` et `quit` le changent :
   * une tentative dépassée (fin, nouvelle tentative) s'arrête à son prochain
   * `await` au lieu de créer un moteur ou de préparer un écran pour rien. */
  let generation = 0
  const stale = (gen: number) => gen !== generation || ended
  /** Dernière liste des écrans de l'ordinateur. */
  let displays: DisplayInfo[] | null = null
  /** Relances de l'image après une erreur de capture ou d'encodage. */
  let pipelineRestarts = 0
  let stableTimer: ReturnType<typeof setTimeout> | null = null
  const MAX_PIPELINE_RESTARTS = 3
  let gameInForeground = false
  /** La console a préparé l'écran : il faudra le lui rendre. */
  let prepared = false
  /** Champs ajoutés aux corps du signal de vie et de la fin : cet appareil. */
  const extra = () => ({ device_id: deviceId() })
  /** La session de la console (le cookie et le jeton durent 12 h) a expiré. */
  const expired = () => t('session.error.expired')

  // Signal de vie depuis un Worker : il continue dans un onglet masqué.
  const heartbeat = new Heartbeat((r) => {
    if (ended || phase === 'ended') return
    if (r.status === 401) fail(expired())
    // Fin demandée depuis la console, ou un autre appareil prend la main.
    else if (r.end) fail(t('session.error.taken'))
  })

  /** Jeton Kyber relu sur la console (reconnexion, puis toutes les heures),
   * pour que le moteur ne se reconnecte pas avec un jeton périmé. */
  let ticketAt = 0
  async function refreshTicket(resume = false) {
    if (!resume && Date.now() - ticketAt < 30_000) return
    ticketAt = Date.now()
    try {
      const t = await backend.ticket()
      ticketToken = t.token
      engine?.updateCredentials(t.token, resume)
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) fail(expired())
    }
  }

  // Valeur envoyée à la console (« Jeu », « Bureau ») : ne se traduit pas.
  const profileLabel = $derived(prefs.profile === 'jeu' ? 'Jeu' : 'Bureau')
  const profileText = $derived(prefs.profile === 'jeu' ? t('session.profile.game') : t('session.profile.desktop'))
  /** « 8,3 » en français, « 8.3 » en anglais. */
  const decimal = (text: string) => (i18n.lang === 'fr' ? text.replace('.', ',') : text)
  const latencyText = $derived(latency ? `${decimal(latency.total.toFixed(latency.total < 10 ? 1 : 0))} ms` : '— ms')
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
          { label: t('session.metrics.computer'), sub: t('session.metrics.computerSub'), v: latency.server, cls: 'a' },
          { label: t('session.metrics.network'), sub: t('session.metrics.networkSub'), v: latency.network, cls: 'b' },
          { label: t('session.metrics.browser'), sub: t('session.metrics.browserSub'), v: latency.client, cls: 'c' },
        ]
      : [],
  )

  function ms(v: number): string {
    return `${decimal(v.toFixed(1))} ms`
  }

  /** Clavier de l'appareil ouvert : la zone visible (`visualViewport`, que le
   * clavier réduit sur iPhone et iPad sans toucher à la page) donne la place
   * de l'image et celle de la rangée de touches, juste au-dessus du clavier. */
  function trackViewport() {
    const vv = window.visualViewport
    const el = stage?.parentElement
    if (!el) return
    if (vv && keyboardOpen) {
      el.style.setProperty('--vv-top', `${vv.offsetTop}px`)
      el.style.setProperty('--vv-h', `${vv.height}px`)
    } else {
      el.style.removeProperty('--vv-top')
      el.style.removeProperty('--vv-h')
    }
    fit()
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
      // La couche tactile recouvre la toile : le pointeur d'une souris ou d'un
      // pavé (iPad avec clavier) y prend celui de l'ordinateur.
      if (touchLayer) touchLayer.style.cursor = canvas.style.cursor
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
    const gen = ++generation
    error = ''
    phase = 'preparing'
    try {
      // La session d'avant doit avoir rendu son écran et arrêté son moteur.
      await allStopped()
      if (stale(gen)) return
      const [refresh, codecs, ticket] = await Promise.all([measureRefresh(), supportedCodecs(), backend.ticket()])
      if (stale(gen)) return
      mode = screenMode(refresh)
      codecList = codecs
      // Moteur chargé : corrigé, il lit lui-même le champ du clavier de l'appareil.
      textInput?.toggleAttribute(TEXT_ATTRIBUTE, engineInputs())
      ticketToken = ticket.token
      const device = deviceName()
      await backend.prepare({
        mode,
        audio: prefs.audio,
        virtualDisplay: prefs.virtualDisplay,
        profile: profileLabel,
        device: { id: deviceId(), name: device.name, kind: deviceKind(), os: device.os, app: device.app },
      })
      if (stale(gen)) {
        // Finie pendant la préparation (Annuler, fin demandée) : l'écran est rendu.
        retire(null, () => backend.end(null, extra()))
        return
      }
      prepared = true
      await connect(preferredProtocol(), gen)
    } catch (e) {
      if (!stale(gen)) fail(e instanceof ApiError && e.status === 401 ? expired() : errorText(e))
    }
  }

  function clearWatches() {
    if (imageWatch) clearTimeout(imageWatch)
    if (connectWatch) clearTimeout(connectWatch)
    imageWatch = null
    connectWatch = null
    imageWatchFrom = null
  }

  async function connect(protocol: 'kymux' | 'websocket', gen = generation) {
    if (!mode || stale(gen)) return
    phase = 'connecting'
    connectStep = 'kyber'
    clearWatches()
    plan = choose(prefs, mode, codecList, protocol)
    // Jamais deux moteurs à la fois : chacun écoute souris et clavier sur la page.
    await allStopped()
    if (stale(gen)) return
    await tick()
    if (stale(gen)) return
    freshCanvas()
    const e: Engine = new Engine('kn-canvas', (ev) => onEngine(ev, e, protocol))
    engine = e
    connectWatch = setTimeout(() => noControlPlane(e), 25000)
    await e.connect(streamHost(), location.port || '443', ticketToken, plan)
  }

  /** Pas de plan de contrôle : Kyber ne répond pas (ce n'est pas une affaire de transport). */
  function noControlPlane(e: Engine) {
    connectWatch = null
    if (engine !== e || ended || phase !== 'connecting') return
    fail(t('session.error.noReply', { host: hostname }))
  }

  /** Kyber a démarré l'image (`streaming`) : elle doit arriver dans les 12 s
   * (20 s en WebSocket). Le délai part d'ici, pas de la connexion : préparer
   * l'écran virtuel prend à lui seul jusqu'à 8 s. Dès la connexion
   * (`connected`), un délai plus long couvre un démarrage qui ne vient pas. */
  function watchImage(e: Engine, protocol: 'kymux' | 'websocket', from: 'connected' | 'streaming') {
    if (engine !== e || phase !== 'connecting' || imageWatchFrom === 'streaming' || imageWatchFrom === from) return
    if (imageWatch) clearTimeout(imageWatch)
    imageWatchFrom = from
    const delay = from === 'connected' ? 25000 : protocol === 'kymux' ? 12000 : 20000
    imageWatch = setTimeout(() => noImage(e, protocol, from), delay)
  }

  /** Connecté, mais rien n'arrive : WebTransport bloqué (réseau, navigateur),
   * on passe par WebSocket ; en WebSocket, on le dit. */
  async function noImage(e: Engine, protocol: 'kymux' | 'websocket', from: 'connected' | 'streaming') {
    imageWatch = null
    imageWatchFrom = null
    const gen = generation
    if (engine !== e || phase !== 'connecting' || stale(gen)) return
    if (protocol === 'kymux') {
      console.warn('KyberNow : connecté, mais pas d’image en WebTransport ; essai par WebSocket')
      reconnecting = t('session.notice.otherPath')
      engine = null
      await retire(e)
      if (!stale(gen) && phase === 'connecting') {
        connect('websocket', gen).catch((err) => {
          if (!stale(gen)) fail(errorText(err))
        })
      }
    } else if (from === 'connected') {
      fail(t('session.error.noImageStart', { host: hostname }))
    } else {
      fail(t('session.error.noImageArrive', { host: hostname }))
    }
  }

  function showNotice(text: string) {
    notice = text
    if (noticeTimer) clearTimeout(noticeTimer)
    noticeTimer = setTimeout(() => (notice = ''), 5000)
  }

  /** Relance de l'image sans recharger la page ni se déconnecter. */
  function restartVideo(text: string) {
    if (!engine || phase !== 'live') return
    showNotice(text)
    engine.restartVideo()
  }

  /** Écran suivi disparu ou à une autre taille : on relance l'image (Kyber
   * reprend alors l'écran à jour, ou le premier de la liste). */
  function onDisplays(list: DisplayInfo[]) {
    const before = displays
    displays = list
    const id = currentDisplayId()
    if (phase !== 'live' || id === null) return
    const now = list.find((d) => d.id === id)
    const was = before?.find((d) => d.id === id)
    if (!now || (was && (was.width !== now.width || was.height !== now.height))) {
      console.warn('KyberNow : écran suivi', id, now ? `passé en ${now.width} × ${now.height}` : 'disparu')
      restartVideo(t('session.notice.displayChanged'))
    }
  }

  /** Erreur de la chaîne d'image de l'ordinateur : relance bornée. */
  function onPipeline(kind: string, stage: string, event: string) {
    console.warn('KyberNow : chaîne de l’ordinateur', kind, stage, event)
    if (event !== 'error' || phase !== 'live') return
    if (kind === 'audio') {
      showNotice(t('session.notice.soundStopped'))
      return
    }
    if (kind !== 'video') return
    if (stableTimer) clearTimeout(stableTimer)
    stableTimer = null
    if (pipelineRestarts >= MAX_PIPELINE_RESTARTS) {
      fail(t('session.error.pipeline', { host: hostname, max: MAX_PIPELINE_RESTARTS }))
      return
    }
    pipelineRestarts++
    const n = pipelineRestarts
    setTimeout(() => restartVideo(t('session.notice.imageRestart', { n, max: MAX_PIPELINE_RESTARTS })), 1000)
  }

  function connectError(message: string): string {
    if (message === 'unauthorized') return t('session.error.unauthorized', { host: hostname })
    if (message === 'streaming_start_failed' || message.startsWith('start_streaming')) return t('session.error.startFailed', { host: hostname })
    return t('session.error.noReply', { host: hostname })
  }

  function onEngine(e: import('./engine').EngineEvent, src: Engine, protocol: 'kymux' | 'websocket') {
    // Un moteur remplacé ne décide plus de rien.
    if (src !== engine) return
    switch (e.type) {
      case 'connected':
        if (connectWatch) clearTimeout(connectWatch)
        connectWatch = null
        connectStep = 'image'
        watchImage(src, protocol, 'connected')
        break
      case 'reconnecting':
        reconnecting = t('session.notice.reconnecting', { attempt: e.attempt, max: e.max })
        refreshTicket()
        break
      case 'auth-required':
        refreshTicket(true)
        break
      case 'reconnected':
        reconnecting = ''
        break
      case 'streaming':
        reconnecting = ''
        watchImage(src, protocol, 'streaming')
        break
      case 'resized':
        canvasSize = { width: e.width, height: e.height }
        fit()
        break
      case 'video-stopped':
        // Relance de l'image : l'ancienne toile appartient au moteur, il en
        // faut une neuve ; l'écran visé et la connexion des entrées changent.
        forgetInputSocket()
        freshCanvas()
        break
      case 'displays':
        onDisplays(e.displays)
        break
      case 'pipeline':
        onPipeline(e.kind, e.stage, e.event)
        break
      case 'gamepad':
        gamepads = e.plugged ? [...gamepads.filter((g) => g.index !== e.index), { index: e.index, name: e.name }] : gamepads.filter((g) => g.index !== e.index)
        break
      case 'first-image': {
        // Aussi après une relance de l'image : la session est déjà en cours.
        const first = phase !== 'live'
        clearWatches()
        phase = 'live'
        reconnecting = ''
        if (!heartbeat.running) heartbeat.start(heartbeatBody(null, profileLabel, extra()))
        learnDisplayId().then((list) => {
          if (list && !displays) displays = list.map((d) => ({ id: d.id, name: String(d.name ?? ''), width: Number(d.width) || 0, height: Number(d.height) || 0 }))
        })
        probeAudio(src)
        // Une image tenue 30 s remet le compte des relances à zéro.
        if (stableTimer) clearTimeout(stableTimer)
        stableTimer = setTimeout(() => (pipelineRestarts = 0), 30_000)
        if (first) {
          // Pas après une relance : le clavier de l'appareil se fermerait.
          document.getElementById('kn-canvas')?.focus()
          scheduleBarHide()
          setTimeout(() => (hintOn = false), 7000)
        }
        break
      }
      case 'error':
        console.warn('KyberNow : connexion impossible', e.message)
        fail(connectError(e.message))
        break
      case 'stopped':
        if (ended) return
        if (e.reason === 'restart') {
          // Le moteur a perdu sa toile en se reconnectant : on recommence proprement.
          restart()
        } else if (e.reason === 'decode_error') {
          fail(t('session.error.decode'))
        } else if (e.reason === 'controlplane_closed') {
          fail(t('session.error.closed', { host: hostname }))
        } else if (e.reason === 'requested') {
          // Arrêt demandé par ce client, sans que la page l'ait voulu.
          fail(t('session.error.stopped'))
        } else {
          fail(t('session.error.interrupted', { host: hostname }))
        }
        break
    }
  }

  async function restart() {
    const e = engine
    engine = null
    const gen = generation
    await retire(e)
    if (!stale(gen)) start()
  }

  function fail(message: string) {
    generation++
    clearWatches()
    gamepads = []
    soundBlocked = false
    error = message
    phase = 'ended'
    barVisible = true
    heartbeat.stop()
    // Le moteur s'arrête, puis l'écran préparé pour cette session est rendu.
    const e = engine
    engine = null
    const wasPrepared = prepared
    prepared = false
    retire(e, wasPrepared ? () => backend.end(null, extra()) : undefined)
  }

  async function quit() {
    if (ended) return
    ended = true
    generation++
    clearWatches()
    const median = engine?.sessionMedian() ?? null
    const e = engine
    engine = null
    // On rend la main tout de suite ; le moteur et la console finissent derrière.
    const wasPrepared = prepared
    prepared = false
    heartbeat.stop()
    releasePointer()
    if (document.fullscreenElement) document.exitFullscreen().catch(() => {})
    onEnd()
    await retire(e, wasPrepared ? () => backend.end(median, extra()) : undefined)
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

  const mouseLabel = $derived(mouseMode === 'auto' ? t('session.mouse.auto') : mouseMode === 'jeu' ? t('session.mouse.game') : t('session.mouse.desktop'))

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

  /** Sonde du son en cours (une seule à la fois). */
  let audioProbe = 0

  /** Après la première image : le son a-t-il démarré, ou le navigateur
   * attend-il un geste ? Le moteur crée son lecteur au premier paquet de son,
   * parfois quelques secondes après l'image : on sonde toutes les 200 ms
   * jusqu'à le savoir, 30 s au plus (un ordinateur sans son n'en envoie pas). */
  async function probeAudio(e: Engine) {
    if (!prefs.audio) return
    const id = ++audioProbe
    const t0 = performance.now()
    while (id === audioProbe && engine === e && phase === 'live' && performance.now() - t0 < 30_000) {
      const state = await e.audioState()
      if (id !== audioProbe || engine !== e) return
      if (state !== 'absent') {
        soundBlocked = state === 'suspended'
        return
      }
      await new Promise((r) => setTimeout(r, 200))
    }
  }

  /** Puce « Toucher pour le son » : la reprise se demande dans le geste. */
  async function unlockSound() {
    const e = engine
    if (!e) return
    await e.audioResume()
    soundBlocked = (await e.audioState()) === 'suspended'
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

  // Clavier de l'appareil : texte en Unicode (par le moteur s'il est corrigé), touches par leur code.
  const typed = new TypedText(
    () => textInput ?? null,
    canvas,
    () => mods,
    () => (mods = new Set()),
  )

  async function openKeyboard() {
    keyboardOpen = true
    touchBar = false
    await tick()
    textInput.focus()
    trackViewport()
  }

  function closeKeyboard() {
    keyboardOpen = false
    textInput?.blur()
    tick().then(trackViewport)
  }

  /** Clavier fermé par l'appareil (bouton du clavier, autre app) : la rangée
   * de touches part avec lui. Les touches de la rangée gardent le focus. */
  function onTextBlur() {
    if (keyboardOpen) closeKeyboard()
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
    // Avant le moteur : seuls les clics sur l'image partent à l'ordinateur, et
    // tout ce qui y est tenu enfoncé est relâché quand la page perd la main.
    installInputGuard({
      isVideo: (t) => t === canvas() || (!!touchLayer && t === touchLayer),
      canvas,
      onRelease: () => {
        if (mods.size) mods = new Set()
      },
    })
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
            `capture ${document.pointerLockElement ? 'oui' : 'non'} · pointeur de l'ordinateur ${isRemoteCursorHidden() ? 'masqué' : 'visible'} · transport ${plan?.protocol ?? '—'} · moteur ${isFixedEngine() ? 'corrigé' : 'd’origine'}${engineInputs() ? ', entrées par le moteur' : ''}`,
            `clavier tactile : frappe ${diag.lastKey} · saisie ${diag.lastInput}`,
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
    const viewport = () => keyboardOpen && trackViewport()
    window.visualViewport?.addEventListener('resize', viewport)
    window.visualViewport?.addEventListener('scroll', viewport)
    // Ctrl+W, ⌘W, ⌘Q en pleine session : le navigateur demande confirmation.
    const beforeUnload = (e: BeforeUnloadEvent) => {
      if (phase !== 'live' || ended) return
      e.preventDefault()
      e.returnValue = ''
    }
    window.addEventListener('beforeunload', beforeUnload)
    // Tout clic ou toute touche est un geste : le son peut y reprendre.
    const unlockAudio = async () => {
      if (engine && (await engine.audioState()) === 'running') soundBlocked = false
    }
    const leave = () => {
      if (ended || !prepared) return
      fetch('/api/stream/end', { method: 'POST', keepalive: true, credentials: 'same-origin', headers: { 'x-kyber-admin': '1', 'content-type': 'application/json' }, body: JSON.stringify(endBody(null, extra())) })
    }
    // Onglet qui passe en arrière-plan : un signal tout de suite.
    const hidden = () => {
      if (document.visibilityState === 'hidden' && heartbeat.running) heartbeat.now()
    }
    window.addEventListener('resize', resize)
    document.addEventListener('fullscreenchange', onFullscreenChange)
    window.addEventListener('pointerdown', unlockAudio)
    window.addEventListener('keydown', unlockAudio)
    window.addEventListener('pagehide', leave)
    document.addEventListener('visibilitychange', hidden)
    if (document.fullscreenElement) onFullscreenChange()

    // Mesures et signal de vie.
    const metrics = setInterval(() => {
      if (!engine || phase !== 'live') return
      latency = engine.latency.median()
      fps = engine.latency.fps()
      if (latency) spark = [...spark.slice(-31), latency.total]
    }, 500)
    // La latence du signal de vie (le Worker envoie la dernière connue).
    const sampler = setInterval(() => {
      if (!engine || phase !== 'live') return
      const l = engine.sample()
      heartbeat.update(heartbeatBody(l?.total ?? null, profileLabel, extra()))
    }, 5000)
    const ticketTimer = setInterval(() => phase === 'live' && refreshTicket(), 3_600_000)

    // Safari : pas de correction automatique (elle remplacerait un mot déjà parti).
    textInput.setAttribute('autocorrect', 'off')
    // Écran tactile, même avec une souris ou un pavé à côté (iPad avec
    // clavier) : les doigts passent par la couche tactile. Avec un pointeur
    // précis à côté, le doigt touche directement (le pointeur reste la souris).
    if (hasTouchScreen) {
      touch = new TouchInput(touchLayer, canvas, isTouch ? touchMode : 'direct', (p) => (cursor = isTouch && touchMode === 'trackpad' ? p : null))
      if (isTouch && touchMode === 'trackpad') cursor = touch.cursor
    }

    return () => {
      clearInterval(metrics)
      clearInterval(sampler)
      clearInterval(ticketTimer)
      heartbeat.stop()
      clearInterval(foreground)
      if (diagTimer) clearInterval(diagTimer)
      cursorWatch?.disconnect()
      document.removeEventListener('pointerlockchange', cursorFix)
      clearWatches()
      if (noticeTimer) clearTimeout(noticeTimer)
      if (stableTimer) clearTimeout(stableTimer)
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
      window.visualViewport?.removeEventListener('resize', viewport)
      window.visualViewport?.removeEventListener('scroll', viewport)
      window.removeEventListener('beforeunload', beforeUnload)
      document.removeEventListener('fullscreenchange', onFullscreenChange)
      window.removeEventListener('pointerdown', unlockAudio)
      window.removeEventListener('keydown', unlockAudio)
      window.removeEventListener('pagehide', leave)
      document.removeEventListener('visibilitychange', hidden)
      if (!ended) {
        ended = true
        retire(engine, prepared ? () => backend.end(null, extra()) : undefined)
      }
    }
  })
</script>

<div class="session" class:touch={isTouch} onpointermove={onPointerMove} role="application" aria-label={t('session.aria', { host: hostname })}>
  <div class="stage" class:kb={keyboardOpen} bind:this={stage} oncontextmenu={(e) => e.preventDefault()} role="presentation">
    <div class="canvas-box" bind:this={canvasBox}></div>
    <div class="touch-layer" class:active={hasTouchScreen && phase === 'live'} bind:this={touchLayer}></div>
    {#if cursor && phase === 'live'}
      <div class="tcursor" style="left: {cursor.x}px; top: {cursor.y}px" aria-hidden="true"></div>
    {/if}
  </div>

  {#if phase !== 'live'}
    <div class="overlay" role="status">
      {#if phase === 'ended'}
        <div class="card">
          <Mark size={34} state="now" />
          <h2 class="disp">{t('session.start.ended')}</h2>
          <p>{error}</p>
          <div class="row">
            <button type="button" class="primary" onclick={() => { ended = false; restart() }}>{t('session.start.reconnect')}</button>
            <button type="button" class="ghost" onclick={quit}>{t('session.start.home')}</button>
          </div>
        </div>
      {:else}
        <div class="card">
          <Mark size={34} state="now" lit breathe />
          <h2 class="disp">{t('session.start.entering', { host: hostname })}</h2>
          <ul class="steps mono">
            <li class:done={phase === 'connecting'}>{phase === 'preparing' ? t('session.start.preparing') : t('session.start.display', { width: mode?.width, height: mode?.height, refresh: mode?.refresh })}</li>
            {#if phase === 'connecting'}
              <li class:done={connectStep === 'image'}>{connectStep === 'image' ? t('session.start.connected') : reconnecting || t('session.start.connecting')}</li>
              {#if connectStep === 'image'}<li>{reconnecting || t('session.start.firstImages')}</li>{/if}
            {/if}
          </ul>
          <button type="button" class="ghost" onclick={quit}>{t('session.start.cancel')}</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if phase === 'live' && (reconnecting || notice)}
    <div class="toast glass" role="status">{reconnecting || notice}</div>
  {/if}

  {#if phase === 'live' && soundBlocked && prefs.audio && !muted}
    <button type="button" class="sound glass" onpointerdown={(e) => e.preventDefault()} onclick={unlockSound}>
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" /><path d="M17 9a4 4 0 0 1 0 6" /><path d="M19.5 6.5a8 8 0 0 1 0 11" /></svg>
      {isTouch ? t('session.sound.tap') : t('session.sound.click')}
    </button>
  {/if}

  <!-- Barre de session (souris et clavier) -->
  {#if !isTouch && phase === 'live'}
    {#if !barVisible}
      <button type="button" class="edge" aria-label={t('session.bar.show')} onclick={showBar}><span></span></button>
    {/if}
    <div class="bar glass" class:hidden={!barVisible} role="toolbar" tabindex="-1" aria-label="Session" onpointerenter={() => barTimer && clearTimeout(barTimer)} onpointerleave={scheduleBarHide}>
      <Mark size={22} state="now" lit breathe />
      <span class="host">{hostname}</span>
      <span class="sep" aria-hidden="true"></span>
      <button type="button" class="lat" aria-expanded={metricsOpen} aria-label={t('session.bar.metrics')} onclick={() => { metricsOpen = !metricsOpen; keysOpen = false }}>
        <span class="mono value">{latencyText}</span>
        <svg width="64" height="22" viewBox="0 0 64 22" aria-hidden="true"><path d={sparkPath} fill="none" stroke="#9A8CFF" stroke-width="1.5" stroke-linejoin="round" /></svg>
        <span class="mono fmt">{formatText}</span>
      </button>
      <span class="sep" aria-hidden="true"></span>
      <span class="chip static">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9A8CFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></svg>
        <span class="label">{profileText}</span>
      </span>
      <button type="button" class="chip" aria-label={mouseLabel} title={t('session.mouse.hint')} onclick={cycleMouse}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round" aria-hidden="true"><path d="M5 3l14 7-6 2-2 6z" /></svg>
        <span class="label">{mouseLabel}</span>
      </button>
      {#if gamepads.length}
        <span class="chip static" title={gamepads.map((g) => g.name).join(', ')} aria-label={gamepadAria}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9A8CFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 8h10a4 4 0 0 1 4 4v1a3 3 0 0 1-5.4 1.8L14.5 13h-5l-1.1 1.8A3 3 0 0 1 3 13v-1a4 4 0 0 1 4-4z" /><path d="M8 10.5v3M6.5 12h3" /></svg>
          <span class="label">{gamepadText}</span>
        </span>
      {/if}
      {#if prefs.audio}
        <button type="button" class="chip" aria-pressed={muted} aria-label={muted ? t('session.sound.unmute') : t('session.sound.mute')} onclick={toggleMute}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" />{#if muted}<path d="M17 9l4 6M21 9l-4 6" />{:else}<path d="M17 9a4 4 0 0 1 0 6" />{/if}</svg>
          <span class="label">{muted ? t('session.sound.muted') : t('session.sound.on')}</span>
        </button>
      {/if}
      <button type="button" class="chip" aria-label={t('session.bar.keys')} aria-expanded={keysOpen} onclick={() => { keysOpen = !keysOpen; metricsOpen = false }}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
        <span class="label">{t('session.bar.keys')}</span>
      </button>
      <span class="sep" aria-hidden="true"></span>
      <button type="button" class="chip icon" aria-label={fullscreen ? t('session.bar.exitFullscreen') : t('session.bar.fullscreen')} onclick={toggleFullscreen}>
        {#if fullscreen}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" /></svg>
        {:else}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></svg>
        {/if}
      </button>
      <button type="button" class="quit" aria-label={t('session.bar.quit')} onclick={quit}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 3v9" /><path d="M6.3 6.3a8 8 0 1 0 11.4 0" /></svg>
        <span class="label">{t('session.bar.quit')}</span>
      </button>

      {#if keysOpen}
        <div class="menu glass" role="menu" aria-label={t('session.keys.title')}>
          <button type="button" role="menuitem" onclick={() => { engine?.ctrlAltDel(); keysOpen = false }}>{t('session.keys.cad')}<span class="mono">{t('session.keys.cadSub')}</span></button>
          <button type="button" role="menuitem" onclick={() => { sendKey(canvas(), 'Meta', 'MetaLeft', new Set()); keysOpen = false }}>{t('session.keys.win')}<span class="mono">{t('session.keys.winSub')}</span></button>
          <button type="button" role="menuitem" onclick={() => { sendCombo([{ key: 'Alt', code: 'AltLeft' }], { key: 'Tab', code: 'Tab' }); keysOpen = false }}>{t('session.keys.altTab')}<span class="mono">{t('session.keys.altTabSub')}</span></button>
          <button type="button" role="menuitem" onclick={() => { sendKey(canvas(), 'PrintScreen', 'PrintScreen', new Set()); keysOpen = false }}>{t('session.keys.print')}<span class="mono">{t('session.keys.printSub')}</span></button>
        </div>
      {/if}
    </div>

    {#if metricsOpen}
      <section class="metrics glass" aria-label={t('session.bar.metrics')}>
        <div class="mhead">
          <span>{t('session.metrics.title')}</span>
          <span class="mono muted">{t('session.metrics.median')}</span>
        </div>
        {#if latency}
          <div class="bars">
            {#each parts as p (p.cls)}<div class={p.cls} style="flex: {Math.max(p.v, 0.3)}">{p.label}</div>{/each}
          </div>
          <div class="nums">
            {#each parts as p (p.cls)}<div><span class="lit-number">{ms(p.v)}</span><span class="muted">{p.sub}</span></div>{/each}
          </div>
        {:else}
          <p class="muted">{t('session.metrics.waiting')}</p>
        {/if}
        <div class="mono foot">
          <span>{t('session.metrics.fps', { fps })}</span><span>{plan?.mbps ?? '—'} Mbit/s</span><span>{plan?.reliable ? t('session.metrics.reliable') : t('session.metrics.fast')}</span>
        </div>
        <p class="muted small">{t('session.metrics.browserNote')}</p>
      </section>
    {/if}

    {#if hintOn}
      <div class="hint glass" role="status">
        {#if keyboardLocked}
          {t('session.hint.locked', { host: hostname })} <kbd class="mono">Ctrl</kbd> + <kbd class="mono">Alt</kbd> + <kbd class="mono">{t('session.hint.shift')}</kbd>
        {:else}
          {t('session.hint.bar')} <kbd class="mono">Ctrl</kbd> + <kbd class="mono">Alt</kbd> + <kbd class="mono">{t('session.hint.shift')}</kbd>{t('session.hint.shortcuts')}
        {/if}
      </div>
    {/if}
  {/if}

  <!-- Tactile : languette, barre compacte, clavier -->
  {#if isTouch && phase === 'live'}
    {#if !touchBar}
      <button type="button" class="tab glass" aria-label={t('session.bar.show')} onpointerdown={(e) => e.preventDefault()} onclick={() => { touchBar = true; guide = false }}>
        <span class="mono">{latencyText}</span>
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6" /></svg>
      </button>
    {:else}
      <div class="tbar glass" role="toolbar" tabindex="-1" aria-label="Session" onpointerdown={(e) => e.preventDefault()}>
        <Mark size={20} state="now" lit />
        <span class="mono tlat">{latencyText}</span>
        {#if gamepads.length}
          <span class="tpad" title={gamepads.map((g) => g.name).join(', ')} aria-label={gamepadAria}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#9A8CFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 8h10a4 4 0 0 1 4 4v1a3 3 0 0 1-5.4 1.8L14.5 13h-5l-1.1 1.8A3 3 0 0 1 3 13v-1a4 4 0 0 1 4-4z" /><path d="M8 10.5v3M6.5 12h3" /></svg>
            {#if gamepads.length > 1}<span class="mono">{gamepads.length}</span>{/if}
          </span>
        {/if}
        <span class="sep" aria-hidden="true"></span>
        <div class="tmodes" role="radiogroup" aria-label={t('session.touch.modes')}>
          <button type="button" role="radio" aria-checked={touchMode === 'trackpad'} onclick={() => setTouchMode('trackpad')}>{t('session.touch.trackpad')}</button>
          <button type="button" role="radio" aria-checked={touchMode === 'direct'} onclick={() => setTouchMode('direct')}>{t('session.touch.direct')}</button>
        </div>
        <button type="button" class="tb" onclick={openKeyboard} aria-label={t('session.touch.keyboard')}>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
        </button>
        {#if prefs.audio}
          <button type="button" class="tb" aria-pressed={muted} onclick={toggleMute} aria-label={muted ? t('session.sound.unmute') : t('session.sound.mute')}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" />{#if muted}<path d="M17 9l4 6M21 9l-4 6" />{:else}<path d="M17 9a4 4 0 0 1 0 6" />{/if}</svg>
          </button>
        {/if}
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="tb" onclick={() => (touchBar = false)} aria-label={t('session.touch.hideBar')}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 15l6-6 6 6" /></svg>
        </button>
        <button type="button" class="tb danger-tb" onclick={quit} aria-label={t('session.bar.quit')}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 3v9" /><path d="M6.3 6.3a8 8 0 1 0 11.4 0" /></svg>
        </button>
      </div>
    {/if}

    {#if !keyboardOpen}
      <button type="button" class="fab glass" onpointerdown={(e) => e.preventDefault()} onclick={openKeyboard} aria-label={t('session.touch.openKeyboard')}>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
      </button>
    {:else}
      <div class="keys" role="toolbar" aria-label={t('session.touch.specialKeys')}>
        {#each SPECIAL_KEYS as k (k.code)}
          <button type="button" class="key" aria-pressed={k.sticky ? mods.has(k.key) : undefined} aria-label={k.aria} onpointerdown={(e) => e.preventDefault()} onclick={() => pressSpecial(k)}>{k.label}</button>
        {/each}
        <button type="button" class="key" onpointerdown={(e) => e.preventDefault()} onclick={() => engine?.ctrlAltDel()}>{t('session.touch.cad')}</button>
        <button type="button" class="key close" aria-label={t('session.touch.closeKeyboard')} onclick={closeKeyboard}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6" /></svg>
        </button>
      </div>
    {/if}

    {#if guide}
      <section class="guide glass" aria-label={t('session.touch.gestures')}>
        <strong>{touchMode === 'trackpad' ? t('session.touch.trackpad') : t('session.touch.direct')}</strong>
        {#if touchMode === 'trackpad'}
          <p>{t('session.touch.guideTrackpad')}</p>
        {:else}
          <p>{t('session.touch.guideDirect')}</p>
        {/if}
        <button type="button" onclick={closeGuide}>{t('session.touch.gotIt')}</button>
      </section>
    {/if}
  {/if}

  {#if diagEnabled && diagText}
    <pre class="diag mono" aria-hidden="true">{diagText}</pre>
  {/if}

  <textarea
    class="ime"
    bind:this={textInput}
    onkeydown={typed.keydown}
    onbeforeinput={typed.beforeinput}
    oncompositionstart={typed.compositionstart}
    oncompositionend={typed.compositionend}
    oninput={typed.input}
    onblur={onTextBlur}
    autocapitalize="off"
    autocomplete="off"
    spellcheck="false"
    aria-label={t('session.textInput')}
  ></textarea>
</div>

<style>
  .session {
    /* Hauteur de la rangée de touches au-dessus du clavier de l'appareil. */
    --keys-h: 52px;
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

  /* Clavier ouvert : l'image au-dessus de la rangée de touches, elle-même
     juste au-dessus du clavier (variables posées par trackViewport). */
  .stage.kb {
    top: var(--vv-top, 0px);
    bottom: auto;
    height: calc(var(--vv-h, 100%) - var(--keys-h));
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

  /* Son en attente d'un geste : au-dessus de tout, en bas au centre. */
  .sound {
    position: absolute;
    left: 50%;
    bottom: max(24px, env(safe-area-inset-bottom));
    translate: -50% 0;
    height: 46px;
    padding: 0 18px;
    border: 0;
    border-radius: 23px;
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
    z-index: 7;
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

  .tpad {
    display: flex;
    align-items: center;
    gap: 3px;
    margin-right: 6px;
    font-size: 11px;
    color: var(--signal-light);
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
    top: calc(var(--vv-top, 0px) + var(--vv-h, 100%) - var(--keys-h));
    height: var(--keys-h);
    box-sizing: border-box;
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
