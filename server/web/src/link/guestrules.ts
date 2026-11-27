// Les règles d'un invité du partage, sans état (docs/link-ui.md, « Partage » ; machine à états dans
// guest.svelte.ts) : ses droits selon son rôle, les avis quand l'hôte les change, l'avis d'une image
// ajustée par la Station, et ce que demande un changement de « Qualité de l'image ».

import type { GuestImage, ShareRole } from '../lib/api'
import { imageQuality } from '../lib/format'
import { t } from '../lib/i18n/index.svelte'
import { qualityName } from './share'

type Size = { width: number; height: number }

/** Deux droits indépendants : les manettes (`player`), le clavier et la souris (`kbm`), ou les deux. */
export function rights(role: ShareRole): { pads: boolean; kbm: boolean } {
  return { pads: role === 'player' || role === 'player_kbm', kbm: role === 'kbm' || role === 'player_kbm' }
}

/**
 * Les avis quand l'hôte change les droits : un par droit donné ou repris ; plus aucun droit, « Vous
 * regardez de nouveau. » à la place des avis de reprise. Rend les clés du catalogue, dans l'ordre.
 */
export function roleNotices(before: ShareRole, next: ShareRole): string[] {
  const was = rights(before)
  const now = rights(next)
  const keys: string[] = []
  if (now.pads && !was.pads) keys.push('share.guest.canPlay')
  if (now.kbm && !was.kbm) keys.push('share.guest.gotKbm')
  if (next === 'spectator') keys.push('share.guest.watching')
  else {
    if (!now.pads && was.pads) keys.push('share.guest.lostPads')
    if (!now.kbm && was.kbm) keys.push('share.guest.lostKbm')
  }
  return keys
}

/** Deux tailles d'image presque égales (à 8 pixels près : arrondis des encodeurs). */
export function near(a: Size, b: Size): boolean {
  return Math.abs(a.width - b.width) <= 8 && Math.abs(a.height - b.height) <= 8
}

/**
 * L'avis d'une image ajustée par la Station (docs/link-ui.md, « Image ajustée par la Station »), et sa
 * clé : un avis par changement, pas à chaque signe de vie. En retard sur l'image de l'hôte : « Image
 * allégée pour suivre. » ; pas d'image de plus possible : « L'ordinateur ne peut pas faire une image de
 * plus : vous recevez 720p · 30 Hz. ». Image demandée : clé vide, pas d'avis.
 */
export function imageNotice(image: GuestImage): { key: string; text: string } {
  if (image.reason !== 'lag' && image.reason !== 'limit') return { key: '', text: '' }
  const key = `${image.reason} ${image.width}x${image.height}@${image.fps}`
  const text = image.reason === 'lag' ? t('share.guest.lightened') : t('share.guest.noMoreImage', { quality: imageQuality(image) || qualityName(image.quality) })
  return { key, text }
}

/**
 * Ce que demande « Qualité de l'image » changée, une fois la Station d'accord et le jeton renouvelé :
 * `none` (préréglage inchangé, ou même taille), `restart` (le format change : 4:4:4 de l'image de l'hôte
 * ou 4:2:0 d'une image à part, l'image est relancée proprement), ou la taille à attendre dans le flux
 * (`null` : la Station ne l'a pas dite).
 */
export function qualityStep(
  presetBefore: string | null,
  preset: string | null,
  format: { codec: string; yuv444: boolean } | null,
  plan: { codec: string; yuv444: boolean },
  next: GuestImage,
  decoded: Size | null,
): 'none' | 'restart' | { target: Size | null } {
  if (preset && preset === presetBefore) return 'none'
  if (format && (format.codec !== plan.codec || format.yuv444 !== plan.yuv444)) return 'restart'
  const target = next.width && next.height ? { width: next.width, height: next.height } : null
  if (target && decoded && near(target, decoded)) return 'none'
  return { target }
}

/**
 * L'image décodée a rapetissé sans que l'invité ait rien changé, et ce n'est pas l'écran de l'hôte (qui a
 * pu changer de définition) : la Station l'a allégée.
 */
export function lightened(before: Size | null, size: Size, hostScreens: Size[]): boolean {
  if (!before || size.width * size.height >= before.width * before.height) return false
  return !hostScreens.some((d) => near(size, d))
}
