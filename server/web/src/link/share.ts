// KyberNow Share, côté invité du navigateur (docs/kybernow-share.md, « Rejoindre ») :
// le code (lien `?join=` ou saisi), le prénom retenu, les refus de la Station dits
// avec les mots de docs/link-ui.md, le format de l'image lu avant d'entrer, et le
// départ du partage.

import type { CodecSupport } from './engine'
import { JoinRefusal, type GuestJoin } from './backend'
import { deviceName } from './device'
import { t } from '../lib/i18n/index.svelte'

/** Ce que reçoit une session d'invité : la réponse de `join` et ce que ce navigateur décode. */
export interface GuestStart {
  join: GuestJoin
  codecs: CodecSupport[]
}

const NAME_KEY = 'kybernow.share.name'

/** Le code du lien `/link/?join=123456` (ou `123 456`), ou `null` s'il n'y en a pas. */
export function codeFromUrl(): string | null {
  const raw = new URLSearchParams(location.search).get('join')
  return raw === null ? null : digits(raw)
}

/** Seulement les chiffres, six au plus. */
export function digits(text: string): string {
  return text.replace(/\D/g, '').slice(0, 6)
}

/** « 123 456 », groupé par trois comme sur l'écran de l'hôte. */
export function groupCode(text: string): string {
  const d = digits(text)
  return d.length > 3 ? `${d.slice(0, 3)} ${d.slice(3)}` : d
}

/** Prénom retenu pour la fois suivante, sinon le nom de cet appareil (« Chrome sur Mac »). */
export function loadName(): string {
  try {
    const saved = localStorage.getItem(NAME_KEY)
    if (saved?.trim()) return saved
  } catch {
    // Stockage indisponible (navigation privée) : nom de l'appareil.
  }
  return deviceName().name
}

export function saveName(name: string) {
  try {
    localStorage.setItem(NAME_KEY, name.trim())
  } catch {
    // Sans stockage, le prénom vaut pour cette visite.
  }
}

/** Retire le code de l'adresse (partage fini : un rechargement ne le redonne pas). */
export function forgetCodeInUrl() {
  try {
    const url = new URL(location.href)
    if (!url.searchParams.has('join')) return
    url.searchParams.delete('join')
    history.replaceState(null, '', url)
  } catch {
    // Sans history, l'adresse garde son code.
  }
}

/** Le refus de la Station, dans les mots de link-ui (« Aucun partage avec ce code… »). */
export function joinRefusalText(e: unknown): string {
  if (!(e instanceof JoinRefusal)) return e instanceof Error ? e.message : String(e)
  switch (e.kind) {
    case 'no_share':
      return t('share.join.noShare')
    case 'full':
      return t('share.join.full')
    case 'too_many':
      return t('share.join.tooMany')
    case 'unavailable':
      return t('share.join.unavailable')
    case 'network':
      return t('web.app.noConsole')
    case 'bad_request':
      return t('share.join.badRequest')
    default:
      return e.message || t('share.join.badRequest')
  }
}

const LABELS: Record<string, string> = { h264: 'H.264', hevc: 'HEVC', av1: 'AV1' }

/** Le nom du format de l'image (« HEVC 4:4:4 »). */
export function formatLabel(codec: string, yuv444: boolean): string {
  const name = LABELS[codec.toLowerCase()] ?? codec.toUpperCase()
  return yuv444 ? `${name} 4:4:4` : name
}

/**
 * Ce navigateur sait-il lire l'image de l'hôte ? Rend le nom du format qu'il ne lit pas,
 * ou `null` (il le lit, ou l'image n'a pas encore démarré : l'invité choisira le sien).
 * L'invité reçoit le format de l'hôte : il ne peut pas en demander un autre.
 */
export function unreadableFormat(codec: string | null, yuv444: boolean, codecs: CodecSupport[]): string | null {
  const wanted = codec?.trim().toLowerCase()
  if (!wanted) return null
  const support = codecs.find((c) => c.codec === wanted)
  if (!support || (yuv444 && !support.yuv444)) return formatLabel(wanted, yuv444)
  return null
}

/** Sort du partage : à la fermeture de la page comme sur « Quitter le partage » (`keepalive`). */
export function leaveShare(join: GuestJoin): Promise<unknown> {
  return fetch('/api/share/leave', {
    method: 'POST',
    keepalive: true,
    credentials: 'same-origin',
    headers: { 'x-kyber-admin': '1', 'content-type': 'application/json', 'x-kybernow-guest': join.secret },
    body: JSON.stringify({ guest: join.guest }),
  }).catch(() => {})
}
