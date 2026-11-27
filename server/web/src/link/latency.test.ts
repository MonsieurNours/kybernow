// Les mesures d'une image (latency.ts) : les six étapes du trajet, la latence, la seconde écoulée. `npm test`.

import { describe, expect, it } from 'vitest'
import { frameChain, frameLatency, LatencyWindow } from './latency'

// Une image : capture à 1 000 µs, encodage 1 ms plus tard, 4 ms d'encodage, envoi, réseau, décodage, écran.
const frame = { acquired: 1000, encoding: 2000, encoded: 6000, sent: 7000, received: 9000, decoded: 11000, displayed: 15000 }

describe('trajet d’une image', () => {
  it('six étapes, comme le cœur', () => {
    expect(frameChain(frame)).toEqual([1, 4, 1, 2, 2, 4])
    expect(frameLatency(frame)).toBe(14)
  })

  it('une étape sans ses dates reste sans mesure', () => {
    const { encoding: _e, encoded: _d, ...old } = frame
    expect(frameChain(old)).toEqual([null, null, null, 2, 2, 4])
    expect(frameLatency({ ...frame, acquired: undefined })).toBeNull()
  })

  it('l’écran à défaut de l’affichage, et un réseau un rien négatif ramené à zéro', () => {
    expect(frameChain({ ...frame, displayed: undefined, prepared: 12000 })[5]).toBe(1)
    expect(frameChain({ ...frame, received: 6900 })[3]).toBe(0)
  })
})

describe('fenêtre des mesures', () => {
  it('la seconde écoulée : médianes, part mesurée, images sautées', () => {
    let now = 0
    const w = new LatencyWindow(() => now)
    for (let i = 0; i < 4; i++) {
      now += 100
      w.push(frame)
    }
    w.push({ ...frame, acquired: undefined })
    w.push({ skipped: 8000 })
    const s = w.second()
    expect(s.latency).toBe(14)
    expect(s.chain[1]).toBe(4)
    expect(s.shown).toBe(5)
    expect(s.measuredPct).toBe(80)
    expect(s.skipped).toBe(1)
    expect(w.second().skipped).toBe(0)
  })

  it('pas de médiane avant cinq mesures', () => {
    let now = 0
    const w = new LatencyWindow(() => now)
    for (let i = 0; i < 4; i++) {
      now += 100
      w.push(frame)
    }
    expect(w.median()).toBeNull()
    w.push(frame)
    expect(w.median()).toBe(14)
  })
})
