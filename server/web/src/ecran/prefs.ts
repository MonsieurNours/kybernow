// Réglages du client web, et dernière session, gardés dans ce navigateur.

export type Profile = 'jeu' | 'bureau' | 'max'
export type Transport = 'auto' | 'fiable' | 'rapide'
export type Codec = 'auto' | 'h264' | 'hevc'
export type TouchMode = 'trackpad' | 'direct'
export type MouseMode = 'auto' | 'bureau' | 'jeu'

export interface Prefs {
  profile: Profile
  /** L'ordinateur crée un écran à la taille de celui-ci. */
  virtualDisplay: boolean
  /** Débit réglé à la main (Mbit/s), ou null : celui du profil. */
  bitrate: number | null
  audio: boolean
  clipboard: boolean
  fullscreen: boolean
  transport: Transport
  codec: Codec
  touch: TouchMode
  /** Souris : capturée en mode jeu (déplacements bruts), libre en mode bureau. */
  mouse: MouseMode
  /** Fiche des gestes déjà vue. */
  touchGuideSeen: boolean
}

const KEY = 'kybernow.ecran.prefs'
const DEVICE_KEY = 'kybernow.ecran.device'
const LAST_KEY = 'kybernow.ecran.lastSession'

export const PROFILES: Profile[] = ['jeu', 'bureau', 'max']

export const defaults: Prefs = {
  profile: 'bureau',
  virtualDisplay: true,
  bitrate: null,
  audio: true,
  clipboard: true,
  fullscreen: true,
  transport: 'auto',
  codec: 'auto',
  touch: 'trackpad',
  mouse: 'auto',
  touchGuideSeen: false,
}

export function loadPrefs(): Prefs {
  try {
    const raw = localStorage.getItem(KEY)
    if (raw) {
      const p = { ...defaults, ...JSON.parse(raw) }
      if (!PROFILES.includes(p.profile)) p.profile = defaults.profile
      return p
    }
  } catch {
    // Stockage indisponible (navigation privée) : réglages par défaut.
  }
  return { ...defaults }
}

export function savePrefs(p: Prefs) {
  try {
    localStorage.setItem(KEY, JSON.stringify(p))
  } catch {
    // Sans stockage, les réglages valent pour cette visite.
  }
}

/** Réglages qui s'écartent du profil : format, débit ou transport choisis à la main. */
export function isCustom(p: Prefs): boolean {
  return p.bitrate !== null || p.codec !== 'auto' || p.transport !== 'auto'
}

/** Choisir un profil reprend toutes ses valeurs (plus de « Personnalisé »). */
export function pickProfile(p: Prefs, profile: Profile) {
  p.profile = profile
  p.bitrate = null
  p.codec = 'auto'
  p.transport = 'auto'
}

/** Nom du profil envoyé à la console (« Jeu », « Bureau », « Max »,
 * « Personnalisé ») : il ne se traduit pas, comme dans les apps. */
export function profileName(p: Prefs): string {
  if (isCustom(p)) return 'Personnalisé'
  return p.profile === 'jeu' ? 'Jeu' : p.profile === 'max' ? 'Max' : 'Bureau'
}

/** Identifiant stable de ce navigateur pour la liste des appareils de la console. */
export function deviceId(): string {
  try {
    let id = localStorage.getItem(DEVICE_KEY)
    if (!id) {
      id = crypto.randomUUID()
      localStorage.setItem(DEVICE_KEY, id)
    }
    return id
  } catch {
    return 'navigateur'
  }
}

/** Dernière session de ce navigateur sur cet ordinateur (rubrique Statistiques). */
export interface LastSession {
  /** Médiane de bout en bout, en ms. */
  median: number
  /** 95e centile, en ms. */
  p95: number | null
  /** Durée de l'image, en secondes. */
  duration: number
  /** Fin de la session (secondes depuis 1970). */
  endedAt: number
  width: number
  height: number
  refresh: number
  /** « HEVC », « H.264 », suivi de « 4:4:4 » s'il y était. */
  codec: string
  mbps: number
}

/** Session gardée à partir de 3 s d'image. */
export const MIN_SESSION_SECONDS = 3

export function loadLastSession(): LastSession | null {
  try {
    const raw = localStorage.getItem(LAST_KEY)
    if (!raw) return null
    const s = JSON.parse(raw)
    return typeof s?.median === 'number' && Number.isFinite(s.median) ? (s as LastSession) : null
  } catch {
    return null
  }
}

export function saveLastSession(s: LastSession) {
  try {
    localStorage.setItem(LAST_KEY, JSON.stringify(s))
  } catch {
    // Sans stockage, rien n'est gardé.
  }
}
