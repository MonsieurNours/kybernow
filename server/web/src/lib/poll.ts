// Relevés périodiques de la console et du client web (WEB-19, WEB-04) : un tour à la fois (le suivant est sauté tant
// que le précédent n'a pas répondu), en pause quand l'onglet est masqué, relevé tout de suite à son retour.

export interface PollOptions {
  /** Premier relevé tout de suite (défaut : oui). */
  immediate?: boolean
}

/** Lance `fn` toutes les `ms` ; rend la fonction qui arrête. */
export function poll(fn: () => unknown, ms: number, opts: PollOptions = {}): () => void {
  let busy = false
  let stopped = false
  const hidden = () => typeof document !== 'undefined' && document.hidden
  const tick = async () => {
    if (busy || stopped || hidden()) return
    busy = true
    try {
      await fn()
    } catch {
      // Le relevé suivant réessaie.
    } finally {
      busy = false
    }
  }
  const timer = setInterval(tick, ms)
  const onVisible = () => {
    if (!hidden()) void tick()
  }
  if (typeof document !== 'undefined') document.addEventListener('visibilitychange', onVisible)
  if (opts.immediate !== false) void tick()
  return () => {
    stopped = true
    clearInterval(timer)
    if (typeof document !== 'undefined') document.removeEventListener('visibilitychange', onVisible)
  }
}

/**
 * Numéro de génération d'un état relevé : une écriture locale (`bump`) rend périmées les réponses des relevés partis
 * avant elle (`fresh`), qui ne l'écrasent plus (WEB-04).
 */
export class Generation {
  private n = 0
  bump(): number {
    return ++this.n
  }
  current(): number {
    return this.n
  }
  /** Vrai si rien n'a été écrit depuis `at` (pris avant la requête). */
  fresh(at: number): boolean {
    return at === this.n
  }
}
