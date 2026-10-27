// Un signal de vie vers la console : utilisé par le Worker et, à défaut, par la page.
// Celui de l'hôte (`/api/stream/heartbeat`, avec la session d'Admin) ou celui
// d'un invité du partage (`/api/share/beat`, avec le secret de l'invité).

/** Où part le signal : la route et les en-têtes en plus (le secret d'un invité). */
export interface HeartbeatTarget {
  path: string
  headers: Record<string, string>
}

/** Signal de vie de l'hôte, celui de la session d'Admin. */
export const HOST_TARGET: HeartbeatTarget = { path: '/api/stream/heartbeat', headers: {} }

/** Signal de vie d'un invité : son secret dans `x-kybernow-guest` (`body` : `{ guest }`). */
export function guestTarget(secret: string): HeartbeatTarget {
  return { path: '/api/share/beat', headers: { 'x-kybernow-guest': secret } }
}

export interface HeartbeatResult {
  /** Code HTTP, 0 si la console n'a pas répondu. */
  status: number
  /** La console demande la fin de la session (l'hôte), ou l'invité est sorti (`data.end` dit pourquoi). */
  end: boolean
  /** Le corps de la réponse, `null` s'il est vide ou illisible. */
  data: any
}

/** `origin` : celle de la page. Un Worker intégré (URL `blob:`) ne sait pas
 * résoudre une adresse relative : il lui faut l'adresse complète. */
export async function sendHeartbeat(origin: string, body: unknown, target: HeartbeatTarget = HOST_TARGET): Promise<HeartbeatResult> {
  try {
    const res = await fetch(`${origin}${target.path}`, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'x-kyber-admin': '1', 'content-type': 'application/json', ...target.headers },
      body: JSON.stringify(body ?? {}),
    })
    let data: any = null
    try {
      data = await res.json()
    } catch {
      // Réponse vide ou illisible.
    }
    return { status: res.status, end: !!data?.end, data }
  } catch {
    return { status: 0, end: false, data: null }
  }
}
