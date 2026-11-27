// La barre trop large (overflow.ts) : barre étroite, puis menu « Plus » dans l'ordre de link-ui. `npm test`.

import { describe, expect, it } from 'vitest'
import { BAR_MARGIN, FULL_BAR, fitBar, MORE_ORDER } from './overflow'

const items = { keys: 44, mouse: 44, micro: 44, clipboard: 44, fullscreen: 44 }

describe('barre de session trop large', () => {
  it('garde le texte des puces quand elle tient', () => {
    expect(fitBar(1400, 1200, 800, items, 44)).toBe(FULL_BAR)
  })

  it('passe en barre étroite avant tout menu « Plus »', () => {
    expect(fitBar(1000, 1200, 800, items, 44)).toEqual({ narrow: true, more: [] })
  })

  it('garde 8 points de marge de chaque côté', () => {
    expect(fitBar(800 + 2 * BAR_MARGIN, 1200, 800, items, 44).more).toEqual([])
    expect(fitBar(800 + 2 * BAR_MARGIN - 1, 1200, 800, items, 44).more).toEqual(['keys', 'mouse'])
  })

  it('les commandes passent une à une, dans l’ordre de link-ui', () => {
    // « Plus » prend la place de la première : retirer « Touches » seul ne fait rien gagner.
    expect(fitBar(700, 1200, 800, items, 44).more).toEqual(['keys', 'mouse', 'micro', 'clipboard'])
    expect(fitBar(100, 1200, 800, items, 44).more).toEqual([...MORE_ORDER])
  })

  it('une commande absente de la barre ne compte pas', () => {
    const noMic = { keys: 44, mouse: 44, clipboard: 44, fullscreen: 44 }
    expect(fitBar(700, 1200, 800, noMic, 44).more).toEqual(['keys', 'mouse', 'clipboard', 'fullscreen'])
  })
})
