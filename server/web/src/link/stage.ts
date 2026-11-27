// La toile de la session dans la page : une toile neuve par image (l'ancienne appartient au moteur),
// l'image aux proportions de la toile, la place que laisse le clavier de l'appareil, le pointeur de
// l'ordinateur gardé visible hors capture, et la position d'un point de la page sur l'écran de
// l'ordinateur.

import { setRemoteCursorHidden } from './pointer'

type Size = { width: number; height: number }

export const CANVAS_ID = 'kn-canvas'

export function canvas(): HTMLElement | null {
  return document.getElementById(CANVAS_ID)
}

export class Stage {
  /** La zone de l'image, la boîte de la toile, et la couche tactile posée dessus. */
  el: HTMLElement | null = null
  box: HTMLElement | null = null
  touchLayer: HTMLElement | null = null
  /** La taille de l'image décodée. */
  private size: () => Size
  /** Le rectangle de la toile, gardé tant qu'elle ne change ni de taille ni de place (WEB-18 : sans lui,
   * une mise en page forcée à chaque mouvement de souris). */
  private rect: { el: Element; rect: DOMRect } | null = null
  private rectWatch: ResizeObserver | null = null
  private cursorWatch: MutationObserver | null = null
  private cursorFix = () => {}

  constructor(size: () => Size) {
    this.size = size
  }

  forgetRect = () => {
    this.rect = null
  }

  private canvasRect(el: Element): DOMRect {
    if (this.rect?.el === el) return this.rect.rect
    this.rectWatch?.disconnect()
    this.rectWatch = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(this.forgetRect) : null
    this.rectWatch?.observe(el)
    this.rect = { el, rect: el.getBoundingClientRect() }
    return this.rect.rect
  }

  /** La toile a les proportions de l'image : un point de la toile donne une position sur l'écran de
   * l'ordinateur (taille de l'image reçue) ; hors de la toile, aucune. */
  position = (cx: number, cy: number): { x: number; y: number } | null => {
    const c = canvas()
    const size = this.size()
    if (!c || !size.width) return null
    const r = this.canvasRect(c)
    if (cx < r.left || cy < r.top || cx >= r.right || cy >= r.bottom) return null
    return {
      x: Math.min(size.width - 1, ((cx - r.left) * size.width) / r.width),
      y: Math.min(size.height - 1, ((cy - r.top) * size.height) / r.height),
    }
  }

  fit = () => {
    const c = canvas()
    const size = this.size()
    if (!c || !size.width || !this.el) return
    const r = this.el.getBoundingClientRect()
    const scale = Math.min(r.width / size.width, r.height / size.height)
    c.style.width = `${Math.floor(size.width * scale)}px`
    c.style.height = `${Math.floor(size.height * scale)}px`
  }

  /** Clavier de l'appareil ouvert : la zone visible (`visualViewport`, que le clavier réduit sur iPhone et
   * iPad sans toucher à la page) donne la place de l'image et celle de la rangée de touches, juste
   * au-dessus du clavier. */
  trackViewport(keyboardOpen: boolean) {
    const vv = window.visualViewport
    const page = this.el?.parentElement
    if (!page) return
    if (vv && keyboardOpen) {
      page.style.setProperty('--vv-top', `${vv.offsetTop}px`)
      page.style.setProperty('--vv-h', `${vv.height}px`)
    } else {
      page.style.removeProperty('--vv-top')
      page.style.removeProperty('--vv-h')
    }
    this.fit()
  }

  /** Une toile neuve pour le moteur (la précédente lui appartient). */
  freshCanvas = () => {
    if (!this.box) return
    this.box.querySelector('canvas')?.remove()
    const c = document.createElement('canvas')
    c.id = CANVAS_ID
    c.width = 800
    c.height = 600
    c.tabIndex = 0
    this.box.appendChild(c)
    this.keepPointerVisible(c)
  }

  /** Le moteur dessine le pointeur de l'ordinateur en curseur de la toile, et le masque (`cursor: none`)
   * quand l'ordinateur le dit caché, ce que Windows fait sans cesse sans souris branchée. Hors capture,
   * on garde une flèche. */
  private keepPointerVisible(c: HTMLCanvasElement) {
    this.cursorWatch?.disconnect()
    setRemoteCursorHidden(false)
    const fix = () => {
      const cur = c.style.cursor
      // Le moteur pose `none` (masqué) ou l'image du pointeur ; `default` vient de nous.
      if (cur === 'none') setRemoteCursorHidden(true)
      else if (cur && cur !== 'default') setRemoteCursorHidden(false)
      if (cur === 'none' && !document.pointerLockElement) c.style.cursor = 'default'
      // La couche tactile recouvre la toile : le pointeur d'une souris ou d'un pavé (iPad avec clavier)
      // y prend celui de l'ordinateur.
      if (this.touchLayer) this.touchLayer.style.cursor = c.style.cursor
    }
    this.cursorWatch = new MutationObserver(fix)
    this.cursorWatch.observe(c, { attributes: true, attributeFilter: ['style'] })
    // Souris rendue après une capture : la flèche revient aussi.
    document.removeEventListener('pointerlockchange', this.cursorFix)
    this.cursorFix = fix
    document.addEventListener('pointerlockchange', this.cursorFix)
    fix()
  }

  destroy() {
    this.cursorWatch?.disconnect()
    this.rectWatch?.disconnect()
    document.removeEventListener('pointerlockchange', this.cursorFix)
  }
}
