// Tactile : le moteur de Kyber ne lit que la souris et le clavier. Une
// couche posée sur l'image traduit les gestes en événements de souris, et un
// champ caché ouvre le clavier de l'appareil.

import type { TouchMode } from './prefs'

type Point = { x: number; y: number }

const TAP_MS = 220
const TAP_SLOP = 10
const LONG_PRESS_MS = 550
const TRACKPAD_SPEED = 1.8

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
    el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: p.x, clientY: p.y, button, buttons, ...extra }))
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
        this.scrollAcc += prev.y - t.clientY
        if (Math.abs(this.scrollAcc) >= 8) {
          this.wheel(this.cursor, this.scrollAcc * 2)
          this.scrollAcc = 0
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

/** Touches que le clavier d'un téléphone n'a pas. */
export const SPECIAL_KEYS: { label: string; key: string; code: string; sticky?: boolean; aria?: string }[] = [
  { label: 'Échap', key: 'Escape', code: 'Escape' },
  { label: 'Tab', key: 'Tab', code: 'Tab' },
  { label: 'Ctrl', key: 'Control', code: 'ControlLeft', sticky: true },
  { label: 'Alt', key: 'Alt', code: 'AltLeft', sticky: true },
  { label: '⊞', key: 'Meta', code: 'MetaLeft', sticky: true, aria: 'Touche Démarrer' },
  { label: '←', key: 'ArrowLeft', code: 'ArrowLeft', aria: 'Flèche gauche' },
  { label: '↑', key: 'ArrowUp', code: 'ArrowUp', aria: 'Flèche haut' },
  { label: '↓', key: 'ArrowDown', code: 'ArrowDown', aria: 'Flèche bas' },
  { label: '→', key: 'ArrowRight', code: 'ArrowRight', aria: 'Flèche droite' },
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

/** Texte tapé au clavier de l'appareil, caractère par caractère. */
export function sendText(target: HTMLElement | null, text: string, mods: Set<string>) {
  for (const ch of text) {
    if (ch === '\n') sendKey(target, 'Enter', 'Enter', mods)
    else sendKey(target, ch, codeFor(ch), mods)
  }
}
