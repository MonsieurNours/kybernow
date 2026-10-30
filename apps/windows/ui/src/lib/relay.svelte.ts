// Le lien du relais d'un code (partage, prêt de l'ordinateur), tel que la barre le montre
// (docs/link-ui.md, « Lien du relais en préparation ») : joint (le code et le lien valent
// ailleurs), injoignable, pas de relais, ou `waiting` : lien du relais en préparation
// (`connecting`) depuis moins de 10 s, comptées dès que la barre le voit pour ce code ;
// « Copier le lien » et le QR code attendent. Au-delà, il vaut `unreachable` : « Copier le
// lien » donne le lien du réseau et l'aide dit « Pas encore joignable hors de chez vous. ».

import type { RelayState } from './api'

/** `share::RELAY_LINK_WAIT` du cœur. */
export const RELAY_WAIT_MS = 10000

export type RelayPhase = 'joined' | 'unreachable' | 'waiting' | null

/**
 * Suit le relais d'un code : `source` rend son état (`null` sans code ou sans relais) et une clé
 * qui change avec le code (l'attente repart pour un nouveau code). À appeler à l'initialisation
 * d'un composant : l'horloge de l'attente vit avec lui.
 */
export function relayPhase(source: () => { state: RelayState | null | undefined; key: string }) {
  let wait: { key: string; since: number } | null = null
  let clock = $state(Date.now())
  const phase = $derived.by((): RelayPhase => {
    const { state, key } = source()
    if (state !== 'connecting') {
      wait = null
      return state ?? null
    }
    if (wait?.key !== key) wait = { key, since: Date.now() }
    return clock - wait.since < RELAY_WAIT_MS ? 'waiting' : 'unreachable'
  })
  // Le panneau passe de lui-même à « Pas encore joignable… » au bout des 10 s.
  $effect(() => {
    if (phase !== 'waiting') return
    const timer = setInterval(() => (clock = Date.now()), 500)
    return () => clearInterval(timer)
  })
  return {
    get value() {
      return phase
    },
  }
}
