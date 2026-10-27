// Manettes d'un invité du partage (docs/kybernow-share.md, « Les invités »).
//
// Le moteur de Kyber lit les manettes lui-même (API Gamepad) et les envoie sur sa propre
// connexion d'entrées, où la page ne peut rien filtrer. Pour un invité qui ne joue pas
// (spectateur), la page ne lui montre donc aucune manette : `getGamepads()` rend des places
// vides et les événements de branchement ne lui arrivent pas ; quand l'hôte le fait jouer, les
// manettes déjà branchées lui sont annoncées, et elles lui sont reprises de même. Kyber refuse de
// toute façon ce que le rôle ne permet pas : ceci évite surtout d'offrir au moteur des manettes
// qu'il ne doit pas envoyer (et d'en montrer le voyant à un spectateur).

const native = typeof Navigator !== 'undefined' ? Navigator.prototype.getGamepads : undefined

let allowed: () => boolean = () => true
let installed = false
/** Nos propres événements de branchement (rôle changé) passent la garde. */
let ours = false

function onPlug(e: Event) {
  if (!ours && !allowed()) e.stopImmediatePropagation()
}

/** La page ne montre les manettes au moteur que tant que `isAllowed()` est vrai. */
export function installGamepadGate(isAllowed: () => boolean) {
  allowed = isAllowed
  if (installed || !native) return
  installed = true
  Navigator.prototype.getGamepads = function (this: Navigator) {
    const list = native.call(this)
    return allowed() ? list : list.map(() => null)
  }
  window.addEventListener('gamepadconnected', onPlug, true)
  window.addEventListener('gamepaddisconnected', onPlug, true)
}

export function removeGamepadGate() {
  if (installed && native) {
    Navigator.prototype.getGamepads = native
    window.removeEventListener('gamepadconnected', onPlug, true)
    window.removeEventListener('gamepaddisconnected', onPlug, true)
  }
  installed = false
  allowed = () => true
}

/**
 * Le rôle a changé pendant que des manettes sont branchées : le moteur les voit arriver (`now` vrai,
 * l'hôte fait jouer) ou partir (`now` faux, il fait regarder). À appeler après le changement de `allowed()`.
 */
export function gamepadsChanged(now: boolean) {
  const list = native?.call(navigator) ?? []
  ours = true
  try {
    for (const pad of list) {
      if (!pad) continue
      try {
        window.dispatchEvent(new GamepadEvent(now ? 'gamepadconnected' : 'gamepaddisconnected', { gamepad: pad }))
      } catch {
        // Navigateur sans GamepadEvent constructible : le moteur le verra à sa prochaine lecture.
      }
    }
  } finally {
    ours = false
  }
}
