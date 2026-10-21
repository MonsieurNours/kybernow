// Appels à l'app (commandes Tauri de src/ui.rs) et ses nouvelles (événements).
// L'API de Tauri est exposée en global (`withGlobalTauri`).


type Tauri = {
  core: { invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> }
  event: { listen<T>(name: string, cb: (e: { payload: T }) => void): Promise<() => void> }
  window: { getCurrentWindow(): { minimize(): Promise<void>; toggleMaximize(): Promise<void>; close(): Promise<void> } }
}

import { mockTauri } from './mock'

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
  vsync: boolean
  mouse: 'auto' | 'bureau' | 'jeu'
  gamepad: 'xbox360' | 'fidele'
  clipboard: boolean
  audio: 'auto' | '2' | '6' | '8' | '0'
  virtual_display: boolean
  battery: 'alleger' | 'garder'
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

export type Snapshot = { computers: Computer[]; discovered: Found[]; this_pc: ThisPc | null; session: string | null; version: string }

export type Phase = { id: string; phase: 'preparing' | 'connecting' | 'first_images' | 'streaming' | 'reconnecting' | 'ending' | 'ended'; detail: string | null }

export type Summary = { error: string | null; median_ms: number | null; p95_ms: number | null; seconds: number }

export type Stats = { latency_ms: number | null; fps: number; width: number; height: number; decoder: string; relative: boolean; rtt_ms: number | null }

/** « 8,9 » : nombres à la française. */
export const ms = (v: number | null | undefined, digits = 1) => (v == null ? '—' : v.toFixed(digits).replace('.', ','))

export const duration = (s: number) => {
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  return h > 0 ? `${h} h ${String(m).padStart(2, '0')}` : `${m} min`
}

/** Un profil retouché (format, couleurs, débit, transport) est « Personnalisé ». */
export const isCustom = (s: Settings) => s.codec != null || s.yuv444 != null || s.mbps != null || s.reliable != null

export const profileLabel = (s: Settings) => (isCustom(s) ? 'Personnalisé' : { jeu: 'Jeu', bureau: 'Bureau', max: 'Max' }[s.profile])

/** Teinte de la signature d'un ordinateur, tirée de l'empreinte de sa console. */
export const hue = (fp: string | null, salt = 0) => {
  let h = 2166136261 + salt
  for (const c of fp ?? 'kybernow') h = Math.imul(h ^ c.charCodeAt(0), 16777619)
  return Math.abs(h) % 360
}
