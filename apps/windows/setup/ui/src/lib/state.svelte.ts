// État de l'installation, alimenté par les événements de l'installateur.

import { onEvent, send, type Arch, type BackendEvent, type CheckStatus, type TaskStatus } from './backend'
import { t } from './i18n/index.svelte'

export type Screen = 'welcome' | 'install' | 'done' | 'about'

/** Trois étapes : l'analyse du PC tourne pendant la bienvenue. */
export const STEPS: { screens: Screen[]; label: string }[] = [
  { screens: ['welcome'], label: 'steps.welcome' },
  { screens: ['install'], label: 'steps.install' },
  { screens: ['done'], label: 'steps.done' },
]

export const CHECK_ORDER = ['windows', 'arch', 'gpu', 'display', 'network', 'webview']

export interface CheckItem {
  id: string
  status: CheckStatus | 'pending'
  title: string
  detail: string
}

export const TASK_ORDER = ['close', 'files', 'webview', 'shortcuts', 'register']

const freshChecks = () => CHECK_ORDER.map((id) => ({ id, status: 'pending', title: t(`checks.${id}`), detail: '' }) as CheckItem)

export const app = $state({
  screen: 'welcome' as Screen,
  previous: 'welcome' as Screen,
  checks: freshChecks(),
  checksStarted: false,
  checksDone: false,
  arch: 'x64' as Arch,
  /** Version déjà installée : c'est une mise à jour. */
  existing: null as string | null,
  running: false,
  dir: '',
  desktop: false,
  installing: false,
  percent: 0,
  tasks: Object.fromEntries(TASK_ORDER.map((id) => [id, 'pending'])) as Record<string, TaskStatus | 'pending'>,
  log: [] as string[],
  failure: null as { task: string; message: string } | null,
  finished: false,
})

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
      app.arch = event.arch
      app.existing = event.existing
      app.running = event.running
      app.desktop = event.desktop
      app.dir = event.dir
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
      app.installing = false
      app.finished = true
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
  app.checks = freshChecks()
  app.checksDone = false
  send({ cmd: 'runChecks' })
}

export function startInstall() {
  app.installing = true
  app.finished = false
  app.failure = null
  app.percent = 0
  app.log = []
  app.tasks = Object.fromEntries(TASK_ORDER.map((id) => [id, 'pending']))
  app.screen = 'install'
  send({ cmd: 'install', desktop: app.desktop })
}

/** Index de l'étape affichée dans la colonne (« À propos » garde l'étape précédente). */
export function stepIndex(): number {
  const screen = app.screen === 'about' ? app.previous : app.screen
  return Math.max(0, STEPS.findIndex((s) => s.screens.includes(screen)))
}

export const reducedMotion =
  typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
