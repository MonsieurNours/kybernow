// Le plan d'une session et ses replis quand l'image ne démarre pas (docs/link-ui.md,
// « Image qui ne démarre pas »). `npm test`.

import { describe, expect, it, vi } from 'vitest'

// `device.ts` lit le navigateur dès son chargement : un Chrome de bureau.
vi.mock('./device', () => ({ hasWebTransport: true, isFirefox: false, isTouch: false, isWebKit: false }))
vi.mock('./inputs', () => ({ engineInputs: () => true }))

import { choose } from './plan'
import { applyFallbacks, NO_FALLBACK, nextFallback, take } from './fallback'
import { startFailureDetail, startFailureNote, isStartFailure } from './engine'
import { defaults } from './prefs'

const mode = { width: 1920, height: 1200, refresh: 60 }
const both = [
  { codec: 'h264', yuv444: false, hardware: true },
  { codec: 'hevc', yuv444: false, hardware: true },
]

describe('plan', () => {
  it('prend le HEVC en Auto quand la carte le décode', () => {
    expect(choose({ ...defaults, profile: 'jeu' }, mode, both, 'websocket').codec).toBe('hevc')
  })

  it('prend le H.264 quand le HEVC ne passe que par le processeur', () => {
    const soft = [both[0], { ...both[1], hardware: false }]
    expect(choose(defaults, mode, soft, 'websocket').codec).toBe('h264')
  })

  it('donne plus de débit au H.264 qu’au HEVC', () => {
    const hevc = choose({ ...defaults, profile: 'jeu' }, mode, both, 'websocket')
    const h264 = choose({ ...defaults, profile: 'jeu', codec: 'h264' }, mode, both, 'websocket')
    expect(h264.mbps).toBeGreaterThan(hevc.mbps)
  })
})

describe('replis au démarrage', () => {
  const hevc = { codec: 'hevc' as const, audio: true }

  it('essaie le H.264, puis sans le son, puis s’arrête', () => {
    let done = NO_FALLBACK
    const first = nextFallback(hevc, done, both, 'streaming_start_failed')
    expect(first).toBe('h264')
    done = take(done, first!)
    const second = nextFallback({ codec: 'h264', audio: true }, done, both, 'streaming_start_failed')
    expect(second).toBe('no_audio')
    done = take(done, second!)
    expect(nextFallback({ codec: 'h264', audio: false }, done, both, 'streaming_start_failed')).toBeNull()
  })

  it('coupe le son d’abord quand le moteur accuse le lecteur audio', () => {
    expect(nextFallback(hevc, NO_FALLBACK, both, "streaming_start_failed: AbortError: Unable to load a worklet's module")).toBe('no_audio')
  })

  it('pas de H.264 si ce navigateur ne le lit pas', () => {
    expect(nextFallback(hevc, NO_FALLBACK, [both[1]], 'streaming_start_failed')).toBe('no_audio')
  })

  it('une image qui ne se décode pas ne coupe jamais le son', () => {
    expect(nextFallback(hevc, NO_FALLBACK, both, 'decode_error', true)).toBe('h264')
    expect(nextFallback({ codec: 'h264', audio: true }, { h264: true, noAudio: false }, both, 'decode_error', true)).toBeNull()
  })

  it('applique les replis sans toucher au reste', () => {
    const plan = { codec: 'hevc' as const, yuv444: true, audio: true, bitrate: 1 }
    expect(applyFallbacks(plan, { h264: true, noAudio: true }, both)).toEqual({ codec: 'h264', yuv444: false, audio: false, bitrate: 1 })
    expect(applyFallbacks(plan, NO_FALLBACK, both)).toBe(plan)
  })
})

describe('raison du moteur', () => {
  it('relit la raison écrite en console', () => {
    const lines = [{ at: 100, text: 'Streaming start failed: Js(JsValue(NotAllowedError: no output device)). Stop client' }]
    expect(startFailureNote(lines, 200)).toBe('Js(JsValue(NotAllowedError: no output device))')
    expect(startFailureDetail('streaming_start_failed', 'X')).toBe('streaming_start_failed: X')
    expect(startFailureNote(lines, 10_000)).toBeNull()
  })

  it('reconnaît un échec au démarrage', () => {
    expect(isStartFailure('streaming_start_failed')).toBe(true)
    expect(isStartFailure('start_streaming: Error: No decoding support for Hevc')).toBe(true)
    expect(isStartFailure('unauthorized')).toBe(false)
  })
})
