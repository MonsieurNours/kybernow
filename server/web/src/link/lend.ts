// Prêt de l'ordinateur, côté ami du navigateur (docs/pret.md) : l'identifiant du lien
// `?borrow=`, le compte retenu (le mot de passe seulement si l'ami le demande), le prêt en
// cours qu'on peut reprendre pendant 15 min, ce que ce navigateur sait afficher et décoder,
// et les refus, préavis et fins dans les mots de docs/link-ui.md.

import { LendRefusal, type BorrowAnswer, type BorrowRequest, type LendBeat } from './backend'
import { measureRefresh, screenMode, type Mode } from './device'
import { autoCaps } from './share'
import { t } from '../lib/i18n/index.svelte'

/** Ce que reçoit une session d'emprunt : ce qui ouvre le prêt, le prénom, l'hôte s'il est connu, et la réponse si elle est déjà là. */
export interface BorrowStart {
  request: BorrowRequest
  name: string
  /** Prénom de l'hôte (état de la carte), `null` avec un code tant qu'Admin ne l'a pas dit. */
  host: string | null
  answer?: BorrowAnswer | null
}

/** L'identifiant du lien `/link/?borrow=lea`, ou `null`. */
export function loginFromUrl(): string | null {
  const raw = new URLSearchParams(location.search).get('borrow')
  return raw === null ? null : raw.trim().toLowerCase()
}

/** Prénom de l'hôte porté par le lien d'invitation (`&name=Matthieu`) : « {hôte} vous prête son ordinateur. » avant le premier essai. */
export function hostFromUrl(): string | null {
  const raw = new URLSearchParams(location.search).get('name')?.trim().slice(0, 64)
  return raw ? raw : null
}

const ACCOUNTS_KEY = 'kybernow.lend.accounts'
const ACTIVE_KEY = 'kybernow.lend.active'
/** Le prêt continue 15 minutes sans nouvelles de l'ami (docs/pret.md, « Pendant le prêt »). */
export const COME_BACK_MS = 15 * 60 * 1000

/** Un compte d'ami retenu et pas utilisé depuis ce temps est oublié, mot de passe compris (WEB-15). */
export const FORGET_AFTER_MS = 90 * 24 * 3600 * 1000

/** Ce que ce navigateur retient d'un compte d'ami : l'hôte et l'ordinateur ; le mot de passe si l'ami l'a voulu ;
 * la dernière fois qu'il a servi (`used`, ms). */
export interface SavedAccount {
  host?: string
  computer?: string
  password?: string
  used?: number
}

/** Les comptes retenus, sans ceux qui n'ont pas servi depuis `FORGET_AFTER_MS` (retirés du stockage). Un compte
 * retenu par une version d'avant, sans date, compte à partir d'aujourd'hui. */
export function readAccounts(now = Date.now()): Record<string, SavedAccount> {
  try {
    const raw = localStorage.getItem(ACCOUNTS_KEY)
    const v = raw ? JSON.parse(raw) : {}
    if (!v || typeof v !== 'object') return {}
    let changed = false
    for (const [login, a] of Object.entries(v as Record<string, SavedAccount>)) {
      if (typeof a?.used !== 'number') {
        v[login] = { ...a, used: now }
        changed = true
      } else if (now - a.used > FORGET_AFTER_MS) {
        delete v[login]
        changed = true
      }
    }
    if (changed) localStorage.setItem(ACCOUNTS_KEY, JSON.stringify(v))
    return v
  } catch {
    return {}
  }
}

export function loadAccount(login: string): SavedAccount {
  return readAccounts()[login] ?? {}
}

/** `password` : `undefined` le garde tel quel, `null` l'oublie. */
export function saveAccount(login: string, account: { host?: string; computer?: string; password?: string | null }) {
  try {
    const all = readAccounts()
    const next: SavedAccount = { ...all[login], used: Date.now() }
    if (account.host) next.host = account.host
    if (account.computer) next.computer = account.computer
    if (account.password === null) delete next.password
    else if (account.password !== undefined) next.password = account.password
    all[login] = next
    localStorage.setItem(ACCOUNTS_KEY, JSON.stringify(all))
  } catch {
    // Sans stockage, rien n'est retenu.
  }
}

/** La clé d'un prêt pour « Revenir » : le compte, ou une empreinte du code (WEB-15 : le code lui-même n'est pas
 * écrit dans le stockage du navigateur) ; jamais le mot de passe. */
export function lendKey(req: { code: string } | { login: string }): string {
  return 'code' in req ? `code#${fingerprint(req.code)}` : `login:${req.login}`
}

/** Empreinte courte (FNV-1a 32 bits, en hexadécimal) : de quoi reconnaître le même code, pas de quoi le relire. */
function fingerprint(s: string): string {
  let h = 0x811c9dc5
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i)
    h = Math.imul(h, 0x01000193) >>> 0
  }
  return h.toString(16).padStart(8, '0')
}

/** Le prêt de ce navigateur vit : retenu, avec l'heure du dernier signe de vie. */
export function markActive(req: BorrowRequest) {
  try {
    localStorage.setItem(ACTIVE_KEY, JSON.stringify({ key: lendKey(req), at: Date.now() }))
  } catch {
    // Sans stockage : « Revenir » n'est proposé que dans cette page.
  }
}

export function clearActive() {
  try {
    localStorage.removeItem(ACTIVE_KEY)
  } catch {
    // idem
  }
}

/** Ce navigateur a un prêt en cours pour ce compte (ou ce code), quitté depuis moins de 15 min. */
export function isActive(req: { code: string } | { login: string }): boolean {
  try {
    const v = JSON.parse(localStorage.getItem(ACTIVE_KEY) ?? 'null')
    if (v && !(Date.now() - Number(v.at) < COME_BACK_MS)) {
      // Passé les 15 min, le prêt ne se reprend plus : rien n'en reste.
      localStorage.removeItem(ACTIVE_KEY)
      return false
    }
    return v?.key === lendKey(req)
  } catch {
    return false
  }
}

/**
 * Ce que l'écran virtuel de l'ami doit être, et ce que son décodeur lit (`borrow`) : l'écran de cet
 * appareil (comme une session de l'hôte), les formats que le navigateur accepte, HEVC d'abord.
 */
export async function borrowCaps(): Promise<{ screen: Mode; codecs: string[] }> {
  const [refresh, caps] = await Promise.all([measureRefresh(), autoCaps().catch(() => null)])
  const found = caps?.codecs ?? []
  const codecs = found.includes('hevc') ? ['hevc', ...found.filter((c) => c !== 'hevc')] : found.length ? found : ['h264']
  return { screen: screenMode(refresh), codecs }
}

/** Le refus de la Station, dans les mots de link-ui (« Identifiant ou mot de passe refusé. »). */
export function lendRefusalText(e: unknown, host: string | null): string {
  if (!(e instanceof LendRefusal)) return e instanceof Error ? e.message : String(e)
  switch (e.kind) {
    case 'bad_credentials':
      return t('lend.refusal.badCredentials')
    case 'no_code':
    case 'unknown':
      return t('lend.refusal.noCode')
    case 'lent':
      return t('lend.refusal.lent')
    case 'locked':
      return t('lend.refusal.locked')
    case 'off':
      return t('lend.refusal.off')
    case 'too_many':
      return t('lend.refusal.tooMany')
    case 'refused':
      return host ? t('lend.refusal.refused', { host }) : t('share.join.refused')
    case 'expired':
      return host ? t('lend.refusal.expired', { host }) : t('share.join.expired')
    case 'relay':
      return t('share.join.relayDown')
    case 'network':
      return t('web.app.noConsole')
    case 'bad_request':
      return t('share.join.badRequest')
    default:
      return e.message || t('share.join.badRequest')
  }
}

/** Le préavis de 2 min : reprise par l'hôte, fin prévue, fin du créneau. */
export function lendNoticeText(reason: NonNullable<LendBeat['notice']>['reason'], host: string): string {
  if (reason === 'taken_back') return t('lend.notice.takenBack', { host })
  if (reason === 'slot_end') return t('lend.notice.slotEnd')
  return t('lend.notice.timeUp')
}

/** La fin du prêt. */
export function lendEndText(end: NonNullable<LendBeat['end']>, host: string): string {
  switch (end) {
    case 'taken_back':
      return t('lend.end.takenBack', { host })
    case 'time_up':
      return t('lend.end.timeUp')
    case 'slot_end':
      return t('lend.end.slotEnd')
    case 'left':
      return t('lend.end.left')
    default:
      return t('lend.end.stopped')
  }
}
