// Worker du signal de vie : ses minuteries ne sont pas ralenties quand
// l'onglet est masqué, contrairement à celles de la page.

import { sendHeartbeat, type HeartbeatTarget } from './heartbeat-call'

type Message =
  | { type: 'start'; origin: string; body: unknown; period: number; target: HeartbeatTarget }
  | { type: 'body'; body: unknown }
  | { type: 'now' }

const scope = self as unknown as {
  postMessage(message: unknown): void
  onmessage: ((e: MessageEvent<Message>) => void) | null
}

let origin = ''
let target: HeartbeatTarget | undefined
let body: unknown = {}
let timer: ReturnType<typeof setInterval> | null = null
let period = 5000
/** Un signal à la fois (WEB-11) : les réponses arrivent dans l'ordre, un jeton plus ancien n'écrase plus le récent. */
let busy = false

async function beat() {
  if (!origin || busy) return
  busy = true
  try {
    scope.postMessage(await sendHeartbeat(origin, body, target, period))
  } finally {
    busy = false
  }
}

scope.onmessage = (e) => {
  const m = e.data
  switch (m.type) {
    case 'start':
      origin = m.origin
      target = m.target
      body = m.body
      period = m.period
      if (timer) clearInterval(timer)
      timer = setInterval(beat, m.period)
      break
    case 'body':
      body = m.body
      break
    case 'now':
      beat()
      break
  }
}
