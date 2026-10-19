// Pont avec l'installateur Rust (window.ipc fourni par la vue web).
// Dans un navigateur ordinaire, une simulation joue le même rôle.

export type CheckStatus = 'running' | 'ok' | 'warn' | 'error'
export type TaskStatus = 'running' | 'done' | 'skipped' | 'failed'

export type BackendEvent =
  | { type: 'check'; id: string; status: CheckStatus; title: string; detail: string }
  | {
      type: 'checksDone'
      streamPort: number
      consolePort: number
      busy: { port: number; process: string | null } | null
      lanIp: string | null
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
  maxPerformance: boolean
  startWithWindows: boolean
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
        ['gpu', 'ok', 'Carte graphique NVIDIA', 'NVIDIA GeForce RTX (simulation) · encodeur matériel NVENC disponible'],
        ['driver', 'ok', 'Pilote graphique', 'Version 000.00 (simulation)'],
        ['network', 'ok', 'Réseau local', 'Ethernet · 192.168.1.100'],
        ['ports', 'ok', 'Ports réseau', `Le port habituel 8080 est déjà utilisé par httpd.exe. Tirés au hasard et vérifiés libres : stream ${stream} · console ${consolePort}`],
        ['display', 'ok', 'Écran principal', '3840 × 2160 · 160 Hz'],
        ['power', 'warn', "Mode d'alimentation : Équilibré", "Le GPU ralentit pour économiser l'énergie, ce qui peut tripler le temps d'encodage. Nous proposerons de le régler à l'étape Options."],
      ]
      for (const [id, status, title, detail] of items) {
        emit({ type: 'check', id, status: 'running', title, detail: '' })
        await wait(650)
        emit({ type: 'check', id, status, title, detail })
      }
      emit({ type: 'checksDone', streamPort: stream, consolePort, busy: { port: 8080, process: 'httpd.exe' }, lanIp: '192.168.1.100' })
      break
    }
    case 'validatePorts': {
      await wait(250)
      const bad = (p: number, other: number) =>
        p < 1024 || p > 65535 ? 'Choisissez un port entre 1024 et 65535.' : p === other ? 'Le stream et la console ont besoin de deux ports différents.' : p === 8080 ? 'Le port 8080 est déjà utilisé par httpd.exe.' : null
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
