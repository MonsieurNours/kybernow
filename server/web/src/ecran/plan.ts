// Ce que demande une session : format, débit, transport. Mêmes règles que
// le cœur des apps (`apps/core/src/profile.rs`), bornées à ce qu'un
// navigateur décode bien (150 Mbit/s au plus).

import type { CodecSupport, StreamSettings } from './engine'
import { hasWebTransport, isFirefox, isTouch, isWebKit, type Mode } from './device'
import { engineInputs } from './inputs'
import type { Prefs } from './prefs'

/** Pourquoi l'image passe par WebSocket dans ce navigateur. */
export type WebSocketReason = 'touch' | 'webkit' | 'firefox' | 'unsupported'

/**
 * Transport de l'image. WebTransport (direct, vérifié par l'empreinte du
 * certificat) quand c'est possible ; WebSocket (relayé par la console) :
 * - en tactile avec le moteur d'origine, ou dans Safari et sur iPhone et
 *   iPad : c'est sur la connexion WebSocket des entrées que la page envoie le
 *   texte tapé (paquets Unicode) et la position du doigt quand le pointeur de
 *   l'ordinateur est masqué ; en WebTransport, le moteur ne laisse rien
 *   glisser dans son flux. Le moteur corrigé fait ces envois lui-même
 *   (`engineInputs()`) : WebSocket n'est pas nécessaire pour eux ;
 * - dans Safari et sur iPhone et iPad : leur WebTransport ne vérifie pas un
 *   certificat par son empreinte ;
 * - dans Firefox (avec H.264 par défaut), la combinaison retenue pour lui ;
 * - sans WebTransport.
 * À lire une fois le moteur chargé (`supportedCodecs()`), qui dit s'il est corrigé.
 */
export function websocketReason(): WebSocketReason | null {
  if (isTouch && !engineInputs()) return 'touch'
  if (isWebKit) return 'webkit'
  if (isFirefox) return 'firefox'
  if (!hasWebTransport) return 'unsupported'
  return null
}

export function preferredProtocol(): 'kymux' | 'websocket' {
  return websocketReason() ? 'websocket' : 'kymux'
}

const REFERENCE_PIXEL_RATE = 3840 * 2160 * 60
export const MIN_MBPS = 10
export const MAX_MBPS = 150

/** Débit de base du profil, calé sur la 4K à 60 Hz. */
function baseMbps(p: Prefs): number {
  return p.profile === 'jeu' ? 60 : p.profile === 'max' ? 300 : 90
}

export interface Plan extends StreamSettings {
  /** Débit en Mbit/s (affiché). */
  mbps: number
  /** Le profil voulait le 4:4:4 mais ce navigateur ne le décode pas. */
  lost444: boolean
}

export function choose(p: Prefs, mode: Mode, codecs: CodecSupport[], protocol: 'kymux' | 'websocket' = preferredProtocol()): Plan {
  const hevc = codecs.find((c) => c.codec === 'hevc')
  const h264 = codecs.find((c) => c.codec === 'h264')
  // Auto : HEVC si la carte graphique le décode, sauf dans Firefox (H.264).
  let codec: 'h264' | 'hevc' = p.codec === 'h264' ? 'h264' : p.codec === 'hevc' ? 'hevc' : hevc?.hardware && !isFirefox ? 'hevc' : 'h264'
  if (codec === 'hevc' && !hevc) codec = 'h264'
  const support = codec === 'hevc' ? hevc : h264
  // Bureau et Max : texte net (4:4:4) quand ce navigateur le décode.
  const want444 = p.profile === 'bureau' || p.profile === 'max'
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
    protocol,
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
