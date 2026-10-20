// Ce que ce navigateur et cet écran savent faire.

export interface Mode {
  width: number
  height: number
  refresh: number
}

export const isTouch = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches && !matchMedia('(pointer: fine)').matches

export const hasWebTransport = typeof (globalThis as any).WebTransport === 'function'

/** Safari et tous les navigateurs d'iPhone et d'iPad : leur WebTransport ne
 * vérifie pas un certificat par son empreinte, l'image passe par WebSocket. */
export const isWebKit = /AppleWebKit/.test(navigator.userAgent) && !/Chrome|Chromium|Edg\/|OPR\//.test(navigator.userAgent)

/** Firefox : image par WebSocket et en H.264 par défaut (`plan.ts`). */
export const isFirefox = /Firefox\//.test(navigator.userAgent)

export const isIsolated = (globalThis as any).crossOriginIsolated === true
export const hasKeyboardLock = typeof (navigator as any).keyboard?.lock === 'function'
export const canFullscreen = typeof document.documentElement.requestFullscreen === 'function'

/** Installée sur l'écran d'accueil (plein écran sans barre du navigateur). */
export const isStandalone = typeof matchMedia === 'function' && matchMedia('(display-mode: standalone)').matches

export function deviceKind(): 'phone' | 'tablet' | 'browser' {
  if (!isTouch) return 'browser'
  return Math.min(screen.width, screen.height) < 600 ? 'phone' : 'tablet'
}

function browserName(ua: string): string {
  if (/Edg\//.test(ua)) return 'Edge'
  if (/OPR\//.test(ua)) return 'Opera'
  if (/Firefox\//.test(ua)) return 'Firefox'
  if (/Chrome\//.test(ua)) return 'Chrome'
  if (/Safari\//.test(ua)) return 'Safari'
  return 'Navigateur'
}

function systemName(ua: string): string {
  if (/iPhone/.test(ua)) return 'iPhone'
  if (/iPad/.test(ua) || (/Macintosh/.test(ua) && isTouch)) return 'iPad'
  if (/Android/.test(ua)) return 'Android'
  if (/CrOS/.test(ua)) return 'ChromeOS'
  if (/Windows/.test(ua)) return 'Windows'
  if (/Macintosh/.test(ua)) return 'Mac'
  if (/Linux/.test(ua)) return 'Linux'
  return ''
}

/** « Chrome sur Windows », pour la liste des appareils de la console. */
export function deviceName(): { name: string; os: string; app: string } {
  const ua = navigator.userAgent
  const browser = browserName(ua)
  const os = systemName(ua)
  return { name: os ? `${browser} sur ${os}` : browser, os, app: `${browser} · KyberNow web` }
}

/** Taille de cet écran en pixels réels, bornée à la 4K, en nombres pairs. */
export function screenMode(refresh: number): Mode {
  const dpr = window.devicePixelRatio || 1
  let w = Math.round(screen.width * dpr)
  let h = Math.round(screen.height * dpr)
  // Écran tourné : la largeur suit l'orientation actuelle.
  const landscape = typeof matchMedia === 'function' && matchMedia('(orientation: landscape)').matches
  if (landscape !== w >= h) [w, h] = [h, w]
  const scale = Math.min(1, 3840 / Math.max(w, h), 2160 / Math.min(w, h))
  w = Math.round((w * scale) / 2) * 2
  h = Math.round((h * scale) / 2) * 2
  return { width: w, height: h, refresh }
}

const RATES = [50, 60, 75, 90, 100, 120, 144, 165, 240]

/** Fréquence de l'écran, mesurée sur une demi-seconde d'images. */
export function measureRefresh(): Promise<number> {
  return new Promise((resolve) => {
    const times: number[] = []
    const step = (t: number) => {
      times.push(t)
      if (times.length < 32) requestAnimationFrame(step)
      else {
        const deltas = times.slice(1).map((v, i) => v - times[i]).sort((a, b) => a - b)
        const median = deltas[Math.floor(deltas.length / 2)] || 16.7
        const hz = 1000 / median
        resolve(RATES.reduce((best, r) => (Math.abs(r - hz) < Math.abs(best - hz) ? r : best), 60))
      }
    }
    requestAnimationFrame(step)
    // Onglet en arrière-plan : pas d'images, 60 Hz.
    setTimeout(() => times.length < 32 && resolve(60), 1500)
  })
}
