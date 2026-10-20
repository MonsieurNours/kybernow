// Un signal de vie vers la console : utilisé par le Worker et, à défaut, par la page.

export interface HeartbeatResult {
  /** Code HTTP, 0 si la console n'a pas répondu. */
  status: number
  /** La console demande la fin de la session. */
  end: boolean
}

/** `origin` : celle de la page. Un Worker intégré (URL `blob:`) ne sait pas
 * résoudre une adresse relative : il lui faut l'adresse complète. */
export async function sendHeartbeat(origin: string, body: unknown): Promise<HeartbeatResult> {
  try {
    const res = await fetch(`${origin}/api/stream/heartbeat`, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'x-kyber-admin': '1', 'content-type': 'application/json' },
      body: JSON.stringify(body ?? {}),
    })
    let data: any = null
    try {
      data = await res.json()
    } catch {
      // Réponse vide ou illisible.
    }
    return { status: res.status, end: !!data?.end }
  } catch {
    return { status: 0, end: false }
  }
}
