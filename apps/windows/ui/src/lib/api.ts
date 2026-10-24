// Appels à l'app (commandes Tauri de src/ui.rs) et ses nouvelles (événements).
// L'API de Tauri est exposée en global (`withGlobalTauri`).


type Tauri = {
  core: { invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> }
  event: { listen<T>(name: string, cb: (e: { payload: T }) => void): Promise<() => void> }
  window: { getCurrentWindow(): { minimize(): Promise<void>; toggleMaximize(): Promise<void>; close(): Promise<void> } }
}

import { mockTauri } from './mock'
import { i18n, t } from './i18n/index.svelte'

// Dans un navigateur (développement), une imitation de l'app.
const tauri: Tauri =
  (window as unknown as { __TAURI__?: Tauri }).__TAURI__ ?? (import.meta.env.DEV ? (mockTauri() as unknown as Tauri) : (undefined as never))

export const invoke = <T>(cmd: string, args?: Record<string, unknown>) => tauri.core.invoke<T>(cmd, args)
export const listen = <T>(name: string, cb: (payload: T) => void) => tauri.event.listen<T>(name, (e) => cb(e.payload))
export const currentWindow = () => tauri.window.getCurrentWindow()

export type Settings = {
  profile: 'jeu' | 'bureau' | 'max'
  codec: 'h264' | 'hevc' | 'av1' | null
  yuv444: boolean | null
  mbps: number | null
  reliable: boolean | null
  max_4k: boolean
  /** Écran virtuel sur mesure ; sinon comme cet écran. */
  custom: { width: number; height: number; refresh: number } | null
  vsync: boolean
  mouse: 'auto' | 'bureau' | 'jeu'
  gamepad: 'xbox360' | 'fidele'
  clipboard: boolean
  audio: 'auto' | '2' | '6' | '8' | '0'
  virtual_display: boolean
  battery: 'alleger' | 'garder'
  keep_last_image: boolean
}

export type Info = { hostname: string; os: string; gpu: string | null; av1: boolean; virtual_display: { available: boolean } }

export type LastSession = { median_ms: number | null; p95_ms: number | null; seconds: number; label: string; at: number }

export type Computer = {
  id: string
  name: string
  host: string
  port: number
  user: string
  fingerprint: string | null
  os: string
  gpu: string | null
  settings: Settings
  last: LastSession | null
  online: boolean | null
  info: Info | null
  preview: string | null
  /** last : dernière image de votre session ; preview : aperçu de son écran. */
  preview_source: 'last' | 'preview' | null
}

export type Found = { id: string; name: string; host: string; port: number; fingerprint: string | null }

export type Candidate = { host: string; port: number; name: string | null; fingerprint: string | null }

export type ThisPc = {
  name: string
  gpu: string
  h264: boolean
  hevc: boolean
  hevc444: boolean
  av1: boolean
  width: number
  height: number
  refresh: number
  audio_channels: number
  os: string
  on_battery: boolean
}

export type Update = { version: string; channel: 'stable' | 'beta'; notes: string; page: string; file: string; size: number }

/** `language` : choix des réglages ; `lang` : langue appliquée. */
export type AppInfo = { version: string; channel: 'stable' | 'beta'; language: 'auto' | 'fr' | 'en'; lang: 'fr' | 'en'; update: Update | null; arch: string }

/** Ce que l'ouverture demande (lien, barre des tâches, notification, installateur). */
export type Action =
  | { kind: 'connect'; id: string }
  | { kind: 'add' }
  | { kind: 'link'; link: string }
  | { kind: 'updated' }
  | { kind: 'open' }

export type Snapshot = { computers: Computer[]; discovered: Found[]; this_pc: ThisPc | null; session: string | null; version: string }

export type Phase = { id: string; phase: 'preparing' | 'connecting' | 'first_images' | 'streaming' | 'reconnecting' | 'ending' | 'ended'; detail: string | null }

export type Summary = { error: string | null; median_ms: number | null; p95_ms: number | null; seconds: number }

export type Stats = { latency_ms: number | null; fps: number; width: number; height: number; decoder: string; relative: boolean; rtt_ms: number | null; audio_ms: number | null; audio_gaps: number | null }

/** « 8,9 » en français, « 8.9 » en anglais. */
export const ms = (v: number | null | undefined, digits = 1) => {
  if (v == null) return '—'
  const s = v.toFixed(digits)
  return i18n.lang === 'fr' ? s.replace('.', ',') : s
}

export const duration = (s: number) => {
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  return h > 0 ? t('time.hours', { h, m: String(m).padStart(2, '0') }) : t('time.minutes', { m })
}

/** Un profil retouché (format, couleurs, débit, transport) est « Personnalisé ». */
export const isCustom = (s: Settings) => s.codec != null || s.yuv444 != null || s.mbps != null || s.reliable != null

/** Valeurs d'un profil pour ce PC, comme le cœur les calcule (`StreamSettings::for_profile`, profile.rs). */
export function profileBase(profile: Settings['profile'], pc: ThisPc | null) {
  const codec: 'h264' | 'hevc' = (pc?.hevc ?? true) ? 'hevc' : 'h264'
  let [yuv444, mbps, reliable] = { jeu: [false, 60, false], bureau: [true, 90, true], max: [true, 300, false] }[profile] as [boolean, number, boolean]
  if (yuv444 && !(codec === 'hevc' && pc?.hevc444)) {
    yuv444 = false
    mbps += Math.floor(mbps / 3)
  }
  if (codec === 'h264') mbps += Math.floor(mbps / 2)
  return { codec, yuv444, mbps, reliable }
}

type Mode = { width: number; height: number; refresh: number }

/** `fit` de plan.rs : réduit pour tenir dans `maxW` × `maxH`, proportions gardées, dimensions paires (arrondi vers le bas). */
function fit(m: Mode, maxW: number, maxH: number): Mode {
  if (m.width <= maxW && m.height <= maxH) return m
  const k = Math.min(maxW / m.width, maxH / m.height)
  return { width: Math.floor(m.width * k) & ~1, height: Math.floor(m.height * k) & ~1, refresh: m.refresh }
}

/**
 * Ce que la session demandera à l'ordinateur : `session_plan` (ui.rs) et
 * `plan` (plan.rs), règle pour règle. Mode de l'écran virtuel et débit en
 * Mbit/s ; `null` sans écran connu (ni ce PC ni taille sur mesure).
 */
export function sessionPlan(s: Settings, pc: ThisPc | null): (Mode & { mbps: number }) | null {
  const custom = s.custom && s.custom.width >= 640 && s.custom.height >= 360 && s.custom.refresh >= 24 ? s.custom : null
  let mode: Mode | null = custom ?? (pc ? { width: pc.width, height: pc.height, refresh: pc.refresh } : null)
  if (!mode) return null
  if (s.max_4k && !s.custom) mode = fit(mode, 3840, 2160)
  if (s.battery === 'alleger' && pc?.on_battery) mode = { ...mode, refresh: Math.min(30, mode.refresh) }
  const base = profileBase(s.profile, pc)
  // NVENC n'encode pas l'H.264 au-delà de 4096 de large.
  if ((s.codec ?? base.codec) === 'h264') mode = fit(mode, 4096, 4096)
  // Débit du profil pour ce mode (`StreamSettings::bitrate`), sauf débit réglé.
  const ratio = (mode.width * mode.height * Math.max(1, mode.refresh)) / (3840 * 2160 * 60)
  const scale = Math.min(3, Math.max(0.4, Math.pow(ratio, 0.75)))
  const mbps = s.mbps ?? Math.min(4294, Math.max(5, Math.round(base.mbps * scale)))
  return { ...mode, mbps }
}

export const profileLabel = (s: Settings) => (isCustom(s) ? t('profiles.custom') : t(`profiles.${s.profile}`))

/** Noms des profils tels que l'app les écrit (« Bureau · HEVC 4:4:4 », barre et dernière session). */
const profileNames: Record<string, string> = { Jeu: 'jeu', Bureau: 'bureau', Max: 'max', Personnalisé: 'custom' }

/** « Bureau · HEVC 4:4:4 » dans la langue de l'interface. */
export const localLabel = (label: string) => {
  const [first, ...rest] = label.split(' · ')
  const key = profileNames[first]
  return key ? [t(`profiles.${key}`), ...rest].join(' · ') : label
}

/** Teinte de la signature d'un ordinateur, tirée de l'empreinte de son Admin. */
export const hue = (fp: string | null, salt = 0) => {
  let h = 2166136261 + salt
  for (const c of fp ?? 'kybernow') h = Math.imul(h ^ c.charCodeAt(0), 16777619)
  return Math.abs(h) % 360
}
