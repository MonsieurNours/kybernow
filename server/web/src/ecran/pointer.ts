// Souris : le moteur de Kyber capture le pointeur (déplacements bruts) dès
// que l'ordinateur masque son curseur, ce que Windows fait sans cesse quand
// aucune souris n'y est branchée. Comme l'app Mac, on décide nous-mêmes :
// capturé en mode jeu seulement.

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

/** Pointeur de l'ordinateur visible, hors capture : le moteur envoie à chaque
 * mouvement un déplacement relatif puis la position absolue, et l'ordinateur
 * applique les deux. Le relatif passe par l'accélération de Windows et se perd
 * hors de la fenêtre : les pointeurs se décalent. On ne laisse partir que la
 * position (déplacement nul), comme l'app Mac en mode bureau.
 * Pointeur masqué (Windows le cache à la moindre frappe) : le déplacement
 * passe, Windows réaffiche son pointeur, le moteur revient aux positions. */
function absoluteOnly(e: MouseEvent) {
  if (!e.isTrusted || remoteHidden || document.pointerLockElement || (!e.movementX && !e.movementY)) return
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
