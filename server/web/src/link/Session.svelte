<!--
  Session dans le navigateur : la console prépare l'écran, le moteur de Kyber
  dessine l'image dans une toile, la barre de session reste à portée (en haut
  au survol, ou sous la languette en tactile). L'écran de connexion montre
  ses trois étapes, puis l'image seule : rien ne s'affiche par-dessus quand
  elle arrive (docs/link-ui.md).
-->
<script lang="ts">
  import { onMount, tick, untrack } from 'svelte'
  import { backend, endBody, heartbeatBody, type GuestBeat, type Info } from './backend'
  import { Heartbeat } from './heartbeat'
  import { guestTarget, sendHeartbeat, type HeartbeatResult } from './heartbeat-call'
  import { ApiError, type GuestImage, type ShareQuality, type ShareRole, type ShareView } from '../lib/api'
  import { Engine, allStopped, isFixedEngine, retire, supportedCodecs, type DisplayInfo, type Latency } from './engine'
  import { choose, preferredProtocol, type Plan } from './plan'
  import { deviceId, isCustom, MIN_SESSION_SECONDS, profileName, saveLastSession, type MouseMode, type Prefs, type TouchMode } from './prefs'
  import { deviceKind, deviceName, hasKeyboardLock, hasTouchScreen, isTouch, measureRefresh, screenMode, type Mode } from './device'
  import { installAbsoluteMouse, installPointerGate, isRemoteCursorHidden, releasePointer, removeAbsoluteMouse, removePointerGate, setPositionMapper, setRemoteCursorHidden } from './pointer'
  import { allowPackets, currentDisplayId, engineInputs, forgetInputSocket, injected, learnDisplayId, watchInputSocket } from './inputs'
  import { installInputGuard, releaseAll, removeInputGuard } from './guard'
  import { SPECIAL_KEYS, TEXT_ATTRIBUTE, TouchInput, TypedText, sendKey } from './touch'
  import { errorText } from '../lib/store.svelte'
  import Mark from '../lib/Mark.svelte'
  import Icon from '../lib/Icon.svelte'
  import { diag, diagEnabled, installDiag } from './diag'
  import { i18n, t, tn } from '../lib/i18n/index.svelte'
  import { tName } from './names'
  import { lastResortAllowed, restartForConnection } from './station.svelte'
  import { autoCaps, formatLabel, guestFormat, leaveShare, QUALITIES, qualityName, redact, saveQuality, tokenPreset, type GuestStart } from './share'
  import { imageQuality } from '../lib/format'
  import { gamepadsChanged, installGamepadGate, removeGamepadGate } from './gamepads'
  import ShareBox from './ShareBox.svelte'

  /** `guest` : la session d'un invité du partage (KyberNow Share, docs/kybernow-share.md) : pas de
   * préparation de l'écran ni de jeton d'Admin, une barre réduite, les droits de son rôle. */
  let { hostname, prefs = $bindable(), guest = null, onEnd }: { hostname: string; prefs: Prefs; guest?: GuestStart | null; onEnd: () => void } = $props()
  /** La session d'un invité, fixée pour toute la durée de la page. */
  const g = untrack(() => guest)
  /** Titre de la session : le nom de l'ordinateur, ou « Invité sur {nom} ». */
  const title = $derived(g ? t('share.guest.on', { name: hostname }) : hostname)

  /** `restarting` : dernier recours, la Station redémarre (station.svelte.ts). */
  type Phase = 'preparing' | 'connecting' | 'restarting' | 'live' | 'ended'
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
  let barVisible = $state(false)
  let metricsOpen = $state(false)
  let keysOpen = $state(false)
  let touchMode = $state<TouchMode>(untrack(() => prefs.touch))
  let touchBar = $state(false)
  let keyboardOpen = $state(false)
  /** Fiche des gestes : à la première ouverture de la barre tactile, jamais à l'arrivée de l'image. */
  let guide = $state(false)
  let cursor = $state<{ x: number; y: number } | null>(null)
  let mods = $state(new Set<string>())
  // Un invité n'a pas le mode « auto » (il ne sait pas si un jeu est au premier plan) et ne change pas les réglages.
  let mouseMode = $state<MouseMode>(untrack(() => (g && prefs.mouse === 'auto' ? 'bureau' : prefs.mouse)))
  /** Le navigateur attend un geste pour jouer le son. */
  let soundBlocked = $state(false)
  /** Information passagère (écran changé, image relancée). */
  let notice = $state('')
  let noticeTimer: ReturnType<typeof setTimeout> | null = null
  /** Manettes vues par le moteur (et transmises à l'ordinateur). */
  let gamepads = $state<{ index: number; name: string }[]>([])
  const gamepadText = $derived(tn('session.gamepads.count', gamepads.length))
  const gamepadAria = $derived(t('session.gamepads.aria', { count: gamepadText, names: gamepads.map((p) => p.name).join(', ') }))
  /** Droits d'un invité : son rôle, changé en cours de partage par le signe de vie. L'hôte a tout. */
  let role = $state<ShareRole>(g?.join.role ?? 'player_kbm')
  // Deux droits indépendants : les manettes (`player`), le clavier et la souris (`kbm`), ou les deux.
  const canPlay = $derived(!g || role === 'player' || role === 'player_kbm')
  const canKbm = $derived(!g || role === 'kbm' || role === 'player_kbm')
  const roleText = $derived(role === 'spectator' ? t('share.guest.spectator') : t('share.guest.player'))
  /** Le partage de cet appareil (signe de vie de l'hôte), le panneau « Partager », et ce que la Station en dit. */
  let share = $state<ShareView | null>(null)
  let shareOpen = $state(false)
  let shareSupport = $state<'ok' | 'old' | 'off'>('ok')
  const shareHint = $derived(shareSupport === 'old' ? t('share.bar.tooOld') : shareSupport === 'off' ? t('share.off') : '')
  /** Invités du partage ouvert : la pastille « 2 invités » (rien quand il n'y en a pas encore). */
  const guestCount = $derived(share?.guests.length ?? 0)
  /** Le dernier appui sur « Partager » est un doigt ou un stylet (pas une souris ni le clavier). */
  let shareTouch = false
  let diagText = $state('')
  /** « Qualité de l'image » d'un invité (docs/link-ui.md, « Partage ») : celle qu'il demande (retenue
   * quand il la choisit), et ce que la Station dit de son image (`null` avant sa première réponse). */
  let quality = $state<ShareQuality>(g?.quality ?? 'auto')
  let image = $state<GuestImage | null>(null)
  /** La Station connaît la qualité d'un invité (son signe de vie donne `image`) : le choix est dans la barre. */
  let qualityKnown = $state(false)
  let qualityOpen = $state(false)
  let qualityBusy = $state(false)
  /** Le menu de la qualité s'ouvre sous sa puce (barre de la souris). */
  let qualityChip = $state<HTMLElement | null>(null)
  let qualityLeft = $state(0)
  /** La barre montre la qualité reçue (« 1080p · 60 Hz »), celle demandée tant que Kyber ne la dit pas. */
  const receivedText = $derived(imageQuality(image) || qualityName(quality))
  /** Qualité changée en cours de session : les étapes (« Premières images ») jusqu'à la nouvelle image. */
  let switching = $state(false)
  let switchTarget: { width: number; height: number } | null = null
  let switchTimer: ReturnType<typeof setTimeout> | null = null
  /** Taille de l'image décodée (`resized`) ; `null` après une relance de l'image. */
  let decoded: { width: number; height: number } | null = null
  /** Dernier avis d'image ajustée (raison et taille) : un avis par changement, pas à chaque signe de vie. */
  let imageNoticeKey = ''
  /** Avis à montrer à la prochaine image (après une relance). */
  let pendingNotice = ''
  /** Dernier recours après une erreur de décodage : l'invité est déjà repassé en 720p30. */
  let fallbackTried = false

  let stage: HTMLElement
  let canvasBox: HTMLElement
  let touchLayer: HTMLElement
  let textInput: HTMLTextAreaElement
  let canvasSize = { width: 0, height: 0 }
  let engine: Engine | null = null
  let touch: TouchInput | null = null
  let barTimer: ReturnType<typeof setTimeout> | null = null
  /** Largeur de la barre : celle de la zone du bord haut (sa largeur naturelle tant qu'elle est cachée). */
  let barWidth = $state(0)
  let barEl = $state<HTMLElement | null>(null)
  let sessionEl: HTMLElement
  /** Le pointeur s'est posé sur la barre (ou contre le bord qui l'ouvre) depuis qu'elle est ouverte. */
  let barSeen = false
  /** Minuteur du pointeur posé contre le bord haut : la barre s'ouvre à son échéance. */
  let dwellTimer: ReturnType<typeof setTimeout> | null = null
  /** Dernière position de la souris sur la session (null : elle n'y est plus). */
  let hoverPos: { x: number; y: number } | null = null
  /** La page a le focus (le trait du bord haut n'apparaît que si le geste est possible). */
  let pageFocused = $state(document.hasFocus())
  /** Le pointeur est capturé (souris des jeux). */
  let pointerLocked = $state(!!document.pointerLockElement)
  /** Appareil tactile sans souris : le trait reste, la page n'a que les doigts. */
  const mouseless = hasTouchScreen && typeof matchMedia === 'function' && !matchMedia('(any-pointer: fine)').matches
  const edgeShown = $derived(!barVisible && (mouseless || (pageFocused && !pointerLocked)))
  /** Un bouton de la souris est tenu (dernier état connu) : une fenêtre glissée contre le bord haut n'ouvre pas la barre. */
  let buttonHeld = false
  /** Le pointeur doit d'abord sortir de la zone du bord haut avant de rouvrir la barre. */
  let edgeReady = true
  /** Le dernier appui sur la languette est un doigt. */
  let edgeTouch = false
  let ended = false
  /** Jeton de Kyber : celui d'Admin (`/api/web/ticket`), ou celui de l'invité, renouvelé par son signe de vie. */
  let ticketToken = g?.join.kyber.token ?? ''
  /** Écran du partage que reçoit l'invité (il change si l'écran de l'hôte est recréé). */
  let guestDisplay = g?.join.display
  /** L'invité est déjà sorti du partage (`share/leave` envoyé). */
  let left = false
  let codecList: import('./engine').CodecSupport[] = []
  /** Kyber a démarré l'image, mais elle n'arrive pas : on essaie l'autre transport. */
  let imageWatch: ReturnType<typeof setTimeout> | null = null
  /** Le plan de contrôle ne se connecte pas : Kyber ne répond pas. */
  let connectWatch: ReturnType<typeof setTimeout> | null = null
  let connectStep = $state<'kyber' | 'image'>('kyber')
  /** L'image est déjà arrivée dans cette session (l'échec n'est plus « de connexion »). */
  let wasLive = $state(false)
  /** Médiane de chaque seconde d'image, et l'heure de la première image :
   * la dernière session (Statistiques) et la médiane envoyée à la fin. */
  let secondMedians: number[] = []
  let firstImageAt = 0
  let lastSaved = false
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

  // Signal de vie depuis un Worker : il continue dans un onglet masqué. Celui
  // d'un invité du partage va à `/api/share/beat`, avec son secret.
  const heartbeat = new Heartbeat(
    (r) => {
      if (ended || phase === 'ended') return
      if (g) {
        onGuestBeat(r)
        return
      }
      if (r.status === 401) fail(expired())
      // Fin demandée depuis la console, ou un autre appareil prend la main.
      else if (r.end) fail(t('session.error.taken'))
      // Le partage de cet appareil, absent sans partage.
      else if (r.status === 200) share = r.data?.share?.open ? (r.data.share as ShareView) : null
    },
    g ? g.join.beat * 1000 : 5000,
    g ? guestTarget(g.join.secret) : undefined,
  )

  /** Signe de vie d'un invité : son rôle, le jeton renouvelé, l'écran du partage, ou la fin. */
  function onGuestBeat(r: HeartbeatResult) {
    // Parti (sorti sans nouvelles, partage rouvert) : comme un partage arrêté.
    if (r.status === 404) return endedByHost('stopped')
    // Station injoignable, ou passagèrement sans clé (503) : le signe suivant le dira.
    if (r.status !== 200 || !r.data) return
    const beat = r.data as GuestBeat
    if (beat.end) return endedByHost(beat.end)
    if (beat.role && beat.role !== role) changeRole(beat.role)
    if (beat.token) ticketToken = beat.token
    if (beat.image) {
      qualityKnown = true
      onImage(beat.image)
    }
    const display = beat.display
    const moved = !!display && !!guestDisplay && display.id !== guestDisplay.id
    if (display && guestDisplay) {
      // Le format annoncé suit l'écran de l'hôte (taille et fréquence : `display_list_updated` relance l'image).
      if (display.width !== guestDisplay.width || display.height !== guestDisplay.height || display.refresh !== guestDisplay.refresh) {
        mode = { width: display.width, height: display.height, refresh: display.refresh }
      }
      guestDisplay = display
    }
    if (moved) {
      // L'hôte s'est préparé de nouveau : son écran a un autre identifiant, que Kyber ne prend
      // qu'à une nouvelle connexion, avec le jeton renouvelé. L'écran que visent les positions
      // et la liste de Kyber sont à réapprendre (sinon les positions iraient à l'ancien écran).
      if (phase === 'live' || phase === 'connecting') {
        forgetInputSocket()
        displays = null
        restart()
      }
    } else if (beat.token) {
      engine?.updateCredentials(beat.token)
    }
  }

  /** La Station dit pourquoi l'invité est sorti : l'hôte a arrêté le partage ou l'a retiré. */
  function endedByHost(end: 'stopped' | 'removed') {
    fail(t(end === 'removed' ? 'share.guest.removed' : 'share.guest.stopped', { host: g?.join.host ?? '' }))
  }

  /** Un signe de vie tout de suite, sans attendre le Worker : le jeton renouvelé et l'image. */
  async function beatNow(): Promise<HeartbeatResult | null> {
    if (!g) return null
    const r = await sendHeartbeat(location.origin, { guest: g.join.guest }, guestTarget(g.join.secret))
    if (!ended && phase !== 'ended') onGuestBeat(r)
    return r
  }

  /**
   * Ce que la Station dit de l'image de l'invité. Quand elle l'ajuste (docs/link-ui.md, « Image ajustée par
   * la Station »), un avis bref, une fois par changement : en retard sur l'image de l'hôte, « Image
   * allégée pour suivre. » ; pas d'image de plus possible, « L'ordinateur ne peut pas faire une image de
   * plus : vous recevez 720p · 30 Hz. ».
   */
  function onImage(next: GuestImage) {
    image = next
    const adjusted = next.reason === 'lag' || next.reason === 'limit'
    const key = adjusted ? `${next.reason} ${next.width}x${next.height}@${next.fps}` : ''
    if (key === imageNoticeKey) return
    imageNoticeKey = key
    if (!adjusted) return
    const text = next.reason === 'lag' ? t('share.guest.lightened') : t('share.guest.noMoreImage', { quality: imageQuality(next) || qualityName(next.quality) })
    // Déjà dit à l'instant (l'image décodée a rapetissé) : pas deux fois.
    if (notice === text) return
    if (phase === 'live' && !switching) showNotice(text)
    else pendingNotice = text
  }

  /**
   * L'image décodée a changé de taille. Pendant un changement de qualité, c'est la nouvelle image qui
   * arrive. Sinon, une image plus petite sans que l'invité ait rien changé, et qui n'est pas l'écran de
   * l'hôte (l'hôte a pu changer de définition) : la Station l'a allégée, l'avis tout de suite (« Image
   * allégée pour suivre. »), la raison exacte au signe de vie suivant, demandé aussitôt.
   */
  function onDecoded(size: { width: number; height: number }) {
    const before = decoded
    decoded = size
    if (!g) return
    if (switching) {
      if (!switchTarget || near(size, switchTarget)) endSwitch()
      return
    }
    const hostScreen = (!!guestDisplay && near(size, guestDisplay)) || !!displays?.some((d) => near(size, d))
    if (before && phase === 'live' && size.width * size.height < before.width * before.height && !hostScreen) {
      showNotice(t('share.guest.lightened'))
      void beatNow()
    }
  }

  const near = (a: { width: number; height: number }, b: { width: number; height: number }) => Math.abs(a.width - b.width) <= 8 && Math.abs(a.height - b.height) <= 8

  /** Les étapes jusqu'à la nouvelle image : elle arrive dans le flux (Kyber déplace l'image de l'invité :
   * en-têtes puis image clé). Pas de nouvelle taille en 4 s : l'image est relancée proprement. */
  function startSwitch(target: { width: number; height: number } | null) {
    endSwitch()
    switching = true
    switchTarget = target
    const e = engine
    switchTimer = setTimeout(() => {
      if (!switching || engine !== e || !e) return
      console.warn('KyberNow : nouvelle image pas arrivée dans le flux ; image relancée')
      e.restartVideo()
      // La relance a son propre délai (5 s, puis la session entière) : les étapes ne restent pas au-delà.
      switchTimer = setTimeout(endSwitch, 10_000)
    }, 4000)
  }

  function endSwitch() {
    if (switchTimer) clearTimeout(switchTimer)
    switchTimer = null
    switching = false
    switchTarget = null
  }

  /** Le format que l'invité demande à Kyber, d'après le préréglage de son jeton (`kn_quality`). */
  function guestFormatNow(): { codec: 'h264' | 'hevc'; yuv444: boolean } | null {
    if (!g) return null
    const preset = tokenPreset(ticketToken)
    return guestFormat(g.join.codec, g.join.yuv444, codecList, preset === null || preset === 'host')
  }

  function toggleQuality() {
    qualityOpen = !qualityOpen
    if (!qualityOpen) return
    metricsOpen = false
    keysOpen = false
    // Sous la puce, dans la barre (positionnée) qui la contient.
    if (qualityChip) qualityLeft = qualityChip.offsetLeft
  }

  /**
   * « Qualité de l'image » changée dans la barre : retenue pour la fois suivante, envoyée à la Station
   * (`POST /api/share/quality`), qui fait relancer à Kyber l'image de l'invité seulement. Le jeton
   * renouvelé porte le nouveau préréglage. Préréglage inchangé (« Auto » qui donnait déjà 1080p60) : rien
   * ne bouge. Format à changer (4:4:4 de l'image de l'hôte ou 4:2:0 d'une image à part) : l'image est
   * relancée proprement ; sinon la nouvelle image arrive dans le flux.
   */
  async function pickQuality(q: ShareQuality) {
    qualityOpen = false
    if (!g || q === quality || qualityBusy || phase !== 'live') return
    const before = quality
    const presetBefore = tokenPreset(ticketToken)
    const gen = generation
    quality = q
    qualityBusy = true
    try {
      const auto = q === 'auto' ? await autoCaps() : undefined
      const next = await backend.shareQuality(g.join, q, auto)
      saveQuality(q)
      if (stale(gen)) return
      onImage(next)
      await beatNow()
      if (stale(gen) || phase !== 'live' || !engine || !plan) return
      const preset = tokenPreset(ticketToken)
      if (preset && preset === presetBefore) return
      const format = guestFormatNow()
      if (format && (format.codec !== plan.codec || format.yuv444 !== plan.yuv444)) {
        plan = { ...plan, ...format }
        engine.updateSettings(plan)
        startSwitch(null)
        engine.restartVideo()
        return
      }
      const target = next.width && next.height ? { width: next.width, height: next.height } : null
      // Même taille (la fréquence seule change) : rien à attendre à l'écran.
      if (target && decoded && near(target, decoded)) return
      startSwitch(target)
    } catch (e) {
      console.warn('KyberNow : qualité non changée', redact(e instanceof Error ? e.message : e, [g.join.secret, ticketToken]))
      if (stale(gen)) return
      quality = before
      showNotice(t('share.quality.failed'))
    } finally {
      qualityBusy = false
    }
  }

  /**
   * Dernier recours d'un invité dont ce navigateur ne décode pas l'image (le moteur a redemandé une image
   * clé, en vain) : il rejoint de nouveau son image en 720p30 (sa qualité retenue ne change pas), avec
   * l'avis « Image allégée pour suivre. ». Déjà en 720p30, ou une seconde fois : l'échec est dit.
   */
  async function guestFallback() {
    if (!g) return
    fallbackTried = true
    const gen = generation
    const e = engine
    engine = null
    clearWatches()
    endSwitch()
    phase = 'connecting'
    connectStep = 'image'
    await retire(e)
    if (stale(gen)) return
    try {
      onImage(await backend.shareQuality(g.join, '720p30'))
      // Le jeton renouvelé porte 720p30 (`kn_quality`) : Kyber le prend à la nouvelle connexion.
      await beatNow()
    } catch {
      if (!stale(gen)) fail(t('share.guest.decode', { host: hostname }))
      return
    }
    if (stale(gen)) return
    quality = '720p30'
    pendingNotice = t('share.guest.lightened')
    start()
  }

  /**
   * Droits changés par l'hôte : un avis bref par droit donné ou repris (plus aucun droit : « Vous regardez
   * de nouveau. » à la place de l'avis de reprise), et ce que le nouveau rôle ne permet plus est relâché.
   */
  function changeRole(next: ShareRole) {
    const pads = (r: ShareRole) => r === 'player' || r === 'player_kbm'
    const kbm = (r: ShareRole) => r === 'kbm' || r === 'player_kbm'
    const before = role
    role = next
    const notices: string[] = []
    if (pads(next) && !pads(before)) notices.push(t('share.guest.canPlay'))
    if (kbm(next) && !kbm(before)) notices.push(t('share.guest.gotKbm'))
    if (next === 'spectator') notices.push(t('share.guest.watching'))
    else {
      if (!pads(next) && pads(before)) notices.push(t('share.guest.lostPads'))
      if (!kbm(next) && kbm(before)) notices.push(t('share.guest.lostKbm'))
    }
    if (notices.length) showNotice(notices.join(' '))
  }

  /** Sort du partage (une seule fois) : « Quitter le partage », fermeture de la page, échec. */
  function leaveGuest() {
    if (!g || left) return
    left = true
    void leaveShare(g.join)
  }

  /** Connexion coupée sous un invité : l'hôte l'a peut-être retiré, ou arrêté le partage (Kyber
   * coupe tout de suite). Le signe de vie dit pourquoi ; sans réponse en 2,5 s, l'échec est dit tel quel. */
  function guestClosed(message: string) {
    heartbeat.now()
    const gen = generation
    setTimeout(() => !stale(gen) && fail(message), 2500)
  }

  /** Jeton Kyber relu sur la console (reconnexion, puis toutes les heures),
   * pour que le moteur ne se reconnecte pas avec un jeton périmé. Un invité
   * n'a pas de jeton d'Admin : son signe de vie renouvelle le sien. */
  let ticketAt = 0
  async function refreshTicket(resume = false) {
    if (g) {
      heartbeat.now()
      if (resume) engine?.updateCredentials(ticketToken, true)
      return
    }
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

  // Valeur envoyée à la console (« Jeu », « Bureau », « Max », « Personnalisé ») : ne se traduit pas.
  const profileLabel = $derived(profileName(prefs))
  const profileText = $derived(isCustom(prefs) ? t('web.profile.custom') : t(`web.profile.${prefs.profile}`))
  /** « 8,3 » en français, « 8.3 » en anglais. */
  const decimal = (text: string) => (i18n.lang === 'fr' ? text.replace('.', ',') : text)
  const latencyText = $derived(latency ? `${decimal(latency.total.toFixed(latency.total < 10 ? 1 : 0))} ms` : '— ms')
  const formatText = $derived.by(() => {
    if (!mode || !plan) return ''
    // Un invité : l'image qu'il reçoit (une image à part est en 4:2:0), une fois que Kyber la dit.
    if (g && image?.width && image.height) {
      return `${image.width} × ${image.height} · ${image.fps ?? mode.refresh} Hz · ${formatLabel(image.codec ?? plan.codec, plan.yuv444 && image.follows_host !== false)}`
    }
    return `${mode.width} × ${mode.height} · ${mode.refresh} Hz · ${formatLabel(plan.codec, plan.yuv444)}`
  })
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

  /** Médiane des médianes de chaque seconde, et leur 95e centile, comme le
   * cœur des apps (`apps/core/src/latency.rs`). */
  function sessionStats(): { median: number; p95: number } | null {
    if (!secondMedians.length) return null
    const v = [...secondMedians].sort((a, b) => a - b)
    return { median: v[Math.floor(v.length / 2)], p95: v[Math.min(v.length - 1, Math.floor(v.length * 0.95))] }
  }

  /** Garde la dernière session (rubrique Statistiques) à partir de 3 s d'image. */
  function keepLastSession() {
    // Un invité n'a pas de statistiques à garder : ce n'est pas son ordinateur.
    if (g || lastSaved || !firstImageAt || !mode || !plan) return
    const stats = sessionStats()
    const duration = (Date.now() - firstImageAt) / 1000
    if (!stats || duration < MIN_SESSION_SECONDS) return
    lastSaved = true
    saveLastSession({
      median: stats.median,
      p95: stats.p95,
      duration,
      endedAt: Date.now() / 1000,
      width: canvasSize.width || mode.width,
      height: canvasSize.height || mode.height,
      refresh: mode.refresh,
      codec: `${plan.codec === 'hevc' ? 'HEVC' : 'H.264'}${plan.yuv444 ? ' 4:4:4' : ''}`,
      mbps: plan.mbps,
    })
  }

  /** Session suivante (Se reconnecter après un échec) : mesures à zéro. */
  function resetStats() {
    secondMedians = []
    firstImageAt = 0
    lastSaved = false
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
    endSwitch()
    phase = 'preparing'
    try {
      // La session d'avant doit avoir rendu son écran et arrêté son moteur.
      await allStopped()
      if (stale(gen)) return
      if (g && guestDisplay) {
        // Invité : ni écran à préparer ni jeton d'Admin. Le jeton et l'écran viennent de `join`
        // (le moteur est chargé, le format lu avant d'entrer : `Join.svelte`).
        mode = { width: guestDisplay.width, height: guestDisplay.height, refresh: guestDisplay.refresh }
        codecList = g.codecs
        textInput?.toggleAttribute(TEXT_ATTRIBUTE, engineInputs() && canKbm)
        await connect(preferredProtocol(), gen)
        return
      }
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
    if (g) plan = guestPlan(plan)
    // Jamais deux moteurs à la fois : chacun écoute souris et clavier sur la page.
    await allStopped()
    if (stale(gen)) return
    await tick()
    if (stale(gen)) return
    freshCanvas()
    const e: Engine = new Engine('kn-canvas', (ev) => onEngine(ev, e, protocol))
    engine = e
    connectWatch = setTimeout(() => noControlPlane(e), 25000)
    await e.connect(streamHost(), location.port || '443', ticketToken, plan, g ? guestDisplay?.id : undefined)
  }

  /** L'invité demande le format de l'hôte s'il le lit (il partage alors son image), sinon un format qu'il
   * lit : Kyber lui fait une image à part (docs/kybernow-share.md, « L'image d'un invité »). Le 4:4:4
   * seulement pour l'image de l'hôte (`host` dans son jeton) ; jamais de presse-papiers. Image de l'hôte
   * pas encore démarrée : le format que ce navigateur lit le mieux (l'hôte garde le sien). */
  function guestPlan(p: Plan): Plan {
    const preset = tokenPreset(ticketToken)
    const format = guestFormatNow()
    if (!format) return { ...p, yuv444: p.yuv444 && (preset === null || preset === 'host'), clipboard: false }
    return { ...p, ...format, clipboard: false }
  }

  /** Pas de plan de contrôle : Kyber ne répond pas (ce n'est pas une affaire de transport). */
  function noControlPlane(e: Engine) {
    connectWatch = null
    if (engine !== e || ended || phase !== 'connecting') return
    failOrRestart(tName('session.error.noReply', 'host', hostname))
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
      engine = null
      await retire(e)
      if (!stale(gen) && phase === 'connecting') {
        connect('websocket', gen).catch((err) => {
          if (!stale(gen)) fail(errorText(err))
        })
      }
    } else if (from === 'connected') {
      failOrRestart(t('session.error.noImageStart', { host: hostname }))
    } else {
      fail(g ? t('share.guest.noImage', { host: hostname }) : t('session.error.noImageArrive', { host: hostname }))
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
      fail(tName('session.error.pipeline', 'host', hostname, { max: MAX_PIPELINE_RESTARTS }))
      return
    }
    pipelineRestarts++
    const n = pipelineRestarts
    setTimeout(() => restartVideo(t('session.notice.imageRestart', { n, max: MAX_PIPELINE_RESTARTS })), 1000)
  }

  function connectError(message: string): string {
    if (message === 'unauthorized') return g ? t('share.guest.unauthorized', { host: hostname }) : t('session.error.unauthorized', { host: hostname })
    if (message === 'streaming_start_failed' || message.startsWith('start_streaming')) return t('session.error.startFailed', { host: hostname })
    return tName('session.error.noReply', 'host', hostname)
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
        onDecoded({ width: e.width, height: e.height })
        break
      case 'video-stopped':
        // Relance de l'image : l'ancienne toile appartient au moteur, il en
        // faut une neuve ; l'écran visé et la connexion des entrées changent.
        forgetInputSocket()
        freshCanvas()
        decoded = null
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
        endSwitch()
        phase = 'live'
        reconnecting = ''
        if (pendingNotice) {
          showNotice(pendingNotice)
          pendingNotice = ''
        }
        if (!heartbeat.running) heartbeat.start(heartbeatBody(null, profileLabel, extra()))
        learnDisplayId().then((list) => {
          if (list && !displays) displays = list.map((d) => ({ id: d.id, name: String(d.name ?? ''), width: Number(d.width) || 0, height: Number(d.height) || 0 }))
        })
        probeAudio(src)
        // Une image tenue 30 s remet le compte des relances à zéro.
        if (stableTimer) clearTimeout(stableTimer)
        stableTimer = setTimeout(() => (pipelineRestarts = 0), 30_000)
        if (first) {
          // Un invité apprend tout de suite ce que la Station dit de son image (et si elle connaît la qualité).
          if (g) void beatNow()
          // Pas après une relance : le clavier de l'appareil se fermerait.
          document.getElementById('kn-canvas')?.focus()
          wasLive = true
          if (!firstImageAt) firstImageAt = Date.now()
          // Le pointeur du pavé tactile part du centre : sa couche était masquée quand il a été placé.
          tick().then(() => {
            if (!touch || !touchLayer) return
            const r = touchLayer.getBoundingClientRect()
            touch.cursor = { x: r.left + r.width / 2, y: r.top + r.height / 2 }
            if (isTouch && touchMode === 'trackpad') cursor = touch.cursor
          })
        }
        break
      }
      case 'error':
        console.warn('KyberNow : connexion impossible', redact(e.message, g ? [g.join.secret, ticketToken] : []))
        // Un jeton refusé ne se répare pas en redémarrant la Station.
        if (e.message === 'unauthorized') {
          // Un invité retiré n'a plus de jeton : son signe de vie dit pourquoi.
          if (g) guestClosed(connectError(e.message))
          else fail(connectError(e.message))
        } else failOrRestart(connectError(e.message))
        break
      case 'stopped':
        if (ended) return
        if (e.reason === 'restart') {
          // Le moteur a perdu sa toile en se reconnectant : on recommence proprement.
          restart()
        } else if (e.reason === 'decode_error') {
          // Le moteur a redemandé une image clé, en vain (correctif 0020) : un invité repasse en 720p30, une fois.
          if (g && !fallbackTried && tokenPreset(ticketToken) !== '720p30') void guestFallback()
          else fail(g ? t('share.guest.decode', { host: hostname }) : t('session.error.decode'))
        } else if (e.reason === 'controlplane_closed') {
          const message = t('session.error.closed', { host: hostname })
          if (g) guestClosed(message)
          else fail(message)
        } else if (e.reason === 'requested') {
          // Arrêt demandé par ce client, sans que la page l'ait voulu.
          fail(t('session.error.stopped'))
        } else {
          const message = t('session.error.interrupted', { host: hostname })
          if (g) guestClosed(message)
          else fail(message)
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
    endSwitch()
    qualityOpen = false
    keepLastSession()
    leaveGuest()
    clearWatches()
    gamepads = []
    soundBlocked = false
    error = message
    phase = 'ended'
    barVisible = false
    heartbeat.stop()
    // Le moteur s'arrête, puis l'écran préparé pour cette session est rendu.
    const e = engine
    engine = null
    const wasPrepared = prepared
    prepared = false
    retire(e, wasPrepared ? () => backend.end(null, extra()) : undefined)
  }

  /** Kyber ne se connecte pas ou ne démarre pas l'image alors qu'Admin
   * répond : la Station redémarre une fois (dernier recours, docs/link-ui.md),
   * puis la page reprend la connexion après le mot de passe. Pas pour une
   * image qui part mais n'arrive pas : c'est ce navigateur ou le réseau. */
  function failOrRestart(message: string) {
    // Un invité ne redémarre jamais la Station.
    if (g || wasLive || !lastResortAllowed()) fail(message)
    else void lastResort(message)
  }

  async function lastResort(message: string) {
    const gen = ++generation
    clearWatches()
    phase = 'restarting'
    heartbeat.stop()
    // Le moteur s'arrête et l'écran préparé est rendu d'abord : Admin
    // compterait sinon cette session et refuserait de redémarrer.
    const e = engine
    engine = null
    const wasPrepared = prepared
    prepared = false
    await retire(e, wasPrepared ? () => backend.end(null, extra()) : undefined)
    if (stale(gen)) return
    const out = await restartForConnection(() => !ended)
    if (stale(gen)) return
    // Refusé (un autre appareil en session, trop tôt…) : l'échec d'origine.
    fail(out === 'slow' ? t('web.station.slow') : message)
  }

  async function quit() {
    if (ended) return
    ended = true
    generation++
    clearWatches()
    keepLastSession()
    leaveGuest()
    const median = sessionStats()?.median ?? null
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
    if (!g) prefs.mouse = m
    if (!pointerAllowed()) releasePointer()
  }

  function pointerAllowed(): boolean {
    if (!canKbm) return false
    return mouseMode === 'jeu' || (mouseMode === 'auto' && gameInForeground)
  }

  function cycleMouse() {
    setMouseMode(mouseMode === 'auto' ? 'bureau' : mouseMode === 'bureau' ? 'jeu' : g ? 'bureau' : 'auto')
  }

  const mouseLabel = $derived(mouseMode === 'auto' ? t('session.mouse.auto') : mouseMode === 'jeu' ? t('session.mouse.game') : t('session.mouse.desktop'))

  /** Largeur de la languette du haut, zone du bord tant que la barre n'est pas mesurée. */
  const EDGE_WIDTH = 220
  /** Distance au bord haut de la session, en pixels, qui compte comme « contre le bord ». */
  const EDGE_REACH = 2
  /** Le pointeur doit rester contre le bord haut ce temps-là pour ouvrir la barre. */
  const EDGE_DWELL = 250
  /** La barre se referme ce temps après que le pointeur l'a quittée. */
  const BAR_LEAVE = 400
  /** Barre ouverte sans pointeur dessus (doigt, raccourci) : elle se referme après ce temps. */
  const BAR_IDLE = 2500

  /** Le bord haut de l'image, centré sur la largeur de la barre. */
  function inEdgeZone(x: number, y: number): boolean {
    const r = sessionEl.getBoundingClientRect()
    return y >= r.top && y - r.top <= EDGE_REACH && Math.abs(x - (r.left + r.width / 2)) <= (barWidth || EDGE_WIDTH) / 2
  }

  /** Le pointeur est sur la barre, ses menus, ou dans l'espace qui la sépare du bord haut. */
  function barInside(): boolean {
    if (!hoverPos) return false
    const { x, y } = hoverPos
    if (document.elementFromPoint(x, y)?.closest('.bar, .metrics, .sharebox')) return true
    const r = sessionEl.getBoundingClientRect()
    // Dispositions sans transformation : la barre en cours d'apparition compte déjà.
    const bottom = r.top + (barEl ? barEl.offsetTop + barEl.offsetHeight : 0)
    return y >= r.top && y <= bottom && Math.abs(x - (r.left + r.width / 2)) <= (barWidth || EDGE_WIDTH) / 2
  }

  function clearBarTimer() {
    if (barTimer) clearTimeout(barTimer)
    barTimer = null
  }

  function cancelDwell() {
    if (dwellTimer) clearTimeout(dwellTimer)
    dwellTimer = null
  }

  function showBar(byTouch = false) {
    cancelDwell()
    barVisible = true
    barSeen = !byTouch && barInside()
    settleBar()
  }

  function hideBar() {
    clearBarTimer()
    barVisible = false
    edgeReady = !(hoverPos && inEdgeZone(hoverPos.x, hoverPos.y))
  }

  /** Arme la fermeture : jamais pendant qu'un menu est ouvert. */
  function scheduleBarHide(delay: number) {
    clearBarTimer()
    if (isTouch || metricsOpen || keysOpen || shareOpen || qualityOpen || phase !== 'live') return
    barTimer = setTimeout(() => {
      barTimer = null
      if (!metricsOpen && !keysOpen && !shareOpen && !qualityOpen) hideBar()
    }, delay)
  }

  /** Barre ouverte : sous le pointeur, elle reste ; sinon elle se referme (vite
   * si le pointeur l'a déjà vue, après un délai si elle est venue au doigt ou au raccourci). */
  function settleBar() {
    if (!barVisible) return
    if (barInside()) clearBarTimer()
    else scheduleBarHide(barSeen ? BAR_LEAVE : BAR_IDLE)
  }

  // Un menu qui se ferme sous un pointeur parti ailleurs laisse la barre se refermer.
  $effect(() => {
    if (barVisible && !metricsOpen && !keysOpen && !shareOpen && !qualityOpen) untrack(settleBar)
  })

  // Un invité sans clavier ni souris n'envoie rien de la page, et ce qu'il tenait est relâché
  // (touches, boutons, pointeur capturé, clavier de l'appareil, menu Touches).
  $effect(() => {
    allowPackets(canKbm)
    if (canKbm) return
    untrack(() => {
      releaseAll()
      releasePointer()
      keysOpen = false
      if (keyboardOpen) closeKeyboard()
    })
  })

  // Le clavier du navigateur et le champ de texte du moteur suivent les mêmes droits : en plein écran
  // (Chrome, Edge), toutes les touches vont à l'ordinateur seulement avec le clavier et la souris ; sans,
  // elles reviennent au navigateur (Échap, raccourcis), et le champ que le moteur lit n'écoute plus.
  $effect(() => {
    const kbm = canKbm
    untrack(() => {
      textInput?.toggleAttribute(TEXT_ATTRIBUTE, engineInputs() && kbm)
      if (!g || !hasKeyboardLock || !document.fullscreenElement) return
      const keyboard = (navigator as any).keyboard
      if (kbm) keyboard.lock().catch(() => {})
      else keyboard.unlock?.()
    })
  })

  // Les manettes d'un invité suivent son rôle (voir `gamepads.ts`) : le moteur ne les voit que s'il joue.
  let padsAllowed = untrack(() => canPlay)
  $effect(() => {
    const play = canPlay
    untrack(() => {
      if (play === padsAllowed) return
      padsAllowed = play
      gamepadsChanged(play)
    })
  })

  /** Le pointeur est resté contre le bord haut : la barre s'ouvre si la page a le focus et la souris n'est pas capturée. */
  function openFromEdge() {
    dwellTimer = null
    if (barVisible || phase !== 'live' || buttonHeld || !document.hasFocus() || document.pointerLockElement) return
    if (hoverPos && inEdgeZone(hoverPos.x, hoverPos.y)) showBar()
  }

  function onPointerMove(e: PointerEvent) {
    if (isTouch || e.pointerType === 'touch') return
    hoverPos = { x: e.clientX, y: e.clientY }
    buttonHeld = e.buttons !== 0
    if (barVisible) {
      if (barInside()) {
        barSeen = true
        clearBarTimer()
      } else if (barSeen && !barTimer) scheduleBarHide(BAR_LEAVE)
      return
    }
    if (!inEdgeZone(e.clientX, e.clientY)) {
      edgeReady = true
      cancelDwell()
    } else if (buttonHeld) {
      cancelDwell()
    } else if (edgeReady && !dwellTimer && !document.pointerLockElement && document.hasFocus()) {
      dwellTimer = setTimeout(openFromEdge, EDGE_DWELL)
    }
  }

  function onPointerLeave(e: PointerEvent) {
    if (isTouch || e.pointerType === 'touch') return
    hoverPos = null
    cancelDwell()
    if (barVisible && barSeen && !barTimer) scheduleBarHide(BAR_LEAVE)
  }

  async function toggleFullscreen() {
    if (document.fullscreenElement) await document.exitFullscreen().catch(() => {})
    else await document.documentElement.requestFullscreen({ navigationUI: 'hide' }).catch(() => {})
  }

  async function onFullscreenChange() {
    fullscreen = !!document.fullscreenElement
    // Plein écran : toutes les touches vont à l'ordinateur (Chrome, Edge).
    if (fullscreen && hasKeyboardLock && canKbm) {
      await (navigator as any).keyboard.lock().catch(() => {})
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

  /**
   * « Partager » : le panneau de l'hôte ; les autres menus de la barre se ferment. Impossible (Station
   * trop ancienne, partage coupé) : à la souris et au clavier, sans effet (l'infobulle dit pourquoi) ;
   * au toucher, il ouvre le panneau réduit à la raison (docs/link-ui.md, « Partage »).
   */
  function toggleShare() {
    const touched = shareTouch || isTouch
    shareTouch = false
    if (shareSupport !== 'ok' && !touched && !shareOpen) return
    shareOpen = !shareOpen
    if (shareOpen) {
      metricsOpen = false
      keysOpen = false
    }
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
    guide = false
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
      allowed: () => ({ keyboard: canKbm, mouse: canKbm }),
    })
    installPointerGate(pointerAllowed)
    installAbsoluteMouse()
    watchInputSocket()
    if (g) installGamepadGate(() => canPlay)
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
    // Un invité signale sa présence dès l'arrivée : sans nouvelles pendant 20 s, il sort du partage.
    if (g) heartbeat.start({ guest: g.join.guest })
    else {
      // Ce que la Station sait du partage : une Station d'avant la 0.8.0 ne le connaît pas,
      // et il est coupé quand `multi_client` l'est (docs/link-ui.md).
      backend
        .info()
        .then((i: Info) => (shareSupport = !i.share ? 'old' : i.share.enabled ? 'ok' : 'off'))
        .catch(() => {})
    }
    // Ctrl + Alt + Maj : la barre, et la souris rendue.
    const shortcut = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.altKey && e.shiftKey && ['Control', 'Alt', 'Shift'].includes(e.key)) {
        releasePointer()
        if (barVisible) hideBar()
        else showBar()
      }
    }
    window.addEventListener('keydown', shortcut, true)
    const onBlur = () => {
      pageFocused = false
      cancelDwell()
    }
    const onFocus = () => (pageFocused = true)
    const onLockChange = () => {
      pointerLocked = !!document.pointerLockElement
      if (pointerLocked) cancelDwell()
    }
    pageFocused = document.hasFocus()
    window.addEventListener('blur', onBlur)
    window.addEventListener('focus', onFocus)
    document.addEventListener('pointerlockchange', onLockChange)
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
      // Un invité n'a pas de compte pour le demander : sa souris reste celle du bureau (ou des jeux, à la main).
      if (g || phase !== 'live' || mouseMode !== 'auto') return
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
      // Un invité sort sans façon : il rejoint de nouveau avec le code.
      if (g || phase !== 'live' || ended) return
      e.preventDefault()
      e.returnValue = ''
    }
    window.addEventListener('beforeunload', beforeUnload)
    // Tout clic ou toute touche est un geste : le son peut y reprendre.
    const unlockAudio = async () => {
      if (engine && (await engine.audioState()) === 'running') soundBlocked = false
    }
    const leave = () => {
      keepLastSession()
      // Un invité sort du partage à la fermeture de la page.
      leaveGuest()
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
    // Médiane de chaque seconde d'image, pour la dernière session.
    const seconds = setInterval(() => {
      if (!engine || phase !== 'live') return
      const v = engine.latency.lastSecond()
      if (v !== null) secondMedians.push(v)
    }, 1000)
    // La latence du signal de vie (le Worker envoie la dernière connue).
    const sampler = setInterval(() => {
      // Le signe de vie d'un invité porte seulement son identifiant.
      if (g || !engine || phase !== 'live') return
      const l = engine.sample()
      heartbeat.update(heartbeatBody(l?.total ?? null, profileLabel, extra()))
    }, 5000)
    const ticketTimer = setInterval(() => phase === 'live' && !g && refreshTicket(), 3_600_000)

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
      clearInterval(seconds)
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
      endSwitch()
      window.removeEventListener('keydown', shortcut, true)
      window.removeEventListener('blur', onBlur)
      window.removeEventListener('focus', onFocus)
      document.removeEventListener('pointerlockchange', onLockChange)
      releasePointer()
      removePointerGate()
      removeAbsoluteMouse()
      removeInputGuard()
      removeGamepadGate()
      allowPackets(true)
      setPositionMapper(() => null)
      forgetInputSocket()
      clearBarTimer()
      cancelDwell()
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
        keepLastSession()
        leaveGuest()
        retire(engine, prepared ? () => backend.end(null, extra()) : undefined)
      }
    }
  })
</script>

<div class="session" class:touch={isTouch} bind:this={sessionEl} onpointermove={onPointerMove} onpointerleave={onPointerLeave} role="application" aria-label={t('session.aria', { host: hostname })}>
  <div class="stage" class:kb={keyboardOpen} bind:this={stage} oncontextmenu={(e) => e.preventDefault()} role="presentation">
    <div class="canvas-box" bind:this={canvasBox}></div>
    <div class="touch-layer" class:active={hasTouchScreen && phase === 'live' && canKbm} bind:this={touchLayer}></div>
    {#if cursor && phase === 'live'}
      <div class="tcursor" style="left: {cursor.x}px; top: {cursor.y}px" aria-hidden="true"></div>
    {/if}
  </div>

  {#if phase !== 'live' || switching}
    <div class="overlay" role="status">
      {#if phase === 'ended'}
        <div class="card">
          <Mark size={34} state="now" />
          <h2 class="disp">{wasLive ? t('session.start.ended') : t('session.start.failed')}</h2>
          <p>{error}</p>
          <div class="row">
            {#if g}
              <!-- Un invité sorti du partage ne le reprend pas : il rejoint de nouveau avec un code. -->
              <button type="button" class="primary" onclick={quit}>{t('share.join.button')}</button>
            {:else}
              <button type="button" class="primary" onclick={() => { ended = false; wasLive = false; resetStats(); restart() }}>{t('session.start.reconnect')}</button>
              <button type="button" class="ghost" onclick={quit}>{t('session.start.home')}</button>
            {/if}
          </div>
        </div>
      {:else}
        {@const current = phase === 'live' ? 2 : phase === 'preparing' ? 0 : connectStep === 'kyber' ? 1 : 2}
        <div class="card">
          <Mark size={34} state="now" lit breathe />
          <h2 class="disp">{title}</h2>
          <ol class="steps">
            {#if phase === 'restarting'}
              <li class="current" aria-current="step"><span class="mark" aria-hidden="true"></span>{t('web.station.restarting')}</li>
            {:else}
              {#each [t('session.start.preparing'), t('session.start.connecting'), t('session.start.firstImages')] as label, i (i)}
                <li class:done={i < current} class:current={i === current} aria-current={i === current ? 'step' : undefined}><span class="mark" aria-hidden="true"></span>{label}</li>
              {/each}
            {/if}
          </ol>
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
    {#if edgeShown}
      <button type="button" class="edge" aria-label={t('session.bar.show')} title={t('session.bar.showHint')} onpointerdown={(e) => (edgeTouch = e.pointerType === 'touch')} onclick={() => showBar(edgeTouch)}><span></span></button>
    {/if}
    <!-- Souris : la fermeture suit le pointeur (onPointerMove). Doigt : elle attend un temps sans toucher. -->
    <div class="bar glass" class:hidden={!barVisible} role="toolbar" tabindex="-1" aria-label="Session" bind:this={barEl} bind:clientWidth={barWidth} onpointerenter={(e) => e.pointerType === 'touch' && clearBarTimer()} onpointerleave={(e) => e.pointerType === 'touch' && scheduleBarHide(BAR_IDLE)}>
      <Mark size={22} state="now" lit breathe />
      <span class="host">{title}</span>
      <span class="sep" aria-hidden="true"></span>
      <button type="button" class="lat" aria-expanded={metricsOpen} aria-label={t('session.bar.metrics')} onclick={() => { metricsOpen = !metricsOpen; keysOpen = false; shareOpen = false; qualityOpen = false }}>
        <span class="mono value">{latencyText}</span>
        <svg width="64" height="22" viewBox="0 0 64 22" aria-hidden="true"><path d={sparkPath} fill="none" stroke="#9A8CFF" stroke-width="1.5" stroke-linejoin="round" /></svg>
        <span class="mono fmt">{formatText}</span>
      </button>
      <span class="sep" aria-hidden="true"></span>
      {#if g}
        <!-- Invité : son rôle, ni profil ni réglages de l'ordinateur. -->
        <span class="chip static role">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9A8CFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            {#if role === 'spectator'}<path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12z" /><circle cx="12" cy="12" r="2.6" />{:else}<path d="M7 8h10a4 4 0 0 1 4 4v1a3 3 0 0 1-5.4 1.8L14.5 13h-5l-1.1 1.8A3 3 0 0 1 3 13v-1a4 4 0 0 1 4-4z" /><path d="M8 10.5v3M6.5 12h3" />{/if}
          </svg>
          <span class="label">{roleText}</span>
        </span>
        {#if qualityKnown}
          <!-- « Qualité de l'image » : la qualité reçue ; le menu la change (l'image de l'invité seulement). -->
          <button type="button" class="chip" bind:this={qualityChip} aria-haspopup="menu" aria-expanded={qualityOpen} aria-label="{t('share.quality.title')} · {receivedText}" title={t('share.quality.title')} disabled={qualityBusy} onclick={toggleQuality}>
            <Icon name="display" size={16} stroke={1.7} />
            <span class="label">{receivedText}</span>
          </button>
        {/if}
      {:else}
        <span class="chip static">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9A8CFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></svg>
          <span class="label">{profileText}</span>
        </span>
      {/if}
      {#if canKbm}
        <button type="button" class="chip" aria-label={mouseLabel} title={t('session.mouse.hint')} onclick={cycleMouse}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round" aria-hidden="true"><path d="M5 3l14 7-6 2-2 6z" /></svg>
          <span class="label">{mouseLabel}</span>
        </button>
      {/if}
      {#if gamepads.length && canPlay}
        <span class="chip static" title={gamepads.map((p) => p.name).join(', ')} aria-label={gamepadAria}>
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
      {#if canKbm}
        <button type="button" class="chip" aria-label={t('session.bar.keys')} aria-expanded={keysOpen} onclick={() => { keysOpen = !keysOpen; metricsOpen = false; shareOpen = false; qualityOpen = false }}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
          <span class="label">{t('session.bar.keys')}</span>
        </button>
      {/if}
      {#if !g}
        <!-- « Partager » (KyberNow Share) : inactif si la Station ne le sait pas ou l'a coupé ; le panneau dit pourquoi. -->
        <button type="button" class="chip" class:inactive={shareSupport !== 'ok'} aria-expanded={shareOpen} aria-disabled={shareSupport !== 'ok'} aria-label={guestCount ? `${t('share.bar.share')} · ${tn('share.guests', guestCount)}` : t('share.bar.share')} title={shareHint} onpointerdown={(e) => (shareTouch = e.pointerType === 'touch' || e.pointerType === 'pen')} onclick={toggleShare}>
          <Icon name="share" size={16} stroke={1.7} />
          <span class="label">{t('share.bar.share')}</span>
          {#if guestCount}<span class="count" aria-hidden="true"><span class="full">{tn('share.guests', guestCount)}</span><span class="num">{guestCount}</span></span>{/if}
        </button>
      {/if}
      <span class="sep" aria-hidden="true"></span>
      <button type="button" class="chip icon" aria-label={fullscreen ? t('session.bar.exitFullscreen') : t('session.bar.fullscreen')} onclick={toggleFullscreen}>
        {#if fullscreen}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" /></svg>
        {:else}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></svg>
        {/if}
      </button>
      <button type="button" class="quit" aria-label={g ? t('share.guest.leave') : t('session.bar.quit')} onclick={quit}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 3v9" /><path d="M6.3 6.3a8 8 0 1 0 11.4 0" /></svg>
        <span class="label">{g ? t('share.guest.leave') : t('session.bar.quit')}</span>
      </button>

      {#if qualityOpen && g}
        <div class="menu glass qmenu" role="menu" aria-label={t('share.quality.title')} style:left="{qualityLeft}px">
          {#each QUALITIES as q (q)}
            <button type="button" role="menuitemradio" aria-checked={q === quality} onclick={() => pickQuality(q)}>{qualityName(q)}{#if q === quality}<Icon name="check" size={15} stroke={2} />{/if}</button>
          {/each}
        </div>
      {/if}

      {#if keysOpen && canKbm}
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
          <span>{t('session.metrics.fps', { fps })}</span>{#if !g}<span>{plan?.mbps ?? '—'} Mbit/s</span>{/if}<span>{plan?.reliable ? t('session.metrics.reliable') : t('session.metrics.fast')}</span>
        </div>
        <p class="muted small">{t('session.metrics.browserNote')}</p>
      </section>
    {/if}

  {/if}

  <!-- Tactile : languette, barre compacte, clavier -->
  {#if isTouch && phase === 'live'}
    {#if !touchBar}
      <button type="button" class="tab glass" aria-label={t('session.bar.show')} onpointerdown={(e) => e.preventDefault()} onclick={() => { touchBar = true; guide = canKbm && !prefs.touchGuideSeen }}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6" /></svg>
      </button>
    {:else}
      <div class="tbar glass" role="toolbar" tabindex="-1" aria-label="Session" onpointerdown={(e) => e.preventDefault()}>
        <Mark size={20} state="now" lit />
        <span class="mono tlat">{latencyText}</span>
        {#if g}<span class="trole">{roleText}</span>{/if}
        {#if g && qualityKnown}
          <button type="button" class="tb" aria-haspopup="menu" aria-expanded={qualityOpen} aria-label="{t('share.quality.title')} · {receivedText}" disabled={qualityBusy} onclick={() => (qualityOpen = !qualityOpen)}>
            <Icon name="display" size={20} stroke={1.7} />
          </button>
        {/if}
        {#if gamepads.length && canPlay}
          <span class="tpad" title={gamepads.map((p) => p.name).join(', ')} aria-label={gamepadAria}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#9A8CFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 8h10a4 4 0 0 1 4 4v1a3 3 0 0 1-5.4 1.8L14.5 13h-5l-1.1 1.8A3 3 0 0 1 3 13v-1a4 4 0 0 1 4-4z" /><path d="M8 10.5v3M6.5 12h3" /></svg>
            {#if gamepads.length > 1}<span class="mono">{gamepads.length}</span>{/if}
          </span>
        {/if}
        <span class="sep" aria-hidden="true"></span>
        {#if canKbm}
          <div class="tmodes" role="radiogroup" aria-label={t('session.touch.modes')}>
            <button type="button" role="radio" aria-checked={touchMode === 'trackpad'} onclick={() => setTouchMode('trackpad')}>{t('session.touch.trackpad')}</button>
            <button type="button" role="radio" aria-checked={touchMode === 'direct'} onclick={() => setTouchMode('direct')}>{t('session.touch.direct')}</button>
          </div>
          <button type="button" class="tb" onclick={openKeyboard} aria-label={t('session.touch.keyboard')}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
          </button>
        {/if}
        {#if prefs.audio}
          <button type="button" class="tb" aria-pressed={muted} onclick={toggleMute} aria-label={muted ? t('session.sound.unmute') : t('session.sound.mute')}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" />{#if muted}<path d="M17 9l4 6M21 9l-4 6" />{:else}<path d="M17 9a4 4 0 0 1 0 6" />{/if}</svg>
          </button>
        {/if}
        {#if !g}
          <button type="button" class="tb" class:inactive={shareSupport !== 'ok'} aria-expanded={shareOpen} aria-disabled={shareSupport !== 'ok'} aria-label={guestCount ? `${t('share.bar.share')} · ${tn('share.guests', guestCount)}` : t('share.bar.share')} onclick={toggleShare}>
            <Icon name="share" size={20} stroke={1.7} />
            {#if guestCount}<span class="tcount mono" aria-hidden="true">{guestCount}</span>{/if}
          </button>
        {/if}
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="tb" onclick={() => { touchBar = false; guide = false; qualityOpen = false }} aria-label={t('session.touch.hideBar')}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 15l6-6 6 6" /></svg>
        </button>
        <button type="button" class="tb danger-tb" onclick={quit} aria-label={g ? t('share.guest.leave') : t('session.bar.quit')}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 3v9" /><path d="M6.3 6.3a8 8 0 1 0 11.4 0" /></svg>
        </button>
        {#if qualityOpen && g}
          <div class="menu glass tmenu" role="menu" aria-label={t('share.quality.title')}>
            <span class="mhead-q">{t('share.quality.title')} · {receivedText}</span>
            {#each QUALITIES as q (q)}
              <button type="button" role="menuitemradio" aria-checked={q === quality} onclick={() => pickQuality(q)}>{qualityName(q)}{#if q === quality}<Icon name="check" size={16} stroke={2} />{/if}</button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    {#if !keyboardOpen}
      {#if canKbm}
        <button type="button" class="fab glass" onpointerdown={(e) => e.preventDefault()} onclick={openKeyboard} aria-label={t('session.touch.openKeyboard')}>
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
        </button>
      {/if}
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

  {#if shareOpen && !g && phase === 'live'}
    <ShareBox {share} support={shareSupport} computer={hostname} onshare={(view) => (share = view)} onclose={() => (shareOpen = false)} />
  {/if}

  {#if diagEnabled && diagText}
    <pre class="diag mono" aria-hidden="true">{diagText}</pre>
  {/if}

  <textarea
    class="ime"
    bind:this={textInput}
    disabled={!canKbm}
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

  /* Les trois étapes de la connexion : faite, en cours (point qui respire), à venir. */
  .steps {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 14px;
    color: var(--ghost);
  }

  .steps li {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .steps li.done,
  .steps li.current {
    color: var(--text);
  }

  .steps .mark {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    box-shadow: inset 0 0 0 1px var(--ghost);
  }

  .steps li.done .mark {
    background: var(--signal-light);
    box-shadow: 0 0 6px rgb(154 140 255 / 0.8);
  }

  .steps li.current .mark {
    width: 10px;
    height: 10px;
    margin: 0 -1px;
    box-shadow: inset 0 0 0 2px var(--text);
    animation: step 1.2s ease-in-out infinite alternate;
  }

  @keyframes step {
    to {
      opacity: 0.35;
    }
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
    position: relative;
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

  .chip.role {
    color: var(--signal-light);
  }

  /* « Partager » impossible (la Station ne le sait pas, ou l'a coupé) : estompé ; à la souris sans effet
     (l'infobulle dit pourquoi), au toucher il ouvre le panneau réduit à la raison. */
  .chip.inactive,
  .tb.inactive {
    opacity: 0.5;
  }

  button.chip.inactive:hover {
    background: none;
    color: var(--text-2);
  }

  /* Pastille du partage ouvert : « 2 invités ». */
  .count {
    margin-left: 2px;
    padding: 2px 9px;
    border-radius: 10px;
    background: rgb(154 140 255 / 0.2);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
  }

  .count .num {
    display: none;
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

  /* « Qualité de l'image » : sous sa puce (barre de la souris), sous la barre tactile. */
  .menu.qmenu {
    right: auto;
    width: 220px;
  }

  .menu.tmenu {
    top: calc(100% + 8px);
    right: 0;
    width: 240px;
    white-space: normal;
  }

  .menu button[aria-checked='true'] {
    color: #fff;
  }

  .menu button[aria-checked='true'] :global(svg) {
    color: var(--signal-light);
  }

  .mhead-q {
    padding: 8px 11px 6px;
    font-size: 11.5px;
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

  .toast {
    position: absolute;
    left: 50%;
    translate: -50% 0;
    max-width: calc(100% - 32px);
    padding: 10px 16px;
    border-radius: 12px;
    font-size: 13px;
    color: var(--text-2);
    display: flex;
    align-items: center;
    gap: 10px;
    pointer-events: none;
    top: 80px;
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

  /* Tactile */
  .tab {
    position: absolute;
    left: 50%;
    top: 0;
    translate: -50% 0;
    width: 72px;
    height: 32px;
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

  .trole {
    font-size: 12px;
    color: var(--text-2);
    margin-right: 6px;
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
    position: relative;
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

  .tcount {
    position: absolute;
    right: 3px;
    top: 3px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--signal-strong);
    color: #fff;
    font-size: 10px;
    line-height: 16px;
    text-align: center;
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

  /* Barre de l'hôte, avec « Partager » : le nombre d'invités passe en pastille sur l'icône avant que la barre ne déborde. */
  @media (max-width: 1400px) {
    .count {
      position: absolute;
      right: 1px;
      top: 1px;
      margin: 0;
      min-width: 16px;
      height: 16px;
      padding: 0 4px;
      border-radius: 8px;
      background: var(--signal-strong);
      box-shadow: none;
      font-size: 10px;
      line-height: 16px;
      text-align: center;
    }

    .count .full {
      display: none;
    }

    .count .num {
      display: inline;
    }
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
    .steps li.current .mark {
      animation: none;
      transition: none;
    }
  }
</style>
