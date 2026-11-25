// Règles PyroWave du client web (docs/link-ui.md, « PyroWave » ;
// docs/pyrowave-client-web.md). Débit nécessaire et définition ramenée :
// même calcul que le cœur des apps (apps/core/src/pyrowave.rs), pour un écran.

import type { Mode } from '../device'

/** 4:2:0 (SDR ou HDR10) : le haut de ce que donne PyroWave à netteté pleine. */
export const BITS_PER_PIXEL = 1.5
/** 4:4:4 : moitié plus. */
export const BITS_PER_PIXEL_444 = 2.25
export const MIN_HEIGHT = 720
export const MIN_REFRESH = 30
const REFRESH_STEPS = [120, 100, 60, 50, 30]

/** Ce que la Station dit de PyroWave (`pyrowave` de `GET /api/stream/info`). */
export interface StationPyroWave {
  available: boolean
  reason?: string | null
  max_mbps?: number | null
}

/** Pourquoi « PyroWave » est inactif dans « Format vidéo ». */
export type Unavailable = 'station' | 'browser'

/**
 * La valeur « PyroWave » est-elle offerte ? L'ordinateur d'abord (une Station
 * plus ancienne n'a pas le champ : jamais demandé), puis ce navigateur (sonde
 * WebGPU passée, moteur de Kyber qui connaît `pwv1`).
 */
export function unavailable(station: StationPyroWave | null | undefined, browserDecodes: boolean): Unavailable | null {
  if (!station?.available) return 'station'
  if (!browserDecodes) return 'browser'
  return null
}

/** Avertissement d'un choix PyroWave : le navigateur ne sait jamais s'il est
 * relié par câble ; par le relais, celui du relais. */
export type Warning = 'relay' | 'network'

export function warning(relay: boolean): Warning {
  return relay ? 'relay' : 'network'
}

function bitsPerSecond(m: Mode, yuv444: boolean): number {
  return m.width * m.height * Math.max(1, m.refresh) * (yuv444 ? BITS_PER_PIXEL_444 : BITS_PER_PIXEL)
}

/** Débit dont PyroWave a besoin pour cette définition, en Mbit/s (arrondi au-dessus). */
export function neededMbps(m: Mode, yuv444: boolean): number {
  return Math.ceil(bitsPerSecond(m, yuv444) / 1e6)
}

/** Le plus petit des plafonds connus (Station, débit réglé à la main). */
export function ceiling(...values: (number | null | undefined)[]): number | null {
  const known = values.filter((v): v is number => typeof v === 'number' && v > 0)
  return known.length ? Math.min(...known) : null
}

export interface Lowered {
  width: number
  height: number
  refresh: number
  /** Le plafond qu'elle respecte. */
  mbps: number
}

const even = (v: number) => Math.max(2, Math.round(v / 2) * 2)

function scaled(m: Mode, k: number, refresh: number): Mode {
  if (k >= 1) return { ...m, refresh: Math.min(m.refresh, refresh) }
  return { width: Math.min(even(m.width * k), m.width), height: Math.min(even(m.height * k), m.height), refresh: Math.min(m.refresh, refresh) }
}

/**
 * La définition ramenée, ou `null` quand l'écran tient sous le plafond : la
 * plus grande définition paire qui tient (proportions gardées) à la fréquence
 * de l'écran, jusqu'à 720 lignes ; puis 120, 100, 60, 50 ou 30 Hz ; rien ne
 * tient : 720 lignes à 30 Hz. La netteté ne baisse jamais.
 */
export function lowered(m: Mode, yuv444: boolean, ceilingMbps: number): Lowered | null {
  if (!m.width || !m.height) return null
  const budget = ceilingMbps * 1e6
  const fits = (c: Mode) => bitsPerSecond(c, yuv444) <= budget
  if (fits(m)) return null
  const native = Math.max(1, m.refresh)
  const minH = Math.min(MIN_HEIGHT, m.height)
  const rates = [native, ...REFRESH_STEPS.filter((r) => r < native && r >= MIN_REFRESH)]
  const done = (c: Mode): Lowered => ({ width: c.width, height: c.height, refresh: c.refresh, mbps: ceilingMbps })
  for (const refresh of rates) {
    const full = scaled(m, 1, refresh)
    if (fits(full)) return done(full)
    const guess = Math.sqrt(budget / bitsPerSecond(full, yuv444))
    let h = Math.min(even(m.height * guess) + 4, m.height & ~1)
    while (h >= minH) {
      const c = scaled(m, h / m.height, refresh)
      if (fits(c)) return done(c)
      h -= 2
    }
  }
  return done(scaled(m, minH / m.height, rates[rates.length - 1]))
}

/** Ce qu'une session PyroWave demande : la définition (ramenée ou non) et le débit. */
export interface PyroWavePlan {
  mode: Mode
  mbps: number
  lowered: Lowered | null
}

/** Kyber reçoit au moins le débit nécessaire à la définition retenue, jamais plus que le plafond. */
export function plan(m: Mode, yuv444: boolean, stationMax: number | null | undefined, manualMbps: number | null): PyroWavePlan {
  const cap = ceiling(stationMax, manualMbps)
  const low = cap === null ? null : lowered(m, yuv444, cap)
  const mode = low ? { width: low.width, height: low.height, refresh: low.refresh } : m
  const need = neededMbps(mode, yuv444)
  return { mode, mbps: cap === null ? need : Math.min(need, cap), lowered: low }
}
