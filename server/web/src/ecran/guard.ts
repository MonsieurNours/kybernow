// Garde des entrées. Le moteur de Kyber écoute les boutons de la souris sur
// toute la fenêtre (`window`, en propagation), sans regarder la cible : un
// clic dans la barre de session cliquait aussi sur l'ordinateur, sous la barre.
// Installée avant lui, en capture, la garde ne lui laisse que les appuis qui
// visent l'image.
//
// `click` et `auxclick` ne sont pas arrêtés : le moteur ne les lit pas, et les
// arrêter en capture sur `window` couperait aussi les boutons de la page.

let isVideo: (t: EventTarget | null) => boolean = () => false

/** Boutons enfoncés sur l'image : leur relâchement part à l'ordinateur, où
 * que la souris soit lâchée (sinon le bouton resterait enfoncé là-bas). */
const buttons = new Set<number>()

function onMouseButton(e: MouseEvent) {
  if (e.type === 'mousedown') {
    if (isVideo(e.target)) {
      buttons.add(e.button)
      return
    }
  } else if (buttons.delete(e.button)) {
    return
  }
  e.stopImmediatePropagation()
}

export function installInputGuard(video: (t: EventTarget | null) => boolean) {
  isVideo = video
  window.addEventListener('mousedown', onMouseButton, true)
  window.addEventListener('mouseup', onMouseButton, true)
}

export function removeInputGuard() {
  window.removeEventListener('mousedown', onMouseButton, true)
  window.removeEventListener('mouseup', onMouseButton, true)
  buttons.clear()
  isVideo = () => false
}
