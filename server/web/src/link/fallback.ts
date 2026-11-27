// L'image ne démarre pas dans ce navigateur (docs/link-ui.md, « Image qui ne
// démarre pas ») : la page recommence une fois en H.264 si elle demandait du
// HEVC, puis une fois sans le son, avant de dire l'échec. PyroWave qui ne
// s'ouvre pas (« PyroWave ne s'ouvre pas sur l'appareil ») repart d'abord dans
// le format qu'aurait pris Auto. Les réglages de l'appareil ne changent pas :
// les replis ne valent que pour cette session.

import type { CodecSupport, StreamSettings } from './engine'

export type StartFallback = 'no_pyrowave' | 'h264' | 'no_audio'

/** Replis déjà pris par cette session. */
export interface Fallbacks {
  noPyroWave: boolean
  h264: boolean
  noAudio: boolean
}

export const NO_FALLBACK: Fallbacks = { noPyroWave: false, h264: false, noAudio: false }

/** Une raison du moteur qui désigne le son (lecteur audio, worklet) : on coupe le son d'abord. */
function blamesAudio(reason: string): boolean {
  return /audio|worklet/i.test(reason)
}

/** PyroWave qui ne s'ouvre pas (le décodeur ou la carte graphique de la page, au début de l'image) : le
 * format d'Auto, une fois, avant les replis ordinaires (`nextFallback`). */
export function pyrowaveFallback(plan: Pick<StreamSettings, 'codec'>, done: Fallbacks): 'no_pyrowave' | null {
  return plan.codec === 'pyrowave' && !done.noPyroWave ? 'no_pyrowave' : null
}

/**
 * Le repli à essayer après un échec au démarrage de l'image, ou `null` : il n'en reste plus.
 * H.264 si l'image demandée était en HEVC (ou AV1) et que ce navigateur lit le H.264 ; puis le son
 * coupé s'il était demandé. Une raison qui désigne le son coupe le son d'abord (le format peut
 * rester). `decode` : une image qui n'a pas pu se décoder, où le son n'y est pour rien.
 */
export function nextFallback(
  plan: Pick<StreamSettings, 'codec' | 'audio'>,
  done: Fallbacks,
  codecs: CodecSupport[],
  reason = '',
  decode = false,
): StartFallback | null {
  const toH264 = !done.h264 && plan.codec !== 'h264' && codecs.some((c) => c.codec === 'h264')
  const mute = !decode && !done.noAudio && plan.audio
  if (mute && blamesAudio(reason)) return 'no_audio'
  if (toH264) return 'h264'
  if (mute) return 'no_audio'
  return null
}

/** Le repli retenu. */
export function take(done: Fallbacks, next: StartFallback): Fallbacks {
  return next === 'no_pyrowave' ? { ...done, noPyroWave: true } : next === 'h264' ? { ...done, h264: true } : { ...done, noAudio: true }
}

/** Les réglages de l'image avec les replis pris : H.264 (sans 4:4:4 s'il ne le lit pas), sans le son.
 * PyroWave laissé (`noPyroWave`) se règle avant, sur les préférences de la session : le format d'Auto
 * a son propre débit et sa propre définition (`choose`). */
export function applyFallbacks<P extends Pick<StreamSettings, 'codec' | 'yuv444' | 'audio'>>(plan: P, done: Fallbacks, codecs: CodecSupport[]): P {
  let out = plan
  if (done.h264 && out.codec !== 'h264') {
    const h264 = codecs.find((c) => c.codec === 'h264')
    out = { ...out, codec: 'h264', yuv444: out.yuv444 && !!h264?.yuv444 }
  }
  if (done.noAudio && out.audio) out = { ...out, audio: false }
  return out
}
