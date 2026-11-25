// Le décodeur PyroWave des sessions : le moteur web de Kyber (kywebplayer,
// correctif 0025-kyctlweb) lui passe les paquets `pwv1` au lieu de WebCodecs,
// par `globalThis.__kybernowPyroWave.create(output)`. Chaque image décodée
// est dessinée dans une toile WebGPU hors écran, puis rendue au moteur en
// VideoFrame par `output` : il la dessine dans la toile de la session comme
// une image de WebCodecs (même cadence, mêmes mesures).

import { configFromSequenceHeader, parseConfig, sameConfig, type PyroWaveConfig } from './bitstream'
import { PyroWaveDecoder, type PyroWaveGPU } from './decoder'
import { probePyroWave, pyrowaveGPU } from './probe'

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

/** Ce que mesurent les décodeurs de la page (une session à la fois). */
const window_: { samples: Sample[]; events: { at: number; lost: number; ok: number }[] } = { samples: [], events: [] }

function note(lost: number, ok: number) {
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
export function resetPyroWaveStats() {
  window_.samples = []
  window_.events = []
}

class LiveDecoder {
  private config: PyroWaveConfig | null = null
  private decoder: PyroWaveDecoder | null = null
  private canvas: OffscreenCanvas | null = null
  private ctx: GPUCanvasContext | null = null
  private format: GPUTextureFormat = 'bgra8unorm'
  private closed = false
  private lastPts: number | null = null
  /** Intervalle entre deux images (pts), lissé : sert à compter les images perdues. */
  private interval: number | null = null
  private incidents = 0

  constructor(
    private gpu: PyroWaveGPU,
    private output: (frame: VideoFrame) => void,
  ) {}

  private apply(config: PyroWaveConfig) {
    if (sameConfig(config, this.config) && this.decoder) return
    this.decoder?.destroy()
    this.decoder = null
    this.config = config
    try {
      this.decoder = new PyroWaveDecoder(this.gpu, config)
      if (!this.canvas) {
        this.canvas = new OffscreenCanvas(config.width, config.height)
        this.ctx = this.canvas.getContext('webgpu') as GPUCanvasContext | null
        this.format = navigator.gpu.getPreferredCanvasFormat()
      } else {
        this.canvas.width = config.width
        this.canvas.height = config.height
      }
      this.ctx?.configure({ device: this.gpu.device, format: this.format, alphaMode: 'opaque' })
      console.info(
        `KyberNow : décodeur PyroWave ${config.width}x${config.height} ${config.chroma444 ? '4:4:4' : '4:2:0'} ${config.hdr ? 'HDR10' : 'SDR'} ${config.fullRange ? 'pleine' : 'réduite'}${this.gpu.subgroups ? ', sous-groupes' : ''}`,
      )
    } catch (e) {
      console.warn('KyberNow : décodeur PyroWave refusé', e)
      this.decoder = null
    }
  }

  private incident(what: string) {
    this.incidents++
    if (this.incidents === 1 || this.incidents % 120 === 0) console.warn(`KyberNow : PyroWave, ${what} (${this.incidents})`)
  }

  /** Images perdues en route : trou dans les pts plus grand qu'un intervalle et demi. */
  private countGap(pts: number): number {
    const last = this.lastPts
    this.lastPts = pts
    if (last === null || pts <= last) return 0
    const d = pts - last
    if (this.interval === null) {
      this.interval = d
      return 0
    }
    const lost = d > this.interval * 1.5 ? Math.round(d / this.interval) - 1 : 0
    if (!lost) this.interval = this.interval * 0.9 + d * 0.1
    return Math.min(lost, 120)
  }

  /** Vrai si une image sortira par `output` (le moteur compte ses dates d'arrivée). */
  push(data: Uint8Array, pts: number, isConfig: boolean): boolean {
    if (this.closed) return false
    if (isConfig) {
      const c = parseConfig(data)
      if (c) this.apply(c)
      else this.incident('paquet de configuration illisible')
      return false
    }
    // Sans paquet de configuration (perdu, Station plus ancienne) : l'en-tête du flux.
    if (!this.decoder && parseConfig(data)) {
      this.apply(parseConfig(data)!)
      return false
    }
    if (!this.decoder) {
      const fallback = configFromSequenceHeader(data)
      if (fallback) {
        console.info('KyberNow : PyroWave, pas de paquet de configuration, définition lue dans le flux')
        this.apply(fallback)
      }
    }
    const gap = this.countGap(pts)
    const dec = this.decoder
    if (!dec || !this.ctx || !this.canvas || this.gpu.lost) {
      note(gap + 1, 0)
      return false
    }
    const t0 = performance.now()
    const s = dec.submit(data, this.ctx, this.format)
    if (s.status !== 'decoded') {
      this.incident(s.status === 'busy' ? 'carte graphique en retard, image sautée' : s.status === 'incomplete' ? 'image incomplète, sautée' : 'image illisible, sautée')
      note(gap + 1, 0)
      return false
    }
    note(gap, 1)
    // Le contenu de la toile, pris tout de suite (la commande est soumise) ;
    // rendu au moteur quand la carte graphique a fini.
    const bitmap = this.canvas.transferToImageBitmap()
    s.done!.then(() => {
      const ms = performance.now() - t0
      window_.samples.push({ at: performance.now(), decodeMs: ms })
      if (window_.samples.length > 600) window_.samples.splice(0, 300)
      let frame: VideoFrame | null = null
      try {
        frame = new VideoFrame(bitmap, { timestamp: pts })
        bitmap.close()
        if (this.closed) {
          frame.close()
          return
        }
        this.output(frame)
      } catch (e) {
        frame?.close()
        bitmap.close()
        if (!this.closed) this.incident(`image non rendue (${e})`)
      }
    })
    return true
  }

  close() {
    this.closed = true
    this.decoder?.destroy()
    this.decoder = null
    this.ctx?.unconfigure()
  }
}

/** Sonde au chargement ; si elle passe, installe le décodeur que le moteur
 * cherche (`__kybernowPyroWave`). Rend le résultat de la sonde. */
export async function installPyroWave() {
  const result = await probePyroWave()
  const gpu = result.ok ? await pyrowaveGPU() : null
  if (gpu) {
    ;(globalThis as any).__kybernowPyroWave = {
      create(output: (frame: VideoFrame) => void) {
        // Le device de la sonde (recréé s'il a été perdu entre-temps : il l'est
        // alors au prochain démarrage de l'image).
        if (gpu.lost) {
          pyrowaveGPU().catch(() => {})
          return null
        }
        resetPyroWaveStats()
        return new LiveDecoder(gpu, output)
      },
    }
  }
  return result
}
