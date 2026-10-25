// Souris : le moteur de Kyber capture le pointeur (déplacements bruts) dès
// que l'ordinateur masque son curseur, ce que Windows fait sans cesse quand
// aucune souris n'y est branchée. Comme l'app Mac, on décide nous-mêmes :
// capturé en mode Jeu, ou en Auto quand un jeu est en plein écran sur
// l'ordinateur (`pointerAllowed()` dans Session.svelte).

const original = Element.prototype.requestPointerLock
let allowed: () => boolean = () => true

export function installPointerGate(isAllowed: () => boolean) {
  allowed = isAllowed
  Element.prototype.requestPointerLock = function (this: Element, options?: PointerLockOptions) {
    if (!allowed()) return Promise.reject(new DOMException('Souris en mode bureau', 'NotAllowedError'))
    return original.call(this, options) as Promise<void>
  } as typeof original
}

export function removePointerGate() {
  Element.prototype.requestPointerLock = original
}

/** Rend le pointeur s'il est capturé. */
export function releasePointer() {
  if (document.pointerLockElement) document.exitPointerLock()
}

/** Le pointeur de l'ordinateur est-il masqué ? Le moteur le dit en posant
 * `cursor: none` sur la toile ; il passe alors en souris relative et n'envoie
 * plus la position, seulement le déplacement. */
let remoteHidden = false

export function setRemoteCursorHidden(hidden: boolean) {
  remoteHidden = hidden
}

export function isRemoteCursorHidden() {
  return remoteHidden
}

import { engineInputs, sendPosition } from './inputs'

/** Position sur l'écran de l'ordinateur d'un point de la page, ou null hors de l'image. */
let toHost: (clientX: number, clientY: number) => { x: number; y: number } | null = () => null

export function setPositionMapper(fn: typeof toHost) {
  toHost = fn
}

/** Envoie nous-mêmes la position d'un point de la page sur l'ordinateur
 * (WebSocket seulement). `false` hors de l'image ou sans WebSocket. */
export function sendHostPosition(clientX: number, clientY: number): boolean {
  const p = toHost(clientX, clientY)
  return !!p && sendPosition(p.x, p.y)
}

/** Pointeur de l'ordinateur visible, hors capture : le moteur envoie à chaque
 * mouvement un déplacement relatif puis la position absolue, et l'ordinateur
 * applique les deux. Le relatif passe par l'accélération de Windows et se perd
 * hors de la fenêtre : les pointeurs se décalent. On ne laisse partir que la
 * position (déplacement nul), comme l'app Mac en mode bureau.
 * Pointeur masqué (Windows le cache à la moindre frappe) : le déplacement
 * passe, Windows réaffiche son pointeur, le moteur revient aux positions.
 * Le moteur corrigé garde alors les positions lui-même (`engineInputs()`). */
function absoluteOnly(e: MouseEvent) {
  if (!e.isTrusted || document.pointerLockElement || (!e.movementX && !e.movementY)) return
  if (remoteHidden && !engineInputs()) {
    // Le moteur n'enverra que le déplacement, et Windows ne réaffiche son
    // pointeur que sur une position absolue : on l'envoie nous-mêmes (en
    // WebSocket seulement). Sinon, le déplacement passe tel quel.
    if (!sendHostPosition(e.clientX, e.clientY)) return
  }
  e.stopImmediatePropagation()
  ;(e.target ?? window).dispatchEvent(
    new MouseEvent('mousemove', {
      bubbles: true,
      cancelable: true,
      clientX: e.clientX,
      clientY: e.clientY,
      screenX: e.screenX,
      screenY: e.screenY,
      button: e.button,
      buttons: e.buttons,
      ctrlKey: e.ctrlKey,
      altKey: e.altKey,
      shiftKey: e.shiftKey,
      metaKey: e.metaKey,
      movementX: 0,
      movementY: 0,
    }),
  )
}

export function installAbsoluteMouse() {
  window.addEventListener('mousemove', absoluteOnly, true)
}

export function removeAbsoluteMouse() {
  window.removeEventListener('mousemove', absoluteOnly, true)
}
