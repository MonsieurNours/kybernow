// Paquet de configuration `KNPW` (docs/kyber-changes.md, « En-tête KNPW version 2 ») : mêmes cas que le cœur
// (apps/core/src/pyrowave.rs, `paquet_de_configuration`), plus la version 2 et ses métadonnées HDR10. `npm test`.

import { describe, expect, it } from 'vitest'

import { CONFIG_V2_SIZE, configFromSequenceHeader, encodeConfig, isConfig, parseConfig, type PyroWaveConfig } from './bitstream'

const c: PyroWaveConfig = { chroma444: false, hdr: true, fullRange: false, width: 3840, height: 2160 }
const v1 = encodeConfig(c)

/** Un en-tête version 2 écrit à la main, octet par octet, comme `pyrowave_common.h` (correctif FFmpeg 0021). */
function v2Bytes(flags: number): Uint8Array {
  const b = new Uint8Array(CONFIG_V2_SIZE)
  b.set(v1)
  b[4] = 2
  const v = new DataView(b.buffer)
  v.setUint8(16, flags)
  // Primaires de Display P3 (rouge, vert, bleu), blanc D65, unités de 0,00002.
  ;[34000, 16000, 13250, 34500, 7500, 3000, 15635, 16450].forEach((x, i) => v.setUint16(20 + i * 2, x, true))
  v.setUint32(36, 1000 * 10000, true) // 1 000 nits
  v.setUint32(40, 1, true) // 0,0001 nit
  v.setUint16(44, 800, true) // MaxCLL
  v.setUint16(46, 400, true) // MaxFALL
  return b
}

const close = (a: number[], b: number[]) => a.forEach((x, i) => expect(x).toBeCloseTo(b[i], 6))

describe('paquet de configuration KNPW', () => {
  it('version 1 de 16 octets (Station 0.11.7)', () => {
    expect(v1.length).toBe(16)
    expect(isConfig(v1)).toBe(true)
    const p = parseConfig(v1)
    expect(p).toEqual(c)
    expect(p && 'metadata' in p).toBe(false)
  })

  it('version 1 plus longue : octets en trop ignorés', () => {
    const long = new Uint8Array(40)
    long.set(v1)
    long.fill(0xaa, 16)
    expect(parseConfig(long)).toEqual(c)
  })

  it('lu à un décalage quelconque du tampon', () => {
    const buf = new Uint8Array(3 + 16)
    buf.set(v1, 3)
    expect(parseConfig(buf.subarray(3))).toEqual(c)
  })

  it('version 2 complète : mastering display et niveau de lumière', () => {
    const p = parseConfig(v2Bytes(3))!
    expect({ ...p, metadata: undefined }).toEqual(c)
    const m = p.metadata!
    expect(m.flags).toBe(3)
    close(m.mastering!.red, [0.68, 0.32])
    close(m.mastering!.green, [0.265, 0.69])
    close(m.mastering!.blue, [0.15, 0.06])
    close(m.mastering!.white, [0.3127, 0.329])
    expect(m.mastering!.maxNits).toBe(1000)
    expect(m.mastering!.minNits).toBeCloseTo(0.0001, 10)
    expect([m.maxCll, m.maxFall]).toEqual([800, 400])
    // Aller et retour.
    expect(parseConfig(encodeConfig(p))).toEqual(p)
  })

  it('version 2 : un champ sans son bit est absent', () => {
    const mastering = parseConfig(v2Bytes(1))!.metadata!
    expect(mastering.mastering?.maxNits).toBe(1000)
    expect([mastering.maxCll, mastering.maxFall]).toEqual([null, null])
    const light = parseConfig(v2Bytes(2))!.metadata!
    expect(light.mastering).toBe(null)
    expect([light.maxCll, light.maxFall]).toEqual([800, 400])
  })

  it('version 2 sans drapeaux (SDR) : métadonnées vides', () => {
    const sdr = v2Bytes(0)
    sdr[6] = 0
    const p = parseConfig(sdr)!
    expect(p.hdr).toBe(false)
    expect(p.metadata).toEqual({ flags: 0, mastering: null, maxCll: null, maxFall: null })
  })

  it('version 2 plus longue : octets en trop ignorés', () => {
    const long = new Uint8Array(64)
    long.set(v2Bytes(3))
    long.fill(0xff, 48)
    expect(parseConfig(long)).toEqual(parseConfig(v2Bytes(3)))
  })

  it('version 2 trop courte : la tête se lit, sans métadonnées (comme le cœur)', () => {
    const full = v2Bytes(3)
    for (const n of [16, 44, 47]) {
      const c2 = parseConfig(full.subarray(0, n))
      expect(c2).not.toBe(null)
      expect(c2?.metadata).toBe(undefined)
      expect(c2?.width).toBe(3840)
    }
  })

  it('version plus récente : la tête et les métadonnées de la version 2 se lisent ; version 0 refusée', () => {
    for (const version of [3, 255]) {
      const b = v2Bytes(3)
      b[4] = version
      expect(parseConfig(b)).toEqual(parseConfig(v2Bytes(3)))
    }
    const b = v2Bytes(3)
    b[4] = 0
    expect(parseConfig(b)).toBe(null)
  })

  it('refusés comme dans le cœur : trop court, autre en-tête, octet inconnu, 4:2:0 impair, trop grand', () => {
    expect(parseConfig(v1.subarray(0, 15))).toBe(null)
    expect(isConfig(v1.subarray(0, 15))).toBe(false)
    const bad = (i: number, v: number) => {
      const x = v1.slice()
      x[i] = v
      return parseConfig(x)
    }
    expect(bad(0, 0x58)).toBe(null)
    expect([bad(5, 2), bad(6, 2), bad(7, 2)]).toEqual([null, null, null])
    expect(parseConfig(encodeConfig({ ...c, width: 1921 }))).toBe(null)
    expect(parseConfig(encodeConfig({ ...c, chroma444: true, width: 1921 }))).not.toBe(null)
    expect(parseConfig(encodeConfig({ ...c, width: 16385 }))).toBe(null)
    expect(parseConfig(encodeConfig({ ...c, width: 0 }))).toBe(null)
    expect(parseConfig(new Uint8Array(16))).toBe(null)
  })

  it('une image n’est pas un paquet de configuration', () => {
    const header = new Uint8Array(8)
    new DataView(header.buffer).setUint32(0, (1919 | (1079 << 14) | (1 << 31)) >>> 0, true)
    expect(isConfig(header)).toBe(false)
    expect(parseConfig(header)).toBe(null)
    expect(configFromSequenceHeader(header)).toEqual({ chroma444: false, hdr: false, fullRange: true, width: 1920, height: 1080 })
    // 4:2:0 impair dans l'en-tête de séquence : image impossible.
    new DataView(header.buffer).setUint32(0, (1920 | (1079 << 14) | (1 << 31)) >>> 0, true)
    expect(configFromSequenceHeader(header)).toBe(null)
  })
})
