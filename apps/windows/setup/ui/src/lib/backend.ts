// Pont avec l'installateur Rust (window.ipc fourni par la vue web).
// Dans un navigateur ordinaire, une simulation joue le même rôle.

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
// ?arm : PC ARM ; ?echec : l'installation échoue.

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
        arm
          ? ['arch', 'ok', 'Processeur ARM64', "La version ARM64 de KyberNow s'installe : elle tourne sans émulation, avec le décodage vidéo de la puce."]
          : ['arch', 'ok', 'Processeur x64', "La version x64 de KyberNow s'installe."],
        ['gpu', 'ok', 'Carte graphique', `${arm ? 'Qualcomm Adreno X1-85' : 'Intel Iris Xe Graphics'} : elle décode l'image, pour moins de latence et moins de batterie.`],
        ['display', 'ok', 'Écran', '2560 × 1600 · 120 Hz : votre ordinateur peut prendre exactement cette taille pendant la session.'],
        ['network', 'ok', 'Réseau local', '192.168.1.20 : un ordinateur sur ce réseau apparaît tout seul dans KyberNow.'],
        ['webview', 'ok', 'Moteur d’affichage WebView2', 'Version 141.0.3537.71, de Microsoft : il dessine l’interface de KyberNow.'],
      ]
      if (q.has('maj')) items.push(['existing', 'info', 'KyberNow 0.2.7 est installée', 'Elle sera mise à jour. Vos ordinateurs, leurs mots de passe et vos réglages restent.'])
      if (q.has('ouverte')) items.push(['running', 'warn', 'KyberNow est ouverte', 'Elle sera fermée au moment d’installer : une session en cours s’arrêtera.'])
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
          emit({ type: 'failed', task: id, message: 'Impossible de poser les fichiers dans C:\\Users\\Camille\\AppData\\Local\\Programs\\KyberNow (simulation).' })
          return
        }
        last = target
        emit({ type: 'task', id, status: 'done' })
      }
      emit({ type: 'log', line: 'installation terminée' })
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
