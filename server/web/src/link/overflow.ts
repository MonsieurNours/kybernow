// Barre de session trop large pour la fenêtre (docs/link-ui.md, « Barre étroite » et « Menu
// « Plus » ») : d'abord la barre étroite (icônes seules, pastilles réduites au nombre, format des
// mesures caché) ; si elle ne tient toujours pas, avec 8 points de marge de chaque côté, ses
// commandes passent une à une dans « Plus », dans l'ordre de link-ui, jusqu'à ce qu'elle tienne.

/** Les commandes de la barre du client web qui peuvent passer dans « Plus ». Le client web n'a ni
 * « Ordinateurs », ni « Garder le son », ni caméra, ni écran de l'ordinateur. */
export type MoreItem = 'keys' | 'mouse' | 'micro' | 'clipboard' | 'fullscreen'

/** Ordre de passage dans « Plus » (link-ui : « Ordinateurs », « Touches », la souris, « Garder le
 * son », le micro, la caméra, « Presse-papiers », l'écran de l'ordinateur, « Plein écran »). Ne
 * passent jamais : l'ordinateur, la latence, le profil, « Partager », « Prêter l'ordinateur… »,
 * l'enregistrement, « Par le relais », « Se déconnecter ». */
export const MORE_ORDER: readonly MoreItem[] = ['keys', 'mouse', 'micro', 'clipboard', 'fullscreen']

/** Marge de chaque côté de la barre. */
export const BAR_MARGIN = 8

export type BarFit = { narrow: boolean; more: MoreItem[] }

export const FULL_BAR: BarFit = { narrow: false, more: [] }

/**
 * La forme de la barre pour une fenêtre de `available` points : `full` et `narrow`, ses largeurs
 * (texte des puces, puis icônes seules) ; `items`, la largeur de chaque commande de la barre
 * étroite qui peut passer dans « Plus » (absente : pas dans cette barre) ; `moreWidth`, celle du
 * bouton « Plus », qui prend la place de la première commande retirée.
 */
export function fitBar(available: number, full: number, narrow: number, items: Partial<Record<MoreItem, number>>, moreWidth: number): BarFit {
  if (full <= available) return FULL_BAR
  const room = available - 2 * BAR_MARGIN
  let width = narrow
  const more: MoreItem[] = []
  for (const id of MORE_ORDER) {
    if (width <= room) break
    const w = items[id]
    if (w === undefined) continue
    width -= w
    if (!more.length) width += moreWidth
    more.push(id)
  }
  return { narrow: true, more }
}
