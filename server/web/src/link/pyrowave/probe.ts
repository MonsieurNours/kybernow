// Sonde PyroWave du client web (docs/pyrowave-client-web.md) : WebGPU présent,
// un adaptateur, et le décodage pour de bon d'une image de test, relue et
// vérifiée. `pyrowave` n'est annoncé que si elle passe. Le device gardé sert
// ensuite aux sessions.

import { PyroWaveDecoder, PyroWaveGPU } from './decoder'

export type ProbeFailure = 'webgpu' | 'adapter' | 'decode'

export interface ProbeResult {
  ok: boolean
  /** Ce qui manque, quand `ok` est faux. */
  reason?: ProbeFailure
  /** Variante à sous-groupes de la déquantification prise. */
  subgroups?: boolean
  adapter?: string
  /** Durée de la sonde (device, shaders, image de test), en ms. */
  ms?: number
}

/** Image de test du client Mac (apps/macos/KyberKit/PyroWaveDecoder.swift) :
 * 128 × 128, 4:2:0, rampe horizontale de Y de 16 à 235 (plage réduite), Cb et
 * Cr à 128 ; encodée par l'encodeur Metal du port, 1 812 octets. */
const sample =
  'f8AfkAsAAAABABUQgAAAAA8AWAPPz8zM/PwD8A8wA8/PzMz8/APwDzAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA/PwABABEQgAEAAA8AVsDA8wDwDPPwA8DA8wDwDPPwAwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADAwAAEABRCABAAADwBw////////AAAAAQAFEIAIAAAPAHD///////8AAAABABMQgAwAAFX/VQMAAAwDMwMAAAwDMwMAAAwDMwMAAAwDM8DA8ADwADD8wMDwAPAAMPzAwPAA8AAw/MDA8ADwADD8MAzDDMMwAAAAMwAjEHgVAAD/VQD//1UA/1BTUFMDA8MDA8MDA8MDA8MzMzMzAAwDAAwDAAwDAAwDwMDwwAAAwMDwwAAAwMDwwAAAwMDwwAAAAwPDAwPDAwPDAwPDMzMzMwAMAwAMAwAMAwAMA8DA8MAAAMDA8MAAAMDA8MAAAMDA8MAAAAAAzMwzMzMzAADMzDMzMzP//1cQcB4AAKpVVVVVVQD/qlVVVVVVAP+qVVVVVVUA/6pVVVVVVQD/UHBwYVBwcGFQcHBhUHBwYc8zzzPPM88z//////Pz8/P8/Pz8PDw8PM/Pz8//////wMDAA8DAwAPAwMADwMDAA88zzzPPM88z//////Pz8/P8/Pz8PDw8PM/Pz8//////wMDAA8DAwAPAwMADwMDAA88zzzPPM88z//////Pz8/P8/Pz8PDw8PM/Pz8//////wMDAA8DAwAPAwMADwMDAA88zzzPPM88z//////Pz8/P8/Pz8PDw8PM/Pz8//////wMDAA8DAwAPAwMADwMDAAwAAAAD8/Pz8wzAMMAzD//8AAAD/////MzMAAAAA/Pz8/MMwDDAMw///AAAA/////zMzAAAAAPz8/PzDMAwwDMP//wAAAP////8zMwAAAAD8/Pz8wzAMMAzD//8AAAD/////MzP//z8QaCcAAFVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVQEBAQEBAQEBAQEBAQEBAQMzMzMzMzMzMPDw8PDMzMzPz8/PzzMzMzMzMzMw8PDw8zMzMzMzMzMw8PDw8MzMzM/Pz8/PMzMzMzMzMzDw8PDzMzMzMzMzMzDw8PDwzMzMz8/Pz88zMzMzMzMzMPDw8PMzMzMzMzMzMPDw8PDMzMzPz8/PzzMzMzMzMzMw8PDw8AAD//zMzAADP8zwAAP//MzMAAP//MzMAAM/zPAAA//8zMwAA//8zMwAAz/M8AAD//zMzAAD//zMzAADP8zwAAP//MzP//0QQaCgAAFVVVVVVVVWqVVVVVVVVVapVVVVVVVVVqlVVVVVVVVWqUFBAYFBQQGBQUEBgUFBAYDMzMzPz8/PzzMzMzMzMzMwzMzMzMzMzM/Pz8/PAzMDMwMzAzDMzMzPz8/PzzMzMzMzMzMwzMzMzMzMzM/Pz8/PAzMDMwMzAzDMzMzPz8/PzzMzMzMzMzMwzMzMzMzMzM/Pz8/PAzMDMwMzAzDMzMzPz8/PzzMzMzMzMzMwzMzMzMzMzM/Pz8/PAzMDMwMzAzMzMz/M8AAD//wAAzMzP8zwAAMzMz/M8AAD//wAAzMzP8zwAAMzMz/M8AAD//wAAzMzP8zwAAMzMz/M8AAD//wAAzMzP8zwAAP//PxBoKQAAVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVAQEBAQEBAQEBAQEBAQEBAzMzMzMzMzMw8PDw8MzMzM/Pz8/PMzMzMzMzMzDw8PDzMzMzMzMzMzDw8PDwzMzMz8/Pz88zMzMzMzMzMPDw8PMzMzMzMzMzMPDw8PDMzMzPz8/PzzMzMzMzMzMw8PDw8zMzMzMzMzMw8PDw8MzMzM/Pz8/PMzMzMzMzMzDw8PDwAAP//MzMAAM/zPAAA//8zMwAA//8zMwAAz/M8AAD//zMzAAD//zMzAADP8zwAAP//MzMAAP//MzMAAM/zPAAA//8zM///RBBoKgAAVVVVVVVVVapVVVVVVVVVqlVVVVVVVVWqVVVVVVVVVapQUEBgUFBAYFBQQGBQUEBgMzMzM/Pz8/PMzMzMzMzMzDMzMzMzMzMz8/Pz88DMwMzAzMDMMzMzM/Pz8/PMzMzMzMzMzDMzMzMzMzMz8/Pz88DMwMzAzMDMMzMzM/Pz8/PMzMzMzMzMzDMzMzMzMzMz8/Pz88DMwMzAzMDMMzMzM/Pz8/PMzMzMzMzMzDMzMzMzMzMz8/Pz88DMwMzAzMDMzMzP8zwAAP//AADMzM/zPAAAzMzP8zwAAP//AADMzM/zPAAAzMzP8zwAAP//AADMzM/zPAAAzMzP8zwAAP//AADMzM/zPAAA'

function decodeBase64(s: string): Uint8Array {
  const raw = atob(s)
  const out = new Uint8Array(raw.length)
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i)
  return out
}

/** Décode l'image de test et vérifie quelques points, à 4 près (codes 8 bits). */
async function checkSample(gpu: PyroWaveGPU): Promise<boolean> {
  const dec = new PyroWaveDecoder(gpu, { chroma444: false, hdr: false, fullRange: false, width: 128, height: 128 })
  try {
    gpu.device.pushErrorScope('validation')
    const s = dec.submit(decodeBase64(sample))
    if (s.status !== 'decoded') return false
    await s.done
    const [y, cb, cr] = await dec.readPlanes()
    if (await gpu.device.popErrorScope()) return false
    const near = (v: number, expected: number) => Math.abs(v * 255 - expected) <= 4
    for (const x of [0, 32, 64, 96, 127]) if (!near(y[64 * 128 + x], 16 + (x * 219) / 127)) return false
    for (const x of [0, 63, 127]) if (!near(cb[32 * 64 + Math.min(x, 63)], 128) || !near(cr[32 * 64 + Math.min(x, 63)], 128)) return false
    return true
  } catch {
    return false
  } finally {
    dec.destroy()
  }
}

let shared: Promise<{ result: ProbeResult; gpu: PyroWaveGPU | null }> | null = null

async function runProbe(): Promise<{ result: ProbeResult; gpu: PyroWaveGPU | null }> {
  const t0 = performance.now()
  if (typeof navigator === 'undefined' || !navigator.gpu) return { result: { ok: false, reason: 'webgpu' }, gpu: null }
  // Sous-groupes d'abord ; la variante sans sous-groupes si l'image est fausse.
  for (const subgroups of [true, false]) {
    const gpu = await PyroWaveGPU.create({ subgroups })
    if (!gpu) return { result: { ok: false, reason: 'adapter' }, gpu: null }
    if (await checkSample(gpu)) {
      return { result: { ok: true, subgroups: gpu.subgroups, adapter: gpu.adapterName, ms: Math.round(performance.now() - t0) }, gpu }
    }
    const tried = gpu.subgroups
    gpu.destroy()
    if (!tried) break
  }
  return { result: { ok: false, reason: 'decode', ms: Math.round(performance.now() - t0) }, gpu: null }
}

/** Sonde, une fois par page (au chargement). */
export async function probePyroWave(): Promise<ProbeResult> {
  if (!shared) shared = runProbe().catch(() => ({ result: { ok: false, reason: 'adapter' as const }, gpu: null }))
  return (await shared).result
}

/** Le device de la sonde, s'il a passé ; recréé après une perte (pilote, veille). */
export async function pyrowaveGPU(): Promise<PyroWaveGPU | null> {
  if (!shared) await probePyroWave()
  const s = await shared!
  if (s.gpu?.lost) {
    shared = null
    await probePyroWave()
    return (await shared!).gpu
  }
  return s.gpu
}
