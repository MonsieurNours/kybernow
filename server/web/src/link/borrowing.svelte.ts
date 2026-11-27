// L'ami qui emprunte l'ordinateur (docs/pret.md, docs/link-ui.md « Prêt de l'ordinateur ») :
// « Préparation de l'ordinateur », l'attente de l'accord de l'hôte, le signe de vie du prêt (jeton
// renouvelé, fin prévue, préavis de 2 min, fin), le temps qui reste (« Encore 1 h 20 »), « Revenir »
// après une fenêtre fermée et « Rendre l'ordinateur ».

import { t } from '../lib/i18n/index.svelte'
import { leftText } from '../lib/lend'
import { lend as lendApi, leaveLend, LendRefusal, type BorrowAnswer, type BorrowPending, type LendBeat } from './backend'
import { isIsolated } from './device'
import { supportedCodecs } from './engine'
import { lendTarget, type HeartbeatResult } from './heartbeat-call'
import { borrowCaps, clearActive, lendEndText, lendNoticeText, lendRefusalText, markActive, type BorrowStart } from './lend'
import { tName } from './names'
import { preferredProtocol, setRelay } from './plan'
import type { LiveSession } from './livesession.svelte'

export class Borrowing {
  private s: LiveSession
  readonly start: BorrowStart
  /** Réponse de `borrow` (ou de `wait`) : le jeton, l'écran, l'hôte ; renouvelée par « Revenir ». */
  answer = $state<BorrowAnswer | null>(null)
  /** L'hôte et l'ordinateur que dit la réponse `202` (l'attente de l'accord), même avec un code. */
  private asked = $state<{ host?: string; computer?: string } | null>(null)
  /** Le prêt est fini (rendu, repris, terminé) : « Revenir » n'a plus lieu d'être. */
  over = $state(false)
  /** La fin prévue, et celle du préavis en cours (secondes Unix). */
  private until = $state<number | null>(null)
  noticeAt = $state<number | null>(null)
  private noticeKey = ''
  /** L'heure de la page (secondes), relue toutes les 10 s pour le temps qui reste. */
  private clock = $state(Date.now() / 1000)
  /** La demande qui attend l'accord de l'hôte, et de quoi arrêter l'attente. */
  private pending: BorrowPending | null = null
  private abort: AbortController | null = null
  /** Le prêt s'est ouvert une fois dans cette page (un refus de « Revenir » se dit sur la carte de fin). */
  private opened = false

  /** Le nom de l'ordinateur (celui que donne Admin à l'ami, une fois le prêt ouvert ou demandé). */
  readonly computer = $derived(this.answer?.computer ?? this.asked?.computer ?? null)
  /** Prénom de l'hôte qui prête l'ordinateur. */
  readonly host = $derived.by(() => this.answer?.host ?? this.asked?.host ?? this.start.host ?? '')
  /** « Prêté par {hôte} ». */
  readonly title = $derived(this.host ? t('lend.bar.lentBy', { host: this.host }) : t('lend.friend.titleUnknown'))
  /** Titre de l'écran de connexion : « Ordinateur de {hôte} ». */
  readonly startTitle = $derived(this.host ? tName('lend.friend.card', 'host', this.host) : t('lend.friend.titleUnknown'))
  /** « Encore 1 h 20 », ou « Jusqu'à ce que {hôte} reprenne ». */
  readonly timeText = $derived.by(() => {
    const at = this.noticeAt ?? this.until
    return at ? leftText(at - this.clock) : t('lend.bar.untilTaken', { host: this.host })
  })

  constructor(s: LiveSession, start: BorrowStart) {
    this.s = s
    this.start = start
    this.answer = start.answer ?? null
    this.until = start.answer?.until ?? null
  }

  /** Le temps qui reste, relu toutes les 10 s. */
  tick() {
    this.clock = Date.now() / 1000
  }

  /** Le secret du prêt, à retirer des messages écrits. */
  secret(): string {
    return this.answer?.secret ?? ''
  }

  /**
   * Signe de vie (`POST /api/lend/beat`) : le jeton renouvelé, la fin prévue, le préavis de 2 min, ou
   * la fin du prêt. Un prêt inconnu (secret refusé, prêt fini sans nouvelles) : il est terminé.
   */
  onBeat(r: HeartbeatResult) {
    if ([401, 403, 404, 410].includes(r.status)) return this.ended('stopped')
    if (r.status !== 200 || !r.data) return
    const beat = r.data as LendBeat
    if (beat.end) return this.ended(beat.end)
    markActive(this.start.request)
    if (beat.token) {
      this.s.ticketToken = beat.token
      this.s.engine?.updateCredentials(beat.token)
    }
    this.until = beat.until ?? null
    const n = beat.notice && beat.notice.reason !== 'slot_start' ? beat.notice : null
    this.noticeAt = n?.at ?? null
    const key = n ? `${n.reason} ${n.at}` : ''
    const live = this.s.phase === 'live'
    if (key && key !== this.noticeKey && live) this.s.showNotice(lendNoticeText(n!.reason, this.host), 15000)
    if (live || !key) this.noticeKey = key
  }

  /** Le prêt est fini : la carte de fin le dit, avec « Fermer » seulement. */
  private ended(end: NonNullable<LendBeat['end']>) {
    this.over = true
    clearActive()
    this.s.fail(lendEndText(end, this.host))
  }

  /**
   * Emprunte l'ordinateur (`POST /api/lend/borrow`) : l'écran de cet appareil et ce que son décodeur lit.
   * Ordinateur utilisé : la demande attend l'accord de l'hôte (« En attente de {hôte}… », `wait` rappelé
   * tant qu'il attend). `null` : la session a changé ou l'attente est annulée. Refus : `LendRefusal`.
   */
  private async borrow(gen: number): Promise<BorrowAnswer | null> {
    const caps = await borrowCaps()
    if (this.s.stale(gen)) return null
    const abort = new AbortController()
    this.abort = abort
    try {
      const out = await lendApi.borrow(this.start.request, { name: this.start.name, ...caps }, abort.signal)
      if (this.s.stale(gen)) return null
      if (!('request' in out)) return out
      this.pending = out
      this.asked = { host: out.host, computer: out.computer }
      this.s.phase = 'waiting'
      let failures = 0
      while (!abort.signal.aborted && !this.s.stale(gen)) {
        try {
          const answer = await lendApi.wait(out, abort.signal)
          if (answer) return answer
          failures = 0
        } catch (e) {
          if (abort.signal.aborted) return null
          // Le relais ou le réseau hoquette : trois essais, deux secondes d'écart, avant de le dire.
          if (!(e instanceof LendRefusal && (e.kind === 'relay' || e.kind === 'network')) || ++failures >= 3) throw e
          await new Promise((done) => setTimeout(done, 2000))
        }
      }
      return null
    } finally {
      if (this.abort === abort) this.abort = null
      this.pending = null
    }
  }

  /**
   * « Préparation de l'ordinateur » (Admin passe le PC en mode console et crée son écran, 30 s au plus),
   * l'accord de l'hôte s'il le faut, puis la connexion à Kyber avec le jeton du prêt. Le cookie du prêt
   * est posé par la réponse : le moteur de Kyber peut se charger. Un refus avant toute image ramène à
   * l'écran d'avant, qui le dit (`onLendEnd`).
   */
  async open(gen: number) {
    const s = this.s
    if (!isIsolated) {
      s.ended = true
      s.lendEnd(t('web.home.blockedIsolation'), false)
      return
    }
    let answer = this.answer
    if (!answer) {
      try {
        answer = await this.borrow(gen)
      } catch (e) {
        if (s.stale(gen)) return
        const text = lendRefusalText(e, this.host || null)
        clearActive()
        if (!this.opened) {
          s.ended = true
          s.lendEnd(text, false)
        } else {
          // « Revenir » refusé (prêt fini, repris) : la carte de fin le dit.
          this.over = true
          s.fail(text)
        }
        return
      }
      if (!answer || s.stale(gen)) return
      this.answer = answer
    }
    this.opened = true
    s.phase = 'preparing'
    this.until = answer.until
    s.ticketToken = answer.kyber.token
    if (answer.relay) {
      setRelay(true)
      s.relayed = true
    }
    markActive(this.start.request)
    // Le signe de vie part tout de suite : sans nouvelles pendant 20 s, l'image de l'ami s'arrête.
    s.heartbeat.start({ lend: answer.lend }, { target: lendTarget(answer.secret), period: Math.max(1, answer.beat) * 1000 })
    let codecs
    try {
      codecs = await supportedCodecs()
    } catch {
      // Le moteur de Kyber ne se charge pas (Kyber arrêté) : « Revenir » le recharge.
      if (!s.stale(gen)) s.fail(t('web.home.blockedEngine'))
      return
    }
    if (s.stale(gen)) return
    s.codecList = codecs
    s.mode = { width: answer.display.width, height: answer.display.height, refresh: answer.display.refresh }
    s.engineReadsText()
    await s.connect(preferredProtocol(), gen)
  }

  /** L'écran virtuel de l'ami, que vise le moteur. */
  displayId(): number | undefined {
    return this.answer?.display.id
  }

  /** « Annuler » pendant l'attente de l'accord : la demande est retirée chez l'hôte, et l'écran d'avant revient. */
  cancel() {
    const p = this.pending
    this.abort?.abort()
    if (p) void leaveLend(p.secret, { request: p.request })
    void this.s.quit()
  }

  /** La page se ferme pendant l'attente de l'accord : la demande est retirée. */
  leaveWaiting() {
    if (this.pending) void leaveLend(this.pending.secret, { request: this.pending.request })
  }

  /** « Rendre l'ordinateur », confirmé : le prêt finit pour de bon (`leave`), la page revient à la carte. */
  giveBack() {
    if (this.answer) void leaveLend(this.answer.secret, { lend: this.answer.lend })
    this.over = true
    clearActive()
    void this.s.quit()
  }

  /** « Revenir » après une fenêtre fermée ou un réseau perdu : le même prêt, de nouveau par `borrow`. */
  comeBack() {
    this.s.ended = false
    this.s.wasLive = false
    this.answer = null
    void this.s.restart()
  }

  /** La session s'arrête : l'attente de l'accord aussi. */
  stopWaiting() {
    this.abort?.abort()
  }
}
