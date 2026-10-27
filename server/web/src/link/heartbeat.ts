// Signal de vie de la session, toutes les 5 s, depuis un Worker dédié. Dans
// un onglet masqué, les minuteries de la page sont ralenties (une fois par
// minute dans Chrome au bout de cinq minutes) : sans signal de vie pendant
// 20 s, la console abandonnerait la session alors que l'image continue.
// Le Worker est intégré à la page (URL `blob:`), donc de la même origine et
// sous la même isolation ; il reçoit l'adresse complète de la console. Sans
// Worker, ou s'il n'a jamais de réponse, la page envoie elle-même.

import HeartbeatWorker from './heartbeat.worker?worker&inline'
import { HOST_TARGET, sendHeartbeat, type HeartbeatResult, type HeartbeatTarget } from './heartbeat-call'

export type { HeartbeatResult }

export class Heartbeat {
  private worker: Worker | null = null
  private timer: ReturnType<typeof setInterval> | null = null
  private body: unknown = {}
  /** Le Worker a-t-il déjà eu une réponse de la console ? */
  private answered = false
  private silent = 0

  constructor(
    private onResult: (r: HeartbeatResult) => void,
    private period = 5000,
    /** Signal de l'hôte par défaut ; un invité du partage a le sien (`guestTarget`). */
    private target: HeartbeatTarget = HOST_TARGET,
  ) {}

  get running(): boolean {
    return !!this.worker || !!this.timer
  }

  start(body: unknown) {
    this.stop()
    this.body = body
    this.answered = false
    this.silent = 0
    try {
      const w = new HeartbeatWorker()
      w.onmessage = (e: MessageEvent<HeartbeatResult>) => this.fromWorker(e.data)
      w.onerror = () => this.fallback()
      w.postMessage({ type: 'start', origin: location.origin, body, period: this.period, target: this.target })
      this.worker = w
    } catch {
      this.fallback()
    }
  }

  /** Nouveau contenu (latence), envoyé au prochain signal. */
  update(body: unknown) {
    this.body = body
    this.worker?.postMessage({ type: 'body', body })
  }

  /** Un signal tout de suite (onglet qui passe en arrière-plan). */
  now() {
    if (this.worker) this.worker.postMessage({ type: 'now' })
    else if (this.timer) this.beat()
  }

  stop() {
    this.worker?.terminate()
    this.worker = null
    if (this.timer) clearInterval(this.timer)
    this.timer = null
  }

  /** Trois envois sans aucune réponse dès le départ : le Worker ne joint pas la
   * console (environnement qui le bride), la page prend le relais. Une
   * coupure plus tard est celle du réseau, la page n'y ferait pas mieux. */
  private fromWorker(r: HeartbeatResult) {
    if (r.status) this.answered = true
    else if (!this.answered && ++this.silent >= 3) {
      this.fallback()
      return
    }
    this.onResult(r)
  }

  private fallback() {
    this.worker?.terminate()
    this.worker = null
    if (!this.timer) this.timer = setInterval(() => this.beat(), this.period)
  }

  private async beat() {
    this.onResult(await sendHeartbeat(location.origin, this.body, this.target))
  }
}
