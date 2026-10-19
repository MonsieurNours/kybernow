// État partagé entre les pages.

import { api, type Status } from './api'

export const app = $state({
  status: null as Status | null,
  toast: null as { kind: 'ok' | 'error'; text: string } | null,
})

let toastTimer: ReturnType<typeof setTimeout> | undefined

export function notify(kind: 'ok' | 'error', text: string) {
  app.toast = { kind, text }
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (app.toast = null), kind === 'error' ? 6000 : 3000)
}

export async function refreshStatus() {
  try {
    app.status = await api.status()
  } catch {
    // L'erreur 401 est gérée globalement ; le reste réapparaîtra au prochain cycle.
  }
}

/** À appeler après une modification qui demande un redémarrage de Kyber. */
export function markRestartPending() {
  if (app.status) app.status.restart_pending = true
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}
