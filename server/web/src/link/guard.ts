// Garde des entrées. Le moteur de Kyber écoute les boutons de la souris sur
// toute la fenêtre (`window`, en propagation) et le clavier sur `document`,
// sans regarder la cible ni le focus. Installée avant lui, en capture :
// - elle ne lui laisse que les appuis de souris qui visent l'image (un clic
//   dans la barre de session cliquerait aussi sur l'ordinateur, sous la barre) ;
// - elle suit les touches et les boutons enfoncés, et les relâche sur
//   l'ordinateur quand la page perd la main (autre fenêtre, onglet masqué,
//   souris rendue) : sinon Maj, Alt ou un clic resteraient tenus là-bas ;
// - elle remet la molette en pixels : le moteur ignore tout `deltaMode` autre
//   que DOM_DELTA_PIXEL (Firefox compte en lignes : sa molette serait muette),
//   et sur la couche tactile, qui recouvre la toile, la molette n'atteindrait
//   pas le moteur (il l'écoute sur la toile).
//
// `click` et `auxclick` ne sont pas arrêtés : le moteur ne les lit pas, et les
// arrêter en capture sur `window` couperait aussi les boutons de la page.

interface GuardOptions {
  /** La cible est-elle l'image (toile ou couche tactile) ? */
  isVideo: (t: EventTarget | null) => boolean
  /** La toile, où renvoyer les relâchements. */
  canvas: () => HTMLElement | null
  /** Après un relâchement général (modificateurs collants à oublier). */
  onRelease: () => void
}

let options: GuardOptions | null = null

/** Boutons enfoncés sur l'image : leur relâchement part à l'ordinateur, où
 * que la souris soit lâchée (sinon le bouton resterait enfoncé là-bas). */
const buttons = new Set<number>()

/** Touches enfoncées, par `code` (ce que lit le moteur), avec leur `key`. */
const keys = new Map<string, string>()

function onMouseButton(e: MouseEvent) {
  if (e.type === 'mousedown') {
    if (options?.isVideo(e.target)) {
      buttons.add(e.button)
      return
    }
  } else if (buttons.delete(e.button)) {
    return
  }
  e.stopImmediatePropagation()
}

function onKey(e: KeyboardEvent) {
  if (!e.code) return
  if (e.type === 'keydown') keys.set(e.code, e.key)
  else keys.delete(e.code)
}

/** Pixels par ligne. Firefox compte trois lignes par cran ; le moteur divise
 * par 120 (un cran pour Windows) : à 40 px la ligne, un cran de Firefox fait
 * un cran sur l'ordinateur (Chrome compte 100 px par cran sous Windows). À
 * 16 px, la taille d'une ligne de texte, il n'en ferait que 0,4. */
const LINE_PX = 40

function onWheel(e: WheelEvent) {
  if (!e.isTrusted || !options || !options.isVideo(e.target)) return
  const canvas = options.canvas()
  if (!canvas) return
  // `deltaMode` lu en premier : Firefox garde alors les lignes d'une molette
  // (et les pixels d'un pavé tactile) au lieu d'un compte approché.
  const mode = e.deltaMode
  if (mode === WheelEvent.DOM_DELTA_PIXEL && e.target === canvas) return
  const k = mode === WheelEvent.DOM_DELTA_LINE ? LINE_PX : mode === WheelEvent.DOM_DELTA_PAGE ? window.innerHeight : 1
  e.preventDefault()
  e.stopImmediatePropagation()
  canvas.dispatchEvent(
    new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: e.clientX,
      clientY: e.clientY,
      deltaX: e.deltaX * k,
      deltaY: e.deltaY * k,
      deltaMode: WheelEvent.DOM_DELTA_PIXEL,
      ctrlKey: e.ctrlKey,
      shiftKey: e.shiftKey,
      altKey: e.altKey,
      metaKey: e.metaKey,
    }),
  )
}

const MENU_KEYS = ['MetaLeft', 'MetaRight', 'AltLeft', 'AltRight']

/** Relâche sur l'ordinateur tout ce que la page y tient enfoncé. */
export function releaseAll() {
  const target = options?.canvas() ?? document.body
  const key = (type: 'keydown' | 'keyup', code: string, k: string) =>
    target.dispatchEvent(new KeyboardEvent(type, { key: k, code, bubbles: true, cancelable: true }))
  // ⊞ ou Alt relâché seul ouvre le menu Démarrer ou la barre de menus de
  // l'app : un appui sur Ctrl entre les deux l'en empêche (comme AutoHotkey).
  if (MENU_KEYS.some((c) => keys.has(c)) && !keys.has('ControlLeft') && !keys.has('ControlRight')) {
    key('keydown', 'ControlLeft', 'Control')
    key('keyup', 'ControlLeft', 'Control')
  }
  for (const [code, k] of [...keys]) key('keyup', code, k)
  keys.clear()
  for (const button of [...buttons]) {
    target.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, cancelable: true, button, buttons: 0 }))
  }
  buttons.clear()
  options?.onRelease()
}

function onBlur() {
  releaseAll()
}

function onVisibility() {
  if (document.visibilityState === 'hidden') releaseAll()
}

function onPointerLock() {
  if (!document.pointerLockElement) releaseAll()
}

export function installInputGuard(o: GuardOptions) {
  options = o
  window.addEventListener('mousedown', onMouseButton, true)
  window.addEventListener('mouseup', onMouseButton, true)
  window.addEventListener('keydown', onKey, true)
  window.addEventListener('keyup', onKey, true)
  window.addEventListener('wheel', onWheel, { capture: true, passive: false })
  window.addEventListener('blur', onBlur)
  document.addEventListener('visibilitychange', onVisibility)
  document.addEventListener('pointerlockchange', onPointerLock)
}

export function removeInputGuard() {
  window.removeEventListener('mousedown', onMouseButton, true)
  window.removeEventListener('mouseup', onMouseButton, true)
  window.removeEventListener('keydown', onKey, true)
  window.removeEventListener('keyup', onKey, true)
  window.removeEventListener('wheel', onWheel, true)
  window.removeEventListener('blur', onBlur)
  document.removeEventListener('visibilitychange', onVisibility)
  document.removeEventListener('pointerlockchange', onPointerLock)
  buttons.clear()
  keys.clear()
  options = null
}
