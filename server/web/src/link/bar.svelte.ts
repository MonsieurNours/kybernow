// La barre de session au pointeur (docs/link-ui.md, « Barre de session au pointeur ») : elle s'ouvre
// quand le pointeur reste un quart de seconde contre le bord haut de l'image (à 2 points au plus), dans
// une zone centrée de sa largeur, seulement si la page a le focus et que la souris n'est pas capturée,
// ni un bouton tenu ; elle se referme 0,4 s après que le pointeur l'a quittée, jamais pendant qu'un de
// ses menus est ouvert. Un seul menu ou panneau à la fois. Sa forme suit la largeur de la fenêtre
// (« Barre étroite », menu « Plus » : overflow.ts). Au toucher, la barre compacte s'ouvre par sa languette.

import { tick, untrack } from 'svelte'
import { hasTouchScreen, isTouch } from './device'
import { fitBar, FULL_BAR, type BarFit, type MoreItem } from './overflow'
import type { LiveSession } from './livesession.svelte'

/** Les menus et panneaux de la barre. */
export type Menu = 'stats' | 'keys' | 'mouse' | 'profile' | 'more' | 'share' | 'lend' | 'giveBack' | 'quality'

/** Largeur du trait du haut, zone du bord tant que la barre n'est pas mesurée. */
export const EDGE_WIDTH = 220
/** Distance au bord haut de la session, en pixels, qui compte comme « contre le bord ». */
const EDGE_REACH = 2
/** Le pointeur doit rester contre le bord haut ce temps-là pour ouvrir la barre. */
const EDGE_DWELL = 250
/** La barre se referme ce temps après que le pointeur l'a quittée. */
export const BAR_LEAVE = 400
/** Barre ouverte sans pointeur dessus (doigt, raccourci) : elle se referme après ce temps. */
export const BAR_IDLE = 2500
/** Écart entre deux commandes de la barre (`gap` de `.bar`). */
const BAR_GAP = 4
/** Largeur d'un menu sous la barre (le menu le plus large). */
const MENU_WIDTH = 300

/** Appareil tactile sans souris : le trait reste, la page n'a que les doigts. */
const mouseless = hasTouchScreen && typeof matchMedia === 'function' && !matchMedia('(any-pointer: fine)').matches

export class PointerBar {
  private s: LiveSession
  visible = $state(false)
  /** Le menu ou panneau ouvert. */
  menu = $state<Menu | null>(null)
  /** Le bord gauche du menu ouvert, sous son bouton (points depuis le bord de la barre). */
  menuLeft = $state(0)
  /** Barre étroite, et ce qui est passé dans « Plus ». */
  fit = $state<BarFit>(FULL_BAR)
  /** La barre compacte du toucher est ouverte. */
  touchOpen = $state(false)
  /** La fiche des gestes : à la première ouverture de la barre tactile, jamais à l'arrivée de l'image. */
  guide = $state(false)
  /** La page a le focus (le trait du bord haut n'apparaît que si le geste est possible). */
  pageFocused = $state(typeof document !== 'undefined' && document.hasFocus())
  /** Le pointeur est capturé (souris des jeux). */
  pointerLocked = $state(typeof document !== 'undefined' && !!document.pointerLockElement)
  /** Largeur de la barre : celle de la zone du bord haut. */
  width = $state(0)
  el = $state<HTMLElement | null>(null)
  /** La session (la page entière) : le bord haut et la largeur disponible. */
  host: HTMLElement | null = null

  /** Le trait du haut, seulement quand le geste est possible. */
  readonly edgeShown = $derived(!this.visible && (mouseless || (this.pageFocused && !this.pointerLocked)))

  private timer: ReturnType<typeof setTimeout> | null = null
  private dwell: ReturnType<typeof setTimeout> | null = null
  /** Dernière position de la souris sur la session (null : elle n'y est plus). */
  private hover: { x: number; y: number } | null = null
  /** Le pointeur s'est posé sur la barre (ou contre le bord qui l'ouvre) depuis qu'elle est ouverte. */
  private seen = false
  /** Un bouton de la souris est tenu : une fenêtre glissée contre le bord haut n'ouvre pas la barre. */
  private held = false
  /** Le pointeur doit d'abord sortir de la zone du bord haut avant de rouvrir la barre. */
  private edgeReady = true
  private layout = 0

  constructor(s: LiveSession) {
    this.s = s
    // Un menu qui se ferme sous un pointeur parti ailleurs laisse la barre se refermer.
    $effect(() => {
      if (this.visible && !this.menu) untrack(() => this.settle())
    })
  }

  /** Ouvre ou ferme un menu, sous son bouton (`opener`) ; les autres se ferment. */
  toggle(menu: Menu, opener?: HTMLElement | null) {
    if (this.menu === menu) {
      this.menu = null
      return
    }
    this.menu = menu
    if (opener && this.el) this.menuLeft = Math.max(8, Math.min(opener.offsetLeft, this.el.clientWidth - MENU_WIDTH - 8))
    if (menu === 'stats') this.s.metrics.refresh()
  }

  close() {
    this.menu = null
  }

  /** Une commande est dans « Plus » plutôt que dans la barre. */
  inMore(id: MoreItem): boolean {
    return this.fit.more.includes(id)
  }

  /** La session s'arrête : la barre et ses menus se ferment. */
  reset() {
    this.menu = null
    this.visible = false
    this.touchOpen = false
  }

  /** Le bord haut de l'image, centré sur la largeur de la barre. */
  private inEdgeZone(x: number, y: number): boolean {
    if (!this.host) return false
    const r = this.host.getBoundingClientRect()
    return y >= r.top && y - r.top <= EDGE_REACH && Math.abs(x - (r.left + r.width / 2)) <= (this.width || EDGE_WIDTH) / 2
  }

  /** Le pointeur est sur la barre, ses menus et panneaux, ou dans l'espace qui la sépare du bord haut. */
  private inside(): boolean {
    if (!this.hover || !this.host) return false
    const { x, y } = this.hover
    if (document.elementFromPoint(x, y)?.closest('.bar, .sessionstats, .sharebox, .lendbox')) return true
    const r = this.host.getBoundingClientRect()
    // Dispositions sans transformation : la barre en cours d'apparition compte déjà.
    const bottom = r.top + (this.el ? this.el.offsetTop + this.el.offsetHeight : 0)
    return y >= r.top && y <= bottom && Math.abs(x - (r.left + r.width / 2)) <= (this.width || EDGE_WIDTH) / 2
  }

  clearTimer() {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
  }

  cancelDwell() {
    if (this.dwell) clearTimeout(this.dwell)
    this.dwell = null
  }

  show(byTouch = false) {
    this.cancelDwell()
    this.visible = true
    this.seen = !byTouch && this.inside()
    this.settle()
  }

  hide() {
    this.clearTimer()
    this.visible = false
    this.edgeReady = !(this.hover && this.inEdgeZone(this.hover.x, this.hover.y))
  }

  /** Ctrl + Alt + Maj : la barre s'ouvre ou se ferme. */
  toggleVisible() {
    if (this.visible) this.hide()
    else this.show()
  }

  /** Arme la fermeture : jamais pendant qu'un menu est ouvert. */
  scheduleHide(delay: number) {
    this.clearTimer()
    if (isTouch || this.menu || this.s.phase !== 'live') return
    this.timer = setTimeout(() => {
      this.timer = null
      if (!this.menu) this.hide()
    }, delay)
  }

  /** Barre ouverte : sous le pointeur, elle reste ; sinon elle se referme (vite si le pointeur l'a déjà
   * vue, après un délai si elle est venue au doigt ou au raccourci). */
  private settle() {
    if (!this.visible) return
    if (this.inside()) this.clearTimer()
    else this.scheduleHide(this.seen ? BAR_LEAVE : BAR_IDLE)
  }

  /** Le pointeur est resté contre le bord haut : la barre s'ouvre si la page a le focus et la souris
   * n'est pas capturée. */
  private openFromEdge() {
    this.dwell = null
    if (this.visible || this.s.phase !== 'live' || this.held || !document.hasFocus() || document.pointerLockElement) return
    if (this.hover && this.inEdgeZone(this.hover.x, this.hover.y)) this.show()
  }

  onPointerMove(e: PointerEvent) {
    if (isTouch || e.pointerType === 'touch') return
    this.hover = { x: e.clientX, y: e.clientY }
    this.held = e.buttons !== 0
    if (this.visible) {
      if (this.inside()) {
        this.seen = true
        this.clearTimer()
      } else if (this.seen && !this.timer) this.scheduleHide(BAR_LEAVE)
      return
    }
    if (!this.inEdgeZone(e.clientX, e.clientY)) {
      this.edgeReady = true
      this.cancelDwell()
    } else if (this.held) {
      this.cancelDwell()
    } else if (this.edgeReady && !this.dwell && !document.pointerLockElement && document.hasFocus()) {
      this.dwell = setTimeout(() => this.openFromEdge(), EDGE_DWELL)
    }
  }

  onPointerLeave(e: PointerEvent) {
    if (isTouch || e.pointerType === 'touch') return
    this.hover = null
    this.cancelDwell()
    if (this.visible && this.seen && !this.timer) this.scheduleHide(BAR_LEAVE)
  }

  /**
   * La forme de la barre pour la largeur de la fenêtre : mesurée avec le texte des puces, puis en barre
   * étroite, puis chaque commande qui peut passer dans « Plus ». Les mesures se font avant que la page
   * ne soit redessinée : rien ne clignote.
   */
  async relayout() {
    const bar = this.el
    const host = this.host
    if (!bar || !host) return
    const run = ++this.layout
    const available = host.clientWidth
    this.fit = FULL_BAR
    await tick()
    if (run !== this.layout) return
    const full = bar.scrollWidth
    if (full <= available) return
    this.fit = { narrow: true, more: [] }
    await tick()
    if (run !== this.layout) return
    const items: Partial<Record<MoreItem, number>> = {}
    for (const node of bar.querySelectorAll<HTMLElement>('[data-more]')) items[node.dataset.more as MoreItem] = node.offsetWidth + BAR_GAP
    this.fit = fitBar(available, full, bar.scrollWidth, items, 40 + BAR_GAP)
  }

  destroy() {
    this.clearTimer()
    this.cancelDwell()
  }
}
