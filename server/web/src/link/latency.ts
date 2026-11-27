// Mesures de l'image dans le navigateur, d'après les dates que le moteur web de Kyber donne pour
// chaque image (`live_metrics_callback` : capture, début et fin de l'encodage, envoi, réception,
// décodage, affichage, en microsecondes, déjà ramenées à l'horloge de cette page). Mêmes étapes que
// le cœur des apps (`apps/core/src/latency.rs`, « Trajet d'une image » de docs/link-ui.md) :
// capture, encodage, envoi, réseau, décodage, affichage. Une date absente (Station plus ancienne,
// moteur qui ne la donne pas) laisse l'étape sans mesure.

/** Les dates d'une image, telles que le moteur les passe (µs ; absentes ou nulles : inconnues). */
export interface FrameDates {
  acquired?: number
  encoding?: number
  encoded?: number
  sent?: number
  received?: number
  decoded?: number
  skipped?: number
  prepared?: number
  displayed?: number
}

/** Les six étapes du trajet d'une image, en ms ; `null` : sans mesure. */
export type Chain = [number | null, number | null, number | null, number | null, number | null, number | null]

export const NO_CHAIN: Chain = [null, null, null, null, null, null]

const known = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v) && v > 0

/** `to - from` en ms, ou `null` si une date manque. Le réseau peut passer un rien sous zéro sur un
 * réseau local (horloges recalées) : ramené à zéro, comme dans le cœur. */
function step(from: number | undefined, to: number | undefined): number | null {
  return known(from) && known(to) ? Math.max(0, (to - from) / 1000) : null
}

/** La fin d'une image : montrée, ou à défaut prête à l'être. */
function shownAt(f: FrameDates): number | undefined {
  return known(f.displayed) ? f.displayed : f.prepared
}

/** Latence de la capture à l'écran, en ms ; `null` sans les dates de l'ordinateur ou valeur absurde. */
export function frameLatency(f: FrameDates): number | null {
  const end = shownAt(f)
  if (!known(f.acquired) || !known(end)) return null
  const ms = (end - f.acquired) / 1000
  return ms > 0 && ms < 2000 ? ms : null
}

/** Les six étapes d'une image. */
export function frameChain(f: FrameDates): Chain {
  return [
    step(f.acquired, f.encoding),
    step(f.encoding, f.encoded),
    step(f.encoded, f.sent),
    step(f.sent, f.received),
    step(f.received, f.decoded),
    step(f.decoded, shownAt(f)),
  ]
}

export function median(values: number[]): number | null {
  if (!values.length) return null
  const v = [...values].sort((a, b) => a - b)
  return v[Math.floor(v.length / 2)]
}

/** Ce qu'a donné une seconde d'image (Statistiques, courbes des 30 dernières secondes). */
export interface Second {
  /** Médiane de la latence ; `null` sans mesure. */
  latency: number | null
  chain: Chain
  /** Images montrées. */
  shown: number
  /** Part des images montrées dont la latence est mesurée, en % ; `null` sans image. */
  measuredPct: number | null
  /** Images que le moteur a sautées (jamais montrées). */
  skipped: number
}

interface Sample {
  t: number
  total: number | null
  chain: Chain
}

/** Une étape sur une fenêtre : médiane de ses mesures, `null` s'il n'y en a aucune. */
function chainMedian(samples: Sample[]): Chain {
  return NO_CHAIN.map((_, i) => median(samples.map((s) => s.chain[i]).filter((v): v is number => v !== null))) as Chain
}

/** Les images des dernières secondes : médianes glissantes (barre, signe de vie), seconde écoulée
 * (Statistiques), images par seconde. `now` : horloge de la page (ms), à remplacer dans les essais. */
export class LatencyWindow {
  private samples: Sample[] = []
  private skippedSince = 0

  constructor(private now: () => number = () => performance.now()) {}

  push(f: FrameDates) {
    if (known(f.skipped) && !known(shownAt(f))) {
      this.skippedSince++
      return
    }
    if (!known(shownAt(f))) return
    const t = this.now()
    this.samples.push({ t, total: frameLatency(f), chain: frameChain(f) })
    while (this.samples.length && t - this.samples[0].t > 3000) this.samples.shift()
  }

  private since(ms: number): Sample[] {
    const from = this.now() - ms
    return this.samples.filter((s) => s.t >= from)
  }

  /** Images montrées par seconde, sur les 3 dernières secondes. */
  fps(): number {
    const s = this.samples
    if (s.length < 2) return 0
    const span = (s[s.length - 1].t - s[0].t) / 1000
    return span > 0 ? Math.round((s.length - 1) / span) : 0
  }

  /** Médiane de bout en bout des 3 dernières secondes (5 mesures au moins). */
  median(): number | null {
    const v = this.samples.map((s) => s.total).filter((x): x is number => x !== null)
    return v.length < 5 ? null : median(v)
  }

  /** Médiane de bout en bout de la dernière seconde (3 mesures au moins). */
  lastSecond(): number | null {
    const v = this.since(1000)
      .map((s) => s.total)
      .filter((x): x is number => x !== null)
    return v.length < 3 ? null : median(v)
  }

  /** La seconde écoulée, et les images sautées depuis l'appel précédent (à appeler chaque seconde). */
  second(): Second {
    const s = this.since(1000)
    const measured = s.filter((x) => x.total !== null)
    const skipped = this.skippedSince
    this.skippedSince = 0
    return {
      latency: median(measured.map((x) => x.total as number)),
      chain: chainMedian(s),
      shown: s.length,
      measuredPct: s.length ? Math.round((100 * measured.length) / s.length) : null,
      skipped,
    }
  }
}
