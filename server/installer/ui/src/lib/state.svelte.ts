// État de l'installation, alimenté par les événements de l'installateur.

import { onEvent, send, type BackendEvent, type CheckStatus, type TaskStatus } from './backend'

export type Screen = 'welcome' | 'account' | 'options' | 'install' | 'done' | 'about'

/** Quatre étapes : l'analyse de l'ordinateur tourne pendant la bienvenue, « Terminé » clôt l'installation. */
export const STEPS: { screens: Screen[]; label: string }[] = [
  { screens: ['welcome'], label: 'Bienvenue' },
  { screens: ['account'], label: 'Votre compte' },
  { screens: ['options'], label: 'Options' },
  { screens: ['install', 'done'], label: 'Installation' },
]

export const CHECK_ORDER = ['gpu', 'driver', 'network', 'ports', 'display', 'power', 'audio']
const CHECK_TITLES: Record<string, string> = {
  gpu: 'Carte graphique NVIDIA',
  driver: 'Pilote graphique',
  network: 'Réseau local',
  ports: 'Ports réseau',
  display: 'Écran principal',
  power: "Mode d'alimentation",
  audio: 'Son à distance',
}

export interface CheckItem {
  id: string
  status: CheckStatus | 'pending'
  title: string
  detail: string
}

export const TASK_ORDER = ['files', 'cert', 'service', 'firewall', 'vdd', 'gamepad', 'audio', 'admin', 'power']

export const app = $state({
  screen: 'welcome' as Screen,
  previous: 'welcome' as Screen,
  checks: CHECK_ORDER.map((id) => ({ id, status: 'pending', title: CHECK_TITLES[id], detail: '' }) as CheckItem),
  checksStarted: false,
  checksDone: false,
  lanIp: null as string | null,
  busyPort: null as { port: number; process: string | null } | null,
  steamSpeakers: false,
  ports: { stream: 0, console: 0 },
  portErrors: { stream: null as string | null, console: null as string | null },
  portsPending: false,
  user: '',
  password: '',
  options: { virtualDisplay: true, gamepads: true, virtualAudio: true, maxPerformance: true, startWithWindows: true },
  installing: false,
  percent: 0,
  tasks: Object.fromEntries(TASK_ORDER.map((id) => [id, 'pending'])) as Record<string, TaskStatus | 'pending'>,
  log: [] as string[],
  failure: null as { task: string; message: string } | null,
  result: null as { lanIp: string | null; streamPort: number; consolePort: number; needReboot: boolean } | null,
})

let portsResolve: ((ok: boolean) => void) | null = null

function handle(event: BackendEvent) {
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
      app.busyPort = event.busy
      app.steamSpeakers = event.steamSpeakers
      if (!event.steamSpeakers) app.options.virtualAudio = false
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
      app.result = { lanIp: event.lanIp, streamPort: event.streamPort, consolePort: event.consolePort, needReboot: event.needReboot }
      app.installing = false
      app.percent = 100
      break
  }
}

export function startBackend() {
  onEvent(handle)
  // L'analyse ne modifie rien : elle commence dès l'ouverture, pendant la bienvenue.
  if (!app.checksStarted) {
    app.checksStarted = true
    send({ cmd: 'runChecks' })
  }
}

export function go(screen: Screen) {
  if (screen === 'about') app.previous = app.screen
  app.screen = screen
}

export function rerunChecks() {
  app.checks = CHECK_ORDER.map((id) => ({ id, status: 'pending', title: CHECK_TITLES[id], detail: '' }))
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

export function startInstall() {
  app.installing = true
  app.failure = null
  app.percent = 0
  app.log = []
  app.tasks = Object.fromEntries(TASK_ORDER.map((id) => [id, 'pending']))
  app.screen = 'install'
  send({
    cmd: 'install',
    user: app.user.trim(),
    password: app.password,
    streamPort: app.ports.stream,
    consolePort: app.ports.console,
    ...app.options,
  })
}

/** Index de l'étape affichée dans la colonne (l'écran « À propos » garde l'étape précédente). */
export function stepIndex(): number {
  const screen = app.screen === 'about' ? app.previous : app.screen
  return Math.max(0, STEPS.findIndex((s) => s.screens.includes(screen)))
}

export const reducedMotion =
  typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
