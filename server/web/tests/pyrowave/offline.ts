// Essai hors ligne du décodeur WebGPU dans Deno (WebGPU de wgpu, Metal sur le Mac) :
//   deno run --allow-read --unstable-webgpu tests/pyrowave/offline.ts <dossier de prepare.sh> [nom…] [--no-subgroups]
// Pour chaque fichier : image 0 comparée à la référence Metal et à la source,
// image tronquée sautée sans casser la suivante, temps de décodage.
import { parseConfig } from '../../src/link/pyrowave/bitstream.ts'
import { PyroWaveDecoder, PyroWaveGPU } from '../../src/link/pyrowave/decoder.ts'
import { compare, readTestFile, stats } from './compare.ts'

const args = Deno.args.filter((a) => !a.startsWith('--'))
const dir = args[0]
const names = args.length > 1 ? args.slice(1) : ['1440p-420', '1440p-444', '2160p-420', '2160p-444']
const subgroups = !Deno.args.includes('--no-subgroups')
const gpu = await PyroWaveGPU.create({ subgroups, timestamps: true })
if (!gpu) throw new Error('WebGPU absent')
console.log(`Adaptateur : ${gpu.adapterName || '?'} ; sous-groupes : ${gpu.subgroups ? 'oui' : 'non'} ; horodatage : ${gpu.timestamps ? 'oui' : 'non'}`)
let failed = false
for (const name of names) {
  const { config, frames } = readTestFile(await Deno.readFile(`${dir}/${name}.pyrowave`))
  const cfg = parseConfig(config)!
  const dec = new PyroWaveDecoder(gpu, cfg)
  gpu.device.pushErrorScope('validation')
  const first = dec.submit(frames[0])
  await first.done
  const err = await gpu.device.popErrorScope()
  if (err) throw new Error(err.message)
  const planes = await dec.readPlanes()
  const ref = new Float32Array((await Deno.readFile(`${dir}/${name}.ref.f32`)).buffer)
  const src = await Deno.readFile(`${dir}/${name}.src.yuv`)
  const c = compare(planes, ref, src)
  const ok = c.maxDiff.every((d) => d <= 4)
  if (!ok) failed = true
  // Image tronquée : sautée, la suivante se décode.
  const cut = dec.submit(frames[1].subarray(0, Math.floor(frames[1].length / 3)))
  const next = dec.submit(frames[2])
  await next.done
  // Cadencé à 60 i/s, puis à la chaîne.
  const wall: number[] = []
  const gpuMs: number[] = []
  for (let i = 0; i < frames.length; i++) {
    const t0 = performance.now()
    const s = dec.submit(frames[i])
    await s.done
    wall.push(performance.now() - t0)
    await new Promise((r) => setTimeout(r, 4))
    if (dec.lastGpuMs !== null) gpuMs.push(dec.lastGpuMs)
    await new Promise((r) => setTimeout(r, 12))
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
  console.log(
    `${name} : écart max à la référence ${c.maxDiff.join('/')} (≤ 4 : ${ok ? 'oui' : 'NON'}), PSNR réf. ${c.psnrRef.join('/')} dB, PSNR source ${c.psnrSrc.join('/')} dB ; ` +
      `tronquée : ${cut.status}, suivante : ${next.status} ; décodage cadencé ${JSON.stringify(stats(wall))} ms, GPU ${JSON.stringify(stats(gpuMs))} ms ; à la chaîne ${fps} i/s`,
  )
  dec.destroy()
}
if (failed) Deno.exit(1)
