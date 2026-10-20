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

/** Hors capture, le moteur envoie à chaque mouvement un déplacement relatif
 * (`movementX/Y`) puis la position absolue, et l'ordinateur applique les deux.
 * Le relatif passe par l'accélération de Windows et se perd hors de la
 * fenêtre : les pointeurs se décalent. Comme l'app Mac en mode bureau, on ne
 * laisse partir que la position : le moteur reçoit un mouvement à déplacement nul. */
function absoluteOnly(e: MouseEvent) {
  if (!e.isTrusted || document.pointerLockElement || (!e.movementX && !e.movementY)) return
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
