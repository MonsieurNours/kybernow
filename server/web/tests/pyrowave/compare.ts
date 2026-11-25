// Comparaison d'une image décodée par le décodeur WebGPU avec la référence
// (décodeur Metal du port, refdump) et avec la source : commun à l'essai
// sans tête (Deno) et à la page de mesure des navigateurs.

export interface Comparison {
  /** Plus grand écart avec la référence, en codes 8 bits, par plan. */
  maxDiff: number[]
  /** PSNR contre la référence et contre la source (8 bits), par plan, en dB. */
  psnrRef: number[]
  psnrSrc: number[]
}

function psnr(se: number, n: number): number {
  const mse = se / n
  return mse === 0 ? 99 : 10 * Math.log10((255 * 255) / mse)
}

/** `planes` : Y, Cb, Cr en float (0 à 1) ; `ref` : même chose à la suite ; `src` : yuv 8 bits. */
export function compare(planes: Float32Array[], ref: Float32Array, src: Uint8Array): Comparison {
  const maxDiff: number[] = []
  const psnrRef: number[] = []
  const psnrSrc: number[] = []
  let off = 0
  for (const p of planes) {
    let max = 0
    let seRef = 0
    let seSrc = 0
    for (let i = 0; i < p.length; i++) {
      const v = Math.min(1, Math.max(0, p[i])) * 255
      const r = Math.min(1, Math.max(0, ref[off + i])) * 255
      const d = Math.abs(v - r)
      if (d > max) max = d
      seRef += d * d
      const s = v - src[off + i]
      seSrc += s * s
    }
    maxDiff.push(Math.round(max * 100) / 100)
    psnrRef.push(Math.round(psnr(seRef, p.length) * 10) / 10)
    psnrSrc.push(Math.round(psnr(seSrc, p.length) * 10) / 10)
    off += p.length
  }
  return { maxDiff, psnrRef, psnrSrc }
}

/** Fichier d'essai : paquet de configuration (16 octets), puis chaque image précédée de sa taille (u32 LE). */
export function readTestFile(data: Uint8Array): { config: Uint8Array; frames: Uint8Array[] } {
  const config = data.subarray(0, 16)
  const frames: Uint8Array[] = []
  const v = new DataView(data.buffer, data.byteOffset, data.byteLength)
  let i = 16
  while (i + 4 <= data.length) {
    const n = v.getUint32(i, true)
    i += 4
    frames.push(data.subarray(i, i + n))
    i += n
  }
  return { config, frames }
}

export function stats(values: number[]): { mean: number; median: number; max: number } {
  const s = [...values].sort((a, b) => a - b)
  const mean = s.reduce((a, b) => a + b, 0) / Math.max(1, s.length)
  return { mean: Math.round(mean * 100) / 100, median: Math.round(s[Math.floor(s.length / 2)] * 100) / 100, max: Math.round(s[s.length - 1] * 100) / 100 }
}
