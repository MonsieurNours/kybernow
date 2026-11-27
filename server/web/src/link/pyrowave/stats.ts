// Mesures des décodeurs PyroWave de la page (une session à la fois), à part de
// `live.ts` : la session les lit sans charger le décodeur (chargé à la demande).

export interface PyroWaveStats {
  /** Temps de décodage médian de la dernière seconde (arrivée → image prête), en ms. */
  decodeMs: number | null
  /** Images perdues ou sautées sur les 10 dernières secondes, en % des images attendues. */
  lostPercent: number | null
}

interface Sample {
  at: number
  decodeMs: number
}

const window_: { samples: Sample[]; events: { at: number; lost: number; ok: number }[] } = { samples: [], events: [] }

/** Une image décodée en `ms`. */
export function sample(ms: number) {
  window_.samples.push({ at: performance.now(), decodeMs: ms })
  if (window_.samples.length > 600) window_.samples.splice(0, 300)
}

/** `lost` images perdues ou sautées, `ok` décodées. */
export function note(lost: number, ok: number) {
  const at = performance.now()
  window_.events.push({ at, lost, ok })
  while (window_.events.length && at - window_.events[0].at > 10_000) window_.events.shift()
}

export function pyrowaveStats(): PyroWaveStats {
  const now = performance.now()
  const recent = window_.samples.filter((s) => now - s.at < 1000).map((s) => s.decodeMs)
  recent.sort((a, b) => a - b)
  const events = window_.events.filter((e) => now - e.at < 10_000)
  const lost = events.reduce((a, e) => a + e.lost, 0)
  const ok = events.reduce((a, e) => a + e.ok, 0)
  return {
    decodeMs: recent.length ? recent[Math.floor(recent.length / 2)] : null,
    lostPercent: lost + ok > 0 ? (100 * lost) / (lost + ok) : null,
  }
}

/** Mesures remises à zéro (nouvelle session). */
export function resetStats() {
  window_.samples = []
  window_.events = []
}

/** Perte de la carte graphique pendant une session PyroWave (WEB-01) : la session relance l'image. */
const lostListeners = new Set<() => void>()

export function onGpuLost(cb: () => void): () => void {
  lostListeners.add(cb)
  return () => lostListeners.delete(cb)
}

export function gpuLost() {
  for (const cb of lostListeners) cb()
}
