// Compte KyberNow (docs/comptes.md) : ce qu'Admin écrit d'un accès entré par
// un jeton (« Session, Partage · hors de chez vous · il y a 2 jours »).

import type { AccountAccess, AccountRight } from './api'
import { ago, dateAt } from './format'
import { t } from './i18n/index.svelte'

/** L'ordre d'affichage des droits. */
export const RIGHTS: AccountRight[] = ['session', 'lend', 'share', 'admin']

/** Un appareil vu pour la première fois il y a moins de ceci est « nouveau ». */
export const NEW_FOR = 24 * 3600

/** « Léa Martin · iPad de Léa ». */
export function who(e: Pick<AccountAccess, 'name' | 'device_name'>): string {
  return [e.name.trim(), e.device_name.trim()].filter(Boolean).join(' · ')
}

/** Les droits, dans l'ordre et dans la langue d'Admin ; les inconnus sont ignorés. */
export function rightsText(rights: readonly string[]): string {
  const known = RIGHTS.filter((r) => rights.includes(r))
  return known.length ? known.map((r) => t(`account.rights.${r}`)).join(', ') : t('account.access.noRights')
}

/** La ligne sous le nom : bloqué, ou droits (« Propriétaire » d'abord), voie, dernière venue. */
export function detail(e: AccountAccess, now = Date.now() / 1000): string {
  if (e.blocked !== null) return t(e.account_blocked ? 'account.access.accountBlockedOn' : 'account.access.blockedOn', { date: dateAt(e.blocked) })
  const parts = [e.owner ? `${t('account.access.owner')}, ${rightsText(e.rights)}` : rightsText(e.rights)]
  if (now - e.first_seen < NEW_FOR) parts.push(t('account.access.newDevice'))
  else parts.push(t(e.via === 'relay' ? 'account.access.relay' : 'account.access.lan'))
  parts.push(ago(e.last_seen))
  return parts.join(' · ')
}
