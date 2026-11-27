// Page de mesure du décodeur WebGPU dans un navigateur (servie par serve.ts) :
// image 0 comparée à la référence Metal, décodage cadencé à 60 i/s (temps de
// la carte graphique et jusqu'à l'image prête), à la chaîne, avec et sans
// sous-groupes, puis le chemin complet du client (toile WebGPU → VideoFrame).
import { encodeConfig, parseConfig } from '../../src/link/pyrowave/bitstream.ts'
import { PyroWaveDecoder, PyroWaveGPU } from '../../src/link/pyrowave/decoder.ts'
import { installPyroWave } from '../../src/link/pyrowave/live.ts'
import { pyrowaveStats } from '../../src/link/pyrowave/stats.ts'
import { compare, readTestFile, stats } from './compare.ts'

const out = document.getElementById('out')!
const lines: string[] = []
function log(s: string) {
  lines.push(s)
  out.textContent = lines.join('\n')
  console.log(s)
}
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const params = new URLSearchParams(location.search)
const names = (params.get('names') ?? '1440p-420,1440p-444,2160p-420,2160p-444').split(',')

async function bin(path: string): Promise<Uint8Array> {
  return new Uint8Array(await (await fetch(path)).arrayBuffer())
}

async function run(subgroups: boolean) {
  const gpu = await PyroWaveGPU.create({ subgroups, timestamps: true })
  if (!gpu) {
    log('WebGPU indisponible')
    return
  }
  log(`Adaptateur : ${gpu.adapterName || '?'} ; sous-groupes : ${gpu.subgroups ? 'oui' : 'non'} ; horodatage : ${gpu.timestamps ? 'oui' : 'non'}`)
  for (const name of names) {
    const { config, frames } = readTestFile(await bin(`data/${name}.pyrowave`))
    const cfg = parseConfig(config)!
    const dec = new PyroWaveDecoder(gpu, cfg)
    await dec.submit(frames[0]).done
    const planes = await dec.readPlanes()
    const c = compare(planes, new Float32Array((await bin(`data/${name}.ref.f32`)).buffer), await bin(`data/${name}.src.yuv`))
    // Cadencé à 60 i/s, comme une session : lecture du flux + envoi (processeur),
    // jusqu'à l'image prête (carte graphique terminée), temps de la carte graphique.
    const cpu: number[] = []
    const ready: number[] = []
    const gpuMs: number[] = []
    for (let rep = 0; rep < 3; rep++)
      for (let i = 0; i < frames.length; i++) {
        const t0 = performance.now()
        const s = dec.submit(frames[i])
        cpu.push(performance.now() - t0)
        await s.done
        ready.push(performance.now() - t0)
        await sleep(8)
        if (dec.lastGpuMs !== null) gpuMs.push(dec.lastGpuMs)
        await sleep(8)
      }
    const t0 = performance.now()
    let n = 0
    const pending: Promise<void>[] = []
    for (let rep = 0; rep < 4; rep++)
      for (const f of frames) {
        while (dec.inFlight >= PyroWaveDecoder.maxInFlight) await pending.shift()
        const s = dec.submit(f)
        if (s.done) pending.push(s.done)
        n++
      }
    await Promise.all(pending)
    const fps = Math.round(n / ((performance.now() - t0) / 1000))
    log(
      `${name} : écart max ${c.maxDiff.join('/')} ; PSNR réf. ${c.psnrRef.join('/')} dB ; PSNR source ${c.psnrSrc.join('/')} dB\n` +
        `  processeur ${JSON.stringify(stats(cpu))} ms ; image prête ${JSON.stringify(stats(ready))} ms ; GPU ${JSON.stringify(stats(gpuMs))} ms ; à la chaîne ${fps} i/s`,
    )
    dec.destroy()
  }
  gpu.destroy()
}

/** Chemin du client : toile WebGPU hors écran → ImageBitmap → VideoFrame, relue en 2D. */
async function fullPath() {
  const gpu = await PyroWaveGPU.create({})
  if (!gpu) return
  const name = names[0]
  const { config, frames } = readTestFile(await bin(`data/${name}.pyrowave`))
  const cfg = parseConfig(config)!
  const dec = new PyroWaveDecoder(gpu, cfg)
  const canvas = new OffscreenCanvas(cfg.width, cfg.height)
  const ctx = canvas.getContext('webgpu') as GPUCanvasContext
  const format = navigator.gpu.getPreferredCanvasFormat()
  ctx.configure({ device: gpu.device, format, alphaMode: 'opaque' })
  const total: number[] = []
  let frame: VideoFrame | null = null
  for (let i = 0; i < frames.length; i++) {
    const t0 = performance.now()
    const s = dec.submit(frames[i], ctx, format)
    const bitmap = canvas.transferToImageBitmap()
    await s.done
    frame?.close()
    frame = new VideoFrame(bitmap, { timestamp: i })
    bitmap.close()
    total.push(performance.now() - t0)
    await sleep(16)
  }
  // Relecture de la première image : couleurs comparées à la source convertie
  // en RGB (BT.709, plage réduite), sur une grille de points.
  const s0 = dec.submit(frames[0], ctx, format)
  const bmp0 = canvas.transferToImageBitmap()
  await s0.done
  frame?.close()
  frame = new VideoFrame(bmp0, { timestamp: 0 })
  bmp0.close()
  const check = new OffscreenCanvas(cfg.width, cfg.height)
  const c2 = check.getContext('2d')!
  c2.drawImage(frame, 0, 0)
  const img = c2.getImageData(0, 0, cfg.width, cfg.height).data
  const src = await bin(`data/${name}.src.yuv`)
  const W = cfg.width
  const H = cfg.height
  const cw = cfg.chroma444 ? W : W / 2
  let sum = 0
  let n = 0
  for (let gy = 1; gy < 20; gy++)
    for (let gx = 1; gx < 20; gx++) {
      const x = Math.floor((gx * W) / 20)
      const y = Math.floor((gy * H) / 20)
      const cx = cfg.chroma444 ? x : x >> 1
      const cy = cfg.chroma444 ? y : y >> 1
      const Y = (src[y * W + x] - 16) / 219
      const Cb = (src[W * H + cy * cw + cx] - 128) / 224
      const Cr = (src[W * H + (H * cw) / (cfg.chroma444 ? 1 : 2) + cy * cw + cx] - 128) / 224
      const rgb = [Y + 1.5748 * Cr, Y - 0.18733 * Cb - 0.46813 * Cr, Y + 1.8556 * Cb].map((v) => Math.min(255, Math.max(0, v * 255)))
      const o = (y * W + x) * 4
      for (let k = 0; k < 3; k++) {
        sum += Math.abs(img[o + k] - rgb[k])
        n++
      }
    }
  log(`Chemin complet (${name}, toile → ImageBitmap → VideoFrame) : ${JSON.stringify(stats(total))} ms ; écart moyen des couleurs à la source ${(sum / n).toFixed(1)} (codes 8 bits, 361 points)`)
  frame?.close()
  dec.destroy()
  gpu.destroy()
}

/** Le pont tel que le moteur l'appelle (kywebplayer, correctif 0025-kyctlweb) :
 * paquet de configuration, images avec leur pts, une image tronquée, une perdue. */
async function bridge() {
  const api = (globalThis as any).__kybernowPyroWave
  if (!api) {
    log('Pont : absent (sonde ratée)')
    return
  }
  const name = names[0]
  const { config, frames } = readTestFile(await bin(`data/${name}.pyrowave`))
  const outputs: { ts: number; w: number; h: number }[] = []
  const dec = api.create((f: VideoFrame) => {
    outputs.push({ ts: f.timestamp, w: f.displayWidth, h: f.displayHeight })
    f.close()
  })
  const cfg = parseConfig(config)!
  const accepted: boolean[] = []
  accepted.push(dec.push(encodeConfig(cfg), 0, true))
  const step = 16667
  for (let i = 0; i < frames.length; i++) {
    if (i === 10) continue // perdue en route
    const data = i === 5 ? frames[i].subarray(0, frames[i].length >> 1) : frames[i]
    accepted.push(dec.push(data, i * step, false))
    await sleep(16)
  }
  await sleep(200)
  const stats = pyrowaveStats()
  dec.close()
  const expected = accepted.filter((a) => a).length
  log(
    `Pont : configuration ${accepted[0] ? 'prise pour une image (FAUX)' : 'ok'} ; ${expected} images acceptées, ${outputs.length} rendues ` +
      `(${outputs.length === expected ? 'ok' : 'ÉCART'}), ${outputs[0]?.w}x${outputs[0]?.h} ; image tronquée ${accepted[6] ? 'acceptée (FAUX)' : 'sautée'} ; ` +
      `images perdues ${stats.lostPercent?.toFixed(1)} % (2 sur ${frames.length} attendu : 6,7 %) ; décodage médian ${stats.decodeMs?.toFixed(1)} ms`,
  )
}

async function main() {
  log(navigator.userAgent)
  const t0 = performance.now()
  const probe = await installPyroWave()
  log(`Sonde : ${JSON.stringify(probe)} en ${Math.round(performance.now() - t0)} ms`)
  if (params.get('sg') !== 'off') await run(true)
  await run(false)
  await fullPath()
  await bridge()
  await fetch('result', { method: 'POST', body: lines.join('\n') })
  log('FIN')
}
main().catch(async (e) => {
  log(`ERREUR ${e?.stack ?? e}`)
  await fetch('result', { method: 'POST', body: lines.join('\n') })
})
