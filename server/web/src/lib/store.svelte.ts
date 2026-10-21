// État partagé entre les pages.

import { api, type Field, type FieldValue, type PendingAccess, type Status } from './api'
import { list, t } from './i18n/index.svelte'

export const app = $state({
  status: null as Status | null,
  toast: null as { kind: 'ok' | 'error'; text: string } | null,
  /** Section demandée dans l'espace en cours (#/systeme/journaux → « journaux »). */
  section: '',
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

/** « donner l'accès à Apple TV et retirer l'accès de iPad » (comme le fil de la console). */
export function accessReason(pending: PendingAccess[]): string {
  const granted = pending.filter((p) => p.grant).map((p) => p.device)
  const revoked = pending.filter((p) => !p.grant).map((p) => p.device)
  const parts = []
  if (granted.length) parts.push(t('store.grant', { names: list(granted) }))
  if (revoked.length) parts.push(t('store.revoke', { names: list(revoked) }))
  return list(parts) || t('store.applyAccounts')
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

/** Réglages de Kyber (schéma et valeurs), chargés une fois et partagés par les espaces. */
export const kyber = $state({
  fields: [] as Field[],
  values: {} as Record<string, FieldValue>,
  loaded: false,
})

export async function loadKyber() {
  if (kyber.loaded) return
  try {
    const [schema, config] = await Promise.all([api.schema(), api.config()])
    kyber.fields = schema.fields
    kyber.values = Object.fromEntries(config.values.map((v) => [v.id, v]))
    kyber.loaded = true
  } catch (e) {
    notify('error', errorText(e))
  }
}

export function field(id: string): Field | undefined {
  return kyber.fields.find((f) => f.id === id)
}

/** Valeur actuelle d'un réglage de Kyber (ou sa valeur par défaut). */
export function value<T = unknown>(id: string): T {
  return (kyber.values[id]?.value ?? field(id)?.default) as T
}

/** Enregistre aussitôt un ou plusieurs réglages de Kyber ; un redémarrage les applique. */
export async function setValues(changes: Record<string, unknown>, message = t('store.saved')) {
  try {
    const res = await api.updateConfig(changes)
    kyber.values = Object.fromEntries(res.values.map((v) => [v.id, v]))
    markRestartPending()
    notify('ok', message)
  } catch (e) {
    notify('error', errorText(e))
  }
}
