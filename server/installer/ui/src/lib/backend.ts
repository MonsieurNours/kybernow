// Pont avec l'installateur Rust (window.ipc fourni par la vue web).
// Dans un navigateur ordinaire, une simulation joue le même rôle.

import { t } from './i18n/index.svelte'

export type CheckStatus = 'running' | 'ok' | 'warn' | 'error'
export type TaskStatus = 'running' | 'done' | 'skipped' | 'warn' | 'failed'

/** KyberNow déjà installé sur l'ordinateur. */
export interface Existing {
  version: string
  streamPort: number
  consolePort: number
  /** Réglages trouvés : « Conserver » possible. */
  settings: boolean
}

export type BackendEvent =
  | { type: 'check'; id: string; status: CheckStatus; title: string; detail: string }
  | {
      type: 'checksDone'
      streamPort: number
      consolePort: number
      busy: { port: number; process: string | null } | null
      lanIp: string | null
      steamSpeakers: boolean
      /** false : déjà en Performances élevées, l'option est décochée. */
      powerNeedsTuning: boolean
      existing: Existing | null
    }
  | { type: 'portsValidated'; streamError: string | null; consoleError: string | null }
  | { type: 'task'; id: string; status: TaskStatus }
  | { type: 'progress'; percent: number }
  | { type: 'log'; line: string }
  | { type: 'done'; lanIp: string | null; streamPort: number; consolePort: number; needReboot: boolean }
  | { type: 'failed'; task: string; message: string }

export interface InstallRequest {
  user: string
  password: string
  streamPort: number
  consolePort: number
  virtualDisplay: boolean
  gamepads: boolean
  virtualAudio: boolean
  maxPerformance: boolean
  startWithWindows: boolean
  /** Réinstallation : garder comptes, ports, certificat et réglages. */
  keepSettings: boolean
}

export type Command =
  | { cmd: 'runChecks' }
  | { cmd: 'validatePorts'; stream: number; console: number }
  | ({ cmd: 'install' } & InstallRequest)
  | { cmd: 'openUrl'; url: string }
  | { cmd: 'reboot' }
  | { cmd: 'minimize' }
  | { cmd: 'drag' }
  | { cmd: 'close' }

declare global {
  interface Window {
    ipc?: { postMessage(message: string): void }
    __knReceive?: (event: BackendEvent) => void
    /** Langue de Windows (« fr », « en »), posée par l'installateur avant la page. */
    __knLang?: string
  }
}

type Listener = (event: BackendEvent) => void
let listener: Listener = () => {}

export const isNative = typeof window !== 'undefined' && !!window.ipc

export function onEvent(fn: Listener) {
  listener = fn
  window.__knReceive = (event) => listener(event)
}

export function send(command: Command) {
  if (isNative) window.ipc!.postMessage(JSON.stringify(command))
  else simulate(command)
}

// --- Simulation (navigateur) ------------------------------------------------

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))
const emit = (event: BackendEvent) => listener(event)
const randomPort = () => 20000 + Math.floor(Math.random() * 25000)

async function simulate(command: Command) {
  switch (command.cmd) {
    case 'runChecks': {
      const stream = randomPort()
      const consolePort = randomPort()
      const items: [string, CheckStatus, string, string][] = [
        ['gpu', 'ok', t('checks.gpu'), t('simulation.gpu')],
        ['driver', 'ok', t('checks.driver'), t('simulation.driver')],
        ['network', 'ok', t('checks.network'), 'Ethernet · 192.168.1.100'],
        ['ports', 'ok', t('checks.ports'), t('simulation.ports', { stream, console: consolePort })],
        ['display', 'ok', t('checks.display'), '3840 × 2160 · 160 Hz'],
        ['power', 'warn', t('simulation.power'), t('simulation.powerDetail')],
        ['audio', 'ok', t('checks.audio'), t('simulation.audioDetail')],
      ]
      // ?existant dans l'adresse : simule une réinstallation par-dessus KyberNow 0.2.5.
      const existing = new URLSearchParams(location.search).has('existant')
        ? { version: '0.2.5', streamPort: 23456, consolePort: 34567, settings: true }
        : null
      if (existing) items.push(['existing', 'ok', t('simulation.existing'), t('simulation.existingDetail')])
      for (const [id, status, title, detail] of items) {
        emit({ type: 'check', id, status: 'running', title, detail: '' })
        await wait(650)
        emit({ type: 'check', id, status, title, detail })
      }
      emit({ type: 'checksDone', streamPort: stream, consolePort, busy: { port: 8080, process: 'httpd.exe' }, lanIp: '192.168.1.100', steamSpeakers: true, powerNeedsTuning: true, existing })
      break
    }
    case 'validatePorts': {
      await wait(250)
      const bad = (p: number, other: number) =>
        p < 1024 || p > 65535 ? t('simulation.portRange') : p === other ? t('simulation.samePort') : p === 8080 ? t('simulation.portInUse') : null
      emit({ type: 'portsValidated', streamError: bad(command.stream, command.console), consoleError: bad(command.console, command.stream) })
      break
    }
    case 'install': {
      const plan: [string, number, boolean][] = [
        ['files', 40, true],
        ['cert', 44, true],
        ['service', 52, true],
        ['firewall', 60, true],
        ['vdd', 70, command.virtualDisplay],
        ['gamepad', 82, command.gamepads],
        ['audio', 86, command.virtualAudio],
        ['admin', 92, true],
        ['power', 96, command.maxPerformance],
        ['finish', 100, true],
      ]
      let last = 0
      for (const [id, target, enabled] of plan) {
        if (!enabled) {
          emit({ type: 'task', id, status: 'skipped' })
          continue
        }
        emit({ type: 'task', id, status: 'running' })
        for (let i = 1; i <= 14; i++) {
          await wait(80)
          emit({ type: 'progress', percent: last + ((target - last) * i) / 14 })
        }
        last = target
        emit({ type: 'log', line: `${id} : terminé (simulation)` })
        emit({ type: 'task', id, status: 'done' })
      }
      emit({ type: 'done', lanIp: '192.168.1.100', streamPort: command.streamPort, consolePort: command.consolePort, needReboot: command.gamepads })
      break
    }
    case 'openUrl':
      window.open(command.url, '_blank', 'noopener')
      break
    default:
      break
  }
}
