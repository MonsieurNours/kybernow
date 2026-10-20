// Entrées envoyées par le moteur en mode WebSocket : chaque message est un
// paquet kynput entier, `[type u8][contenu]` (kymux-types/src/input.rs), en
// gros-boutiste. La page peut glisser ses propres paquets sur cette connexion :
// - type 2, position absolue : écran (u32), x et y (i16) ;
// - type 10, texte : un point de code Unicode (u32), tapé par l'ordinateur
//   avec KEYEVENTF_UNICODE (Entrée, Espace, Tab, Échap et Retour y deviennent
//   leurs touches).
// En WebTransport, les entrées passent dans un flux du moteur où l'on ne peut
// rien glisser : ces envois échouent (`false`).

const MOUSE_POSITION = 2
const UNICODE = 10

let socket: WebSocket | null = null
let displayId: number | null = null
/** Positions envoyées par la page (diagnostic). */
export const injected = { count: 0 }

/** Repère la connexion des entrées (dès sa création) et l'écran visé par le moteur. */
let watching = false

export function watchInputSocket() {
  if (watching) return
  watching = true
  const Native = window.WebSocket
  // Le moteur crée ses connexions par le constructeur global : on la repère
  // avant son premier envoi (du texte peut partir avant le premier mouvement).
  class Watched extends Native {
    constructor(url: string | URL, protocols?: string | string[]) {
      super(url, protocols)
      if (String(url).includes('/websocket/inputs')) socket = this
    }
  }
  window.WebSocket = Watched
  const send = Native.prototype.send
  Native.prototype.send = function (this: WebSocket, data: any) {
    if (this.url.includes('/websocket/inputs')) {
      socket = this
      const bytes = data instanceof Uint8Array ? data : data instanceof ArrayBuffer ? new Uint8Array(data) : null
      if (bytes && bytes.byteLength === 9 && bytes[0] === MOUSE_POSITION) {
        displayId = new DataView(bytes.buffer, bytes.byteOffset, 9).getUint32(1)
      }
    }
    return send.call(this, data)
  }
}

/** Écran visé, à défaut d'une position déjà envoyée par le moteur : le
 * premier de la liste de Kyber (`display_index` 0). Rend la liste lue. */
export async function learnDisplayId(): Promise<any[] | null> {
  try {
    const res = await fetch('/enumerate_displays', { credentials: 'same-origin' })
    const body = await res.json()
    const list: any[] | null = Array.isArray(body) ? body : Array.isArray(body?.displays) ? body.displays : null
    const first = list?.[0]
    if (first && typeof first.id === 'number') displayId ??= first.id
    return list
  } catch {
    // Sans liste, on attend une position du moteur.
    return null
  }
}

/** Écran de l'ordinateur que suit l'image (et que visent les positions). */
export function currentDisplayId(): number | null {
  return displayId
}

/** La connexion WebSocket des entrées est-elle ouverte ? */
export function canSendPackets(): boolean {
  return !!socket && socket.readyState === WebSocket.OPEN
}

/** Envoie un paquet kynput `[type][contenu]`. `false` sans connexion WebSocket. */
export function sendPacket(type: number, payload: Uint8Array): boolean {
  if (!socket || socket.readyState !== WebSocket.OPEN) return false
  const msg = new Uint8Array(1 + payload.byteLength)
  msg[0] = type
  msg.set(payload, 1)
  socket.send(msg)
  return true
}

/** Envoie une position absolue nous-mêmes. `false` si ce n'est pas possible
 * (pas de WebSocket : WebTransport, ou écran encore inconnu). */
export function sendPosition(x: number, y: number): boolean {
  if (displayId === null) return false
  const payload = new Uint8Array(8)
  const view = new DataView(payload.buffer)
  view.setUint32(0, displayId)
  view.setInt16(4, Math.max(-32768, Math.min(32767, Math.round(x))))
  view.setInt16(6, Math.max(-32768, Math.min(32767, Math.round(y))))
  if (!sendPacket(MOUSE_POSITION, payload)) return false
  injected.count++
  return true
}

/** Tape du texte sur l'ordinateur, caractère par caractère (paquets
 * Unicode) : juste quelle que soit la disposition du clavier de l'ordinateur.
 * `false` sans connexion WebSocket (rien n'est envoyé). */
export function sendText(text: string): boolean {
  if (!canSendPackets()) return false
  for (const ch of text.replace(/\r\n?/g, '\n')) {
    const cp = ch.codePointAt(0)!
    // Moitié de paire isolée : l'ordinateur la refuserait.
    if (cp >= 0xd800 && cp <= 0xdfff) continue
    const payload = new Uint8Array(4)
    new DataView(payload.buffer).setUint32(0, cp)
    sendPacket(UNICODE, payload)
  }
  return true
}

/** Fin de session ou relance de l'image : la connexion des entrées et l'écran
 * visé sont à réapprendre (l'écran suivi peut avoir changé). */
export function forgetInputSocket() {
  socket = null
  displayId = null
}
