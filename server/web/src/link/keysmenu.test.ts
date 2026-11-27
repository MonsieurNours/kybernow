// Le menu « Touches » (keysmenu.ts) : les dix entrées d'un PC et les cinq d'une Station Mac, dans l'ordre
// de link-ui, chacune avec son indication. `npm test`.

import { describe, expect, it, vi } from 'vitest'

// `touch.ts` passe par `inputs.ts` et `pointer.ts`, qui lisent le navigateur au chargement.
vi.mock('./touch', () => ({ sendKey: () => {} }))

import { i18n } from '../lib/i18n/index.svelte'
import { MAC_KEYS, PC_KEYS } from './keysmenu'

// Les libellés en français, quelle que soit la langue de la machine des essais.
i18n.lang = 'fr'

describe('menu « Touches »', () => {
  it('dix entrées d’un PC, dans l’ordre', () => {
    expect(PC_KEYS.map((k) => k.label)).toEqual([
      'Ctrl + Alt + Suppr',
      'Touche ⊞ seule',
      'Alt + Tab',
      'Impr. écran',
      '⊞ + D',
      '⊞ + L',
      'Alt + F4',
      'Échap',
      'Gestionnaire des tâches',
      'F11',
    ])
    // « Échap » seule n'a pas d'indication ; les autres l'ont.
    expect(PC_KEYS.filter((k) => !k.sub).map((k) => k.label)).toEqual(['Échap'])
    expect(PC_KEYS[8]).toMatchObject({ mods: ['Control', 'Shift'], code: 'Escape', sub: 'Ctrl + Maj + Échap' })
  })

  it('cinq équivalents face à une Station Mac', () => {
    expect(MAC_KEYS.map((k) => k.label)).toEqual(['⌘ ⇥', '⌘ Espace', '⌃ ⌘ Q', '⇧ ⌘ 5', '⌥ ⌘ Échap'])
    expect(MAC_KEYS.every((k) => k.sub)).toBe(true)
  })
})
