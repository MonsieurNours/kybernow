// Lecture du flux PyroWave (format v1, `bitstream/bitstream.md` de l'amont),
// d'après pyrowave_bitstream.cpp (commit a32b05e, licence MIT) : découpage des
// blocs 32 × 32, table des positions que lit le shader de déquantification.
// Le paquet de configuration `KNPW` est celui du contrat KyberNow (docs/pyrowave.md).

export const DecompositionLevels = 5
const Alignment = 1 << DecompositionLevels
const MinimumImageSize = 4 << DecompositionLevels
export const NumComponents = 3
const SequenceCountMask = 7
/** Position absente de la table : bloc manquant (coefficients nuls). */
export const Missing = 0xffffffff

function align(v: number, a: number): number {
  return (v + a - 1) & ~(a - 1)
}

/** Paquet de configuration de KyberNow (16 octets). */
export interface PyroWaveConfig {
  chroma444: boolean
  /** HDR10 : BT.2020, PQ. Sinon SDR BT.709. */
  hdr: boolean
  fullRange: boolean
  width: number
  height: number
}

export function sameConfig(a: PyroWaveConfig | null, b: PyroWaveConfig | null): boolean {
  return !!a && !!b && a.chroma444 === b.chroma444 && a.hdr === b.hdr && a.fullRange === b.fullRange && a.width === b.width && a.height === b.height
}

/** `KNPW` | version 1 | chroma | couleur | plage | largeur u32 | hauteur u32. */
export function parseConfig(data: Uint8Array): PyroWaveConfig | null {
  if (data.length < 16) return null
  if (data[0] !== 0x4b || data[1] !== 0x4e || data[2] !== 0x50 || data[3] !== 0x57 || data[4] !== 1) return null
  const v = new DataView(data.buffer, data.byteOffset, data.byteLength)
  const width = v.getUint32(8, true)
  const height = v.getUint32(12, true)
  if (width < 1 || height < 1 || width > 16384 || height > 16384) return null
  return { chroma444: data[5] === 1, hdr: data[6] === 1, fullRange: data[7] === 0, width, height }
}

export function encodeConfig(c: PyroWaveConfig): Uint8Array {
  const out = new Uint8Array(16)
  out.set([0x4b, 0x4e, 0x50, 0x57, 1, c.chroma444 ? 1 : 0, c.hdr ? 1 : 0, c.fullRange ? 0 : 1])
  const v = new DataView(out.buffer)
  v.setUint32(8, c.width, true)
  v.setUint32(12, c.height, true)
  return out
}

/** Sans paquet de configuration : définition et chroma lues dans l'en-tête de
 * séquence du flux, en SDR plage pleine (l'encodeur n'y écrit pas la couleur
 * de façon fiable), comme le client Mac. */
export function configFromSequenceHeader(data: Uint8Array): PyroWaveConfig | null {
  let off = 0
  const v = new DataView(data.buffer, data.byteOffset, data.byteLength)
  while (off + 8 <= data.length) {
    const w0 = v.getUint32(off, true)
    const w1 = v.getUint32(off + 4, true)
    if (w0 >>> 31) {
      const width = (w0 & 0x3fff) + 1
      const height = ((w0 >>> 14) & 0x3fff) + 1
      const code = (w1 >>> 24) & 3
      if (code !== 0) return null
      return { chroma444: ((w1 >>> 26) & 1) === 1, hdr: false, fullRange: true, width, height }
    }
    const words = (w0 >>> 16) & 0xfff
    if (words < 2) return null
    off += words * 4
  }
  return null
}

interface BlockInfo {
  offset32: number
  stride32: number
  count32: number
}

/** Disposition des blocs d'une définition (BlockLayout de l'amont). */
export class BlockLayout {
  readonly alignedWidth: number
  readonly alignedHeight: number
  /** [composante][niveau][bande] ; absent pour la chroma du niveau 0 en 4:2:0. */
  readonly meta: (BlockInfo | null)[][][] = []
  readonly blockCount32: number

  constructor(
    readonly width: number,
    readonly height: number,
    readonly chroma444: boolean,
  ) {
    this.alignedWidth = Math.max(align(width, Alignment), MinimumImageSize)
    this.alignedHeight = Math.max(align(height, Alignment), MinimumImageSize)
    for (let c = 0; c < NumComponents; c++) {
      this.meta.push([])
      for (let l = 0; l < DecompositionLevels; l++) this.meta[c].push([null, null, null, null])
    }
    let count = 0
    for (let level = DecompositionLevels - 1; level >= 0; level--) {
      for (let c = 0; c < NumComponents; c++) {
        if (level === 0 && c !== 0 && !chroma444) continue
        for (let band = level === DecompositionLevels - 1 ? 0 : 1; band < 4; band++) {
          const bx = (this.levelWidth(level) + 31) >> 5
          const by = (this.levelHeight(level) + 31) >> 5
          this.meta[c][level][band] = { offset32: count, stride32: bx, count32: bx * by }
          count += bx * by
        }
      }
    }
    this.blockCount32 = count
  }

  levelWidth(level: number): number {
    return Math.max(1, (this.alignedWidth / 2) >> level)
  }

  levelHeight(level: number): number {
    return Math.max(1, (this.alignedHeight / 2) >> level)
  }
}

export type PushResult = 'ok' | 'corrupt'

/** Une image : les blocs reçus mis bout à bout (mots de 32 bits) et la table
 * bloc → position, que le shader de déquantification lit telle quelle. */
export class BitstreamParser {
  /** Position (en mots) de chaque bloc 32 × 32 dans `payload`, `Missing` sinon. */
  readonly offsets: Uint32Array
  private payloadWords: Uint32Array
  payloadLength = 0
  decodedBlocks = 0
  totalBlocks = 0
  private lastSeq = -1
  private decodedCurrent = false
  /** Dernier en-tête de séquence lu (couleur, plage), pour le journal. */
  sequenceHeader: { primaries: number; transfer: number; range: number } | null = null

  constructor(private layout: BlockLayout) {
    this.offsets = new Uint32Array(layout.blockCount32)
    this.payloadWords = new Uint32Array(1 << 18)
    this.clear()
  }

  get payload(): Uint32Array {
    return this.payloadWords.subarray(0, this.payloadLength)
  }

  clear() {
    this.offsets.fill(Missing)
    this.decodedBlocks = 0
    this.lastSeq = -1
    this.decodedCurrent = false
    this.totalBlocks = this.layout.blockCount32
    this.payloadLength = 0
  }

  private append(src: Uint8Array, byteOffset: number, words: number) {
    if (this.payloadLength + words > this.payloadWords.length) {
      const grown = new Uint32Array(Math.max(this.payloadWords.length * 2, this.payloadLength + words))
      grown.set(this.payloadWords.subarray(0, this.payloadLength))
      this.payloadWords = grown
    }
    const abs = src.byteOffset + byteOffset
    if ((abs & 3) === 0) {
      this.payloadWords.set(new Uint32Array(src.buffer, abs, words), this.payloadLength)
    } else {
      const bytes = new Uint8Array(this.payloadWords.buffer, this.payloadLength * 4, words * 4)
      bytes.set(src.subarray(byteOffset, byteOffset + words * 4))
    }
    this.payloadLength += words
  }

  /** Paquets PyroWave bout à bout (un paquet média de la Station). */
  push(data: Uint8Array): PushResult {
    const v = new DataView(data.buffer, data.byteOffset, data.byteLength)
    let off = 0
    let size = data.length
    while (size >= 8) {
      const w0 = v.getUint32(off, true)
      const w1 = v.getUint32(off + 4, true)
      const sequence = (w0 >>> 28) & 7
      if (w0 >>> 31) {
        // En-tête de séquence (début d'image).
        const chroma = (w1 >>> 26) & 1
        if (chroma !== (this.layout.chroma444 ? 1 : 0)) return 'corrupt'
        if (this.lastSeq >= 0) {
          const diff = (sequence - this.lastSeq) & SequenceCountMask
          if (diff > SequenceCountMask / 2) return 'ok'
          if (diff !== 0) {
            this.clear()
            this.lastSeq = sequence
          }
        } else {
          this.clear()
          this.lastSeq = sequence
        }
        const code = (w1 >>> 24) & 3
        if (code !== 0) return 'corrupt'
        const width = (w0 & 0x3fff) + 1
        const height = ((w0 >>> 14) & 0x3fff) + 1
        if (width !== this.layout.width || height !== this.layout.height) return 'corrupt'
        this.totalBlocks = w1 & 0xffffff
        this.sequenceHeader = { primaries: (w1 >>> 27) & 1, transfer: (w1 >>> 28) & 1, range: (w1 >>> 30) & 1 }
        off += 8
        size -= 8
        continue
      }

      const words = (w0 >>> 16) & 0xfff
      const bytes = words * 4
      if (bytes > size) return 'corrupt'
      if (words < 2) return 'corrupt'
      if (this.lastSeq < 0) {
        this.clear()
        this.lastSeq = sequence
      } else {
        const diff = (sequence - this.lastSeq) & SequenceCountMask
        if (diff > SequenceCountMask / 2) return 'ok'
        if (diff !== 0) {
          this.clear()
          this.lastSeq = sequence
        }
      }
      const block = w1 >>> 8
      if (block >= this.layout.blockCount32) return 'corrupt'
      if (this.offsets[block] === Missing) {
        this.offsets[block] = this.payloadLength
        this.decodedBlocks++
        this.append(data, off, words)
      }
      off += bytes
      size -= bytes
    }
    return size === 0 ? 'ok' : 'corrupt'
  }

  /** Les blocs des `bands` bandes les plus basses sont-ils tous là ? */
  private pristine(bands: number): boolean {
    const L = DecompositionLevels
    for (let band = 0; band < bands; band++) {
      for (const comp of this.layout.meta) {
        const list = band === 0 ? [comp[L - 1][0]] : [comp[L - band][1], comp[L - band][2], comp[L - band][3]]
        for (const m of list) {
          if (!m) continue
          for (let i = 0; i < m.count32; i++) if (this.offsets[m.offset32 + i] === Missing) return false
        }
      }
    }
    return true
  }

  /** Comme `decode_is_ready(true)` de l'amont : complète, ou au moins 90 % des
   * blocs avec les deux bandes les plus basses entières. */
  ready(): boolean {
    if (this.decodedCurrent || this.lastSeq < 0) return false
    if (this.decodedBlocks < this.totalBlocks) {
      if (!this.pristine(2)) return false
      if (this.decodedBlocks <= this.totalBlocks * 0.9) return false
    }
    return true
  }

  complete(): boolean {
    return this.decodedBlocks >= this.totalBlocks
  }

  markDecoded() {
    this.decodedCurrent = true
  }
}
