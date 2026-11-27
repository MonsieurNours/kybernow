// La session d'un invité du partage (KyberNow Share, docs/kybernow-share.md ; docs/link-ui.md
// « Partage ») : son rôle, changé par l'hôte en cours de partage ; son signe de vie (jeton renouvelé,
// écran du partage, image, fin) ; « Qualité de l'image », avec les étapes jusqu'à la nouvelle image ;
// l'image ajustée par la Station ; le dernier recours en 720p30 ; la sortie du partage. Les règles sans
// état sont dans guestrules.ts.

import { ApiError, type GuestImage, type ShareQuality, type ShareRole } from '../lib/api'
import { t } from '../lib/i18n/index.svelte'
import { backend, type AutoCaps, type GuestBeat, type GuestJoin } from './backend'
import { retire } from './engine'
import { guestTarget, sendHeartbeat, type HeartbeatResult } from './heartbeat-call'
import { forgetInputSocket } from './inputs'
import type { Plan } from './plan'
import { lightened, near, qualityStep, rights, roleNotices, imageNotice } from './guestrules'
import { autoCaps, guestFormat, leaveShare, qualityName, redact, saveQuality, tokenPreset, type GuestStart } from './share'
import { imageQuality } from '../lib/format'
import type { LiveSession } from './livesession.svelte'

type Size = { width: number; height: number }

export class GuestSession {
  private s: LiveSession
  readonly start: GuestStart
  /** Droits de l'invité : son rôle, changé en cours de partage par le signe de vie. */
  role = $state<ShareRole>('spectator')
  /** « Qualité de l'image » : celle qu'il demande (retenue quand il la choisit). */
  quality = $state<ShareQuality>('auto')
  /** Ce que la Station dit de son image (`null` avant sa première réponse). */
  image = $state<GuestImage | null>(null)
  /** La Station connaît la qualité d'un invité (son signe de vie donne `image`) : le choix est dans la barre. */
  qualityKnown = $state(false)
  qualityBusy = $state(false)
  /** Qualité changée en cours de session : les étapes (« Premières images ») jusqu'à la nouvelle image. */
  switching = $state(false)
  private switchTarget: Size | null = null
  private switchTimer: ReturnType<typeof setTimeout> | null = null
  /** Taille de l'image décodée (`resized`) ; `null` après une relance de l'image. */
  private decoded: Size | null = null
  /** Dernier avis d'image ajustée : un avis par changement, pas à chaque signe de vie. */
  private imageNoticeKey = ''
  /** Dernier recours après une erreur de décodage : déjà repassé en 720p30. */
  private fallbackTried = false
  /** Écran du partage qu'il reçoit (il change si l'écran de l'hôte est recréé). */
  display: GuestJoin['display']
  /** Déjà sorti du partage (`share/leave` envoyé). */
  private left = false

  readonly canPlay = $derived(rights(this.role).pads)
  readonly canKbm = $derived(rights(this.role).kbm)
  /** « Spectateur » ou « Joueur ». */
  readonly roleText = $derived(this.role === 'spectator' ? t('share.guest.spectator') : t('share.guest.player'))
  /** La qualité reçue (« 1080p · 60 Hz »), celle demandée tant que Kyber ne la dit pas. */
  readonly receivedText = $derived(imageQuality(this.image) || qualityName(this.quality))

  constructor(s: LiveSession, start: GuestStart) {
    this.s = s
    this.start = start
    this.role = start.join.role
    this.quality = start.quality
    this.display = start.join.display
  }

  /** Signe de vie : son rôle, le jeton renouvelé, l'écran du partage, ou la fin. */
  onBeat(r: HeartbeatResult) {
    const s = this.s
    // Parti (sorti sans nouvelles, partage rouvert) : comme un partage arrêté.
    if (r.status === 404) return this.endedByHost('stopped')
    // Station injoignable, ou passagèrement sans clé (503) : le signe suivant le dira.
    if (r.status !== 200 || !r.data) return
    const beat = r.data as GuestBeat
    if (beat.end) return this.endedByHost(beat.end)
    s.recording.applyBeat(beat.recording)
    if (beat.role && beat.role !== this.role) this.changeRole(beat.role)
    if (beat.token) s.ticketToken = beat.token
    if (beat.image) {
      this.qualityKnown = true
      this.onImage(beat.image)
    }
    const display = beat.display
    const moved = !!display && display.id !== this.display.id
    if (display) {
      // Le format annoncé suit l'écran de l'hôte (taille et fréquence : `display_list_updated` relance l'image).
      if (display.width !== this.display.width || display.height !== this.display.height || display.refresh !== this.display.refresh) {
        s.mode = { width: display.width, height: display.height, refresh: display.refresh }
      }
      this.display = display
    }
    if (moved) {
      // L'hôte s'est préparé de nouveau : son écran a un autre identifiant, que Kyber ne prend qu'à une
      // nouvelle connexion, avec le jeton renouvelé. L'écran que visent les positions et la liste de
      // Kyber sont à réapprendre (sinon les positions iraient à l'ancien écran).
      if (s.phase === 'live' || s.phase === 'connecting') {
        forgetInputSocket()
        s.displays = null
        void s.restart()
      }
    } else if (beat.token) {
      s.engine?.updateCredentials(beat.token)
    }
  }

  /** La Station dit pourquoi l'invité est sorti : l'hôte a arrêté le partage ou l'a retiré. */
  private endedByHost(end: 'stopped' | 'removed') {
    this.s.fail(t(end === 'removed' ? 'share.guest.removed' : 'share.guest.stopped', { host: this.start.join.host }))
  }

  /** Un signe de vie tout de suite, sans attendre le Worker : le jeton renouvelé et l'image. */
  async beatNow(): Promise<HeartbeatResult> {
    const r = await sendHeartbeat(location.origin, { guest: this.start.join.guest }, guestTarget(this.start.join.secret))
    if (!this.s.ended && this.s.phase !== 'ended') this.onBeat(r)
    return r
  }

  /** Ce que la Station dit de son image ; quand elle l'ajuste, un avis bref, une fois par changement. */
  private onImage(next: GuestImage) {
    this.image = next
    const { key, text } = imageNotice(next)
    if (key === this.imageNoticeKey) return
    this.imageNoticeKey = key
    // Pas d'avis pour une image demandée, ni deux fois (l'image décodée a déjà rapetissé).
    if (!text || this.s.notice === text) return
    if (this.s.phase === 'live' && !this.switching) this.s.showNotice(text)
    else this.s.pendingNotice = text
  }

  /**
   * L'image décodée a changé de taille. Pendant un changement de qualité, c'est la nouvelle image qui
   * arrive. Sinon, une image plus petite que l'invité n'a pas demandée : « Image allégée pour suivre. »
   * tout de suite, la raison exacte au signe de vie suivant, demandé aussitôt.
   */
  onDecoded(size: Size) {
    const before = this.decoded
    this.decoded = size
    if (this.switching) {
      if (!this.switchTarget || near(size, this.switchTarget)) this.endSwitch()
      return
    }
    const hostScreens = [this.display, ...(this.s.displays ?? [])]
    if (this.s.phase === 'live' && lightened(before, size, hostScreens)) {
      this.s.showNotice(t('share.guest.lightened'))
      void this.beatNow()
    }
  }

  /** L'image est relancée : sa taille sera de nouveau apprise. */
  forgetDecoded() {
    this.decoded = null
  }

  /** Les étapes jusqu'à la nouvelle image : elle arrive dans le flux (Kyber déplace l'image de l'invité :
   * en-têtes puis image clé). Pas de nouvelle taille en 4 s : l'image est relancée proprement. */
  private startSwitch(target: Size | null) {
    this.endSwitch()
    this.switching = true
    this.switchTarget = target
    const e = this.s.engine
    this.switchTimer = setTimeout(() => {
      if (!this.switching || this.s.engine !== e || !e) return
      console.warn('KyberNow : nouvelle image pas arrivée dans le flux ; image relancée')
      e.restartVideo()
      // La relance a son propre délai (5 s, puis la session entière) : les étapes ne restent pas au-delà.
      this.switchTimer = setTimeout(() => this.endSwitch(), 10_000)
    }, 4000)
  }

  endSwitch() {
    if (this.switchTimer) clearTimeout(this.switchTimer)
    this.switchTimer = null
    this.switching = false
    this.switchTarget = null
  }

  /** Le format que l'invité demande à Kyber, d'après le préréglage de son jeton (`kn_quality`). */
  private formatNow(): { codec: 'h264' | 'hevc'; yuv444: boolean } | null {
    const preset = tokenPreset(this.s.ticketToken)
    return guestFormat(this.start.join.codec, this.start.join.yuv444, this.s.codecList, preset === null || preset === 'host')
  }

  /** L'invité demande le format de l'hôte s'il le lit (il partage alors son image), sinon un format qu'il
   * lit : Kyber lui fait une image à part (docs/kybernow-share.md, « L'image d'un invité »). Le 4:4:4
   * seulement pour l'image de l'hôte (`host` dans son jeton) ; jamais de presse-papiers. Image de l'hôte
   * pas encore démarrée : le format que ce navigateur lit le mieux (l'hôte garde le sien). */
  plan(p: Plan): Plan {
    const preset = tokenPreset(this.s.ticketToken)
    const format = this.formatNow()
    if (!format) return { ...p, yuv444: p.yuv444 && (preset === null || preset === 'host'), clipboard: false }
    return { ...p, ...format, clipboard: false }
  }

  /** `POST /api/share/quality` : un changement trop rapproché (429, un par seconde et par invité) est refait
   * après le délai donné par la Station, sans avis. `null` : la session a changé pendant l'attente. */
  private async askQuality(gen: number, q: ShareQuality, auto?: AutoCaps): Promise<GuestImage | null> {
    for (let attempt = 1; ; attempt++) {
      try {
        return await backend.shareQuality(this.start.join, q, auto)
      } catch (e) {
        if (!(e instanceof ApiError) || e.status !== 429 || attempt >= 5) throw e
        await new Promise((resolve) => setTimeout(resolve, Math.min(e.retryAfter ?? 1, 10) * 1000))
        if (this.s.stale(gen)) return null
      }
    }
  }

  /**
   * « Qualité de l'image » changée dans la barre : retenue pour la fois suivante, envoyée à la Station,
   * qui fait relancer à Kyber l'image de l'invité seulement. Le jeton renouvelé porte le nouveau
   * préréglage ; ce qu'il faut ensuite à l'écran : voir `qualityStep`.
   */
  async pickQuality(q: ShareQuality) {
    const s = this.s
    if (q === this.quality || this.qualityBusy || s.phase !== 'live') return
    const before = this.quality
    const presetBefore = tokenPreset(s.ticketToken)
    const gen = s.generation
    this.quality = q
    this.qualityBusy = true
    try {
      const auto = q === 'auto' ? await autoCaps() : undefined
      const next = await this.askQuality(gen, q, auto)
      saveQuality(q)
      if (!next || s.stale(gen)) return
      this.onImage(next)
      await this.beatNow()
      if (s.stale(gen) || s.phase !== 'live' || !s.engine || !s.plan) return
      const format = this.formatNow()
      const step = qualityStep(presetBefore, tokenPreset(s.ticketToken), format, s.plan, next, this.decoded)
      if (step === 'none') return
      if (step === 'restart') {
        s.plan = { ...s.plan, ...format! }
        s.engine.updateSettings(s.plan)
        this.startSwitch(null)
        s.engine.restartVideo()
        return
      }
      this.startSwitch(step.target)
    } catch (e) {
      console.warn('KyberNow : qualité non changée', redact(e instanceof Error ? e.message : e, [this.start.join.secret, s.ticketToken]))
      if (s.stale(gen)) return
      this.quality = before
      s.showNotice(t('share.quality.failed'))
    } finally {
      this.qualityBusy = false
    }
  }

  /** Le dernier recours en 720p30 reste possible (pas encore pris, pas déjà en 720p30). */
  canFallback(): boolean {
    return !this.fallbackTried && tokenPreset(this.s.ticketToken) !== '720p30'
  }

  /**
   * Dernier recours d'un invité dont ce navigateur ne décode pas l'image (le moteur a redemandé une image
   * clé, en vain) : il rejoint de nouveau son image en 720p30 (sa qualité retenue ne change pas), avec
   * l'avis « Image allégée pour suivre. ». Une seconde fois : l'échec est dit.
   */
  async fallback() {
    const s = this.s
    this.fallbackTried = true
    const gen = s.generation
    const e = s.engine
    s.engine = null
    s.clearWatches()
    this.endSwitch()
    s.phase = 'connecting'
    s.connectStep = 'image'
    await retire(e)
    if (s.stale(gen)) return
    try {
      const next = await this.askQuality(gen, '720p30')
      if (!next) return
      this.onImage(next)
      // Le jeton renouvelé porte 720p30 (`kn_quality`) : Kyber le prend à la nouvelle connexion.
      await this.beatNow()
    } catch {
      if (!s.stale(gen)) s.fail(t('share.guest.decode', { host: s.hostname }))
      return
    }
    if (s.stale(gen)) return
    this.quality = '720p30'
    s.pendingNotice = t('share.guest.lightened')
    void s.start()
  }

  /** Droits changés par l'hôte : un avis bref par droit donné ou repris ; ce que le nouveau rôle ne
   * permet plus est relâché par la session (ses effets suivent `canKbm` et `canPlay`). */
  private changeRole(next: ShareRole) {
    const keys = roleNotices(this.role, next)
    this.role = next
    if (keys.length) this.s.showNotice(keys.map((k) => t(k)).join(' '))
  }

  /** Sort du partage (une seule fois) : « Quitter le partage », fermeture de la page, échec. */
  leave() {
    if (this.left) return
    this.left = true
    void leaveShare(this.start.join)
  }
}
