// KyberNow Share, côté invité du navigateur (docs/kybernow-share.md, « Rejoindre ») :
// le code (lien `?join=` ou saisi), le prénom et la « Qualité de l'image » retenus,
// ce que ce navigateur sait afficher et décoder (« Auto »), les refus de la Station
// dits avec les mots de docs/link-ui.md, le format de l'image lu avant d'entrer, et
// le départ du partage.

import type { CodecSupport } from './engine'
import { JoinRefusal, type AutoCaps, type GuestJoin } from './backend'
import { deviceName, measureRefresh } from './device'
import type { ShareQuality } from '../lib/api'
import { t } from '../lib/i18n/index.svelte'

/** Ce que reçoit une session d'invité : la réponse de `join`, ce que ce navigateur décode, et la qualité demandée. */
export interface GuestStart {
  join: GuestJoin
  codecs: CodecSupport[]
  quality: ShareQuality
}

const NAME_KEY = 'kybernow.share.name'
const QUALITY_KEY = 'kybernow.share.quality'

/** Les choix de « Qualité de l'image », dans l'ordre de docs/link-ui.md (« Auto » proposé). */
export const QUALITIES: ShareQuality[] = ['auto', 'host', '2160p60', '1080p60', '720p30']

/** « Auto », « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz ». */
export function qualityName(q: ShareQuality): string {
  return t(`share.quality.${q}`)
}

/** La qualité retenue pour la fois suivante, sinon « Auto ». */
export function loadQuality(): ShareQuality {
  try {
    const saved = localStorage.getItem(QUALITY_KEY) as ShareQuality | null
    if (saved && QUALITIES.includes(saved)) return saved
  } catch {
    // Stockage indisponible (navigation privée) : « Auto ».
  }
  return 'auto'
}

export function saveQuality(q: ShareQuality) {
  try {
    localStorage.setItem(QUALITY_KEY, q)
  } catch {
    // Sans stockage, la qualité vaut pour cette visite.
  }
}

/** Ce que Kyber reçoit pour la qualité de l'invité (`kn_quality` de son jeton : « Auto » y est tranché par Admin). */
export type Preset = 'host' | '2160p60' | '1080p60' | '720p30'

/** Le préréglage porté par le jeton de Kyber d'un invité, ou `null` (Station qui ne le met pas). */
export function tokenPreset(token: string): Preset | null {
  try {
    const part = token.split('.')[1] ?? ''
    const b64 = part.replace(/-/g, '+').replace(/_/g, '/')
    const claims = JSON.parse(atob(b64.padEnd(Math.ceil(b64.length / 4) * 4, '=')))
    const q = claims?.kn_quality
    return q === 'host' || q === '2160p60' || q === '1080p60' || q === '720p30' ? q : null
  } catch {
    return null
  }
}

/**
 * Les formats que le moteur web sait lire (kywebplayer : H.264 et HEVC, en 4:2:0 et en 4:4:4 ; pas
 * d'AV1), avec le niveau qui convient à la taille sondée : un niveau trop bas ferait dire « non » à un
 * décodeur qui suit la 4K.
 */
function probes(width: number, height: number): { codec: 'h264' | 'hevc'; yuv444: boolean; webcodec: string }[] {
  const pixels = width * height
  const [avc, hevc] = pixels <= 1920 * 1088 ? ['2a', '123'] : pixels <= 4096 * 2304 ? ['34', '153'] : ['3c', '183']
  return [
    { codec: 'h264', yuv444: false, webcodec: `avc1.6400${avc}` },
    { codec: 'h264', yuv444: true, webcodec: `avc1.f400${avc}` },
    { codec: 'hevc', yuv444: false, webcodec: `hev1.1.2.H${hevc}.90` },
    { codec: 'hevc', yuv444: true, webcodec: `hev1.4.10.L${hevc}.9e.8` },
  ]
}

/** Le décodeur du navigateur accepte-t-il ce format à cette taille (`codedWidth`, `codedHeight`) ? */
async function decodes(webcodec: string, width: number, height: number): Promise<boolean> {
  const VideoDecoder = (globalThis as any).VideoDecoder
  if (typeof VideoDecoder?.isConfigSupported !== 'function') return false
  try {
    const answer = await VideoDecoder.isConfigSupported({ codec: webcodec, codedWidth: width, codedHeight: height, optimizeForLatency: true })
    return answer?.supported === true
  } catch {
    return false
  }
}

const even = (v: number) => Math.max(2, Math.round(v / 2) * 2)

/**
 * « Auto » : ce que ce navigateur sait afficher et décoder (docs/kybernow-share.md, « Échange avec
 * Admin »). L'écran en pixels réels (côté long, côté court, quelle que soit l'orientation), 60 Hz au plus ;
 * les formats que le décodeur accepte à cette taille. S'il n'en accepte aucun (un écran 5K dont le
 * décodeur s'arrête à la 4K, un téléphone qui ne suit pas la 4K), la taille suivante qu'il suit :
 * 3840 × 2160, puis 1920 × 1080, puis 1280 × 720. Admin en tire le préréglage (« 4K · 60 Hz » dès
 * 3840 × 2160 à 60 Hz).
 */
export async function autoCaps(): Promise<AutoCaps> {
  const dpr = window.devicePixelRatio || 1
  const a = screen.width * dpr
  const b = screen.height * dpr
  const long = even(Math.max(a, b))
  const short = even(Math.min(a, b))
  const refresh = measureRefresh()
  const sizes: [number, number][] = [[long, short]]
  for (const [w, h] of [
    [3840, 2160],
    [1920, 1080],
    [1280, 720],
  ] as [number, number][]) {
    if (w <= long && h <= short && (w < long || h < short)) sizes.push([w, h])
  }
  let found: { width: number; height: number; codecs: string[]; yuv444: boolean } | null = null
  for (const [width, height] of sizes) {
    const results = await Promise.all(probes(width, height).map(async (p) => ({ ...p, ok: await decodes(p.webcodec, width, height) })))
    const ok = results.filter((r) => r.ok)
    if (ok.length) {
      found = { width, height, codecs: [...new Set(ok.map((r) => r.codec))], yuv444: ok.some((r) => r.yuv444) }
      break
    }
  }
  const fps = Math.min(60, await refresh)
  return { ...(found ?? { width: long, height: short, codecs: [], yuv444: false }), fps }
}

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
    case 'pending_full':
      return t('share.join.pendingFull')
    case 'refused':
      return t('share.join.refused')
    case 'expired':
      return t('share.join.expired')
    case 'relay':
      return t('share.join.relayDown')
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
 * Ce navigateur peut-il entrer ? Un appareil qui ne lit pas le format de l'hôte reçoit une image à part
 * dans un format qu'il lit (H.264 sinon), et une image en 4:2:0 s'il ne lit pas le 4:4:4 de l'hôte
 * (docs/kybernow-share.md, « L'image d'un invité ») : seul un navigateur qui ne lit aucun des formats du
 * moteur le dit avant d'entrer. Rend le nom du format de l'hôte (« HEVC »), ou `null` s'il peut entrer.
 */
export function unreadableFormat(codec: string | null, yuv444: boolean, codecs: CodecSupport[]): string | null {
  if (codecs.some((c) => c.codec === 'h264' || c.codec === 'hevc')) return null
  return formatLabel(codec?.trim().toLowerCase() || 'h264', yuv444)
}

/**
 * Le format que l'invité demande à Kyber : celui de l'hôte s'il le lit, sinon H.264 (ou HEVC) ; le
 * 4:4:4 seulement pour recevoir l'image de l'hôte en 4:4:4 (`followsHost`) et s'il le lit : une image à
 * part est toujours en 4:2:0. `null` : l'image de l'hôte n'a pas encore démarré (le format de l'invité
 * reste le sien), ou ce navigateur ne lit rien (refusé avant d'entrer).
 */
export function guestFormat(codec: string | null, yuv444: boolean, codecs: CodecSupport[], followsHost: boolean): { codec: 'h264' | 'hevc'; yuv444: boolean } | null {
  const host = codec?.trim().toLowerCase()
  if (!host) return null
  const reads = (c: string) => codecs.some((s) => s.codec === c)
  if (host === 'h264' || host === 'hevc') {
    if (reads(host)) return { codec: host, yuv444: yuv444 && followsHost && codecs.some((s) => s.codec === host && s.yuv444) }
  }
  if (reads('h264')) return { codec: 'h264', yuv444: false }
  if (reads('hevc')) return { codec: 'hevc', yuv444: false }
  return null
}

/**
 * Un texte du moteur ou du navigateur sans le jeton de Kyber (JWT) ni le secret de l'invité, avant
 * qu'il aille dans la console du navigateur : ils ne sont jamais écrits dans un journal, une adresse
 * ni le stockage de la page.
 */
export function redact(text: unknown, secrets: string[] = []): string {
  let out = String(text)
  for (const secret of secrets) if (secret) out = out.split(secret).join('…')
  return out.replace(/eyJ[\w-]{5,}\.[\w-]{5,}(?:\.[\w-]*)?/g, '…')
}

/**
 * Sort du partage : à la fermeture de la page comme sur « Quitter le partage » (`keepalive`). Annule
 * aussi une demande qui attend l'accord de l'hôte (« Annuler »).
 */
export function leaveShare(join: Pick<GuestJoin, 'guest' | 'secret'>): Promise<unknown> {
  return fetch('/api/share/leave', {
    method: 'POST',
    keepalive: true,
    credentials: 'same-origin',
    headers: { 'x-kyber-admin': '1', 'content-type': 'application/json', 'x-kybernow-guest': join.secret },
    body: JSON.stringify({ guest: join.guest }),
  }).catch(() => {})
}
