// Worker du signal de vie : ses minuteries ne sont pas ralenties quand
// l'onglet est masqué, contrairement à celles de la page.

import { sendHeartbeat } from './heartbeat-call'

type Message = { type: 'start'; origin: string; body: unknown; period: number } | { type: 'body'; body: unknown } | { type: 'now' } | { type: 'stop' }

const scope = self as unknown as {
  postMessage(message: unknown): void
  onmessage: ((e: MessageEvent<Message>) => void) | null
}

let origin = ''
let body: unknown = {}
let timer: ReturnType<typeof setInterval> | null = null

async function beat() {
  if (origin) scope.postMessage(await sendHeartbeat(origin, body))
}

scope.onmessage = (e) => {
  const m = e.data
  switch (m.type) {
    case 'start':
      origin = m.origin
      body = m.body
      if (timer) clearInterval(timer)
      timer = setInterval(beat, m.period)
      break
    case 'body':
      body = m.body
      break
    case 'now':
      beat()
      break
    case 'stop':
      if (timer) clearInterval(timer)
      timer = null
      break
  }
}
