// Imitation de l'app pour travailler l'interface dans un navigateur
// (`npm run dev`) : jamais incluse dans l'app (seulement en développement).
// Langue : celle du navigateur, `?lang=en` ou `?lang=fr` pour forcer, ou le
// réglage Langue de l'app (comme dans l'app, sans recharger).

import { i18n, langFromTag, t } from './i18n/index.svelte'
import type { BarComputer, Computer, ConnectInfo, ConnexionView, DisplaysInfo, FondState, GuestBar, Info, KeepSound, LastSession, RouteEvent, RouteView, ScreensEvent, Settings, Share, ShareGuest, ShareImageEvent, ShareJoinOutcome, ShareJoined, ShareQuality, ShareRole, ShareRoleEvent, ShareWaitOutcome, StationInfo, StationOutcome, StationRestarting, WakeTarget } from './api'

const listeners = new Map<string, ((e: { payload: unknown }) => void)[]>()
const emit = (name: string, payload: unknown) => listeners.get(name)?.forEach((cb) => cb({ payload }))

// Rendus : `?vide` (aucun ordinateur), `?installateur` (la mise à jour de la
// Station passe par l'installateur), `?sansimage` (l'ordinateur n'envoie pas d'image),
// `?redemarrage` (l'image ne vient pas : la Station redémarre, puis la connexion reprend),
// `?sansreveil` (l'ordinateur hors ligne n'a pas de carte réseau connue : « Réessayer »),
// `?echecmaj` (la mise à jour de la Station n'aboutit pas), `?ecrans=3` (ce PC a trois écrans : la
// ligne de l'accueil, la connexion et la barre au pluriel ; la Station de Bureau-Salon en crée quatre),
// `?reprise` (en session, la connexion reprend : écrans changés), `/fond.html` (le fond de la connexion
// d'un autre écran) avec `?sanscreation` (l'ordinateur n'a pas pu créer l'écran : sa ligne).
// Plusieurs ordinateurs : `?ordinateurs=2` (les deux premiers ordinateurs sont en session : sur
// l'accueil, « Afficher la session » ; à 4, « Se connecter » inactif avec la raison ; dans
// `/bar.html`, la barre sert le premier, le menu « Ordinateurs » coche les autres en session, et
// « Garder le son »), `/connexion.html?id=c3` (la fenêtre de connexion d'une session suivante,
// jusqu'à la première image) avec `?echec` (la connexion échoue : la raison et « Fermer »).
// KyberNow Share, l'hôte (`/bar.html`) : `?partage=2` (le partage est ouvert, avec 2 invités : le panneau,
// la pastille « 2 invités » ; de 0 à 4, à 4 « Quatre invités au plus. »), `?partage` (« Partager » ouvre le
// partage, puis un invité arrive), `?partage=ancienne` (Station trop ancienne) et `?partage=coupe` (partage
// coupé sur l'ordinateur) : « Partager » inactif, avec le texte ; `?copie=non` : « Copier le lien » échoue.
// L'invité : `/bar.html?invite=spectateur|joueur|clavier|joueur-clavier` (la barre réduite : aucun droit, les
// manettes, le clavier et la souris, les deux ; `&avis` : le changement de droits montré à la place des statistiques,
// celui du dernier droit donné, ou `&avis=pads_back|kbm_back|watch` pour un droit repris ; `&droits` : l'hôte
// change les droits toutes les 3 s, la barre suit ; `&qualite=auto|hote|2160p60|1080p60|720p30` : la « Qualité de l'image »
// demandée au départ, que la puce de la barre montre en qualité reçue et qui se change (l'ordinateur de l'hôte est
// en 3840 × 2160 à 60 Hz, ce PC en 1920 × 1080 à 60 Hz) ; `&image=inconnue` : Kyber ne sait pas encore la qualité
// reçue, la puce dit celle qui est demandée ; `&avis=image|limite` : l'avis « Image allégée pour suivre. » ou
// « L'ordinateur ne peut pas faire une image de plus… ») ; l'accueil avec `?rejoindre` (l'écran « Rejoindre un
// partage » ouvert, `?rejoindre=123456` : le code d'un lien déjà rempli) ; le code 123456 est un partage, 000000
// n'en est pas un, 999999 est complet, 111111 est un partage dont le format n'est pas lu ici (on entre quand même,
// avec une image à part), un autre : introuvable ; avec `?fin`, le partage
// rejoint s'arrête de lui-même (« Léa a arrêté le partage. ») ; `/connexion.html?id=guest-9&invite` : la
// connexion d'une session d'invité dans sa fenêtre.
// Hors de chez vous (le relais, docs/relais.md) : `?relais` (le relais joint : l'accueil dit « Hors de chez vous » pour
// Atelier, qui ne répond pas sur le réseau ; sa session passe par le relais, la puce « Par le relais » dans
// `/bar.html`, la ligne « Chemin » de ses statistiques ; le partage de l'hôte dit « Sur le même réseau ou ailleurs… » et
// deux de ses invités sont venus de loin), `?relais=tcp` (le repli TCP), `?relais=injoignable` (le relais ne répond pas :
// « Pas encore joignable hors de chez vous. », rien à l'accueil), `?relais=coupe` (Atelier est hors de chez vous, mais
// l'accès est coupé dans Admin : sa connexion est refusée avec le texte de link-ui), `?relais=eteint` (l'ordinateur a
// quitté le relais pendant la connexion : « … est éteint ou en veille »). Les demandes de l'hôte (`/bar.html`,
// `?partage=1&relais`) : `?demandes=2` (de 0 à 3 demandes qui attendent « Accepter » ou « Refuser », avec la pastille
// « 2 demandes »). L'invité, « Rejoindre » (`?rejoindre`) hors de chez soi : 424242 est un partage par le relais que l'hôte
// accepte au bout de 4 s (« En attente de l'accord de l'hôte… », « Annuler »), 515151 l'hôte refuse (« L'hôte a refusé. »),
// 626262 l'hôte ne répond pas (« L'hôte n'a pas répondu. »), 737373 le relais ne répond pas, 848484 trois demandes
// attendent déjà.
const flags = new URLSearchParams(location.search)
/** Le relais : `joint`, `tcp`, `injoignable`, `coupe`, `eteint` ; `null` sans `?relais`. */
const relayFlag = flags.has('relais') ? flags.get('relais') || 'joint' : null
/** Le relais tient le partage de l'hôte et les ordinateurs absents du réseau (tout sauf `injoignable`). */
const relayJoined = relayFlag !== null && relayFlag !== 'injoignable'
/** Écrans de ce PC imité. */
const deviceScreens = Math.max(1, Number(flags.get('ecrans') ?? 1) || 1)
/** Ordinateurs en session au départ (plusieurs ordinateurs). */
const sessionCount = Math.max(0, Math.min(4, Number(flags.get('ordinateurs') ?? 0) || 0))

const settings = {
  profile: 'bureau',
  codec: null,
  yuv444: null,
  mbps: null,
  reliable: null,
  max_4k: false,
  custom: null,
  displays: 'auto',
  vsync: true,
  mouse: 'auto',
  gamepad: 'xbox360',
  clipboard: true,
  audio: 'auto',
  virtual_display: true,
  battery: 'alleger',
  keep_last_image: true,
  auto_reconnect: true,
}

/** Empreinte d'Admin imitée (32 octets), tirée d'un mot. */
const fp = (seed: string) => {
  let h = 2166136261
  const bytes: string[] = []
  for (let i = 0; i < 32; i++) {
    h = Math.imul(h ^ seed.charCodeAt(i % seed.length) ^ i, 16777619)
    bytes.push((h >>> 24).toString(16).padStart(2, '0'))
  }
  return `sha256:${bytes.join(':')}`
}

const now = Math.floor(Date.now() / 1000)
const station = (version: string, update: Partial<StationInfo['update']> = {}): StationInfo => ({
  version,
  update: { available: false, version: null, installable: false, requires_setup: false, installing: false, ...update },
  restarting: false,
})
/** Une carte réseau imitée pour le réveil (`wake`), dans le /24 de son adresse. */
const card = (mac: string, ipv4: string, kind: WakeTarget['kind'], armed: boolean | null, name: string): WakeTarget => ({
  mac,
  ipv4,
  broadcast: ipv4.replace(/\.\d+$/, '.255'),
  kind,
  armed,
  name,
})
const wakeOf: Record<string, WakeTarget[]> = {
  a1: [card('3C:7C:3F:12:AB:CD', '192.168.1.100', 'ethernet', true, 'Intel(R) Ethernet Connection (7) I219-V')],
  b2: flags.has('sansreveil') ? [] : [card('D8:BB:C1:5A:20:0E', '192.168.1.120', 'ethernet', true, 'Realtek PCIe 2.5GbE Family Controller')],
  c3: [card('A4:B1:C1:00:11:22', '192.168.1.42', 'wifi', false, 'Intel(R) Wi-Fi 6E AX211 160MHz')],
}

// Un ordinateur par état : en ligne avec une dernière session et une Station
// à mettre à jour (compte propre à l'appareil, dit par Admin), hors ligne et
// réveillable, à jour avec une session gardée par la 0.4.x, et une Station
// trop ancienne (sans `station`).
let computers = [
  {
    id: 'a1',
    name: 'Bureau-Salon',
    host: '192.168.1.100',
    port: 9443,
    user: 'PORTABLE-BUREAU-4821',
    // Ajouté par code avant la 0.5.0 : c'est Admin qui le dit (`account.own_device`).
    by_code: false,
    wake: wakeOf.a1,
    fingerprint: fp('bureau-salon'),
    os: 'Windows 11 Pro 24H2',
    gpu: 'NVIDIA GeForce RTX 3080 Ti',
    settings: { ...settings },
    last: {
      median_ms: 8.9,
      p95_ms: 14.2,
      seconds: 2520,
      at: now - 86400 - 3600,
      width: 1920,
      height: 1200,
      refresh: 60,
      // Avec plusieurs écrans : leur nombre, et le débit total.
      ...(deviceScreens > 1 ? { screens: deviceScreens, mbps: 36 * deviceScreens } : { mbps: 36 }),
      codec: 'HEVC',
    } as LastSession | null,
    online: true as boolean | null,
    info: {
      hostname: 'BUREAU-SALON',
      os: 'Windows 11 Pro 24H2',
      gpu: 'NVIDIA GeForce RTX 3080 Ti',
      av1: false,
      admin_version: '0.5.0',
      virtual_display: { available: true, max_screens: 4 },
      station: station('0.5.0', { available: true, version: '0.5.1', installable: !flags.has('installateur'), requires_setup: flags.has('installateur') }),
      wake: wakeOf.a1,
      account: { own_device: true },
    } as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  },
  {
    id: 'b2',
    name: 'Atelier',
    host: '192.168.1.120',
    port: 9443,
    user: 'matthieu',
    by_code: false,
    wake: wakeOf.b2,
    fingerprint: fp('atelier'),
    os: 'Windows 10 Pro 22H2',
    gpu: 'AMD Radeon RX 6700 XT',
    settings: { ...settings, profile: 'jeu' },
    last: null as LastSession | null,
    online: false as boolean | null,
    info: null as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  },
  {
    id: 'c3',
    name: 'Studio-Photo',
    host: '192.168.1.42',
    port: 31544,
    user: 'matthieu',
    by_code: false,
    wake: wakeOf.c3,
    fingerprint: fp('studio-photo'),
    os: 'Windows 11 Famille 24H2',
    gpu: 'NVIDIA GeForce RTX 4070',
    settings: { ...settings, profile: 'max', mbps: 150 },
    last: { median_ms: 12.4, p95_ms: 21.8, seconds: 640, at: now - 5 * 86400, label: 'Jeu · HEVC' } as LastSession | null,
    online: true as boolean | null,
    info: {
      hostname: 'STUDIO-PHOTO',
      os: 'Windows 11 Famille 24H2',
      gpu: 'NVIDIA GeForce RTX 4070',
      av1: true,
      admin_version: '0.5.1',
      virtual_display: { available: true },
      station: station('0.5.1'),
      wake: wakeOf.c3,
    } as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  },
  {
    id: 'd4',
    name: 'Vieux-PC',
    host: 'fe80::1c2b:3aff:fe4d:5e6f',
    port: 9443,
    user: 'admin',
    by_code: false,
    wake: [] as WakeTarget[],
    fingerprint: fp('vieux-pc'),
    os: 'Windows 10 Famille 22H2',
    gpu: 'NVIDIA GeForce GTX 1060',
    settings: { ...settings, profile: 'jeu', virtual_display: false },
    last: null as LastSession | null,
    online: true as boolean | null,
    info: {
      hostname: 'VIEUX-PC',
      os: 'Windows 10 Famille 22H2',
      gpu: 'NVIDIA GeForce GTX 1060',
      av1: false,
      admin_version: '0.4.9',
      virtual_display: { available: false },
      station: null,
      wake: [],
    } as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  },
]
if (flags.has('vide')) computers = []
// `?relais` : Atelier ne répond pas sur le réseau, mais le relais le tient (« Hors de chez vous ») ; les autres
// ordinateurs sont chez soi, et la dernière session de Bureau-Salon est passée par le réseau (ligne « Chemin »).
type Relayed = (typeof computers)[number] & { away?: boolean; relay?: Computer['relay'] }
for (const c of computers as Relayed[]) {
  if (c.id === 'a1' && c.last) c.last = { ...c.last, route: { transport: 'direct' } }
  if (relayFlag === null) continue
  c.relay = { server: 'share.kybernow.app:8443', id: `${c.id}abcdefghijklmnopqrstuvwx`.slice(0, 26), remote_access: relayFlag !== 'coupe' }
  if (c.id === 'b2' && relayJoined) {
    c.online = true
    c.away = true
    c.last = { median_ms: 23.4, p95_ms: 31.9, seconds: 1260, at: now - 7200, width: 1920, height: 1200, refresh: 60, mbps: 36, codec: 'HEVC', route: { transport: relayFlag === 'tcp' ? 'relay_tcp' : 'relay', relay: 'share.kybernow.app:8443', rtt_client: 14.5, rtt_station: 1 } }
  }
}
// Plusieurs ordinateurs : un cinquième, pour voir la limite de quatre.
if (flags.has('ordinateurs') && !flags.has('vide')) {
  computers.push({
    ...computers[2],
    id: 'e5',
    name: 'Salle-de-jeux',
    host: '192.168.1.60',
    fingerprint: fp('salle-de-jeux'),
    settings: { ...settings, profile: 'jeu' },
    last: null,
    info: computers[2].info && { ...computers[2].info, hostname: 'SALLE-DE-JEUX' },
  })
}

/** Les ordinateurs en session, dans l'ordre d'ouverture (`?ordinateurs=N` : les N premiers, en ligne). */
let sessions: string[] = computers.slice(0, sessionCount).map((c) => c.id)
for (const c of computers) if (sessions.includes(c.id)) c.online = true
const emitSessions = () => {
  emit('sessions', [...sessions])
  emit('bar-computers', barComputers())
}
/** « Garder le son » des sessions imitées. */
const keepSound = new Set<string>()

/** Le menu « Ordinateurs » de la barre, comme `bar_computer_list` de l'app (`menu::pick`). */
const barComputers = (): BarComputer[] =>
  computers.map((c) => {
    const busy = !!c.station_pending
    const pick: BarComputer['pick'] = sessions.includes(c.id)
      ? 'show'
      : c.online === false && busy
        ? 'busy'
        : c.online === false && c.waking
          ? 'waking'
          : c.online === false && c.wake.length > 0
            ? 'wake'
            : c.online === false
              ? 'retry'
              : sessions.length >= 4
                ? 'full'
                : 'connect'
    const state: BarComputer['state'] = c.online === true ? ((c as Relayed).away ? 'away' : 'online') : c.online === false ? 'offline' : 'searching'
    const dot: BarComputer['dot'] = c.online === true ? 'ok' : c.online === false ? (c.waking || busy ? 'warn' : 'bad') : 'faint'
    return { id: c.id, name: c.name, state, dot, pick }
  })

/** L'accueil reçoit une copie (comme de l'app, qui envoie du JSON). */
const emitComputers = () => {
  emit('computers', structuredClone(computers))
  emit('bar-computers', barComputers())
}

const thisPc = {
  name: 'PORTABLE-BUREAU',
  gpu: 'Intel(R) Iris(R) Xe Graphics',
  h264: true,
  hevc: true,
  hevc444: true,
  av1: true,
  width: 1920,
  height: 1200,
  refresh: 60,
  audio_channels: 2,
  os: 'Windows 11 Famille 24H2',
  on_battery: false,
}

const discovered = [{ id: 'son', name: 'Studio-Son', host: '192.168.1.77', port: 31544, fingerprint: fp('studio-son') }]

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

/** Par où passe une session imitée : sur le réseau, ou par le relais (`?relais`, `?relais=tcp` : le repli TCP). */
const directRoute: RouteView = { transport: 'direct', label: t('relay.path_direct'), via_relay: false, rtt_client: null, rtt_station: null }
const routeView = (): RouteView => {
  const tcp = relayFlag === 'tcp'
  return { transport: tcp ? 'relay_tcp' : 'relay', label: t(tcp ? 'relay.path_relay_tcp' : 'relay.path_relay'), via_relay: true, rtt_client: 14.5, rtt_station: 1 }
}

/** Un ordinateur imité change (Station qui redémarre, s'installe, s'arrête) : l'accueil le voit. */
const change = (id: unknown, f: (c: (typeof computers)[number]) => void) => {
  const c = computers.find((x) => x.id === id)
  if (c) f(c)
  emitComputers()
}

// Notes d'une release : français, `<!-- en -->`, anglais (comme sur GitHub).
const notes = '- Client Windows : première version.\n- Console : accès par code appliqué dès que Kyber est libre.\n<!-- en -->\n- Windows client: first version.\n- Console: code access applied as soon as Kyber is free.'
/** La partie des notes dans la langue de l'interface (comme `release_notes` du cœur). */
const notesFor = (lang: string) => {
  const lines = notes.split('\n')
  const at = lines.findIndex((l) => l.trim() === '<!-- en -->')
  if (at < 0) return notes
  return (lang === 'en' ? lines.slice(at + 1) : lines.slice(0, at)).join('\n').trim()
}
let language: 'auto' | 'fr' | 'en' = 'auto'
let battery: 'alleger' | 'garder' = 'alleger'
let openAtLogin = true
/** Fin du suivi de la Station, comme `follow_loop` de l'app. */
const outcome = (o: StationOutcome) => {
  change(o.id, (c) => (c.station_pending = null))
  emit('station-outcome', o)
}
const update = () => ({ version: '0.5.1', channel: 'stable', notes: notesFor(i18n.lang), page: '', file: 'KyberNow-Link-Setup.exe', size: 1 })

/** Écrans que la session d'un ordinateur imité prend (le calcul du cœur, comme `displays_info`). */
const screensFor = (id: string) => {
  const c = computers.find((x) => x.id === id)
  if (!c) return 1
  const s = c.settings as typeof settings
  const enabled = s.virtual_display && c.info?.virtual_display.available !== false && !s.custom
  const perso = s.codec != null || s.yuv444 != null || s.mbps != null || s.reliable != null
  const all = enabled && (s.displays === 'all' || (s.displays === 'auto' && !perso && (s.profile === 'bureau' || s.profile === 'max')))
  return all ? Math.max(1, Math.min(deviceScreens, c.info?.virtual_display.max_screens ?? 1, 4)) : 1
}

// MARK: - KyberNow Share

/** Un QR code imité (25 modules : trois repères et un semis), au tracé SVG que l'app envoie. */
const fakeQr = () => {
  const size = 25
  const finder = (x: number, y: number, cx: number, cy: number) => {
    const dx = x - cx
    const dy = y - cy
    if (dx < 0 || dy < 0 || dx > 6 || dy > 6) return null
    return dx === 0 || dy === 0 || dx === 6 || dy === 6 || (dx >= 2 && dx <= 4 && dy >= 2 && dy <= 4)
  }
  const dark = (x: number, y: number) => {
    for (const [cx, cy] of [[0, 0], [size - 7, 0], [0, size - 7]]) {
      const f = finder(x, y, cx, cy)
      if (f !== null) return f
    }
    if ((x <= 7 && y <= 7) || (x >= size - 8 && y <= 7) || (x <= 7 && y >= size - 8)) return false
    return (x * 7 + y * 13 + x * y) % 5 < 2
  }
  let path = ''
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) if (dark(x, y)) path += `M${x} ${y}h1v1h-1z`
  return { size, path }
}

const shareFlag = flags.get('partage')
const knownGuests: ShareGuest[] = [
  // `image` : sa qualité d'image seulement quand ce n'est pas celle de l'hôte (une image à part) ; `away` : venu par
  // le relais (avec `?relais`, Léa et Camille).
  { id: 'g1', name: 'Léa', app: 'ios', role: 'spectator', since: now - 180, image: null, away: relayJoined },
  { id: 'g2', name: 'Noé', app: 'android', role: 'player', since: now - 1500, image: '720p · 30 Hz', away: false },
  { id: 'g3', name: 'Camille', app: 'web', role: 'player_kbm', since: now - 20, image: '1080p · 60 Hz', away: relayJoined },
  { id: 'g4', name: 'Salon', app: 'tvos', role: 'kbm', since: now - 4320, image: null, away: false },
]
/** Les demandes venues par le relais, qui attendent l'hôte (`?demandes=N`, de 0 à 3). */
const knownRequests: ShareGuest[] = [
  { id: 'r1', name: 'Inès', app: 'ios', role: 'spectator', since: now - 25, image: null, away: true },
  { id: 'r2', name: 'Hugo', app: 'web', role: 'spectator', since: now - 90, image: null, away: true },
  { id: 'r3', name: 'Salon de Maman', app: 'tvos', role: 'spectator', since: now - 5, image: null, away: true },
]
let shareRequests: ShareGuest[] = knownRequests.slice(0, Math.max(0, Math.min(3, Number(flags.get('demandes') ?? 0) || 0)))
let shareOpen = shareFlag !== null && /^\d+$/.test(shareFlag)
let sharedGuests: ShareGuest[] = shareOpen ? knownGuests.slice(0, Math.min(4, Number(shareFlag))) : []
let shareCode = '482 913'
const shareAvailability = (): Share['availability'] => (shareFlag === 'ancienne' ? 'unsupported' : shareFlag === 'coupe' ? 'disabled' : 'available')
const shareText = () => (shareFlag === 'ancienne' ? t('mock.share_unsupported') : shareFlag === 'coupe' ? t('mock.share_disabled') : null)
/** Le panneau de l'hôte, comme `ShareView` de l'app. */
const shareView = (): Share => ({
  availability: shareAvailability(),
  text: shareText(),
  open: shareOpen,
  code: shareOpen ? shareCode : '',
  link: shareOpen ? `kybernow://join?host=192.168.1.100&port=9443&fp=sha256:aa&code=482913${relayJoined ? '&relay=share.kybernow.app%3A8443&sid=abcdefghijklmnopqrstuvwxyz' : ''}` : '',
  // Relais joint : le lien du client web est celui du relais.
  web: shareOpen ? (relayJoined ? 'https://share.kybernow.app:8443/j/482913' : 'https://192.168.1.100:9443/link/?join=482913') : '',
  display: shareOpen ? 'Écran 1' : null,
  guests: structuredClone(sharedGuests),
  requests: structuredClone(shareRequests),
  // Relais joint : « Sur le même réseau ou ailleurs… » ; injoignable : « Pas encore joignable hors de chez vous. » ;
  // sans `?relais`, le relais n'est pas réglé.
  relay: relayFlag === null ? null : relayFlag === 'injoignable' ? 'unreachable' : 'joined',
  full: sharedGuests.length >= 4,
  qr: shareOpen ? fakeQr() : null,
})
const emitShare = () => emit('share-state', { id: sessions[0] ?? 'a1', share: shareView() })

/** La barre d'une session d'invité (`/bar.html?invite=…`) : le rôle, l'hôte et l'image reçue. */
const guestFlag = flags.get('invite')
/** La « Qualité de l'image » demandée (`&qualite=`), puis celle que l'invité choisit. */
let guestQuality: ShareQuality = ({ hote: 'host', '2160p60': '2160p60', '1080p60': '1080p60', '720p30': '720p30' } as Record<string, ShareQuality>)[flags.get('qualite') ?? ''] ?? 'auto'
/** Celle de « Rejoindre », retenue pour la fois suivante (`app.json`, `share_quality`). */
let rememberedQuality: ShareQuality = 'auto'
/** Ce que la Station imitée répond : la qualité reçue (`null` avec `&image=inconnue` : Kyber ne sait pas encore). */
const imageOf = (q: ShareQuality): string | null =>
  flags.get('image') === 'inconnue' ? null : { auto: '1080p · 60 Hz', host: '4K · 60 Hz', '2160p60': '4K · 60 Hz', '1080p60': '1080p · 60 Hz', '720p30': '720p · 30 Hz' }[q]
const guestRole: ShareRole = guestFlag === 'joueur' ? 'player' : guestFlag === 'clavier' ? 'kbm' : guestFlag === 'joueur-clavier' ? 'player_kbm' : 'spectator'
let guestSeq = 0
let joinAction = flags.has('rejoindre')
/** Une demande par le relais qui attend l'hôte : son code, son départ, « Annuler » passé. */
const waits = new Map<number, { code: string; at: number; cancelled: boolean }>()
let waitSeq = 0
/** Les codes qui passent par le relais (l'hôte est hors de chez soi) : accepté au bout de 4 s, refusé au bout de 3, sans réponse au bout de 6. */
const relayCodes = ['424242', '515151', '626262']
/** Ce que renvoie `share_join` quand l'invité entre. */
const joinedAs = (): ShareJoined & { kind: 'joined' } => ({ kind: 'joined', id: `guest-${++guestSeq}`, title: t('mock.guest_on'), host: 'Léa', role: 'spectator', fingerprint: fp('atelier') })

const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
  share_defaults: () => ({ name: thisPc.name, quality: rememberedQuality }),
  share_link_code: (a) => {
    const m = String(a.link).match(/code=(\d{6})/)
    if (!m) throw t('mock.not_a_link')
    return m[1]
  },
  // « Rejoindre » : le code part aux ordinateurs du réseau (jusqu'à 6 s) ; le refus est le texte du cœur.
  share_join: async (a) => {
    await sleep(1200)
    const code = String(a.code)
    if (code === '999999') throw t('mock.share_full')
    // 737373 : le relais ne répond pas ; 848484 : trois demandes attendent déjà l'hôte.
    if (code === '737373') throw t('mock.relay_mute')
    if (code === '848484') throw t('mock.pending_full')
    // 111111 : l'hôte envoie un format que ce PC ne lit pas, on entre quand même (image à part).
    const relayed = relayCodes.includes(code)
    if (!relayed && code !== '123456' && code !== '482913' && code !== '111111') throw t('mock.no_share')
    console.info(`share_join : qualité de l'image ${a.quality}`)
    guestQuality = ['auto', 'host', '2160p60', '1080p60', '720p30'].includes(String(a.quality)) ? (a.quality as ShareQuality) : 'auto'
    rememberedQuality = guestQuality
    // Hors de chez soi : la demande attend l'accord de l'hôte (`share_wait`).
    if (relayed) {
      waits.set(++waitSeq, { code, at: Date.now(), cancelled: false })
      return { kind: 'pending', wait: waitSeq } satisfies ShareJoinOutcome
    }
    return joinedAs() satisfies ShareJoinOutcome
  },
  // « En attente de l'accord de l'hôte… » : l'app tient l'appel 25 s, l'imitation 2 s ; l'hôte accepte, refuse ou se tait.
  share_wait: async (a) => {
    const w = waits.get(Number(a.wait))
    if (!w || w.cancelled) return { kind: 'cancelled' } satisfies ShareWaitOutcome
    await sleep(2000)
    if (w.cancelled) return { kind: 'cancelled' } satisfies ShareWaitOutcome
    const waited = Date.now() - w.at
    if (w.code === '424242' && waited >= 4000) {
      waits.delete(Number(a.wait))
      return joinedAs() satisfies ShareWaitOutcome
    }
    if (w.code === '515151' && waited >= 3000) {
      waits.delete(Number(a.wait))
      throw t('mock.share_refused')
    }
    if (w.code === '626262' && waited >= 6000) {
      waits.delete(Number(a.wait))
      throw t('mock.share_expired')
    }
    return { kind: 'waiting' } satisfies ShareWaitOutcome
  },
  share_cancel: (a) => {
    const w = waits.get(Number(a.wait))
    if (w) w.cancelled = true
    waits.delete(Number(a.wait))
    console.info(`share_cancel : la demande n°${a.wait} est retirée chez l'hôte`)
    return null
  },
  // L'hôte répond à une demande venue par le relais : « Accepter » en fait un invité (quatre places au plus), « Refuser » l'écarte.
  share_accept: (a) => {
    const r = shareRequests.find((g) => g.id === a.guest)
    if (!r) throw t('mock.unknown_guest')
    if (sharedGuests.length >= 4) throw t('mock.share_full')
    shareRequests = shareRequests.filter((g) => g.id !== a.guest)
    sharedGuests = [...sharedGuests, { ...r, since: Math.floor(Date.now() / 1000) }]
    emitShare()
    return shareView()
  },
  share_refuse: (a) => {
    shareRequests = shareRequests.filter((g) => g.id !== a.guest)
    emitShare()
    return shareView()
  },
  // La session d'invité : comme `connect` (passage de l'accueil pour la première, fenêtre pour les suivantes).
  share_connect: (a) => {
    const id = a.id as string
    if (sessions.length >= 4) throw t('common.four_at_most')
    if (sessions.length > 0) {
      sessions = [...sessions, id]
      emitSessions()
      return { screens: 1, windowed: true } satisfies ConnectInfo
    }
    sessions = [id]
    emitSessions()
    void (async () => {
      for (const phase of ['preparing', 'connecting', 'first_images']) {
        emit('session-status', { id, phase, detail: null })
        await sleep(1200)
      }
      emit('session-screens', { id, screens: 1 } satisfies ScreensEvent)
      emit('session-status', { id, phase: 'streaming', detail: null })
      await sleep(600)
      emit('session-shown', id)
      // ?fin : l'hôte arrête le partage.
      if (!flags.has('fin')) return
      await sleep(3000)
      sessions = sessions.filter((s) => s !== id)
      emit('session-ended', { id, summary: { error: t('mock.share_stopped'), median_ms: 9, p95_ms: 13, seconds: 5 } })
      emitSessions()
    })()
    return { screens: 1, windowed: false } satisfies ConnectInfo
  },
  // « Qualité de l'image » depuis la barre : retenue, la Station la prend et dit l'image reçue.
  share_quality: async (a) => {
    await sleep(400)
    guestQuality = a.quality as ShareQuality
    rememberedQuality = guestQuality
    emit('share-image', { id: a.id as string, quality: guestQuality, label: imageOf(guestQuality) } satisfies ShareImageEvent)
    return null
  },
  share_open: async () => {
    await sleep(500)
    if (shareAvailability() !== 'available') throw shareText()
    shareOpen = true
    shareCode = '482 913'
    emitShare()
    // Un invité arrive un peu plus tard (la pastille apparaît).
    if (sharedGuests.length === 0) {
      setTimeout(() => {
        sharedGuests = [knownGuests[0]]
        emitShare()
      }, 2500)
    }
    return shareView()
  },
  share_code: () => {
    shareCode = '731 064'
    emitShare()
    return shareView()
  },
  share_guest: (a) => {
    sharedGuests = sharedGuests.map((g) => (g.id === a.guest ? { ...g, role: a.role as ShareRole } : g))
    emitShare()
    return shareView()
  },
  share_kick: (a) => {
    sharedGuests = sharedGuests.filter((g) => g.id !== a.guest)
    emitShare()
    return shareView()
  },
  share_close: () => {
    shareOpen = false
    sharedGuests = []
    shareRequests = []
    emitShare()
    return shareView()
  },
  copy_text: (a) => {
    if (flags.get('copie') === 'non') throw ''
    console.info(`copy_text : ${a.text}`)
    return null
  },
  snapshot: () => ({ computers: structuredClone(computers), discovered: flags.has('vide') ? [] : discovered, this_pc: thisPc, sessions: [...sessions], version: 'dev' }),
  poll_now: async () => {
    await sleep(700)
    emitComputers()
  },
  probe_address: async (a) => {
    await sleep(500)
    return { host: a.host, port: a.port, name: null, fingerprint: 'sha256:9f:3a:c2:71:0d:5e:88:b4' }
  },
  parse_link: (a) => {
    const m = String(a.link).match(/^kybernow:\/\/([^:/?]+):(\d+)/)
    if (!m) throw t('mock.not_a_link')
    return { host: m[1], port: Number(m[2]), name: t('mock.from_link'), fingerprint: 'sha256:aa' }
  },
  pair_start: async () => {
    await sleep(300)
    return '482913'
  },
  pair_poll: () => 'pending',
  pair_cancel: () => null,
  add_computer: async () => {
    await sleep(500)
    throw t('mock.bad_account')
  },
  save_settings: (a) => {
    computers = computers.map((c) => (c.id === a.id ? { ...c, settings: a.settings as typeof settings } : c))
    emitComputers()
  },
  rename_computer: () => null,
  remove_computer: () => null,
  // Rend tout de suite, comme l'app : la session se déroule ensuite (événements). Pendant une
  // session, une session de plus (plusieurs ordinateurs) : en fenêtre, sa connexion dans sa propre
  // fenêtre (imitée seule : `/connexion.html?id=…`), quatre au plus.
  connect: (a) => {
    const id = a.id as string
    if (sessions.includes(id)) return { screens: 0, windowed: true } satisfies ConnectInfo
    if (sessions.length >= 4) throw t('common.four_at_most')
    if (sessions.length > 0) {
      sessions = [...sessions, id]
      emitSessions()
      return { screens: 1, windowed: true } satisfies ConnectInfo
    }
    sessions = [id]
    emitSessions()
    const screens = screensFor(id)
    // Hors de chez vous : l'ordinateur ne répond pas sur le réseau, la session passe par le relais, ou est refusée.
    const away = !!(computers.find((c) => c.id === id) as Relayed | undefined)?.away
    void (async () => {
      if (away && (relayFlag === 'coupe' || relayFlag === 'eteint')) {
        for (const phase of ['preparing', 'connecting']) {
          emit('session-status', { id: a.id, phase, detail: null })
          await sleep(900)
        }
        sessions = sessions.filter((s) => s !== id)
        // Le refus se lit avec le nom de l'ordinateur (link-ui, « Hors de chez vous »).
        emit('session-ended', { id: a.id, summary: { error: t(relayFlag === 'coupe' ? 'mock.access_off' : 'mock.station_offline'), median_ms: null, p95_ms: null, seconds: 2 } })
        emitSessions()
        return
      }
      for (const phase of ['preparing', 'connecting', 'first_images']) {
        emit('session-status', { id: a.id, phase, detail: null })
        // Le cœur dit le chemin au début de la session.
        if (phase === 'connecting') emit('session-route', { id, route: away ? routeView() : directRoute } satisfies RouteEvent)
        await sleep(1200)
      }
      // Dernier recours, comme le cœur : l'étape « Connexion », puis l'avis, puis tout reprend.
      if (flags.has('redemarrage')) {
        emit('session-status', { id: a.id, phase: 'connecting', detail: null })
        emit('session-station-restarting', { id: a.id as string } satisfies StationRestarting)
        await sleep(4000)
        for (const phase of ['preparing', 'connecting', 'first_images']) {
          emit('session-status', { id: a.id, phase, detail: null })
          await sleep(1200)
        }
      }
      emit('session-screens', { id: a.id as string, screens } satisfies ScreensEvent)
      emit('session-status', { id: a.id, phase: 'streaming', detail: null })
      // ?sansimage : l'ordinateur n'envoie jamais d'image.
      if (flags.has('sansimage')) return
      await sleep(600)
      emit('session-shown', a.id)
      // ?reprise : la connexion reprend en session (écrans changés), les étapes reviennent, puis l'image.
      if (flags.has('reprise')) {
        await sleep(2000)
        emit('session-replan', { id: a.id as string, screens } satisfies ScreensEvent)
        for (const phase of ['preparing', 'connecting', 'first_images', 'streaming']) {
          emit('session-status', { id: a.id, phase, detail: null })
          await sleep(1200)
        }
        emit('session-shown', a.id)
      }
      // ?ordinateurs : la session reste ouverte (l'accueil s'essaie pendant elle).
      if (flags.has('ordinateurs')) return
      await sleep(1500)
      sessions = sessions.filter((s) => s !== id)
      emit('session-ended', { id: a.id, summary: { error: null, median_ms: 9.1, p95_ms: 13, seconds: 60 } })
      emitSessions()
    })()
    return { screens, windowed: false } satisfies ConnectInfo
  },
  show_session: (a) => {
    if (!sessions.includes(a.id as string)) throw t('mock.no_session')
    console.info(`show_session : la fenêtre de ${a.id} passe devant`)
    return null
  },
  session_keep_sound: (a) => {
    if (a.on) keepSound.add(a.id as string)
    else keepSound.delete(a.id as string)
    emit('session-keep-sound', { id: a.id as string, on: Boolean(a.on) } satisfies KeepSound)
    return null
  },
  bar_computers: () => barComputers(),
  // La fenêtre de connexion d'une session suivante (`/connexion.html?id=c3`) : ses étapes, puis la
  // première image (le passage s'ouvre), ou ?echec : la raison.
  connexion_state: () => {
    const id = new URLSearchParams(location.search).get('id') ?? 'c3'
    void (async () => {
      await sleep(400)
      for (const phase of ['connecting', 'first_images']) {
        emit('session-status', { id, phase, detail: null })
        await sleep(1400)
      }
      if (flags.has('echec')) {
        emit('session-ended', { id, summary: { error: t('mock.connect_refused'), median_ms: null, p95_ms: null, seconds: 4 } })
        return
      }
      emit('session-status', { id, phase: 'streaming', detail: null })
      await sleep(600)
      emit('session-shown', id)
    })()
    // `&invite` : la connexion d'une session d'invité, titrée « Invité sur {nom} ».
    const guest = flags.has('invite') ? { title: t('mock.guest_on'), guest: true, fingerprint: fp('atelier') } : {}
    return { id, phase: 'preparing', restarting: false, error: null, ...guest } satisfies ConnexionView
  },
  connexion_close: () => {
    console.info('connexion_close : Annuler, ou Fermer sous un échec')
    return null
  },
  // Le cœur décide (`Screens::choice`) : inactif, sur « Cet écran », avec une définition sur mesure ou sans
  // écran virtuel (pas demandé, ou pas utilisable sur la Station) ; Auto prend tous les écrans en Bureau
  // et Max, un seul en Jeu et en Personnalisé.
  displays_info: (a) => {
    const s = a.settings as typeof settings
    const c = computers.find((x) => x.id === a.id)
    const virtual_display = s.virtual_display && c?.info?.virtual_display.available !== false
    const enabled = virtual_display && !s.custom
    const shown: Settings['displays'] = enabled ? (s.displays as Settings['displays']) : 'this'
    const max = c?.info?.virtual_display.max_screens ?? 1
    const perso = s.codec != null || s.yuv444 != null || s.mbps != null || s.reliable != null
    const all = shown === 'all' || (shown === 'auto' && !perso && (s.profile === 'bureau' || s.profile === 'max'))
    return { enabled, shown, count: all ? Math.max(1, Math.min(deviceScreens, max, 4)) : 1, virtual_display } satisfies DisplaysInfo
  },
  fond_state: () => ({ not_created: flags.has('sanscreation') }) satisfies FondState,
  session_command: () => null,
  bar_hide: () => null,
  // `?rejoindre` : « Rejoindre avec un code » demandé à l'ouverture (`=123456` : par un lien, code rempli).
  take_action: () => {
    if (!joinAction) return null
    joinAction = false
    const code = flags.get('rejoindre')
    return { kind: 'join', link: code ? `kybernow://join?host=192.168.1.77&port=31544&code=${code}` : null }
  },
  app_info: () => ({ version: '0.5.0', channel: 'stable', language, lang: i18n.lang, update: update(), arch: 'x64', battery, open_at_login: openAtLogin }),
  set_open_at_login: (a) => (openAtLogin = Boolean(a.on)),
  set_profile: (a) => {
    computers = computers.map((c) => (c.id === a.id ? { ...c, settings: { ...c.settings, profile: a.profile as string, codec: null, yuv444: null, mbps: null, reliable: null } } : c))
    emitComputers()
  },
  set_battery: (a) => {
    battery = a.battery === 'garder' ? 'garder' : 'alleger'
    return battery
  },
  set_language: (a) => {
    language = a.language as typeof language
    const lang = language === 'auto' ? langFromTag(new URLSearchParams(location.search).get('lang') ?? navigator.languages?.[0]) : language
    emit('lang', lang)
    emit('update', { ...update(), notes: notesFor(lang) })
    return lang
  },
  check_update: async () => {
    await sleep(600)
    return null
  },
  set_channel: async () => null,
  bar_menu: () => null,
  bar_width: () => null,
  session_profile: async (a) => {
    // L'app écrit le profil en français, l'interface le traduit (localLabel).
    const names: Record<string, string> = { jeu: 'Jeu', bureau: 'Bureau', max: 'Max' }
    emit('bar-label', { id: a.id, label: `${names[a.profile as string]} · HEVC`, profile: a.profile })
  },
  // La Station : un appareil est « en session » (le premier appel demande de forcer) ;
  // elle redémarre (Admin ne répond plus 5 s), sa mise à jour s'installe (8 s).
  station_restart: async (a) => {
    await sleep(500)
    if (!a.force) throw { kind: 'in_session', message: t('mock.station_in_session'), device: 'Apple TV du salon', seconds: null }
    change(a.id, (c) => ((c.station_pending = 'restart'), c.info?.station && (c.info.station.restarting = true)))
    setTimeout(() => change(a.id, (c) => (c.online = false)), 1500)
    setTimeout(() => change(a.id, (c) => ((c.online = true), c.info?.station && (c.info.station.restarting = false))), 6500)
    setTimeout(() => outcome({ id: a.id as string, outcome: 'restarted', version: null }), 7000)
    return null
  },
  station_update: async (a) => {
    await sleep(1500)
    change(a.id, (c) => ((c.station_pending = 'update'), c.info?.station && (c.info.station.update.installing = true)))
    setTimeout(() => change(a.id, (c) => (c.online = false)), 3000)
    setTimeout(
      () =>
        change(a.id, (c) => {
          c.online = true
          const s = c.info?.station
          if (!s) return
          // ?echecmaj : la Station revient sur l'ancienne version.
          if (flags.has('echecmaj')) {
            s.update.installing = false
            return
          }
          s.version = s.update.version ?? s.version
          s.update = { available: false, version: null, installable: false, requires_setup: false, installing: false }
        }),
      8000,
    )
    setTimeout(() => outcome({ id: a.id as string, outcome: flags.has('echecmaj') ? 'update_failed' : 'updated', version: '0.5.1' }), 8500)
    return '0.5.1'
  },
  // Réveil : le paquet part vers les cartes gardées ; l'ordinateur hors ligne
  // (Atelier) revient en ligne 6 s plus tard, comme après son démarrage.
  wake: async (a) => {
    await sleep(300)
    const c = computers.find((x) => x.id === a.id)
    // Texte du cœur (`wake.no_target`), dans la langue de l'interface.
    if (!c?.wake.length)
      throw i18n.lang === 'en'
        ? "the computer's network card is unknown: the app learns it from KyberNow Station (0.5.0 or later) while the computer is on"
        : "carte réseau de l'ordinateur inconnue : l'app l'apprend de KyberNow Station (0.5.0 ou plus récente) quand l'ordinateur est allumé"
    if (!c.online) {
      change(a.id, (x) => (x.waking = true))
      setTimeout(
        () =>
          change(a.id, (x) => {
            x.waking = false
            x.online = true
            x.info = { hostname: x.name.toUpperCase(), os: x.os, gpu: x.gpu, av1: false, admin_version: '0.5.0', virtual_display: { available: true }, station: station('0.5.0'), wake: x.wake }
          }),
        6000,
      )
    }
    // Diffusion, 255.255.255.255 et adresse directe, ports 9 et 7.
    return c.wake.length * 6
  },
  open_admin: (a) => {
    const c = computers.find((x) => x.id === a.id)
    console.info(`open_admin : https://${c?.host}:${c?.port}/`)
    return null
  },
  install_update: async () => {
    for (let p = 0; p <= 100; p += 10) {
      emit('update-progress', p)
      await sleep(150)
    }
  },
}

export function mockTauri() {
  // `?copie=non` : le presse-papiers de la page refuse aussi, « Copier le lien » échoue.
  if (flags.get('copie') === 'non') {
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: () => Promise.reject(new Error('refusé')) } })
  }
  return {
    core: {
      invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
        const f = commands[cmd]
        if (!f) throw `commande inconnue : ${cmd}`
        return f(args)
      },
    },
    event: {
      listen: async (name: string, cb: (e: { payload: unknown }) => void) => {
        listeners.set(name, [...(listeners.get(name) ?? []), cb])
        // La barre sert la première session (`a1` sans `?ordinateurs`), ou celle d'un invité (`?invite=…`).
        const served = guestFlag !== null ? 'guest-1' : (sessions[0] ?? 'a1')
        // « Garder le son » à partir de deux sessions (`?ordinateurs=2`) ; sans session imitée, celle de la barre.
        if (name === 'bar-shown') {
          const guest: GuestBar | null = guestFlag !== null ? { role: guestRole, host: 'Léa', quality: guestQuality, image: imageOf(guestQuality) } : null
          // `?relais` : la session de la barre passe par le relais (puce « Par le relais »).
          const route = relayJoined ? routeView() : null
          const shown = { id: served, label: 'Jeu · HEVC', profile: 'jeu', multi: deviceScreens > 1, keep_sound: keepSound.has(served), sessions: Math.max(1, sessions.length), guest, share: guest ? null : shareView(), route }
          setTimeout(() => cb({ payload: shown }), 300)
          // `&avis` : le changement de rôle d'un invité, montré à la place des statistiques.
          if (guest && flags.has('avis')) {
            const kind = flags.get('avis') || (guestRole === 'spectator' ? 'watch' : guestRole === 'player' ? 'play' : 'kbm')
            // `image` : « Image allégée pour suivre. » ; `limite` : plus d'image à part possible, la qualité reçue est celle du texte.
            const text = kind === 'image' ? t('mock.notice_image_lag') : kind === 'limite' ? t('mock.notice_image_limit', { quality: '720p · 30 Hz' }) : t(`mock.notice_${kind}`)
            setTimeout(() => cb({ payload: { ...shown, pinned: true, notice: text } }), 2500)
          }
          // `&droits` : l'hôte donne et reprend les droits à tour de rôle (manettes, clavier et souris, un à la fois).
          if (guest && flags.has('droits')) {
            const steps: ShareRole[] = ['player', 'player_kbm', 'kbm', 'spectator', 'kbm', 'player_kbm']
            steps.forEach((role, i) => setTimeout(() => emit('share-role', { id: served, role } satisfies ShareRoleEvent), 3000 * (i + 1)))
          }
        }
        if (name === 'bar-computers') setTimeout(() => cb({ payload: barComputers() }), 300)
        if (name === 'stats') {
          const timer = setInterval(() => cb({ payload: { id: served, latency_ms: 8 + Math.random() * 3, fps: 60, width: 1920, height: 1200, decoder: `HEVC · ${t('mock.hardware')}`, relative: false, rtt_ms: 2.1, audio_ms: 18 + Math.random() * 4, audio_gaps: 0, screens: deviceScreens } }), 1000)
          return () => clearInterval(timer)
        }
        return () => {}
      },
    },
    window: { getCurrentWindow: () => ({ minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {} }) },
  }
}
