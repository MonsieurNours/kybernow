// Pont avec l'installateur Rust (window.ipc fourni par la vue web).
// Dans un navigateur ordinaire, une simulation joue le même rôle.

import { t } from './i18n/index.svelte'

export type CheckStatus = 'running' | 'ok' | 'warn' | 'error' | 'info'
export type TaskStatus = 'running' | 'done' | 'skipped' | 'warn' | 'failed'
export type Arch = 'x64' | 'arm64'

export type BackendEvent =
  | { type: 'check'; id: string; status: CheckStatus; title: string; detail: string }
  | { type: 'checksDone'; arch: Arch; existing: string | null; running: boolean; desktop: boolean; dir: string }
  | { type: 'task'; id: string; status: TaskStatus }
  | { type: 'progress'; percent: number }
  | { type: 'log'; line: string }
  | { type: 'done' }
  | { type: 'failed'; task: string; message: string }

export type Command =
  | { cmd: 'runChecks' }
  | { cmd: 'install'; desktop: boolean }
  | { cmd: 'launch' }
  | { cmd: 'openUrl'; url: string }
  | { cmd: 'minimize' }
  | { cmd: 'drag' }
  | { cmd: 'close' }

declare global {
  interface Window {
    ipc?: { postMessage(message: string): void }
    __knReceive?: (event: BackendEvent) => void
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
// ?maj : mise à jour d'une version installée ; ?ouverte : l'app est ouverte ;
// ?arm : PC ARM ; ?echec : l'installation échoue ; ?lang=en : en anglais.

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))
const emit = (event: BackendEvent) => listener(event)
const query = () => new URLSearchParams(location.search)

async function simulate(command: Command) {
  switch (command.cmd) {
    case 'runChecks': {
      const q = query()
      const arm = q.has('arm')
      const items: [string, CheckStatus, string, string][] = [
        ['windows', 'ok', 'Windows 11 · 24H2', 'Build 26100'],
        arm ? ['arch', 'ok', t('mock.arm64Title'), t('mock.arm64')] : ['arch', 'ok', t('mock.x64Title'), t('mock.x64')],
        ['gpu', 'ok', t('checks.gpu'), t('mock.gpu', { name: arm ? 'Qualcomm Adreno X1-85' : 'Intel Iris Xe Graphics' })],
        ['display', 'ok', t('checks.display'), t('mock.display')],
        ['network', 'ok', t('checks.network'), t('mock.network')],
        ['webview', 'ok', t('mock.webviewTitle'), t('mock.webview')],
      ]
      if (q.has('maj')) items.push(['existing', 'info', t('mock.existingTitle'), t('mock.existing')])
      if (q.has('ouverte')) items.push(['running', 'warn', t('mock.runningTitle'), t('mock.running')])
      for (const [id, status, title, detail] of items) {
        emit({ type: 'check', id, status: 'running', title, detail: '' })
        await wait(420)
        emit({ type: 'check', id, status, title, detail })
      }
      emit({
        type: 'checksDone',
        arch: arm ? 'arm64' : 'x64',
        existing: q.has('maj') ? '0.2.7' : null,
        running: q.has('ouverte'),
        desktop: false,
        dir: 'C:\\Users\\Camille\\AppData\\Local\\Programs\\KyberNow',
      })
      break
    }
    case 'install': {
      const q = query()
      const plan: [string, number, boolean][] = [
        ['close', 4, q.has('ouverte')],
        ['files', 84, true],
        ['webview', 92, false],
        ['shortcuts', 96, true],
        ['register', 100, true],
      ]
      let last = 0
      for (const [id, target, enabled] of plan) {
        if (!enabled) {
          emit({ type: 'task', id, status: 'running' })
          await wait(150)
          emit({ type: 'task', id, status: 'skipped' })
          continue
        }
        emit({ type: 'task', id, status: 'running' })
        const steps = id === 'files' ? 40 : 8
        for (let i = 1; i <= steps; i++) {
          await wait(70)
          emit({ type: 'progress', percent: last + ((target - last) * i) / steps })
          if (id === 'files' && i % 8 === 0) emit({ type: 'log', line: `  ${['kybernow.exe', 'kynput.dll', 'SDL2.dll', 'WebView2Loader.dll', 'uninstall.exe'][i / 8 - 1]}` })
        }
        if (id === 'files' && q.has('echec')) {
          emit({ type: 'task', id, status: 'failed' })
          emit({ type: 'failed', task: id, message: t('mock.failed') })
          return
        }
        last = target
        emit({ type: 'task', id, status: 'done' })
      }
      emit({ type: 'log', line: t('mock.finished') })
      emit({ type: 'done' })
      break
    }
    case 'openUrl':
      window.open(command.url, '_blank', 'noopener')
      break
    default:
      break
  }
}
