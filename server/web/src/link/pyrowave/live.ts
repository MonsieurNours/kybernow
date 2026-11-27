// Le décodeur PyroWave des sessions : le moteur web de Kyber (kywebplayer,
// correctif 0025-kyctlweb) lui passe les paquets `pwv1` au lieu de WebCodecs,
// par `globalThis.__kybernowPyroWave.create(output)`. Chaque image décodée
// est dessinée dans une toile WebGPU hors écran, puis rendue au moteur en
// VideoFrame par `output` : il la dessine dans la toile de la session comme
// une image de WebCodecs (même cadence, mêmes mesures).

import { configFromSequenceHeader, isConfig as isConfigPacket, parseConfig, sameConfig, type HdrMetadata, type PyroWaveConfig } from './bitstream'
import { PyroWaveDecoder, type PyroWaveGPU } from './decoder'
import { probePyroWave, pyrowaveGPU } from './probe'
import { gpuLost, note, resetStats, sample } from './stats'

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
  private lostReported = false

  constructor(
    private gpu: PyroWaveGPU,
    private output: (frame: VideoFrame) => void,
  ) {}

  private apply(config: PyroWaveConfig) {
    // Les métadonnées HDR10 (en-tête version 2) ne changent pas le décodeur : la toile est SDR, l'image HDR y est
    // ramenée par une courbe fixe (shaders.ts, celle de la Station), qui ne dépend pas de la crête du contenu.
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
        `KyberNow : décodeur PyroWave ${config.width}x${config.height} ${config.chroma444 ? '4:4:4' : '4:2:0'} ${config.hdr ? 'HDR10' : 'SDR'} ${config.fullRange ? 'pleine' : 'réduite'}${this.gpu.subgroups ? ', sous-groupes' : ''}${config.hdr && config.metadata ? `, ${describeHdr(config.metadata)}` : ''}`,
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
    // Paquet de configuration, annoncé comme tel ou arrivé dans le flux des images (`KNPW`) : jamais décodé en image.
    if (isConfig || isConfigPacket(data)) {
      const c = parseConfig(data)
      if (c) this.apply(c)
      else this.incident('paquet de configuration illisible')
      return false
    }
    // Sans paquet de configuration (perdu, Station plus ancienne) : l'en-tête du flux.
    if (!this.decoder) {
      const fallback = configFromSequenceHeader(data)
      if (fallback) {
        console.info('KyberNow : PyroWave, pas de paquet de configuration, définition lue dans le flux')
        this.apply(fallback)
      }
    }
    const gap = this.countGap(pts)
    const dec = this.decoder
    if (this.gpu.lost) {
      // Carte graphique perdue (veille, pilote réinitialisé, processus GPU du navigateur) : la session relance
      // l'image, avec un nouveau device (WEB-01), au lieu de garder une image figée sans rien dire.
      note(gap + 1, 0)
      if (!this.lostReported) {
        this.lostReported = true
        console.warn('KyberNow : PyroWave, carte graphique perdue ; relance de l’image')
        gpuLost()
      }
      return false
    }
    if (!dec || !this.ctx || !this.canvas) {
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
      sample(performance.now() - t0)
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
    }).catch(() => {
      // Device perdu pendant l'image (WEB-02) : l'image n'est pas rendue, son bitmap est fermé.
      bitmap.close()
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

/** Pour le journal : « mastering 0,0050-1000 nits, MaxCLL 1000, MaxFALL 400 » (champs présents seulement). */
function describeHdr(m: HdrMetadata): string {
  const parts: string[] = []
  if (m.mastering) parts.push(`mastering ${m.mastering.minNits}-${m.mastering.maxNits} nits`)
  if (m.maxCll !== null) parts.push(`MaxCLL ${m.maxCll}, MaxFALL ${m.maxFall}`)
  return parts.length ? parts.join(', ') : 'sans métadonnées HDR'
}

/** Sonde au chargement ; si elle passe, installe le décodeur que le moteur
 * cherche (`__kybernowPyroWave`). Rend le résultat de la sonde. */
export async function installPyroWave() {
  const result = await probePyroWave()
  let gpu = result.ok ? await pyrowaveGPU() : null
  if (gpu) {
    let renewing: Promise<void> | null = null
    // Le device perdu est remplacé aussitôt, en fond (WEB-01) : la relance de l'image qui suit le prend ; avant, le
    // premier device restait pour toute la page et toutes les sessions suivantes tombaient en H.264.
    const renew = (): Promise<void> => {
      renewing ??= pyrowaveGPU()
        .then((g) => {
          gpu = g
          if (g) watch(g)
        })
        .catch(() => {
          gpu = null
        })
        .finally(() => {
          renewing = null
        })
      return renewing
    }
    const watch = (g: PyroWaveGPU) => {
      g.device.lost.then(() => {
        if (gpu === g) renew()
      })
    }
    watch(gpu)
    ;(globalThis as any).__kybernowPyroWave = {
      create(output: (frame: VideoFrame) => void) {
        // Device perdu, pas encore remplacé : cette image part dans le format de repli ; la suivante reprend PyroWave.
        if (!gpu || gpu.lost) {
          renew()
          return null
        }
        resetStats()
        return new LiveDecoder(gpu, output)
      },
      /** Le nouveau device est prêt (ou n'a pas pu l'être). */
      ready(): Promise<void> {
        return renewing ?? Promise.resolve()
      },
    }
  }
  return result
}

/** Attend le remplacement d'un device perdu, `ms` au plus (la session relance l'image ensuite). */
export function pyrowaveReady(ms: number): Promise<void> {
  const r: Promise<void> | undefined = (globalThis as any).__kybernowPyroWave?.ready?.()
  return Promise.race([r ?? Promise.resolve(), new Promise<void>((ok) => setTimeout(ok, ms))])
}
