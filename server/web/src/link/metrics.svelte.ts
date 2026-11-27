// Les mesures d'une session : la latence et sa courbe dans la barre, les 30 dernières secondes des
// Statistiques (latence, trajet d'une image, débit visé, images sautées), PyroWave, et la dernière
// session gardée pour la rubrique Statistiques des réglages. L'historique se tient toujours ; l'état
// affiché ne change que quand quelque chose le montre (WEB-18 : barre et statistiques fermées, rien
// n'est à redessiner).

import { median, type Second } from './latency'
import { formatLabel } from './share'
import { MIN_SESSION_SECONDS, saveLastSession } from './prefs'
import { pyrowaveStats, type PyroWaveStats } from './pyrowave/stats'
import type { LiveSession } from './livesession.svelte'

/** Une seconde des Statistiques, avec le débit visé à ce moment-là (Mbit/s ; `null` : inconnu). */
export interface SecondPoint extends Second {
  target: number | null
}

/** Secondes gardées pour les courbes des Statistiques. */
export const HISTORY_SECONDS = 30
/** Médianes de chaque seconde gardées pour la dernière session : 6 h au plus (WEB-18). */
const MAX_SECOND_MEDIANS = 6 * 3600
/** Points de la courbe de la barre. */
const SPARK_POINTS = 32

export class SessionMetrics {
  private s: LiveSession
  /** Latence de bout en bout (médiane des 3 dernières secondes), pour la barre. */
  latency = $state<number | null>(null)
  /** Images montrées par seconde. */
  fps = $state(0)
  /** La courbe de la barre. */
  spark = $state<number[]>([])
  /** Les 30 dernières secondes, la plus récente à la fin (Statistiques ouvertes). */
  seconds = $state<SecondPoint[]>([])
  /** Images sautées depuis le début de la session. */
  skippedTotal = $state(0)
  /** Décodage et images perdues de PyroWave. */
  pyro = $state<PyroWaveStats | null>(null)
  /** Quelque chose montre les mesures (barre, Statistiques, barre tactile). */
  visible: () => boolean = () => false
  /** Les Statistiques sont ouvertes : elles reçoivent chaque seconde. */
  detailed: () => boolean = () => false

  private sparkBuf: number[] = []
  private history: SecondPoint[] = []
  /** Médiane de chaque seconde d'image, et l'heure de la première image : la dernière session et la
   * médiane envoyée à la fin. */
  private medians: number[] = []
  private firstImageAt = 0
  private saved = false

  constructor(s: LiveSession) {
    this.s = s
  }

  /** Toutes les 500 ms pendant l'image : la barre. */
  halfSecond() {
    const e = this.s.engine
    if (!e || this.s.phase !== 'live') return
    const l = e.latency.median()
    if (l !== null) {
      this.sparkBuf.push(l)
      if (this.sparkBuf.length > SPARK_POINTS) this.sparkBuf.shift()
    }
    if (!this.visible()) return
    this.latency = l
    this.fps = e.latency.fps()
    this.pyro = this.s.plan?.pyrowave ? pyrowaveStats() : null
    this.spark = this.sparkBuf.slice()
  }

  /** Chaque seconde pendant l'image : la seconde écoulée. */
  second() {
    const e = this.s.engine
    if (!e || this.s.phase !== 'live') return
    const v = e.latency.lastSecond()
    if (v !== null) this.medians.push(v)
    if (this.medians.length > MAX_SECOND_MEDIANS) this.medians.splice(0, this.medians.length - MAX_SECOND_MEDIANS)
    const point: SecondPoint = { ...e.latency.second(), target: this.s.controls.target()?.mbps ?? null }
    this.skippedTotal += point.skipped
    this.history.push(point)
    if (this.history.length > HISTORY_SECONDS) this.history.shift()
    if (this.detailed()) this.seconds = this.history.slice()
  }

  /** Les Statistiques s'ouvrent : tout de suite les secondes déjà gardées. */
  refresh() {
    this.seconds = this.history.slice()
  }

  firstImage() {
    if (!this.firstImageAt) this.firstImageAt = Date.now()
  }

  /** Médiane des médianes de chaque seconde, et leur 95e centile, comme le cœur des apps. */
  private session(): { median: number; p95: number } | null {
    if (!this.medians.length) return null
    const v = [...this.medians].sort((a, b) => a - b)
    return { median: median(v)!, p95: v[Math.min(v.length - 1, Math.floor(v.length * 0.95))] }
  }

  /** La médiane de la session, envoyée à Admin à la fin. */
  sessionMedian(): number | null {
    return this.session()?.median ?? null
  }

  /** Garde la dernière session (rubrique Statistiques) à partir de 3 s d'image. Un invité ou un ami
   * n'a pas de statistiques à garder : ce n'est pas son ordinateur. */
  keep() {
    const { g, b, mode, plan, canvasSize } = this.s
    if (g || b || this.saved || !this.firstImageAt || !mode || !plan) return
    const stats = this.session()
    const duration = (Date.now() - this.firstImageAt) / 1000
    if (!stats || duration < MIN_SESSION_SECONDS) return
    this.saved = true
    saveLastSession({
      median: stats.median,
      p95: stats.p95,
      duration,
      endedAt: Date.now() / 1000,
      width: canvasSize.width || mode.width,
      height: canvasSize.height || mode.height,
      refresh: mode.refresh,
      codec: formatLabel(plan.codec, plan.yuv444),
      mbps: plan.mbps,
    })
  }

  /** Session suivante (« Se reconnecter » après un échec) : mesures à zéro. */
  reset() {
    this.medians = []
    this.history = []
    this.seconds = []
    this.skippedTotal = 0
    this.firstImageAt = 0
    this.saved = false
  }
}
