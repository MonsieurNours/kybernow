// Tactile : le moteur de Kyber ne lit que la souris et le clavier. Une
// couche posée sur l'image traduit les gestes en événements de souris, et un
// champ caché ouvre le clavier de l'appareil.

import type { TouchMode } from './prefs'
import { engineInputs, sendText as sendUnicode } from './inputs'
import { isRemoteCursorHidden, sendHostPosition } from './pointer'
import { diag, diagEnabled } from './diag'
import { t } from '../lib/i18n/index.svelte'

type Point = { x: number; y: number }

const TAP_MS = 220
const TAP_SLOP = 10
const LONG_PRESS_MS = 550
const TRACKPAD_SPEED = 1.8
/** Un cran de molette pour Windows : le moteur divise les pixels par 120. */
const WHEEL_NOTCH = 120
/** Défilement à deux doigts : pixels de molette par pixel de doigt. */
const SCROLL_GAIN = 2

export class TouchInput {
  mode: TouchMode
  /** Position du pointeur (pavé tactile), dans la page. */
  cursor: Point
  private start = new Map<number, { x: number; y: number; t: number }>()
  private last = new Map<number, Point>()
  private moved = false
  private dragging = false
  private lastTapEnd = 0
  private longPress: ReturnType<typeof setTimeout> | null = null
  private scrollAcc = 0
  /** Dernière position donnée au moteur (déplacement relatif de repli). */
  private lastMove: Point | null = null

  constructor(
    private layer: HTMLElement,
    private target: () => HTMLElement | null,
    mode: TouchMode,
    private onCursor: (p: Point) => void,
  ) {
    this.mode = mode
    const r = layer.getBoundingClientRect()
    this.cursor = { x: r.left + r.width / 2, y: r.top + r.height / 2 }
    layer.addEventListener('touchstart', this.down, { passive: false })
    layer.addEventListener('touchmove', this.move, { passive: false })
    layer.addEventListener('touchend', this.up, { passive: false })
    layer.addEventListener('touchcancel', this.up, { passive: false })
  }

  destroy() {
    this.layer.removeEventListener('touchstart', this.down)
    this.layer.removeEventListener('touchmove', this.move)
    this.layer.removeEventListener('touchend', this.up)
    this.layer.removeEventListener('touchcancel', this.up)
  }

  private bounds(p: Point): Point {
    const el = this.target()
    if (!el) return p
    const r = el.getBoundingClientRect()
    return { x: Math.min(Math.max(p.x, r.left), r.right - 1), y: Math.min(Math.max(p.y, r.top), r.bottom - 1) }
  }

  private mouse(type: string, p: Point, button = 0, extra: Partial<MouseEventInit> = {}) {
    const el = this.target()
    if (!el) return
    const buttons = type === 'mouseup' ? 0 : this.dragging || type === 'mousedown' ? (button === 2 ? 2 : 1) : 0
    const move = type === 'mousemove' ? this.movement(p) : {}
    el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: p.x, clientY: p.y, button, buttons, ...move, ...extra }))
  }

  /** Pointeur de l'ordinateur masqué (Windows le cache sans souris branchée) :
   * le moteur n'envoie plus que des déplacements, nuls pour nos événements.
   * La page envoie alors la position elle-même (WebSocket), ou à défaut un
   * déplacement relatif, approximatif mais qui fait bouger le pointeur.
   * Le moteur corrigé garde les positions tant que le pointeur n'est pas
   * capturé : rien à faire. */
  private movement(p: Point): Partial<MouseEventInit> {
    const last = this.lastMove
    this.lastMove = p
    const engineKeeps = engineInputs() && !document.pointerLockElement
    if (!isRemoteCursorHidden() || engineKeeps || sendHostPosition(p.x, p.y) || !last) return { movementX: 0, movementY: 0 }
    return { movementX: Math.round(p.x - last.x), movementY: Math.round(p.y - last.y) }
  }

  private wheel(p: Point, dy: number) {
    const el = this.target()
    el?.dispatchEvent(new WheelEvent('wheel', { bubbles: true, cancelable: true, clientX: p.x, clientY: p.y, deltaY: dy, deltaMode: 0 }))
  }

  private click(p: Point, button = 0) {
    this.mouse('mousemove', p)
    this.mouse('mousedown', p, button)
    this.mouse('mouseup', p, button)
    if (button === 2) this.mouse('contextmenu', p, 2)
  }

  private setCursor(p: Point) {
    this.cursor = this.bounds(p)
    this.onCursor(this.cursor)
    this.mouse('mousemove', this.cursor)
  }

  private down = (e: TouchEvent) => {
    e.preventDefault()
    const now = performance.now()
    for (const t of Array.from(e.changedTouches)) {
      this.start.set(t.identifier, { x: t.clientX, y: t.clientY, t: now })
      this.last.set(t.identifier, { x: t.clientX, y: t.clientY })
    }
    if (e.touches.length === 1) {
      this.moved = false
      this.scrollAcc = 0
      const t = e.touches[0]
      if (this.mode === 'direct') {
        this.setCursor({ x: t.clientX, y: t.clientY })
        this.longPress = setTimeout(() => {
          if (!this.moved) {
            this.click(this.cursor, 2)
            this.start.clear()
          }
        }, LONG_PRESS_MS)
      } else if (now - this.lastTapEnd < 300) {
        // Toucher deux fois et glisser : sélectionner.
        this.dragging = true
        this.mouse('mousedown', this.cursor, 0)
      }
    } else if (this.longPress) {
      clearTimeout(this.longPress)
      this.longPress = null
    }
  }

  private move = (e: TouchEvent) => {
    e.preventDefault()
    const touches = Array.from(e.touches)
    if (touches.length >= 2) {
      // Deux doigts : défiler.
      const t = touches[0]
      const prev = this.last.get(t.identifier)
      if (prev) {
        // Par crans entiers : de petits pas (0,13 cran) se perdent dans
        // les applications qui attendent un cran complet.
        this.scrollAcc += (prev.y - t.clientY) * SCROLL_GAIN
        const notches = Math.trunc(this.scrollAcc / WHEEL_NOTCH)
        if (notches) {
          this.wheel(this.cursor, notches * WHEEL_NOTCH)
          this.scrollAcc -= notches * WHEEL_NOTCH
        }
      }
      this.moved = true
    } else if (touches.length === 1) {
      const t = touches[0]
      const s = this.start.get(t.identifier)
      const prev = this.last.get(t.identifier)
      if (s && Math.hypot(t.clientX - s.x, t.clientY - s.y) > TAP_SLOP) this.moved = true
      if (this.mode === 'direct') {
        if (this.moved && !this.dragging) {
          if (this.longPress) clearTimeout(this.longPress)
          this.longPress = null
          this.dragging = true
          this.mouse('mousedown', this.cursor, 0)
        }
        this.setCursor({ x: t.clientX, y: t.clientY })
      } else if (prev) {
        this.setCursor({
          x: this.cursor.x + (t.clientX - prev.x) * TRACKPAD_SPEED,
          y: this.cursor.y + (t.clientY - prev.y) * TRACKPAD_SPEED,
        })
      }
    }
    for (const t of touches) this.last.set(t.identifier, { x: t.clientX, y: t.clientY })
  }

  private up = (e: TouchEvent) => {
    e.preventDefault()
    const now = performance.now()
    if (this.longPress) {
      clearTimeout(this.longPress)
      this.longPress = null
    }
    const ended = Array.from(e.changedTouches)
    const fingers = e.touches.length + ended.length
    const first = ended.map((t) => this.start.get(t.identifier)).find(Boolean)
    const quick = first && now - first.t < TAP_MS && !this.moved

    if (this.dragging && e.touches.length === 0) {
      this.dragging = false
      this.mouse('mouseup', this.cursor, 0)
    } else if (quick && e.touches.length === 0) {
      if (fingers >= 2) this.click(this.cursor, 2)
      else this.click(this.cursor, 0)
      this.lastTapEnd = now
    }
    for (const t of ended) {
      this.start.delete(t.identifier)
      this.last.delete(t.identifier)
    }
  }
}

/** Touches que le clavier d'un téléphone n'a pas. Libellés lus à l'affichage
 * (accesseurs) : ils suivent la langue. */
export const SPECIAL_KEYS: { readonly label: string; key: string; code: string; sticky?: boolean; readonly aria?: string }[] = [
  { get label() { return t('web.keys.esc') }, key: 'Escape', code: 'Escape' },
  { get label() { return t('web.keys.tab') }, key: 'Tab', code: 'Tab' },
  { get label() { return t('web.keys.ctrl') }, key: 'Control', code: 'ControlLeft', sticky: true },
  { get label() { return t('web.keys.alt') }, key: 'Alt', code: 'AltLeft', sticky: true },
  { label: '⊞', key: 'Meta', code: 'MetaLeft', sticky: true, get aria() { return t('web.keys.start') } },
  { label: '←', key: 'ArrowLeft', code: 'ArrowLeft', get aria() { return t('web.keys.left') } },
  { label: '↑', key: 'ArrowUp', code: 'ArrowUp', get aria() { return t('web.keys.up') } },
  { label: '↓', key: 'ArrowDown', code: 'ArrowDown', get aria() { return t('web.keys.down') } },
  { label: '→', key: 'ArrowRight', code: 'ArrowRight', get aria() { return t('web.keys.right') } },
]

function codeFor(ch: string): string {
  if (/^[a-z]$/i.test(ch)) return `Key${ch.toUpperCase()}`
  if (/^[0-9]$/.test(ch)) return `Digit${ch}`
  if (ch === ' ') return 'Space'
  return ''
}

/** Envoie une touche (avec les modificateurs collants) à l'image. */
export function sendKey(target: HTMLElement | null, key: string, code: string, mods: Set<string>) {
  if (!target) return
  const init = (type: string): KeyboardEventInit => ({
    key,
    code,
    bubbles: true,
    cancelable: true,
    ctrlKey: mods.has('Control'),
    altKey: mods.has('Alt'),
    metaKey: mods.has('Meta'),
    shiftKey: key.length === 1 && key !== key.toLowerCase(),
  })
  for (const m of mods) target.dispatchEvent(new KeyboardEvent('keydown', { key: m, code: `${m}Left`, bubbles: true }))
  target.dispatchEvent(new KeyboardEvent('keydown', init('keydown')))
  target.dispatchEvent(new KeyboardEvent('keyup', init('keyup')))
  for (const m of mods) target.dispatchEvent(new KeyboardEvent('keyup', { key: m, code: `${m}Left`, bubbles: true }))
}

/** Texte en touches (codes physiques d'un clavier QWERTY) : seulement pour un
 * raccourci avec Ctrl, Alt ou ⊞, ou sans connexion WebSocket. */
function sendTextAsKeys(target: HTMLElement | null, text: string, mods: Set<string>) {
  for (const ch of text) {
    if (ch === '\n') sendKey(target, 'Enter', 'Enter', mods)
    else sendKey(target, ch, codeFor(ch), mods)
  }
}

/** Touches du clavier de l'appareil qui restent des touches (par leur code). */
const DEVICE_KEYS: Record<string, string> = {
  Enter: 'Enter',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Tab: 'Tab',
  Escape: 'Escape',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
}

/** Attribut qui confie le texte d'un champ au moteur corrigé (paquets
 * Unicode, aussi en WebTransport). La page ne le pose qu'avec ce moteur. */
export const TEXT_ATTRIBUTE = 'data-kynput-text'

/** Une frappe de vrai clavier suivie de son texte arrive dans ce délai. */
const HARDWARE_TEXT_MS = 150

/**
 * Saisie dans le champ caché qui ouvre le clavier de l'appareil.
 *
 * Le texte part en paquets Unicode : juste quelle que soit la disposition de
 * l'ordinateur (AZERTY…), avec majuscules, accents et composition (iOS,
 * Android, dictée). Il est lu sur `beforeinput` et `compositionend`, jamais
 * pendant une composition, et le champ n'est vidé qu'en dehors d'elle.
 * Les touches (Entrée, Retour, Suppr, flèches, Début, Fin, pages, Échap,
 * Tab : `DEVICE_KEYS`) restent des touches.
 *
 * Un vrai clavier (iPad avec clavier, clavier Bluetooth) donne un `code` à
 * ses frappes : le moteur les envoie déjà ; le texte qu'elles produisent
 * dans le champ ne repart pas.
 *
 * Champ marqué `data-kynput-text` (moteur corrigé) : le moteur tape le texte
 * lui-même, sur `document`, après nous. La page ne l'envoie alors que pour un
 * raccourci (Ctrl, Alt ou ⊞ collants, en touches), en l'empêchant
 * (`preventDefault`, ou `stopPropagation` pour `compositionend`) pour qu'il
 * ne parte pas deux fois. Les touches restent à la page dans les deux cas.
 */
export class TypedText {
  private composing = false
  private hardwareAt = -Infinity

  constructor(
    private field: () => HTMLTextAreaElement | null,
    private target: () => HTMLElement | null,
    private mods: () => Set<string>,
    private clearMods: () => void,
  ) {}

  private fromHardware(): boolean {
    return performance.now() - this.hardwareAt < HARDWARE_TEXT_MS
  }

  /** Le moteur lit-il ce champ ? (il regarde l'attribut à chaque événement) */
  private engineTypes(): boolean {
    return !!this.field()?.hasAttribute(TEXT_ATTRIBUTE)
  }

  keydown = (e: KeyboardEvent) => {
    if (!e.isTrusted) return
    if (diagEnabled) diag.lastKey = `${e.key}/${e.code || '∅'}`
    // Un `code` : un vrai clavier, même à travers une méthode de saisie.
    if (e.code && e.code !== 'Unidentified') {
      this.hardwareAt = performance.now()
      return
    }
    // Clavier de l'appareil : ses touches spéciales arrivent avec leur nom,
    // sans code. On les envoie, et le champ n'en reçoit rien.
    const code = DEVICE_KEYS[e.key]
    if (code && !e.isComposing && !this.composing) {
      e.preventDefault()
      this.key(e.key, code)
    }
  }

  beforeinput = (e: InputEvent) => {
    const engine = this.engineTypes()
    if (diagEnabled) diag.lastInput = `${e.inputType}${this.fromHardware() ? ' (vrai clavier)' : ''}${engine ? ' (moteur)' : ''}`
    // Le moteur corrigé trie lui-même les frappes d'un vrai clavier.
    if (!engine && this.fromHardware()) return
    switch (e.inputType) {
      case 'insertText':
      case 'insertReplacementText':
      case 'insertFromPaste':
      case 'insertFromDrop': {
        if (e.isComposing || this.composing) return
        // Le moteur tape le texte, sauf un raccourci (en touches, par nous)
        // et un collage sans `data` (texte dans `dataTransfer`, qu'il ne lit pas).
        if (engine && !this.mods().size && e.data) return
        const text = e.data ?? e.dataTransfer?.getData('text/plain') ?? ''
        e.preventDefault()
        if (text) this.text(text)
        break
      }
      case 'insertLineBreak':
      case 'insertParagraph':
        e.preventDefault()
        this.key('Enter', 'Enter')
        break
      case 'deleteContentBackward':
        if (this.composing) return
        e.preventDefault()
        this.key('Backspace', 'Backspace')
        break
      case 'deleteContentForward':
        if (this.composing) return
        e.preventDefault()
        this.key('Delete', 'Delete')
        break
      // insertCompositionText, deleteCompositionText, insertFromComposition :
      // la composition se lit à sa fin.
    }
  }

  compositionstart = () => {
    this.composing = true
  }

  compositionend = (e: CompositionEvent) => {
    this.composing = false
    if (!e.data) {
      // Rien à taper.
    } else if (!this.engineTypes()) {
      if (!this.fromHardware()) this.text(e.data)
    } else if (this.mods().size) {
      // Raccourci : en touches, et le moteur ne doit pas taper le texte aussi.
      e.stopPropagation()
      this.text(e.data)
    }
    // Le navigateur termine la composition après cet événement : on vide ensuite.
    setTimeout(() => this.clear(), 0)
  }

  /** Repli : ce qu'un navigateur aurait inséré malgré tout. */
  input = (e: Event) => {
    if (this.composing || (e as InputEvent).isComposing) return
    this.clear()
  }

  private clear() {
    const f = this.field()
    if (f && !this.composing && f.value) f.value = ''
  }

  private text(text: string) {
    const mods = this.mods()
    // Ctrl, Alt ou ⊞ collants : c'est un raccourci, il lui faut des touches.
    if (mods.size || !sendUnicode(text)) sendTextAsKeys(this.target(), text, mods)
    if (mods.size) this.clearMods()
  }

  private key(key: string, code: string) {
    const mods = this.mods()
    sendKey(this.target(), key, code, mods)
    if (mods.size) this.clearMods()
  }
}
