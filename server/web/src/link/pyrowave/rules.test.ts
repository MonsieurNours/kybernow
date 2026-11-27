// Règles PyroWave du client web : mêmes cas que le cœur (apps/core/src/pyrowave.rs),
// choix dans « Format vidéo », paquet de configuration. `npm test`.

import { describe, expect, it, vi } from 'vitest'

vi.mock('../device', () => ({ hasWebTransport: true, isFirefox: false, isTouch: false, isWebKit: false }))
vi.mock('../inputs', () => ({ engineInputs: () => true }))

import { ceiling, lowered, neededMbps, plan, unavailable, MIN_HEIGHT } from './rules'
import { configFromSequenceHeader, encodeConfig, parseConfig } from './bitstream'
import { choose, setRelay } from '../plan'
import { nextFallback, NO_FALLBACK } from '../fallback'
import { defaults } from '../prefs'
import { colorShader, SDR_KNEE, SDR_SLOPE } from './shaders'

const m = (width: number, height: number, refresh: number) => ({ width, height, refresh })

describe('débit et définition ramenée', () => {
  it('débit nécessaire', () => {
    expect(neededMbps(m(1920, 1080, 60), false)).toBe(187)
    expect(neededMbps(m(3840, 2160, 60), false)).toBe(747)
    expect(neededMbps(m(3840, 2160, 60), true)).toBe(1120)
  })

  it('le plus petit plafond connu', () => {
    expect(ceiling(null, undefined)).toBe(null)
    expect(ceiling(800, null)).toBe(800)
    expect(ceiling(800, 450)).toBe(450)
    expect(ceiling(0, 500)).toBe(500)
  })

  it('rien à ramener sous le plafond', () => {
    expect(lowered(m(3840, 2160, 60), false, 800)).toBe(null)
    expect(lowered(m(1920, 1080, 60), true, 300)).toBe(null)
  })

  it('la plus grande définition paire qui tient', () => {
    const l = lowered(m(3840, 2160, 60), false, 500)!
    expect([l.width % 2, l.height % 2, l.refresh, l.mbps]).toEqual([0, 0, 60, 500])
    expect(neededMbps(l, false)).toBeLessThanOrEqual(500)
    expect(neededMbps(m(Math.round(((l.height + 2) * 16) / 9), l.height + 2, 60), false)).toBeGreaterThan(500)
    expect(Math.abs(l.width / l.height - 16 / 9)).toBeLessThan(0.01)
  })

  it('puis la fréquence, jusqu’à 720p à 30 Hz', () => {
    const l = lowered(m(3840, 2160, 120), false, 150)!
    expect(l.refresh).toBe(100)
    expect(l.height).toBeGreaterThanOrEqual(MIN_HEIGHT)
    expect(lowered(m(3840, 2160, 120), false, 10)).toEqual({ width: 1280, height: 720, refresh: 30, mbps: 10 })
    const small = lowered(m(1280, 600, 60), false, 50)!
    expect([small.width, small.height, small.refresh]).toEqual([1280, 600, 30])
  })

  it('le débit demandé : nécessaire, jamais plus que le plafond', () => {
    expect(plan(m(1920, 1080, 60), false, null, null)).toEqual({ mode: m(1920, 1080, 60), mbps: 187, lowered: null })
    const p = plan(m(3840, 2160, 60), false, 500, null)
    expect(p.lowered).not.toBe(null)
    expect(p.mbps).toBeLessThanOrEqual(500)
    expect(plan(m(3840, 2160, 60), false, 800, 300).lowered?.mbps).toBe(300)
  })
})

describe('format vidéo', () => {
  const codecs = [
    { codec: 'h264', yuv444: false, hardware: true },
    { codec: 'hevc', yuv444: false, hardware: true },
  ]
  const screen = m(2560, 1440, 60)
  const usable = { usable: true, stationMax: null }

  it('valeur inactive : l’ordinateur d’abord, puis ce navigateur', () => {
    expect(unavailable(undefined, true)).toBe('station')
    expect(unavailable({ available: false, reason: 'vulkan' }, true)).toBe('station')
    expect(unavailable({ available: true }, false)).toBe('browser')
    expect(unavailable({ available: true }, true)).toBe(null)
  })

  it('Auto ne prend jamais PyroWave', () => {
    expect(choose(defaults, screen, codecs, 'kymux', usable).codec).toBe('hevc')
  })

  it('PyroWave choisi : repli sur le format d’Auto, 4:4:4 du profil, avertissement « réseau »', () => {
    const p = choose({ ...defaults, profile: 'bureau', codec: 'pyrowave' }, screen, codecs, 'kymux', usable)
    expect([p.codec, p.fallbackCodec, p.yuv444, p.pyrowave?.warning]).toEqual(['pyrowave', 'hevc', true, 'network'])
    expect(p.mbps).toBe(neededMbps(screen, true))
  })

  it('par le relais : l’avertissement du relais, sans plafond', () => {
    setRelay(true)
    const p = choose({ ...defaults, codec: 'pyrowave' }, screen, codecs, 'websocket', usable)
    setRelay(false)
    expect(p.pyrowave?.warning).toBe('relay')
    expect(p.pyrowave?.lowered).toBe(null)
  })

  it('pas offert : le format d’Auto', () => {
    const p = choose({ ...defaults, codec: 'pyrowave' }, screen, codecs, 'kymux', { usable: false, stationMax: null })
    expect([p.codec, p.pyrowave]).toEqual(['hevc', null])
  })

  it('plafond de la Station : définition ramenée', () => {
    const p = choose({ ...defaults, profile: 'jeu', codec: 'pyrowave' }, m(3840, 2160, 60), codecs, 'kymux', { usable: true, stationMax: 300 })
    expect(p.pyrowave?.lowered?.mbps).toBe(300)
    expect(p.mbps).toBeLessThanOrEqual(300)
  })

  it('image qui ne démarre pas en PyroWave : repli en H.264', () => {
    expect(nextFallback({ codec: 'pyrowave', audio: true }, NO_FALLBACK, codecs, '', true)).toBe('h264')
  })
})

describe('paquet de configuration', () => {
  it('aller et retour', () => {
    const c = { chroma444: true, hdr: false, fullRange: true, width: 2560, height: 1440 }
    expect(parseConfig(encodeConfig(c))).toEqual(c)
    expect(parseConfig(new Uint8Array(16))).toBe(null)
  })

  it('en-tête de séquence du flux, quand le paquet manque', () => {
    // width_minus_1 = 127, height_minus_1 = 127, extended ; total_blocks 10, 4:2:0.
    const w0 = 127 | (127 << 14) | (1 << 31)
    const data = new Uint8Array(8)
    new DataView(data.buffer).setUint32(0, w0 >>> 0, true)
    new DataView(data.buffer).setUint32(4, 10, true)
    expect(configFromSequenceHeader(data)).toEqual({ chroma444: false, hdr: false, fullRange: true, width: 128, height: 128 })
  })
})

describe('courbe vers le SDR (rules::hdr::sdr_curve du cœur)', () => {
  it('mêmes constantes que le cœur, écrites dans le shader', () => {
    expect(SDR_KNEE).toBe(0.75)
    expect(SDR_SLOPE).toBeCloseTo(3.6923077, 6)
    // Pente 1 au genou, 4 fois le blanc ramené à 1 : la courbe de la Station.
    const curve = (m: number) => (m <= SDR_KNEE ? m : Math.min(SDR_KNEE + (m - SDR_KNEE) / (1 + SDR_SLOPE * (m - SDR_KNEE)), 1))
    expect(curve(4)).toBeCloseTo(1, 9)
    expect(curve(0.5)).toBe(0.5)
    expect(colorShader).toContain('fn sdr_curve(m: f32) -> f32')
    expect(colorShader).toContain(`1.0 + ${SDR_SLOPE} * x`)
    expect(colorShader).toContain('if (m > 0.75)')
  })
})
