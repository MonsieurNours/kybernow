// Clavier d'un menu (`role="menu"`) : le premier choix prend le focus à
// l'ouverture, ↑ ↓ passent d'un choix à l'autre (en boucle), Début et Fin vont
// au premier et au dernier, Échap ferme le menu et rend le focus au bouton qui
// l'a ouvert. Pour les menus de la barre et de l'accueil.
import type { Action } from 'svelte/action'

const ITEMS = '[role="menuitem"]:not([disabled]), [role="menuitemradio"]:not([disabled]), [role="menuitemcheckbox"]:not([disabled])'

export const menuKeys: Action<HTMLElement, () => void> = (node, close) => {
  let onClose = close
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
  // Les choix marqués, sinon les boutons du menu.
  const items = () => {
    const marked = Array.from(node.querySelectorAll<HTMLElement>(ITEMS))
    return marked.length ? marked : Array.from(node.querySelectorAll<HTMLElement>('button:not([disabled])'))
  }
  const checked = items().find((i) => i.getAttribute('aria-checked') === 'true')
  ;(checked ?? items()[0])?.focus()
  const key = (e: KeyboardEvent) => {
    const list = items()
    const at = list.indexOf(document.activeElement as HTMLElement)
    let next: HTMLElement | undefined
    if (e.key === 'ArrowDown') next = list[(at + 1) % list.length]
    else if (e.key === 'ArrowUp') next = list[(at - 1 + list.length) % list.length]
    else if (e.key === 'Home') next = list[0]
    else if (e.key === 'End') next = list[list.length - 1]
    else if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      onClose?.()
      opener?.focus()
      return
    } else return
    e.preventDefault()
    next?.focus()
  }
  node.addEventListener('keydown', key)
  return {
    update(c) {
      onClose = c
    },
    destroy() {
      node.removeEventListener('keydown', key)
    },
  }
}
