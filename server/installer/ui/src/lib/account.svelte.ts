// Étape « Compte KyberNow » des installateurs de la Station (Windows et Mac),
// pendant l'installation (docs/comptes.md, « Dans les installateurs ») :
// relais (share.kybernow.app, ou « Autre relais… » vérifié en direct), puis,
// dès qu'Admin a démarré, la page du relais dans la fenêtre (vue posée par
// l'installateur sur le cadre de l'écran), jusqu'au rattachement. Facultative.

import { t } from './i18n/index.svelte'

export const DEFAULT_RELAY = 'share.kybernow.app'

export interface RelayProbe {
  input: string
  /** Forme normale de l'adresse (vide : illisible). */
  address: string
  ok: boolean
  version: string | null
  accounts: boolean
  error: string | null
}

export interface Attached {
  relay: string
  owner: string
  email: string | null
  organization: boolean
  accept: boolean
  remoteAccess: boolean
}

export type AccountEvent =
  | ({ type: 'relayProbed' } & RelayProbe)
  | { type: 'accountWaiting' }
  | { type: 'accountPage'; url: string; relay: string; external: boolean }
  | ({ type: 'accountAttached' } & Attached)
  | { type: 'accountRelaySet'; relay: string }
  | { type: 'accountFailed'; code: string; message: string }

export type AccountCommand =
  | { cmd: 'relayProbe'; address: string }
  | { cmd: 'accountStart'; relay: string }
  | { cmd: 'accountLater'; relay: string }
  | { cmd: 'accountCancel' }
  | { cmd: 'accountView'; url: string; x: number; y: number; width: number; height: number; signup: boolean }
  | { cmd: 'accountViewHide' }

/**
 * choose : « Se connecter », « Créer un compte », « Autre relais… », « Plus tard » ;
 * relay : « Autre relais » ; waiting : Admin pas encore démarré ; page : la page du relais ;
 * attached : rattachée ; later : « Plus tard » ; failed : refus (réessayer ou plus tard).
 */
export type AccountPhase = 'choose' | 'relay' | 'waiting' | 'page' | 'attached' | 'later' | 'failed'

export const account = $state({
  /** L'étape a lieu (pas de réinstallation d'une Station déjà rattachée). */
  wanted: true,
  phase: 'choose' as AccountPhase,
  /** Le relais choisi (forme normale). */
  relay: DEFAULT_RELAY,
  /** Les comptes du relais choisi (« Autre relais » sans comptes : pas de rattachement). */
  relayAccounts: true,
  relayInput: '',
  probe: null as RelayProbe | null,
  probing: false,
  intent: 'signin' as 'signin' | 'signup',
  page: null as { url: string; relay: string; external: boolean } | null,
  attached: null as Attached | null,
  failure: null as { code: string; message: string } | null,
})

type Send = (command: AccountCommand) => void
let sender: Send = () => {}

/** Le canal vers l'installateur (Windows : backend.ts ; Mac : bridge.ts). */
export function bindAccount(send: Send) {
  sender = send
}

/** L'étape est finie : rattachée, ou remise à plus tard. */
export function accountResolved(): boolean {
  return !account.wanted || account.phase === 'attached' || account.phase === 'later'
}

/** Un événement de l'étape ; faux si ce n'en est pas un. */
export function handleAccount(event: { type: string }): boolean {
  const e = event as AccountEvent
  switch (e.type) {
    case 'relayProbed':
      // Seule la réponse à la dernière saisie compte.
      if (e.input.trim() === account.relayInput.trim()) {
        const { type: _, ...probe } = e
        account.probe = probe
        account.probing = false
      }
      return true
    case 'accountWaiting':
      if (account.phase !== 'later') account.phase = 'waiting'
      return true
    case 'accountPage':
      account.page = { url: e.url, relay: e.relay, external: e.external }
      account.phase = 'page'
      if (e.external) sender({ cmd: 'accountViewHide' })
      return true
    case 'accountAttached': {
      const { type: _, ...attached } = e
      account.attached = attached
      account.phase = 'attached'
      account.page = null
      sender({ cmd: 'accountViewHide' })
      return true
    }
    case 'accountRelaySet':
      return true
    case 'accountFailed':
      account.failure = { code: e.code, message: e.message }
      account.phase = 'failed'
      account.page = null
      sender({ cmd: 'accountViewHide' })
      return true
    default:
      return false
  }
}

let probeTimer: ReturnType<typeof setTimeout> | undefined

/** « Autre relais » : vérifié en direct, un instant après la dernière frappe. */
export function typeRelay(input: string) {
  account.relayInput = input
  account.probe = null
  clearTimeout(probeTimer)
  if (!input.trim()) {
    account.probing = false
    return
  }
  account.probing = true
  probeTimer = setTimeout(() => sender({ cmd: 'relayProbe', address: input.trim() }), 450)
}

export function openRelay() {
  account.phase = 'relay'
  if (account.relay !== DEFAULT_RELAY && !account.relayInput) typeRelay(account.relay)
}

/** « Utiliser ce relais » : l'adresse vérifiée devient celle de la Station. */
export function useRelay() {
  const p = account.probe
  if (!p?.ok) return
  account.relay = p.address
  account.relayAccounts = p.accounts
  account.phase = 'choose'
}

export function defaultRelay() {
  account.relay = DEFAULT_RELAY
  account.relayAccounts = true
  account.relayInput = ''
  account.probe = null
  account.phase = 'choose'
}

/** « Se connecter » / « Créer un compte » : la page du relais dès qu'Admin a démarré. */
export function startAccount(intent: 'signin' | 'signup') {
  account.intent = intent
  account.failure = null
  account.phase = 'waiting'
  sender({ cmd: 'accountStart', relay: account.relay })
}

/** « Plus tard » : le relais choisi est quand même réglé ; on rattache ensuite dans Admin. */
export function later() {
  account.phase = 'later'
  account.page = null
  sender({ cmd: 'accountViewHide' })
  sender({ cmd: 'accountLater', relay: account.relay })
}

/** « Retour » depuis la page du relais (ou l'attente) : de nouveau le choix. */
export function backToChoice() {
  account.page = null
  account.phase = 'choose'
  sender({ cmd: 'accountViewHide' })
  sender({ cmd: 'accountCancel' })
}

/** Le cadre de la page du relais : l'installateur y pose sa vue. */
export function showView(frame: DOMRect) {
  const p = account.page
  if (!p || p.external) return
  sender({ cmd: 'accountView', url: p.url, x: frame.left, y: frame.top, width: frame.width, height: frame.height, signup: account.intent === 'signup' })
}

export function hideView() {
  sender({ cmd: 'accountViewHide' })
}

/** Texte d'un refus, d'après son code (docs/comptes.md, routes d'Admin). */
export function failureText(): string {
  const f = account.failure
  if (!f) return ''
  const known = ['no_accounts', 'relay_unreachable', 'already_attached', 'expired', 'admin_unreachable', 'bad_relay', 'no_relay']
  return known.includes(f.code) ? t(`accountStep.failed.${f.code}`) : f.message || t('accountStep.failed.relay_unreachable')
}

/** La phrase de l'écran final, Station rattachée. */
export function doneLead(a: Attached): string {
  return a.organization ? t('accountDone.leadOrg', { who: a.owner }) : t('accountDone.lead', { who: a.email ?? a.owner })
}

// --- Simulation (navigateur) ------------------------------------------------

/** Simulation de l'étape dans un navigateur : `emit` rend l'événement à l'interface. */
export function simulateAccount(command: AccountCommand, emit: (event: AccountEvent) => void, adminReady: () => Promise<void>) {
  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))
  switch (command.cmd) {
    case 'relayProbe':
      void wait(600).then(() => {
        const a = command.address.toLowerCase()
        const address = a.replace(/^https:\/\//, '').replace(/\/+$/, '')
        if (/\s/.test(a) || !a.includes('.')) emit({ type: 'relayProbed', input: command.address, address: '', ok: false, version: null, accounts: false, error: 'bad_address' })
        else if (a.includes('injoignable')) emit({ type: 'relayProbed', input: command.address, address, ok: false, version: null, accounts: false, error: 'connexion refusée' })
        else emit({ type: 'relayProbed', input: command.address, address, ok: true, version: '0.9.10', accounts: !a.includes('sans-comptes'), error: null })
      })
      break
    case 'accountStart':
      emit({ type: 'accountWaiting' })
      void adminReady().then(async () => {
        await wait(500)
        if (account.phase !== 'waiting') return
        emit({ type: 'accountPage', url: `https://${command.relay}/compte/rattacher?code=simulation`, relay: command.relay, external: command.relay.includes('#') })
      })
      break
    default:
      break
  }
}
