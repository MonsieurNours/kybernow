// Entrées envoyées par le moteur en mode WebSocket : chaque message est un
// paquet kynput entier, `[type u8][contenu]` (kymux-types/src/input.rs).
// Position absolue : type 2, puis écran (u32), x et y (i16), en gros-boutiste.

const MOUSE_POSITION = 2

let socket: WebSocket | null = null
let displayId: number | null = null
/** Positions envoyées par la page (diagnostic). */
export const injected = { count: 0 }

/** Repère la connexion des entrées et l'écran visé par le moteur. */
let watching = false

export function watchInputSocket() {
  if (watching) return
  watching = true
  const send = WebSocket.prototype.send
  WebSocket.prototype.send = function (this: WebSocket, data: any) {
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
 * premier de la liste de Kyber (`display_index` 0). */
export async function learnDisplayId() {
  if (displayId !== null) return
  try {
    const res = await fetch('/enumerate_displays', { credentials: 'same-origin' })
    const list = await res.json()
    const first = Array.isArray(list) ? list[0] : (list?.displays?.[0] ?? null)
    if (first && typeof first.id === 'number') displayId ??= first.id
  } catch {
    // Sans liste, on attend une position du moteur.
  }
}

/** Envoie une position absolue nous-mêmes. `false` si ce n'est pas possible
 * (pas de WebSocket : WebTransport, ou écran encore inconnu). */
export function sendPosition(x: number, y: number): boolean {
  if (!socket || socket.readyState !== WebSocket.OPEN || displayId === null) return false
  const buf = new ArrayBuffer(9)
  const view = new DataView(buf)
  view.setUint8(0, MOUSE_POSITION)
  view.setUint32(1, displayId)
  view.setInt16(5, Math.max(-32768, Math.min(32767, Math.round(x))))
  view.setInt16(7, Math.max(-32768, Math.min(32767, Math.round(y))))
  socket.send(new Uint8Array(buf))
  injected.count++
  return true
}

export function forgetInputSocket() {
  socket = null
}
