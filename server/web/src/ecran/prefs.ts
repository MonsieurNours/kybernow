// Réglages du client web, gardés dans ce navigateur.

export type Profile = 'bureau' | 'jeu'
export type Transport = 'auto' | 'fiable' | 'rapide'
export type Codec = 'auto' | 'h264' | 'hevc'
export type TouchMode = 'trackpad' | 'direct'

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
  /** Fiche des gestes déjà vue. */
  touchGuideSeen: boolean
}

const KEY = 'kybernow.ecran.prefs'
const DEVICE_KEY = 'kybernow.ecran.device'

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
  touchGuideSeen: false,
}

export function loadPrefs(): Prefs {
  try {
    const raw = localStorage.getItem(KEY)
    if (raw) return { ...defaults, ...JSON.parse(raw) }
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
