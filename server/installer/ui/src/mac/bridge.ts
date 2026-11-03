// Pont avec l'installateur Mac (Rust, vue WKWebView) : mêmes canaux que sous
// Windows (window.ipc.postMessage dans un sens, window.__knReceive dans
// l'autre), messages propres au Mac. Dans un navigateur ordinaire, une
// simulation joue le rôle du Mac (?mac, &mode=update|uninstall|permissions,
// &existant pour une réinstallation).

import { t } from '../lib/i18n/index.svelte'

export type CheckStatus = 'running' | 'ok' | 'warn' | 'error' | 'info'
export type TaskStatus = 'running' | 'done' | 'skipped' | 'warn' | 'failed'
export type Mic = 'granted' | 'denied' | 'undetermined'
export type Mode = 'install' | 'update' | 'uninstall' | 'permissions'

/** KyberNow Station déjà installée sur ce Mac. */
export interface Existing {
  version: string
  streamPort: number
  consolePort: number
  settings: boolean
}

export interface Permissions {
  screen: boolean
  accessibility: boolean
  microphone: Mic
}

export interface UpdateInfo {
  current: string
  version: string | null
  notes: string[]
  page: string | null
  /** Appareil en session, coupé par la mise à jour. */
  session: string | null
  /** Pourquoi la version ne s'installe pas (texte de Admin). */
  refusal: string | null
}

export type MacEvent =
  | { type: 'check'; id: string; status: CheckStatus; title: string; detail: string }
  | {
      type: 'checksDone'
      streamPort: number
      consolePort: number
      lanIp: string | null
      existing: Existing | null
      /** Sortie son KyberNow déjà installée. */
      audioDriver: boolean
    }
  | { type: 'portsValidated'; streamError: string | null; consoleError: string | null }
  | { type: 'task'; id: string; status: TaskStatus; detail?: string }
  | { type: 'progress'; percent: number }
  | { type: 'log'; line: string }
  | { type: 'done'; lanIp: string | null; streamPort: number; consolePort: number }
  | { type: 'failed'; task: string; message: string }
  | ({ type: 'permissions' } & Permissions)
  | ({ type: 'updateInfo' } & UpdateInfo)
  | { type: 'updateProgress'; phase: 'download' | 'install' | 'restart'; percent: number }
  | { type: 'updateDone'; version: string }
  | { type: 'updateFailed'; message: string }
  | { type: 'uninstallDone' }
  | { type: 'uninstallFailed'; message: string }

export interface InstallRequest {
  user: string
  password: string
  streamPort: number
  consolePort: number
  virtualDisplay: boolean
  virtualAudio: boolean
  startWithMac: boolean
  keepReachable: boolean
  keepSettings: boolean
}

export type Which = 'screen' | 'accessibility' | 'microphone'

export type MacCommand =
  | { cmd: 'runChecks' }
  | { cmd: 'validatePorts'; stream: number; console: number }
  | ({ cmd: 'install' } & InstallRequest)
  | { cmd: 'watchPermissions'; on: boolean }
  | { cmd: 'requestPermission'; which: Which }
  | { cmd: 'permissionsDone' }
  | { cmd: 'updateCheck' }
  | { cmd: 'updateStart' }
  | { cmd: 'uninstall'; keepSettings: boolean }
  | { cmd: 'openUrl'; url: string }
  | { cmd: 'copy'; text: string }
  | { cmd: 'drag' }
  | { cmd: 'close' }

declare global {
  interface Window {
    __knPlatform?: string
    __knMode?: string
  }
}

type Listener = (event: MacEvent) => void
let listener: Listener = () => {}

const params = typeof window !== 'undefined' ? new URLSearchParams(location.search) : new URLSearchParams()
export const isNative = typeof window !== 'undefined' && !!window.ipc

/** Interface du Mac : posée par l'installateur, ou `?mac` dans un navigateur. */
export const isMac = typeof window !== 'undefined' && (window.__knPlatform === 'macos' || params.has('mac'))

export const mode: Mode = (() => {
  const m = (typeof window !== 'undefined' && window.__knMode) || params.get('mode') || 'install'
  return (['install', 'update', 'uninstall', 'permissions'].includes(m) ? m : 'install') as Mode
})()

export function onEvent(fn: Listener) {
  listener = fn
  window.__knReceive = (event) => listener(event as unknown as MacEvent)
}

export function send(command: MacCommand) {
  if (isNative) window.ipc!.postMessage(JSON.stringify(command))
  else simulate(command)
}

// --- Simulation (navigateur) ------------------------------------------------

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))
const emit = (event: MacEvent) => listener(event)
const sim = { permissions: { screen: true, accessibility: false, microphone: 'undetermined' as Mic }, watching: 0 }

async function simulate(command: MacCommand) {
  switch (command.cmd) {
    case 'runChecks': {
      const existing = params.has('existant') ? { version: '0.9.4', streamPort: 23456, consolePort: 34567, settings: true } : null
      const items: [string, CheckStatus, string][] = [
        ['chip', 'ok', t('mac.simulation.chip')],
        ['macos', 'ok', t('mac.simulation.macos')],
        ['network', 'ok', t('mac.simulation.network')],
        ['ports', 'ok', t('mac.checks.portsDetail', { stream: 8080, console: 9443 })],
        ['displays', 'warn', t('mac.checks.displaysDetail', { count: 3 })],
        ['audio', 'info', t('mac.checks.audioNew')],
      ]
      if (existing) items.push(['existing', 'info', t('mac.checks.existingDetail', { version: existing.version })])
      for (const [id, status, detail] of items) {
        emit({ type: 'check', id, status: 'running', title: t(`mac.checks.${id}`), detail: '' })
        await wait(420)
        emit({ type: 'check', id, status, title: t(`mac.checks.${id}`), detail })
      }
      emit({ type: 'checksDone', streamPort: 8080, consolePort: 9443, lanIp: '192.168.1.105', existing, audioDriver: false })
      break
    }
    case 'validatePorts': {
      await wait(200)
      const bad = (p: number, other: number) => (p < 1024 || p > 65535 ? t('mac.simulation.portRange') : p === other ? t('mac.simulation.samePort') : null)
      emit({ type: 'portsValidated', streamError: bad(command.stream, command.console), consoleError: bad(command.console, command.stream) })
      break
    }
    case 'install': {
      const plan: [string, number, boolean, string][] = [
        ['kyber', 30, true, 'ditto KyberNow Station.app /Applications'],
        ['cert', 40, !command.keepSettings, 'openssl req -x509 … kyber_cert.pem'],
        ['audio', 55, command.virtualAudio, 'KyberNow.driver → /Library/Audio/Plug-Ins/HAL'],
        ['startup', 70, true, 'launchctl bootstrap gui/501 …station.kyber'],
        ['admin', 90, true, 'kyber-admin : https://localhost:9443'],
        ['firewall', 100, true, 'socketfilterfw --getglobalstate'],
      ]
      let last = 0
      for (const [id, target, enabled, line] of plan) {
        if (!enabled) {
          emit({ type: 'task', id, status: 'skipped' })
          continue
        }
        emit({ type: 'task', id, status: 'running' })
        emit({ type: 'log', line })
        for (let i = 1; i <= 12; i++) {
          await wait(110)
          emit({ type: 'progress', percent: last + ((target - last) * i) / 12 })
        }
        last = target
        emit({ type: 'task', id, status: 'done' })
      }
      emit({ type: 'done', lanIp: '192.168.1.105', streamPort: command.streamPort, consolePort: command.consolePort })
      break
    }
    case 'watchPermissions':
      if (command.on) {
        const id = ++sim.watching
        emit({ type: 'permissions', ...sim.permissions })
        while (sim.watching === id) {
          await wait(1500)
          if (sim.watching === id) emit({ type: 'permissions', ...sim.permissions })
        }
      } else sim.watching++
      break
    case 'requestPermission':
      await wait(1200)
      if (command.which === 'microphone') sim.permissions.microphone = 'granted'
      else sim.permissions[command.which] = true
      emit({ type: 'permissions', ...sim.permissions })
      break
    case 'updateCheck':
      await wait(300)
      emit({
        type: 'updateInfo',
        current: '1.0.0',
        version: '1.0.1',
        notes: [t('mac.simulation.note1'), t('mac.simulation.note2')],
        page: 'https://kybernow.app',
        session: params.has('session') ? 'iPhone de Matthieu' : null,
        refusal: null,
      })
      break
    case 'updateStart':
      for (let p = 0; p <= 100; p += 4) {
        await wait(90)
        emit({ type: 'updateProgress', phase: p < 70 ? 'download' : p < 95 ? 'install' : 'restart', percent: p })
      }
      emit({ type: 'updateDone', version: '1.0.1' })
      break
    case 'uninstall':
      await wait(1800)
      emit({ type: 'uninstallDone' })
      break
    case 'openUrl':
      window.open(command.url, '_blank', 'noopener')
      break
    case 'copy':
      void navigator.clipboard?.writeText(command.text)
      break
    default:
      break
  }
}
