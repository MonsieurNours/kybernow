// Ce que demande une session : format, débit, transport. Mêmes règles que
// l'app Mac (Model.swift), bornées à ce qu'un navigateur décode bien.

import type { CodecSupport, StreamSettings } from './engine'
import type { Mode } from './device'
import type { Prefs } from './prefs'

const REFERENCE_PIXEL_RATE = 3840 * 2160 * 60
export const MIN_MBPS = 10
export const MAX_MBPS = 150

/** Débit de base du profil, calé sur la 4K à 60 Hz. */
function baseMbps(p: Prefs): number {
  return p.profile === 'jeu' ? 60 : 90
}

export interface Plan extends StreamSettings {
  /** Débit en Mbit/s (affiché). */
  mbps: number
  /** Le profil voulait le 4:4:4 mais ce navigateur ne le décode pas. */
  lost444: boolean
}

export function choose(p: Prefs, mode: Mode, codecs: CodecSupport[]): Plan {
  const hevc = codecs.find((c) => c.codec === 'hevc')
  const h264 = codecs.find((c) => c.codec === 'h264')
  let codec: 'h264' | 'hevc' = p.codec === 'h264' ? 'h264' : p.codec === 'hevc' ? 'hevc' : hevc?.hardware ? 'hevc' : 'h264'
  if (codec === 'hevc' && !hevc) codec = 'h264'
  const support = codec === 'hevc' ? hevc : h264
  const want444 = p.profile === 'bureau'
  const yuv444 = want444 && !!support?.yuv444

  let mbps = baseMbps(p)
  if (want444 && !yuv444) mbps += Math.round(mbps / 3)
  if (codec === 'h264') mbps += Math.round(mbps / 2)
  const scale = Math.pow((mode.width * mode.height * mode.refresh) / REFERENCE_PIXEL_RATE, 0.75)
  mbps = Math.round(mbps * Math.min(Math.max(scale, 0.4), 3))
  if (p.bitrate) mbps = p.bitrate
  mbps = Math.min(Math.max(mbps, MIN_MBPS), MAX_MBPS)

  const reliable = p.transport === 'fiable' || (p.transport === 'auto' && p.profile === 'bureau')
  return {
    codec,
    yuv444,
    bitrate: mbps * 1_000_000,
    mbps,
    reliable,
    audio: p.audio,
    clipboard: p.clipboard,
    lost444: want444 && !yuv444,
  }
}
