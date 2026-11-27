// La session dans le navigateur, sans son interface (Session.svelte) : la console prépare l'écran, le
// moteur de Kyber se connecte et dessine l'image, la page surveille qu'elle arrive, reprend ce qui peut
// l'être (transport, relance de l'image, replis du navigateur, dernier recours) et dit l'échec sinon
// (docs/link-ui.md, « Connexion »). Trois rôles : l'hôte (session d'Admin, `hosting`), un invité du
// partage (`guest`), un ami qui emprunte l'ordinateur (`borrowing`). L'enregistrement, le micro, la
// souris, le son et le débit (`controls`) et les mesures ont leurs modules.

import { tick } from 'svelte'
import { ApiError } from '../lib/api'
import { errorText } from '../lib/store.svelte'
import { t } from '../lib/i18n/index.svelte'
import { backend, endOnUnload, heartbeatBody, type Info } from './backend'
import { Borrowing } from './borrowing.svelte'
import { deviceKind, deviceName, measureRefresh, screenMode, type Mode } from './device'
import { Engine, allStopped, isStartFailure, retire, supportedCodecs, type CodecSupport, type DisplayInfo, type EngineEvent } from './engine'
import { applyFallbacks, NO_FALLBACK, nextFallback, pyrowaveFallback, take, type Fallbacks, type StartFallback } from './fallback'
import { GuestSession } from './guest.svelte'
import { Heartbeat } from './heartbeat'
import { guestTarget, lendTarget, sendHeartbeat } from './heartbeat-call'
import { Hosting } from './hosting.svelte'
import { currentDisplayId, engineInputs, forgetInputSocket, learnDisplayId } from './inputs'
import { stationIsMac } from './keyswap'
import type { BorrowStart } from './lend'
import { SessionMetrics } from './metrics.svelte'
import { SessionMic } from './micro.svelte'
import { SessionControls } from './controls.svelte'
import { tName } from './names'
import { choose, preferredProtocol, viaRelay, type Plan } from './plan'
import { releasePointer } from './pointer'
import { deviceId, isCustom, pickProfile, profileName, type Prefs, type Profile } from './prefs'
import { offer, probeAtLoad } from './pyrowave/state.svelte'
import { pyrowaveStats } from './pyrowave/stats'
import { SessionRecording } from './recording.svelte'
import { formatLabel, redact, type GuestStart } from './share'
import { lastResortAllowed, restartForConnection } from './station.svelte'
import { TEXT_ATTRIBUTE } from './touch'
import { sendCombo, type KeyEntry } from './keysmenu'
import { CANVAS_ID, canvas } from './stage'

/** `restarting` : dernier recours, la Station redémarre (station.svelte.ts). `waiting` : un ami attend l'accord de l'hôte. */
export type Phase = 'preparing' | 'waiting' | 'connecting' | 'restarting' | 'live' | 'ended'

/** Ce que la session demande à sa page. */
export interface SessionView {
  /** Une toile neuve pour le moteur (la précédente lui appartient). */
  freshCanvas(): void
  /** L'image aux proportions de la toile, dans la page. */
  fit(): void
  /** Le champ caché du clavier de l'appareil. */
  textInput(): HTMLTextAreaElement | null
  /** Première image de la session : la toile prend le clavier, le pointeur du pavé tactile part du centre. */
  firstImage(): void
  /** La session s'arrête : la barre et ses menus se ferment. */
  closeChrome(): void
}

export interface SessionOptions {
  /** Le nom de l'ordinateur, tel que la page le connaît (un ami apprend le sien d'Admin). */
  hostname: () => string
  prefs: () => Prefs
  guest: GuestStart | null
  borrow: BorrowStart | null
  onEnd: () => void
  /** Un ami : pourquoi le prêt n'a pas pu commencer, ou s'il est fini. */
  onLendEnd: (error: string | null, over: boolean) => void
  view: SessionView
}

/** Relances de l'image après une erreur de capture ou d'encodage. */
const MAX_PIPELINE_RESTARTS = 3

export class LiveSession {
  private o: SessionOptions
  readonly g: GuestStart | null
  readonly b: BorrowStart | null
  readonly guest: GuestSession | null
  readonly borrowing: Borrowing | null
  readonly hosting: Hosting | null
  readonly recording: SessionRecording
  readonly metrics: SessionMetrics
  readonly mic: SessionMic
  readonly controls: SessionControls
  readonly heartbeat: Heartbeat

  phase = $state<Phase>('preparing')
  error = $state('')
  /** Le message du moteur sous l'échec (« Détails »), à recopier. */
  errorDetail = $state('')
  reconnecting = $state('')
  plan = $state<Plan | null>(null)
  mode = $state<Mode | null>(null)
  connectStep = $state<'kyber' | 'image'>('kyber')
  /** L'image est déjà arrivée dans cette session (l'échec n'est plus « de connexion »). */
  wasLive = $state(false)
  /** Avis bref (écran changé, image relancée…). */
  notice = $state('')
  /** Bandeau du début d'une session PyroWave : 8 s, ou jusqu'à la croix. */
  pyroBanner = $state<'network' | 'relay' | null>(null)
  /** La session passe par le relais : puce « Par le relais ». */
  relayed = $state(viaRelay())
  /** Face à une Station Mac (`os` de `/api/stream/info`) : menu « Touches » et touches à l'écran du Mac,
   * « Ctrl agit comme ⌘ ». Hôte seulement : un invité ou un ami garde ceux d'un PC. */
  stationMac = $state(false)

  engine: Engine | null = null
  /** Jeton de Kyber : celui d'Admin (`/api/web/ticket`), celui de l'invité ou de l'ami, renouvelé par leur signe de vie. */
  ticketToken = ''
  /** Numéro de la tentative en cours. `start`, `fail` et `quit` le changent : une tentative dépassée
   * s'arrête à son prochain `await` au lieu de créer un moteur ou de préparer un écran pour rien. */
  generation = 0
  ended = false
  codecList: CodecSupport[] = []
  /** Dernière liste des écrans de l'ordinateur. */
  displays: DisplayInfo[] | null = null
  /** Taille de l'image décodée. */
  canvasSize = $state({ width: 0, height: 0 })
  /** Avis à montrer à la prochaine image (après une relance). */
  pendingNotice = ''

  private noticeTimer: ReturnType<typeof setTimeout> | null = null
  /** Replis pris parce que l'image ne démarrait pas : pour cette session, sans toucher aux réglages. */
  private fallbacks: Fallbacks = NO_FALLBACK
  /** PyroWave : ce que la Station en dit (`/api/stream/info`, relu au démarrage). */
  private stationInfo: Info | null = null
  /** « Passer en HEVC » : PyroWave laissé pour la suite de cette session seulement. */
  private pyrowaveOff = false
  private pyroBannerShown = false
  private pyroBannerTimer: ReturnType<typeof setTimeout> | null = null
  /** L'écran de cet appareil : la définition ramenée de PyroWave part de lui. */
  private screen: Mode | null = null
  /** Un échec a été dit à la Station : elle apprendra aussi comment l'image est partie. */
  private reportedFailure = false
  /** La console a préparé l'écran : il faudra le lui rendre. */
  private prepared = false
  /** Kyber a démarré l'image, mais elle n'arrive pas : on essaie l'autre transport. */
  private imageWatch: ReturnType<typeof setTimeout> | null = null
  /** D'où part le délai de l'image : la connexion (le démarrage peut ne jamais venir) ou le démarrage. */
  private imageWatchFrom: 'connected' | 'streaming' | null = null
  /** Le plan de contrôle ne se connecte pas : Kyber ne répond pas. */
  private connectWatch: ReturnType<typeof setTimeout> | null = null
  private pipelineRestarts = 0
  private pipelineTimer: ReturnType<typeof setTimeout> | null = null
  private stableTimer: ReturnType<typeof setTimeout> | null = null
  /** Manettes vues par le moteur. */
  private gamepads = 0
  /** La Station ne sait pas créer de manettes : un avis bref, une fois par session. Hôte seulement. */
  private padsUnavailable = false
  private padsNoticed = false
  private ticketAt = 0

  /** Le nom de l'ordinateur (celui que donne Admin à l'ami, une fois le prêt ouvert ou demandé). */
  readonly hostname = $derived.by(() => this.borrowing?.computer ?? this.optsHostname())
  /** « Invité sur {nom} », « Prêté par {hôte} », ou le nom de l'ordinateur. */
  readonly title = $derived.by(() => this.g ? t('share.guest.on', { name: this.hostname }) : this.borrowing ? this.borrowing.title : this.hostname)
  /** Droits : un invité a ceux de son rôle, l'hôte et l'ami ont tout. */
  readonly canKbm = $derived.by(() => this.guest ? this.guest.canKbm : true)
  readonly canPlay = $derived.by(() => this.guest ? this.guest.canPlay : true)
  /** « Jeu », « Bureau », « Max », ou « Personnalisé » quand les réglages s'en écartent. */
  readonly profileText = $derived(isCustom(this.prefs) ? t('web.profile.custom') : t(`web.profile.${this.prefs.profile}`))
  /** Le format des mesures de la barre : « 2560 × 1440 · 120 Hz · HEVC » ; un invité, l'image qu'il
   * reçoit (une image à part est en 4:2:0), une fois que Kyber la dit. */
  readonly formatText = $derived.by(() => {
    const mode = this.mode
    const plan = this.plan
    if (!mode || !plan) return ''
    const image = this.guest?.image
    if (image?.width && image.height) {
      return `${image.width} × ${image.height} · ${image.fps ?? mode.refresh} Hz · ${formatLabel(image.codec ?? plan.codec, plan.yuv444 && image.follows_host !== false)}`
    }
    return `${mode.width} × ${mode.height} · ${mode.refresh} Hz · ${formatLabel(plan.codec, plan.yuv444)}`
  })

  constructor(o: SessionOptions) {
    this.o = o
    this.g = o.guest
    this.b = o.borrow
    const prefs = o.prefs()
    this.ticketToken = o.guest?.join.kyber.token ?? ''
    this.guest = o.guest ? new GuestSession(this, o.guest) : null
    this.borrowing = o.borrow ? new Borrowing(this, o.borrow) : null
    this.hosting = !o.guest && !o.borrow ? new Hosting(this) : null
    this.recording = new SessionRecording(this)
    this.metrics = new SessionMetrics(this)
    this.mic = new SessionMic(this)
    this.controls = new SessionControls(this)
    // Signal de vie depuis un Worker : il continue dans un onglet masqué. Celui d'un invité va à
    // `/api/share/beat`, celui d'un ami à `/api/lend/beat`, avec leur secret.
    this.heartbeat = new Heartbeat(
      (r) => {
        if (this.ended || this.phase === 'ended') return
        if (this.guest) return this.guest.onBeat(r)
        if (this.borrowing) return this.borrowing.onBeat(r)
        if (r.status === 401) this.fail(this.expired())
        // Fin demandée depuis la console, un autre appareil prend la main, ou l'ordinateur est prêté
        // (demande acceptée, code créé depuis cette session, créneau d'un ami).
        else if (r.end) {
          const lentTo = this.hosting?.lentTo()
          this.fail(lentTo ? t('lend.host.lentComputer', { name: lentTo }) : t('session.error.taken'))
        } else if (r.status === 200) {
          this.hosting?.onBeat(r.data)
          this.recording.applyBeat(r.data?.recording)
        }
      },
      o.guest ? o.guest.join.beat * 1000 : 5000,
      o.guest ? guestTarget(o.guest.join.secret) : undefined,
    )
  }

  private optsHostname(): string {
    return this.o.hostname()
  }

  get prefs(): Prefs {
    return this.o.prefs()
  }

  /** « Ctrl + Alt + Maj + D » : un raccourci de la session (docs/link-ui.md, « Barre de session »). */
  shortcut(key: string): string {
    return t('session.shortcut', { key })
  }

  stale(gen: number): boolean {
    return gen !== this.generation || this.ended
  }

  /** Valeur envoyée à la console (« Jeu », « Bureau », « Max ») : ne se traduit pas. */
  private profileLabel(): string {
    return profileName(this.prefs)
  }

  /** Champs ajoutés aux corps du signal de vie et de la fin : cet appareil. */
  private extra() {
    return { device_id: deviceId() }
  }

  /** La session de la console (le cookie et le jeton durent 12 h) a expiré. */
  private expired(): string {
    return t('session.error.expired')
  }

  /** Le moteur corrigé lit lui-même le champ du clavier de l'appareil (et pas sans clavier ni souris). */
  engineReadsText() {
    this.o.view.textInput()?.toggleAttribute(TEXT_ATTRIBUTE, engineInputs() && this.canKbm)
  }

  showNotice(text: string, duration = 5000) {
    this.notice = text
    if (this.noticeTimer) clearTimeout(this.noticeTimer)
    this.noticeTimer = setTimeout(() => (this.notice = ''), duration)
  }

  /** Un avis tout de suite pendant l'image, sinon à la prochaine image. */
  say(text: string) {
    if (this.phase === 'live') this.showNotice(text)
    else this.pendingNotice = text
  }

  /** L'ami : son écran de départ, avec la raison (ou `null`). */
  lendEnd(error: string | null, over: boolean) {
    this.o.onLendEnd(error, over)
  }

  // ——— Démarrage ———

  async start() {
    const gen = ++this.generation
    this.error = ''
    this.errorDetail = ''
    this.guest?.endSwitch()
    this.phase = 'preparing'
    try {
      // La session d'avant doit avoir rendu son écran et arrêté son moteur.
      await allStopped()
      if (this.stale(gen)) return
      if (this.guest) {
        // Invité : ni écran à préparer ni jeton d'Admin. Le jeton et l'écran viennent de `join` (le moteur
        // est chargé, le format lu avant d'entrer : `Join.svelte`).
        const d = this.guest.display
        this.mode = { width: d.width, height: d.height, refresh: d.refresh }
        this.codecList = this.guest.start.codecs
        this.engineReadsText()
        await this.connect(preferredProtocol(), gen)
        return
      }
      if (this.borrowing) {
        await this.borrowing.open(gen)
        return
      }
      const [refresh, codecs, ticket, info] = await Promise.all([measureRefresh(), supportedCodecs(), backend.ticket(), backend.info().catch(() => null)])
      // La sonde PyroWave, face à une Station qui le propose seulement (déjà faite par l'accueil, le plus souvent).
      if (info?.pyrowave?.available) await probeAtLoad()
      if (this.stale(gen)) return
      this.screen = screenMode(refresh)
      this.codecList = codecs
      if (info) this.stationInfo = info
      // PyroWave choisi : l'écran demandé à l'ordinateur est à la définition ramenée s'il le faut.
      this.mode = choose(this.sessionPrefs(), this.screen, this.codecList, preferredProtocol(), this.pyroOffer()).pyrowave?.mode ?? this.screen
      this.engineReadsText()
      this.ticketToken = ticket.token
      const device = deviceName()
      await backend.prepare({
        mode: this.mode,
        audio: this.prefs.audio,
        virtualDisplay: this.prefs.virtualDisplay,
        profile: this.profileLabel(),
        device: { id: deviceId(), name: device.name, kind: deviceKind(), os: device.os, app: device.app },
      })
      if (this.stale(gen)) {
        // Finie pendant la préparation (Annuler, fin demandée) : l'écran est rendu.
        void retire(null, () => backend.end(null, this.extra()))
        return
      }
      this.prepared = true
      await this.connect(preferredProtocol(), gen)
    } catch (e) {
      if (!this.stale(gen)) this.fail(e instanceof ApiError && e.status === 401 ? this.expired() : errorText(e))
    }
  }

  async connect(protocol: 'kymux' | 'websocket', gen = this.generation) {
    if (!this.mode || this.stale(gen)) return
    this.phase = 'connecting'
    this.connectStep = 'kyber'
    this.clearWatches()
    // PyroWave : la définition ramenée se calcule depuis l'écran de cet appareil.
    let plan = choose(this.sessionPrefs(), !this.g && !this.b && this.screen ? this.screen : this.mode, this.codecList, protocol, this.pyroOffer())
    if (this.b) plan = { ...plan, clipboard: false }
    if (this.guest) plan = this.guest.plan(plan)
    this.plan = applyFallbacks(plan, this.fallbacks, this.codecList)
    // Jamais deux moteurs à la fois : chacun écoute souris et clavier sur la page.
    await allStopped()
    if (this.stale(gen)) return
    await tick()
    if (this.stale(gen)) return
    this.o.view.freshCanvas()
    const e: Engine = new Engine(CANVAS_ID, (ev) => this.onEngine(ev, e, protocol))
    this.engine = e
    this.controls.capped = false
    this.connectWatch = setTimeout(() => this.noControlPlane(e), 25000)
    const display = this.guest ? this.guest.display.id : this.borrowing?.displayId()
    await e.connect(location.hostname.replace(/^\[|\]$/g, ''), location.port || '443', this.ticketToken, this.plan, display)
  }

  /**
   * Les réglages de cette session. Un ami : le profil Jeu de la Station (docs/pret.md). Repli en H.264 :
   * le débit du H.264. PyroWave pour l'hôte seulement (un invité suit l'image de l'hôte, un ami celle du
   * prêt), et plus après « Passer en HEVC » ni quand il n'a pas pu s'ouvrir dans cette page : le format
   * d'Auto, à la définition de l'écran.
   */
  private sessionPrefs(): Prefs {
    const prefs = this.prefs
    const base: Prefs = this.b ? { ...prefs, profile: 'jeu' } : prefs
    if (this.fallbacks.h264) return { ...base, codec: 'h264' }
    if (base.codec === 'pyrowave' && (this.g || this.b || this.pyrowaveOff || this.fallbacks.noPyroWave)) return { ...base, codec: 'auto' }
    return base
  }

  private pyroOffer() {
    return this.g || this.b ? null : offer(this.stationInfo)
  }

  /** Ce que la Station sait faire (hôte) : partage, prêt, Station Mac, manettes. */
  learnStation() {
    backend
      .info()
      .then((i: Info) => {
        this.hosting?.learn(i)
        this.stationMac = stationIsMac(i.os)
        this.padsUnavailable = i.gamepads?.available === false
        this.noticePads()
      })
      .catch(() => {})
  }

  // ——— PyroWave ———

  /** Bandeau du début (réseau inconnu, relais) et définition ramenée, une fois par session. */
  private showPyroBanner() {
    const pw = this.plan?.pyrowave
    if (!pw || this.pyroBannerShown) return
    this.pyroBannerShown = true
    this.pyroBanner = pw.warning
    if (this.pyroBannerTimer) clearTimeout(this.pyroBannerTimer)
    this.pyroBannerTimer = setTimeout(() => (this.pyroBanner = null), 8000)
    if (pw.lowered) this.showNotice(t('session.pyrowave.lowered', { width: pw.lowered.width, height: pw.lowered.height, mbps: pw.lowered.mbps }), 8000)
  }

  hidePyroBanner() {
    if (this.pyroBannerTimer) clearTimeout(this.pyroBannerTimer)
    this.pyroBannerTimer = null
    this.pyroBanner = null
  }

  /** Le format d'Auto, que prend PyroWave en repli (« HEVC », ou « H.264 » si ce navigateur ne lit pas HEVC). */
  autoFormat(): string {
    return formatLabel(this.plan?.fallbackCodec ?? 'h264', false)
  }

  /** « Passer en HEVC » : l'image repart dans le format d'Auto, à la définition de l'écran, pour cette
   * session seulement (les réglages ne changent pas). */
  leavePyroWave() {
    this.hidePyroBanner()
    if (!this.plan?.pyrowave || this.ended) return
    this.pyrowaveOff = true
    this.pendingNotice = t('session.pyrowave.switched', { format: this.autoFormat() })
    void this.restart()
  }

  /** La carte graphique perdue pendant une session PyroWave (veille, pilote) : l'image est relancée une
   * fois le nouveau device prêt (2 s au plus), au lieu de rester figée sans message (WEB-01). */
  async onGpuLost() {
    if (this.phase !== 'live') return
    const m = await import('./pyrowave/live')
    await m.pyrowaveReady(2000)
    this.restartVideo(t('session.notice.gpuLost'))
  }

  // ——— Surveillance de la connexion ———

  clearWatches() {
    if (this.imageWatch) clearTimeout(this.imageWatch)
    if (this.connectWatch) clearTimeout(this.connectWatch)
    this.imageWatch = null
    this.connectWatch = null
    this.imageWatchFrom = null
  }

  /** Pas de plan de contrôle : Kyber ne répond pas (ce n'est pas une affaire de transport). */
  private noControlPlane(e: Engine) {
    this.connectWatch = null
    if (this.engine !== e || this.ended || this.phase !== 'connecting') return
    this.failOrRestart(tName('session.error.noReply', 'host', this.hostname))
  }

  /** Kyber a démarré l'image (`streaming`) : elle doit arriver dans les 12 s (20 s en WebSocket). Le délai
   * part d'ici, pas de la connexion : préparer l'écran virtuel prend à lui seul jusqu'à 8 s. Dès la
   * connexion (`connected`), un délai plus long couvre un démarrage qui ne vient pas. */
  private watchImage(e: Engine, protocol: 'kymux' | 'websocket', from: 'connected' | 'streaming') {
    if (this.engine !== e || this.phase !== 'connecting' || this.imageWatchFrom === 'streaming' || this.imageWatchFrom === from) return
    if (this.imageWatch) clearTimeout(this.imageWatch)
    this.imageWatchFrom = from
    const delay = from === 'connected' ? 25000 : protocol === 'kymux' ? 12000 : 20000
    this.imageWatch = setTimeout(() => this.noImage(e, protocol, from), delay)
  }

  /** Connecté, mais rien n'arrive. Des images PyroWave arrivent sans qu'aucune ne se décode : PyroWave ne
   * s'ouvre pas ici, l'image repart dans le format d'Auto. Sinon WebTransport bloqué (réseau, navigateur) :
   * on passe par WebSocket ; en WebSocket, on le dit. */
  private async noImage(e: Engine, protocol: 'kymux' | 'websocket', from: 'connected' | 'streaming') {
    this.imageWatch = null
    this.imageWatchFrom = null
    const gen = this.generation
    if (this.engine !== e || this.phase !== 'connecting' || this.stale(gen)) return
    if (this.plan && pyrowaveFallback(this.plan, this.fallbacks) && pyrowaveStats().lostPercent === 100) {
      console.warn('KyberNow : images PyroWave reçues, aucune décodée ; format d’Auto')
      this.retryWith('no_pyrowave')
    } else if (protocol === 'kymux') {
      console.warn('KyberNow : connecté, mais pas d’image en WebTransport ; essai par WebSocket')
      this.engine = null
      await retire(e)
      if (!this.stale(gen) && this.phase === 'connecting') {
        this.connect('websocket', gen).catch((err) => {
          if (!this.stale(gen)) this.fail(errorText(err))
        })
      }
    } else if (from === 'connected') {
      this.failOrRestart(t('session.error.noImageStart', { host: this.hostname }))
    } else {
      this.fail(this.g ? t('share.guest.noImage', { host: this.hostname }) : t('session.error.noImageArrive', { host: this.hostname }))
    }
  }

  /** « Manettes indisponibles sur cet ordinateur. » : à l'arrivée de l'image si une manette est
   * branchée, sinon quand la première se branche ; une fois par session. */
  private noticePads() {
    if (!this.padsUnavailable || this.padsNoticed || this.phase !== 'live' || !this.gamepads) return
    this.padsNoticed = true
    this.showNotice(t('session.gamepads.unavailable'))
  }

  /** Relance de l'image sans recharger la page ni se déconnecter. */
  private restartVideo(text: string) {
    if (!this.engine || this.phase !== 'live') return
    this.showNotice(text)
    this.engine.restartVideo()
  }

  /** Écran suivi disparu ou à une autre taille : on relance l'image (Kyber reprend alors l'écran à jour,
   * ou le premier de la liste). */
  private onDisplays(list: DisplayInfo[], id: number | null) {
    const before = this.displays
    this.displays = list
    if (this.phase !== 'live' || id === null) return
    const now = list.find((d) => d.id === id)
    const was = before?.find((d) => d.id === id)
    if (!now || (was && (was.width !== now.width || was.height !== now.height))) {
      console.warn('KyberNow : écran suivi', id, now ? `passé en ${now.width} × ${now.height}` : 'disparu')
      this.restartVideo(t('session.notice.displayChanged'))
    }
  }

  /** Erreur de la chaîne d'image de l'ordinateur : relance bornée. */
  private onPipeline(kind: string, stage: string, event: string) {
    console.warn('KyberNow : chaîne de l’ordinateur', kind, stage, event)
    if (event !== 'error' || this.phase !== 'live') return
    if (kind === 'audio') {
      this.showNotice(t('session.notice.soundStopped'))
      return
    }
    if (kind !== 'video') return
    if (this.stableTimer) clearTimeout(this.stableTimer)
    this.stableTimer = null
    if (this.pipelineRestarts >= MAX_PIPELINE_RESTARTS) {
      this.fail(tName('session.error.pipeline', 'host', this.hostname, { max: MAX_PIPELINE_RESTARTS }))
      return
    }
    const n = ++this.pipelineRestarts
    if (this.pipelineTimer) clearTimeout(this.pipelineTimer)
    this.pipelineTimer = setTimeout(() => {
      this.pipelineTimer = null
      this.restartVideo(t('session.notice.imageRestart', { n, max: MAX_PIPELINE_RESTARTS }))
    }, 1000)
  }

  private connectError(message: string): string {
    if (message === 'unauthorized') {
      if (this.g) return t('share.guest.unauthorized', { host: this.hostname })
      // Un ami : son signe de vie dit d'abord si le prêt est fini ; sinon, la connexion s'est interrompue.
      return this.b ? t('session.error.interrupted', { host: this.hostname }) : t('session.error.unauthorized', { host: this.hostname })
    }
    if (isStartFailure(message)) return t('session.error.startFailed', { host: this.hostname })
    return tName('session.error.noReply', 'host', this.hostname)
  }

  /** Connexion coupée sous un invité ou un ami : l'hôte l'a peut-être retiré, arrêté le partage ou repris
   * l'ordinateur (Kyber coupe tout de suite). Le signe de vie dit pourquoi ; sans réponse en 2,5 s,
   * l'échec est dit tel quel. */
  private closedRemotely(message: string) {
    this.heartbeat.now()
    const gen = this.generation
    setTimeout(() => !this.stale(gen) && this.fail(message), 2500)
  }

  private onEngine(e: EngineEvent, src: Engine, protocol: 'kymux' | 'websocket') {
    // Un moteur remplacé ne décide plus de rien.
    if (src !== this.engine) return
    switch (e.type) {
      case 'connected':
        if (this.connectWatch) clearTimeout(this.connectWatch)
        this.connectWatch = null
        this.connectStep = 'image'
        this.watchImage(src, protocol, 'connected')
        break
      case 'reconnecting':
        this.reconnecting = t('session.notice.reconnecting', { attempt: e.attempt, max: e.max })
        void this.refreshTicket()
        break
      case 'auth-required':
        void this.refreshTicket(true)
        break
      case 'reconnected':
        this.reconnecting = ''
        break
      case 'streaming':
        this.reconnecting = ''
        this.watchImage(src, protocol, 'streaming')
        break
      case 'resized':
        this.canvasSize = { width: e.width, height: e.height }
        this.o.view.fit()
        this.guest?.onDecoded({ width: e.width, height: e.height })
        break
      case 'video-stopped':
        // Relance de l'image : l'ancienne toile appartient au moteur, il en faut une neuve ; l'écran visé
        // et la connexion des entrées changent.
        forgetInputSocket()
        this.o.view.freshCanvas()
        this.guest?.forgetDecoded()
        break
      case 'displays':
        this.onDisplays(e.displays, currentDisplayId())
        break
      case 'pipeline':
        this.onPipeline(e.kind, e.stage, e.event)
        break
      case 'gamepad':
        this.gamepads = Math.max(0, this.gamepads + (e.plugged ? 1 : -1))
        if (e.plugged) this.noticePads()
        break
      case 'first-image':
        this.onFirstImage(src)
        break
      case 'error': {
        const detail = redact(e.detail ?? e.message, this.secrets())
        console.warn('KyberNow : connexion impossible', detail)
        // Un jeton refusé ne se répare pas en redémarrant la Station.
        if (e.message === 'unauthorized') {
          // Un invité retiré n'a plus de jeton : son signe de vie dit pourquoi (de même pour un prêt fini).
          if (this.g || this.b) this.closedRemotely(this.connectError(e.message))
          else this.fail(this.connectError(e.message))
        } else if (isStartFailure(e.message)) {
          // L'image ne démarre pas dans ce navigateur : le format d'Auto (PyroWave), H.264, puis sans le son,
          // puis l'échec et sa raison.
          const next = this.plan ? (pyrowaveFallback(this.plan, this.fallbacks) ?? nextFallback(this.plan, this.fallbacks, this.codecList, detail)) : null
          this.reportFailure(detail, next)
          if (next) this.retryWith(next)
          else this.failOrRestart(this.connectError(e.message), detail)
        } else this.failOrRestart(this.connectError(e.message))
        break
      }
      case 'stopped':
        this.onStopped(e.reason)
        break
    }
  }

  /** Aussi après une relance de l'image : la session est alors déjà en cours. */
  private onFirstImage(src: Engine) {
    const first = this.phase !== 'live'
    this.clearWatches()
    this.guest?.endSwitch()
    this.phase = 'live'
    this.reconnecting = ''
    if (this.pendingNotice) {
      this.showNotice(this.pendingNotice)
      this.pendingNotice = ''
    }
    if (first) this.showPyroBanner()
    if (this.reportedFailure && this.plan) {
      // La Station apprend que l'image est partie, et dans quel format.
      this.reportedFailure = false
      this.toStation({ started: { codec: this.plan.codec, audio: this.plan.audio } })
    }
    this.noticePads()
    if (!this.heartbeat.running) this.heartbeat.start(heartbeatBody(null, this.profileLabel(), this.extra()))
    learnDisplayId().then((list) => {
      if (list && !this.displays) this.displays = list.map((d) => ({ id: d.id, name: String(d.name ?? ''), width: Number(d.width) || 0, height: Number(d.height) || 0 }))
    })
    void this.controls.probeAudio(src)
    // Un onglet masqué pendant la relance : le débit plafonné repart avec le nouveau moteur.
    this.controls.applyBitrate()
    // Une image tenue 30 s remet le compte des relances à zéro.
    if (this.stableTimer) clearTimeout(this.stableTimer)
    this.stableTimer = setTimeout(() => (this.pipelineRestarts = 0), 30_000)
    if (!first) return
    // Un invité apprend tout de suite ce que la Station dit de son image (et si elle connaît la qualité).
    if (this.guest) void this.guest.beatNow()
    this.wasLive = true
    this.metrics.firstImage()
    // Pas après une relance : le clavier de l'appareil se fermerait.
    this.o.view.firstImage()
  }

  private onStopped(reason: string) {
    if (this.ended) return
    if (reason === 'restart') {
      // Le moteur a perdu sa toile en se reconnectant : on recommence proprement.
      void this.restart()
    } else if (reason === 'decode_error') {
      // Le moteur a redemandé une image clé, en vain (correctif 0020) : le format d'Auto (PyroWave), ou en
      // HEVC le H.264, une fois (tous) ; puis un invité repasse en 720p30, une fois.
      const next = this.plan ? (pyrowaveFallback(this.plan, this.fallbacks) ?? nextFallback(this.plan, this.fallbacks, this.codecList, reason, true)) : null
      if (next) {
        this.reportFailure(reason, next)
        this.retryWith(next)
      } else if (this.guest?.canFallback()) void this.guest.fallback()
      else {
        this.reportFailure(reason, null)
        this.fail(this.g ? t('share.guest.decode', { host: this.hostname }) : t('session.error.decode'), reason)
      }
    } else if (reason === 'controlplane_closed') {
      const message = t('session.error.closed', { host: this.hostname })
      if (this.g || this.b) this.closedRemotely(message)
      else this.fail(message)
    } else if (reason === 'requested') {
      // Arrêt demandé par ce client, sans que la page l'ait voulu.
      this.fail(t('session.error.stopped'))
    } else {
      const message = t('session.error.interrupted', { host: this.hostname })
      if (this.g || this.b) this.closedRemotely(message)
      else this.fail(message)
    }
  }

  /** Les secrets à retirer d'un message avant de l'écrire (console, Station, écran). */
  private secrets(): string[] {
    return [this.ticketToken, this.g?.join.secret ?? '', this.borrowing?.secret() ?? '']
  }

  /** Un rapport de l'ami ou de l'invité à la Station, dans un signe de vie (`error`, `started`). */
  private toStation(body: Record<string, unknown>) {
    const answer = this.borrowing?.answer
    if (answer) void sendHeartbeat(location.origin, { lend: answer.lend, ...body }, lendTarget(answer.secret))
    else if (this.g) void sendHeartbeat(location.origin, { guest: this.g.join.guest, ...body }, guestTarget(this.g.join.secret))
  }

  /** L'image n'a pas démarré : la raison, ce qui était demandé, et le repli essayé ensuite (docs/pret.md). */
  private reportFailure(reason: string, next: StartFallback | null) {
    if (!this.b && !this.g) return
    this.reportedFailure = true
    this.toStation({
      error: {
        reason: reason.slice(0, 300),
        codec: this.plan?.codec ?? '',
        yuv444: !!this.plan?.yuv444,
        audio: !!this.plan?.audio,
        browser: navigator.userAgent.slice(0, 200),
        next,
      },
    })
  }

  /** Recommence la session avec un repli de plus ; son avis vient avec l'image, une fois. */
  private retryWith(next: StartFallback) {
    console.warn('KyberNow : image non démarrée, nouvel essai', next === 'no_pyrowave' ? 'sans PyroWave' : next === 'h264' ? 'en H.264' : 'sans le son')
    this.pendingNotice =
      next === 'no_pyrowave' ? t('session.notice.pyrowave', { format: this.autoFormat() }) : t(next === 'h264' ? 'session.notice.h264' : 'session.notice.noAudio')
    this.fallbacks = take(this.fallbacks, next)
    void this.restart()
  }

  // ——— Fin ———

  async restart() {
    const e = this.engine
    this.engine = null
    const gen = this.generation
    await retire(e)
    if (!this.stale(gen)) void this.start()
  }

  /** « Se reconnecter » après un échec (hôte) : mesures et avis à zéro. */
  reconnect() {
    this.ended = false
    this.wasLive = false
    this.padsNoticed = false
    this.metrics.reset()
    void this.restart()
  }

  fail(message: string, detail = '') {
    this.generation++
    this.errorDetail = detail
    this.borrowing?.stopWaiting()
    this.guest?.endSwitch()
    this.metrics.keep()
    this.guest?.leave()
    this.clearWatches()
    this.gamepads = 0
    this.controls.soundBlocked = false
    this.error = message
    this.phase = 'ended'
    this.o.view.closeChrome()
    this.heartbeat.stop()
    // Le moteur s'arrête, puis l'écran préparé pour cette session est rendu.
    const e = this.engine
    this.engine = null
    const wasPrepared = this.prepared
    this.prepared = false
    void retire(e, wasPrepared ? () => backend.end(null, this.extra()) : undefined)
  }

  /** Kyber ne se connecte pas ou ne démarre pas l'image alors qu'Admin répond : la Station redémarre une
   * fois (dernier recours, docs/link-ui.md), puis la page reprend la connexion après le mot de passe. Pas
   * pour une image qui part mais n'arrive pas : c'est ce navigateur ou le réseau. */
  private failOrRestart(message: string, detail = '') {
    // Un invité ou un ami ne redémarre jamais la Station.
    if (this.g || this.b || this.wasLive || !lastResortAllowed()) this.fail(message, detail)
    else void this.lastResort(message, detail)
  }

  private async lastResort(message: string, detail = '') {
    const gen = ++this.generation
    this.clearWatches()
    this.phase = 'restarting'
    this.heartbeat.stop()
    // Le moteur s'arrête et l'écran préparé est rendu d'abord : Admin compterait sinon cette session et
    // refuserait de redémarrer.
    const e = this.engine
    this.engine = null
    const wasPrepared = this.prepared
    this.prepared = false
    await retire(e, wasPrepared ? () => backend.end(null, this.extra()) : undefined)
    if (this.stale(gen)) return
    const out = await restartForConnection(() => !this.ended)
    if (this.stale(gen)) return
    // Refusé (un autre appareil en session, trop tôt…) : l'échec d'origine.
    this.fail(out === 'slow' ? t('web.station.slow') : message, out === 'slow' ? '' : detail)
  }

  async quit() {
    if (this.ended) return
    this.ended = true
    this.generation++
    this.o.view.closeChrome()
    this.borrowing?.stopWaiting()
    this.clearWatches()
    this.metrics.keep()
    this.guest?.leave()
    const median = this.metrics.sessionMedian()
    const e = this.engine
    this.engine = null
    // On rend la main tout de suite ; le moteur et la console finissent derrière.
    const wasPrepared = this.prepared
    this.prepared = false
    this.heartbeat.stop()
    releasePointer()
    if (document.fullscreenElement) document.exitFullscreen().catch(() => {})
    // Un ami revient à l'écran d'où il vient : le prêt continue s'il n'est ni rendu ni fini (15 min).
    if (this.borrowing) this.o.onLendEnd(null, this.borrowing.over)
    else this.o.onEnd()
    await retire(e, wasPrepared ? () => backend.end(median, this.extra()) : undefined)
  }

  /** La page se ferme : la dernière session gardée, l'invité sort, l'ami qui attend retire sa demande, et
   * l'écran préparé est rendu (`keepalive`). */
  leavePage() {
    this.metrics.keep()
    this.guest?.leave()
    this.borrowing?.leaveWaiting()
    if (this.ended || !this.prepared) return
    endOnUnload(this.extra())
  }

  /** Démontage de la page de session : tout s'arrête, sans attendre. */
  destroy() {
    this.borrowing?.stopWaiting()
    this.heartbeat.stop()
    this.clearWatches()
    this.guest?.endSwitch()
    this.mic.stop()
    if (this.noticeTimer) clearTimeout(this.noticeTimer)
    if (this.stableTimer) clearTimeout(this.stableTimer)
    if (this.pipelineTimer) clearTimeout(this.pipelineTimer)
    this.pipelineTimer = null
    this.hidePyroBanner()
    this.controls.stopProbe()
    if (!this.ended) {
      this.ended = true
      this.metrics.keep()
      this.guest?.leave()
      void retire(this.engine, this.prepared ? () => backend.end(null, this.extra()) : undefined)
    }
    this.engine = null
  }

  // ——— Jeton, signe de vie, premier plan ———

  /** Jeton Kyber relu sur la console (reconnexion, puis toutes les heures), pour que le moteur ne se
   * reconnecte pas avec un jeton périmé. Un invité ou un ami n'a pas de jeton d'Admin : son signe de vie
   * renouvelle le sien. */
  async refreshTicket(resume = false) {
    if (this.g || this.b) {
      this.heartbeat.now()
      if (resume) this.engine?.updateCredentials(this.ticketToken, true)
      return
    }
    if (!resume && Date.now() - this.ticketAt < 30_000) return
    this.ticketAt = Date.now()
    try {
      const ticket = await backend.ticket()
      this.ticketToken = ticket.token
      this.engine?.updateCredentials(ticket.token, resume)
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) this.fail(this.expired())
    }
  }

  /** La latence du signal de vie (hôte ; le Worker envoie la dernière connue). */
  updateBeat() {
    if (this.g || this.b || !this.engine || this.phase !== 'live') return
    this.heartbeat.update(heartbeatBody(this.engine.sample(), this.profileLabel(), this.extra()))
  }

  // ——— Profil, touches ———

  /** Un profil choisi dans la barre (hôte) : il devient celui de cet ordinateur et s'applique tout de
   * suite, l'image repart avec ses réglages. */
  changeProfile(p: Profile) {
    if (this.g || this.b || this.phase !== 'live') return
    if (this.prefs.profile === p && !isCustom(this.prefs)) return
    pickProfile(this.prefs, p)
    void this.restart()
  }

  /** Une entrée du menu « Touches » : Ctrl + Alt + Suppr par le moteur, le reste tel quel sur la toile. */
  pressKey(k: KeyEntry) {
    if ('cad' in k) this.engine?.ctrlAltDel()
    else sendCombo(canvas(), k)
  }
}
