// Diagnostic des entrées (`/link/?diag`) : ce que la souris produit, ce
// qu'on relaie au moteur, et ce que le moteur envoie vraiment à l'ordinateur.

export const diagEnabled = new URLSearchParams(location.search).has('diag')

export const diag = {
  trusted: 0,
  relayed: 0,
  wsInputs: 0,
  wsInputBytes: 0,
  streamWrites: 0,
  lastX: 0,
  lastY: 0,
  outside: 0,
  /** Dernière frappe dans le champ du clavier tactile : `key/code`, et le
   * dernier `inputType` (vérifier qu'un clavier logiciel n'a pas de `code`). */
  lastKey: '—',
  lastInput: '—',
  /** Tailles des messages d'entrées envoyés depuis le dernier relevé. */
  sizes: new Set<number>(),
}

export function installDiag() {
  if (!diagEnabled) return
  const send = WebSocket.prototype.send
  WebSocket.prototype.send = function (this: WebSocket, data: any) {
    if (this.url.includes('/websocket/inputs')) {
      diag.wsInputs++
      const n = data?.byteLength ?? data?.length ?? 0
      diag.wsInputBytes += n
      diag.sizes.add(n)
    }
    return send.call(this, data)
  }
  const write = WritableStreamDefaultWriter.prototype.write
  WritableStreamDefaultWriter.prototype.write = function (this: WritableStreamDefaultWriter, chunk?: any) {
    diag.streamWrites++
    return write.call(this, chunk)
  }
  window.addEventListener(
    'mousemove',
    (e) => {
      if (e.isTrusted) {
        diag.trusted++
        diag.lastX = Math.round(e.clientX)
        diag.lastY = Math.round(e.clientY)
      }
    },
    true,
  )
  window.addEventListener('mousemove', (e) => {
    if (!e.isTrusted) diag.relayed++
  })
  document.documentElement.addEventListener('mouseleave', () => diag.outside++)
}

/**
 * Le panneau du diagnostic : quinze secondes d'historique, relevé chaque seconde (on reproduit, puis on
 * capture après coup). `state` : ce que la session sait d'elle-même au moment du relevé. Rend de quoi
 * l'arrêter ; rien sans `?diag`.
 */
export function watchDiag(show: (text: string) => void, state: () => string[]): () => void {
  if (!diagEnabled) return () => {}
  let prev = { ...diag }
  const hist: { souris: number; relayee: number; entrees: number; flux: number; tailles: string }[] = []
  const timer = setInterval(() => {
    hist.push({
      souris: diag.trusted - prev.trusted,
      relayee: diag.relayed - prev.relayed,
      entrees: diag.wsInputs - prev.wsInputs,
      flux: diag.streamWrites - prev.streamWrites,
      tailles: [...diag.sizes].sort((a, b) => a - b).join('/') || '-',
    })
    diag.sizes.clear()
    if (hist.length > 15) hist.shift()
    const row = (k: 'souris' | 'relayee' | 'entrees' | 'flux') => hist.map((h) => String(h[k]).padStart(3)).join('')
    show(
      [
        `souris  ${row('souris')}`,
        `relayée ${row('relayee')}`,
        `entrées ${row('entrees')}`,
        `flux    ${row('flux')}`,
        `octets  ${hist
          .slice(-8)
          .map((h) => h.tailles)
          .join(' ')} (8 dernières s)`,
        ...state(),
        `clavier tactile : frappe ${diag.lastKey} · saisie ${diag.lastInput}`,
      ].join('\n'),
    )
    prev = { ...diag }
  }, 1000)
  return () => clearInterval(timer)
}
