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
