// État de l'installateur Mac, alimenté par les messages de l'installateur
// (voir bridge.ts). Un seul état pour les quatre fenêtres : installation,
// mise à jour, désinstallation et autorisations.

import { t } from '../lib/i18n/index.svelte'
import { mode, onEvent, send, type CheckStatus, type Existing, type MacEvent, type Permissions, type TaskStatus, type UpdateInfo } from './bridge'

export type Screen = 'welcome' | 'reinstall' | 'account' | 'options' | 'install' | 'permissions' | 'done' | 'about'

/** Cinq étapes dans la colonne ; « Terminé » les coche toutes. */
export const STEPS: { screens: Screen[]; key: string }[] = [
  { screens: ['welcome'], key: 'mac.steps.welcome' },
  { screens: ['reinstall', 'account'], key: 'mac.steps.account' },
  { screens: ['options'], key: 'mac.steps.options' },
  { screens: ['install'], key: 'mac.steps.install' },
  { screens: ['permissions', 'done'], key: 'mac.steps.permissions' },
]

export const CHECK_ORDER = ['chip', 'macos', 'network', 'ports', 'displays', 'audio']
export const TASK_ORDER = ['kyber', 'cert', 'audio', 'startup', 'admin', 'firewall']

export interface CheckItem {
  id: string
  status: CheckStatus | 'pending'
  title: string
  detail: string
}

const freshChecks = () => CHECK_ORDER.map((id) => ({ id, status: 'pending', title: t(`mac.checks.${id}`), detail: '' }) as CheckItem)

export const app = $state({
  screen: 'welcome' as Screen,
  previous: 'welcome' as Screen,
  checks: freshChecks(),
  checksDone: false,
  lanIp: null as string | null,
  audioDriver: false,
  existing: null as Existing | null,
  keepSettings: false,
  ports: { stream: 0, console: 0 },
  portErrors: { stream: null as string | null, console: null as string | null },
  portsPending: false,
  user: '',
  password: '',
  options: { virtualDisplay: true, virtualAudio: true, startWithMac: true, keepReachable: true },
  installing: false,
  percent: 0,
  tasks: Object.fromEntries(TASK_ORDER.map((id) => [id, 'pending'])) as Record<string, TaskStatus | 'pending'>,
  taskDetail: {} as Record<string, string>,
  log: [] as string[],
  failure: null as { task: string; message: string } | null,
  result: null as { lanIp: string | null; streamPort: number; consolePort: number } | null,
  permissions: null as Permissions | null,
  /** Fenêtre de mise à jour. */
  update: {
    info: null as UpdateInfo | null,
    phase: 'checking' as 'checking' | 'ready' | 'working' | 'done' | 'failed',
    step: 'download' as 'download' | 'install' | 'restart',
    percent: 0,
    error: '',
  },
  /** Fenêtre de désinstallation. */
  uninstall: { keepSettings: true, phase: 'confirm' as 'confirm' | 'working' | 'done' | 'failed', error: '' },
})

let portsResolve: ((ok: boolean) => void) | null = null

function handle(event: MacEvent) {
  switch (event.type) {
    case 'check': {
      let item = app.checks.find((c) => c.id === event.id)
      if (!item) {
        app.checks.push({ id: event.id, status: 'pending', title: event.title, detail: '' })
        item = app.checks[app.checks.length - 1]
      }
      item.status = event.status
      item.title = event.title
      item.detail = event.detail
      break
    }
    case 'checksDone':
      app.checksDone = true
      app.lanIp = event.lanIp
      app.audioDriver = event.audioDriver
      app.existing = event.existing
      app.keepSettings = !!event.existing?.settings
      app.ports = { stream: event.streamPort, console: event.consolePort }
      break
    case 'portsValidated':
      app.portErrors = { stream: event.streamError, console: event.consoleError }
      app.portsPending = false
      portsResolve?.(!event.streamError && !event.consoleError)
      portsResolve = null
      break
    case 'task':
      app.tasks[event.id] = event.status
      if (event.detail) app.taskDetail[event.id] = event.detail
      break
    case 'progress':
      app.percent = Math.max(app.percent, Math.min(100, event.percent))
      break
    case 'log':
      app.log.push(event.line)
      break
    case 'failed':
      app.failure = { task: event.task, message: event.message }
      app.installing = false
      break
    case 'done':
      app.result = { lanIp: event.lanIp, streamPort: event.streamPort, consolePort: event.consolePort }
      app.installing = false
      app.percent = 100
      // Les autorisations se donnent à l'app installée : étape suivante.
      setTimeout(() => go('permissions'), 700)
      break
    case 'permissions':
      app.permissions = { screen: event.screen, accessibility: event.accessibility, microphone: event.microphone }
      break
    case 'updateInfo': {
      const { type: _, ...info } = event
      app.update.info = info
      if (app.update.phase === 'checking') app.update.phase = 'ready'
      break
    }
    case 'updateProgress':
      app.update.phase = 'working'
      app.update.step = event.phase
      app.update.percent = Math.max(app.update.percent, event.percent)
      break
    case 'updateDone':
      app.update.phase = 'done'
      app.update.percent = 100
      break
    case 'updateFailed':
      app.update.phase = 'failed'
      app.update.error = event.message
      break
    case 'uninstallDone':
      app.uninstall.phase = 'done'
      break
    case 'uninstallFailed':
      app.uninstall.phase = 'failed'
      app.uninstall.error = event.message
      break
  }
}

export function startBackend() {
  onEvent(handle)
  if (mode === 'install') send({ cmd: 'runChecks' })
  else if (mode === 'update') send({ cmd: 'updateCheck' })
}

export function go(screen: Screen) {
  if (screen === 'about') app.previous = app.screen
  app.screen = screen
}

export function rerunChecks() {
  app.checks = freshChecks()
  app.checksDone = false
  send({ cmd: 'runChecks' })
}

export function validatePorts(stream: number, consolePort: number): Promise<boolean> {
  app.portsPending = true
  return new Promise((resolve) => {
    portsResolve = (ok) => {
      if (ok) app.ports = { stream, console: consolePort }
      resolve(ok)
    }
    send({ cmd: 'validatePorts', stream, console: consolePort })
  })
}

/** Réinstallation en gardant les réglages de la Station en place. */
export function keeping(): boolean {
  return !!app.existing?.settings && app.keepSettings
}

export function beforeOptions(): Screen {
  return keeping() ? 'reinstall' : 'account'
}

export function startInstall() {
  const keep = keeping()
  if (keep) app.ports = { stream: app.existing!.streamPort, console: app.existing!.consolePort }
  app.installing = true
  app.failure = null
  app.percent = 0
  app.log = []
  app.tasks = Object.fromEntries(TASK_ORDER.map((id) => [id, 'pending']))
  app.screen = 'install'
  send({
    cmd: 'install',
    user: keep ? '' : app.user.trim(),
    password: keep ? '' : app.password,
    streamPort: app.ports.stream,
    consolePort: app.ports.console,
    ...app.options,
    keepSettings: keep,
  })
}

/** Étape affichée dans la colonne (« À propos » garde la précédente). */
export function stepIndex(): number {
  const screen = app.screen === 'about' ? app.previous : app.screen
  return Math.max(0, STEPS.findIndex((s) => s.screens.includes(screen)))
}

/** Les trois accords donnés. */
export function allGranted(): boolean {
  const p = app.permissions
  return !!p && p.screen && p.accessibility && p.microphone === 'granted'
}

export const reducedMotion = typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
