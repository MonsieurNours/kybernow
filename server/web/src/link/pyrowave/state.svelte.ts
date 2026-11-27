// PyroWave dans cette page : la sonde du chargement (WebGPU, image de test)
// et ce qu'en déduisent les réglages et la session.

import { engineKnowsPyroWave } from '../engine'
import type { Info } from '../backend'
import type { PyroWaveOffer } from '../plan'
import type { ProbeResult } from './probe'
import { unavailable, type Unavailable } from './rules'

export const pyrowave = $state<{ probe: ProbeResult | null }>({ probe: null })

let started: Promise<ProbeResult> | null = null

/** Sonde (une fois par page) ; installe le décodeur des sessions si elle passe. Demandée seulement face à une Station
 * qui propose PyroWave, pour l'hôte (WEB-17) : un invité ou un ami n'en a pas l'usage, et la sonde réveille la carte
 * graphique « haute performance ». Le décodeur (shaders, image de test) se charge alors, à part du reste de la page. */
export function probeAtLoad(): Promise<ProbeResult> {
  if (!started) {
    started = import('./live')
      .then((m) => m.installPyroWave())
      .catch((): ProbeResult => ({ ok: false, reason: 'adapter' }))
      .then((r) => {
        pyrowave.probe = r
        if (r.ok) console.info(`KyberNow : PyroWave décodé ici (${r.adapter || 'carte graphique'}${r.subgroups ? ', sous-groupes' : ''}, sonde ${r.ms} ms)`)
        else console.info(`KyberNow : PyroWave indisponible dans ce navigateur (${r.reason})`)
        return r
      })
  }
  return started
}

/** Ce navigateur décode PyroWave : sonde passée et moteur de Kyber qui passe les paquets `pwv1` à la
 * page (correctif 0025-kyctlweb). Le moteur doit être chargé (`supportedCodecs()`). */
export function browserDecodes(): boolean {
  return !!pyrowave.probe?.ok && engineKnowsPyroWave()
}

/** Pourquoi « PyroWave » est inactif ; `null` s'il est offert. */
export function whyUnavailable(info: Info | null): Unavailable | null {
  return unavailable(info?.pyrowave, browserDecodes())
}

/** Pour `choose()` : PyroWave possible, et le plafond de la Station. */
export function offer(info: Info | null): PyroWaveOffer {
  return { usable: whyUnavailable(info) === null, stationMax: info?.pyrowave?.max_mbps ?? null }
}
