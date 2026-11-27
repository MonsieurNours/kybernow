// « Télécharger le journal » du client web (docs/link-ui.md, « Réglages de l'app › À propos ») : le navigateur
// télécharge `kybernow-journal.txt`, l'en-tête (version, navigateur, ordinateur, moment) puis les messages de la page
// depuis son ouverture. Rien ne part ailleurs que dans le fichier que l'on télécharge soi-même.
//
// Les messages sont ceux de la console (ceux de la page et ceux du moteur de Kyber), gardés dès le chargement par
// `keepConsole()` : 2 Mio de texte au plus, les plus anciens oubliés d'abord (l'en-tête du fichier le dit).

/** Le texte gardé, au plus (les plus anciennes lignes partent d'abord). */
export const MAX_KEPT = 2 * 1024 * 1024
const LEVELS = ['debug', 'log', 'info', 'warn', 'error'] as const

interface Kept {
  lines: string[]
  size: number
  dropped: number
}

const kept: Kept = { lines: [], size: 0, dropped: 0 }
let installed = false

/** Une valeur de la console en texte : une erreur avec sa pile, un objet en JSON, le reste tel quel. */
export function show(v: unknown): string {
  if (typeof v === 'string') return v
  if (v instanceof Error) return v.stack ?? `${v.name}: ${v.message}`
  if (v === undefined || typeof v === 'function' || typeof v === 'symbol') return String(v)
  try {
    return JSON.stringify(v) ?? String(v)
  } catch {
    return String(v)
  }
}

/** Garde une ligne ; au-delà de `MAX_KEPT`, oublie les plus anciennes. */
export function keep(line: string, max = MAX_KEPT) {
  kept.lines.push(line)
  kept.size += line.length + 1
  while (kept.size > max && kept.lines.length > 1) {
    kept.size -= kept.lines.shift()!.length + 1
    kept.dropped++
  }
}

/** Au chargement de la page : chaque message de la console est aussi gardé pour le journal (une seule fois). */
export function keepConsole() {
  if (installed) return
  installed = true
  for (const level of LEVELS) {
    const original = console[level].bind(console)
    console[level] = (...args: unknown[]) => {
      keep(`${new Date().toISOString()} ${level.toUpperCase()} ${args.map(show).join(' ')}`)
      original(...args)
    }
  }
  addEventListener('error', (e) => keep(`${new Date().toISOString()} ERROR ${show(e.error ?? e.message)}`))
  addEventListener('unhandledrejection', (e) => keep(`${new Date().toISOString()} ERROR promesse rejetée : ${show(e.reason)}`))
}

/** « 2026-11-27 12:00:00.000 UTC ». */
export function utc(d: Date): string {
  return d.toISOString().replace('T', ' ').replace('Z', ' UTC')
}

/** Ce qui décrit l'appareil : le navigateur et sa version, l'ordinateur (d'après ce que le navigateur dit de lui). */
export interface Device {
  browser: string
  system: string
}

/** Le navigateur et le système d'après `navigator` (les noms connus d'abord, sinon l'agent tel quel). */
export function describe(userAgent: string, platform: string): Device {
  const ua = userAgent
  const pick = (re: RegExp, name: string) => {
    const m = ua.match(re)
    return m ? `${name} ${m[1]}` : null
  }
  const browser =
    pick(/Edg\/([\d.]+)/, 'Edge') ??
    pick(/OPR\/([\d.]+)/, 'Opera') ??
    pick(/Firefox\/([\d.]+)/, 'Firefox') ??
    pick(/Chrome\/([\d.]+)/, 'Chrome') ??
    (/Safari\//.test(ua) ? pick(/Version\/([\d.]+)/, 'Safari') : null) ??
    ua
  const system = /iPhone|iPad/.test(ua)
    ? 'iOS'
    : /Android/.test(ua)
      ? 'Android'
      : /Mac OS X|Macintosh/.test(ua)
        ? 'macOS'
        : /Windows/.test(ua)
          ? 'Windows'
          : /CrOS/.test(ua)
            ? 'ChromeOS'
            : /Linux/.test(ua)
              ? 'Linux'
              : platform || '?'
  return { browser, system }
}

/**
 * Le journal : « KyberNow Link {version} », l'appareil et le moment (les lignes que lit la page du relais, même si le
 * fichier ne passe pas par lui), l'agent complet, puis les messages gardés. `english` : les libellés en anglais.
 */
export function journalText(version: string, device: Device, userAgent: string, now: Date, english: boolean, lines = kept.lines, dropped = kept.dropped): string {
  const head = english
    ? [`KyberNow Link ${version} (web client)`, `Device: ${device.browser} · ${device.system}`, `Log written on ${utc(now)}`, `User agent: ${userAgent}`]
    : [`KyberNow Link ${version} (client web)`, `Appareil : ${device.browser} · ${device.system}`, `Journal écrit le ${utc(now)}`, `Agent : ${userAgent}`]
  if (dropped > 0) head.push(english ? `[… ${dropped} older messages removed]` : `[… ${dropped} messages plus anciens retirés]`)
  return `${head.join('\n')}\n\n${lines.join('\n')}${lines.length ? '\n' : ''}`
}

/** Télécharge `kybernow-journal.txt` (le navigateur l'enregistre là où il range ses téléchargements). */
export function downloadJournal(version: string, english: boolean) {
  const text = journalText(version, describe(navigator.userAgent, navigator.platform), navigator.userAgent, new Date(), english)
  const url = URL.createObjectURL(new Blob([text], { type: 'text/plain;charset=utf-8' }))
  const a = document.createElement('a')
  a.href = url
  a.download = 'kybernow-journal.txt'
  document.body.append(a)
  a.click()
  a.remove()
  // Le temps que le navigateur prenne le fichier.
  setTimeout(() => URL.revokeObjectURL(url), 10_000)
}
