// Imitation de l'app pour travailler l'interface dans un navigateur
// (`npm run dev`) : jamais incluse dans l'app (chargée par `api.ts` seulement en
// développement, avec ses textes, `i18n/mock.*.json`).
// Langue : celle du navigateur, `?lang=en` ou `?lang=fr` pour forcer, ou le
// réglage Langue de l'app (comme dans l'app, sans recharger).

import { addSection, i18n, langFromTag, t } from './i18n/index.svelte'
import mockEn from './i18n/mock.en.json'
import mockFr from './i18n/mock.fr.json'
import type { PsAccount, PsBar, PsConsole, PsFound, PsHandoffEvent, PsKept, PsLayout, PsSettings, PsSettingsView, PsState, PsWaiting } from './api'
import type { InviteAdded, InviteRefusal } from './api'
import type { AutoChoice, AutoResult, AutoStep, BarMic, SessionPlan, StationOffer, Values } from './api'
import type { AccountComputer, AccountSignIn, AccountView } from './api'
import type { BarComputer, BorrowBar, BorrowOutcome, Friend, HomeLend, LendEnd, LendPanel, LendRequest, LendWaitOutcome, Computer, ConnectInfo, ConnexionView, DisplaysInfo, FondState, GuestBar, Info, KeepSound, LastSession, RouteEvent, RouteView, ScreensEvent, Settings, Share, ShareGuest, ShareImageEvent, ShareJoinOutcome, ShareJoined, ShareQuality, ShareRole, ShareRoleEvent, ShareWaitOutcome, StationInfo, StationOutcome, StationRestarting, WakeTarget } from './api'

// Les textes de l'imitation, à part des catalogues de l'app.
addSection('mock', { fr: mockFr, en: mockEn })

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
// « Pas encore joignable hors de chez vous. », rien à l'accueil), `?relais=preparation` (le lien du relais se prépare
// 4 s à l'ouverture du partage : « Préparation du lien hors de chez vous… », copie et QR code retenus, puis le relais
// joint), `?relais=lent` (il reste en préparation : au bout de 10 s, la barre dit « Pas encore joignable… » et copie le
// lien du réseau), `?relais=coupe` (Atelier est hors de chez vous, mais
// l'accès est coupé dans Admin : sa connexion est refusée avec le texte de link-ui), `?relais=eteint` (l'ordinateur a
// quitté le relais pendant la connexion : « … est éteint ou en veille »), `?relais=session` (une session de Bureau-Salon,
// qui répond sur le réseau, passe par le relais : ses réglages disent « Hors de chez vous, l'app ne peut ni redémarrer ni
// mettre à jour la Station. » à la place des deux lignes). Les demandes de l'hôte (`/bar.html`,
// `?partage=1&relais`) : `?demandes=2` (de 0 à 3 demandes qui attendent « Accepter » ou « Refuser », avec la pastille
// « 2 demandes »). L'invité, « Rejoindre » (`?rejoindre`) hors de chez soi : 424242 est un partage par le relais que l'hôte
// accepte au bout de 4 s (« En attente de l'accord de l'hôte… », « Annuler »), 515151 l'hôte refuse (« L'hôte a refusé. »),
// 626262 l'hôte ne répond pas (« L'hôte n'a pas répondu. »), 353535 le partage s'arrête pendant l'attente (« Ce partage est arrêté. »),
// 737373 le relais ne répond pas, 848484 trois demandes
// attendent déjà.
// Prêt de l'ordinateur (docs/pret.md). L'ami, sur l'accueil : `?pret` (deux ordinateurs empruntés : celui de Matthieu,
// disponible, avec un créneau mercredi ; celui d'Inès, occupé : « Demander à Inès », accepté au bout de 4 s), `?pret=mdp`
// (le mot de passe de Matthieu n'est pas retenu : il est demandé à « Emprunter » ; « faux » est refusé), `?pret=fin` (la
// session d'emprunt finit au bout de 5 s : « Matthieu a repris l'ordinateur. »), `?pret=retour` (elle finit sans que le prêt
// le dise : « Vous pouvez revenir pendant 15 min. », « Revenir »), `?pret=rendu` (l'ami a rendu l'ordinateur : l'avis « Vous
// avez rendu l'ordinateur. » sur l'accueil, sans carte), `?pret=arrete` (le prêt s'arrête autrement : « Le prêt s'est
// arrêté. ») ; avec `&ordinateurs=1`, « Emprunter » pendant une autre session (la feuille « Préparation de l'ordinateur »
// pendant l'emprunt, puis la fenêtre de connexion) ; `?invitation` (le lien d'invitation d'un compte d'ami ouvert, avec le
// prénom de l'hôte : « Matthieu vous prête son ordinateur. » ; `?invitation=ancien` : un lien d'avant, sans prénom, titré
// « Avec un compte d'ami » ; `?invitation=illisible` : « Ce lien d'invitation est illisible. » ; « faux » est refusé) ;
// « Rejoindre » : 777777 est un code de prêt (le
// passage « Préparation de l'ordinateur »), 787878 un code de prêt que l'hôte accepte au bout de 4 s (« En attente de
// Matthieu… »), 797979 un code de prêt refusé (« Matthieu a refusé. »). L'hôte, sur l'accueil : `?pret=prete` (Bureau-Salon
// est prêté à Léa jusqu'à dans 1 h 20 : « Reprendre l'ordinateur »), `?pret=reprise` (le préavis court : « Reprise dans
// 1:42 », « Reprendre tout de suite »). La barre de l'hôte (`/bar.html`) : `?pret` (« Prêter l'ordinateur… » : durée,
// compte Steam, « Créer un code »), `?pret=code` (le code est créé ; avec `&relais`, `&relais=preparation`, `&relais=lent`
// ou `&relais=injoignable`, son relais comme celui du partage), `?pret=demandes` (deux demandes, la pastille
// « 2 demandes ») avec `&arrivee=pret` (l'avis « Inès veut emprunter l'ordinateur. »), `?pret=creneau` (une demande
// pendant le créneau de Léa : le texte du créneau, « Accepter » seul), `?pret=ancienne` (Station trop ancienne),
// `?pret=impossible` (le prêt n'est pas possible sur cet ordinateur), `?pret=coupe` (prêt coupé dans Admin). La barre de
// l'ami : `/bar.html?emprunt` (« Prêté par Matthieu », « Encore 1 h 20 », « Se déconnecter », « Rendre l'ordinateur »),
// `&sansfin` (« Jusqu'à ce que Matthieu reprenne »), `&preavis` (le préavis de 2 minutes).
// Invitations (docs/invitations.md), « Rejoindre » (`?rejoindre`) : 246810 est un code d'invitation du « PC du salon »
// (la feuille « Ajouter PC du salon » et son mot de passe), 135790 celui d'une Station qui ne dit pas son nom (« Ajouter
// l'ordinateur ») ; un mot de passe quelconque ajoute l'ordinateur (sélectionné sur l'accueil, « PC du salon est
// ajouté. » ; une seconde fois, le même ordinateur prend le nouvel accès), « faux » est refusé (« Mot de passe
// refusé. »), « vieux » : « Ce code ne sert plus. », « plein » : l'invitation a ajouté tous ses appareils, « essais » :
// trop d'essais, « coupe » : l'accès hors de chez vous est coupé.
// Mise à jour des Stations (docs/link-ui.md) : Bureau-Salon a une Station à mettre à jour (le bandeau de l'accueil,
// « · Mise à jour » dans la rangée ; `?installateur` : le bandeau de l'installateur) ; `?maj=loin` : Bureau-Salon est
// hors de chez vous (le bandeau sans « Mettre à jour ») ; `?maj=session` : un appareil est en session sur Bureau-Salon
// (la confirmation dans le bandeau ou l'avis, « Mettre à jour quand même ») ; `?maj=deux` : Studio-Photo aussi a une
// mise à jour ; `?echecmaj` : la mise à jour n'aboutit pas (le texte, « Réessayer », « Plus tard »). L'avis après une
// mise à jour de l'app : `?avis` (Bureau-Salon seul : « Bureau-Salon a aussi une mise à jour »), `?avis&maj=deux`
// (la liste, « Tout mettre à jour »). Plusieurs à la fois : `?maj=deux,session`.
// Enregistrement de la session (docs/link-ui.md) : `/bar.html?enregistrement` (« Enregistrer » ; le clic lance,
// la pastille arrête), `?enregistrement=encours` (déjà 2 min 10 s), `=heure` (59 min 55 s : « 1:00:00 » arrive),
// `=coupe` (coupé dans Admin : estompé, la raison), `=codec` (refusé : « PyroWave et AV1… ») ; l'invité :
// `/bar.html?invite=spectateur&enregistrement=encours` (la pastille seule).
const flags = new URLSearchParams(location.search)
/** `?maj=loin,session,deux`. */
const majFlags = (flags.get('maj') ?? '').split(',')
/** Le relais : `joint`, `tcp`, `injoignable`, `preparation`, `lent`, `coupe`, `eteint`, `session` ; `null` sans `?relais`. */
const relayFlag = flags.has('relais') ? flags.get('relais') || 'joint' : null
// `?pyrowave` : la Station du premier ordinateur propose PyroWave et ce PC le décode ; `=cable` (par défaut) ou
// `=wifi` : ce PC rejoint l'ordinateur par câble ou non ; `=pc` : ce PC ne le décode pas. En session, l'image
// est en PyroWave ; sans câble (ou `&relais`), la puce ambre et le bandeau du début de session ; `=sature` : par
// câble, le bandeau « Le réseau sature » (avis `PyroWaveStrained` du cœur).
const pyroFlag = flags.has('pyrowave') ? flags.get('pyrowave') || 'cable' : null
/** Le relais tient le partage de l'hôte et les ordinateurs absents du réseau (tout sauf `injoignable`). */
const relayJoined = relayFlag !== null && relayFlag !== 'injoignable' && relayFlag !== 'preparation' && relayFlag !== 'lent'
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
    // `?mac` : une Station Mac (réglage « Ctrl agit comme ⌘ », menu Touches du Mac).
    os: flags.has('mac') ? 'macOS 26.1' : 'Windows 11 Pro 24H2',
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
      os: flags.has('mac') ? 'macOS 26.1' : 'Windows 11 Pro 24H2',
      gpu: 'NVIDIA GeForce RTX 3080 Ti',
      av1: false,
      admin_version: '0.5.0',
      virtual_display: { available: true, max_screens: 4 },
      station: station('0.5.0', { available: true, version: '0.5.1', installable: !flags.has('installateur'), requires_setup: flags.has('installateur') }),
      wake: wakeOf.a1,
      account: { own_device: true },
      ...(pyroFlag ? { pyrowave: true } : {}),
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
      station: majFlags.includes('deux') ? station('0.5.1', { available: true, version: '0.5.2', installable: true }) : station('0.5.1'),
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
// `?auto` : Bureau-Salon a un test gardé, profil Auto.
if (flags.has('auto') && !['echec', 'ancienne'].includes(flags.get('auto') ?? '')) {
  computers = computers.map((c) => (c.id === 'a1' ? ({ ...c, settings: { ...c.settings, profile: 'auto' }, auto: autoResult() } as unknown as typeof c) : c))
}
// `?relais` : Atelier ne répond pas sur le réseau, mais le relais le tient (« Hors de chez vous ») ; les autres
// ordinateurs sont chez soi, et la dernière session de Bureau-Salon est passée par le réseau (ligne « Chemin »).
type Relayed = (typeof computers)[number] & { away?: boolean; via_relay?: boolean; relay?: Computer['relay'] }
for (const c of computers as Relayed[]) {
  if (c.id === 'a1' && c.last) c.last = { ...c.last, route: { transport: 'direct' } }
  if (relayFlag === null) continue
  c.relay = { server: 'share.kybernow.app', id: `${c.id}abcdefghijklmnopqrstuvwx`.slice(0, 26), remote_access: relayFlag !== 'coupe' }
  // Une session de cet ordinateur, joignable sur le réseau, passe par le relais : ni redémarrage ni mise à jour de la Station.
  if (c.id === 'a1' && relayFlag === 'session') c.via_relay = true
  if (c.id === 'b2' && relayJoined) {
    c.online = true
    c.away = true
    c.via_relay = true
    c.last = { median_ms: 23.4, p95_ms: 31.9, seconds: 1260, at: now - 7200, width: 1920, height: 1200, refresh: 60, mbps: 36, codec: 'HEVC', route: { transport: relayFlag === 'tcp' ? 'relay_tcp' : 'relay', relay: 'share.kybernow.app', rtt_client: 14.5, rtt_station: 1 } }
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

/** « Plus tard » de la mise à jour de la Station, par ordinateur : la version et quand. */
const snoozed = new Map<string, { version: string; at: number }>()
/** Le bandeau et la puce de la mise à jour de la Station (`station_offer` du cœur, imité). */
const offerOf = (c: (typeof computers)[number]): StationOffer => {
  const s = c.info?.station
  const account = (c as { account?: AccountComputer | null }).account
  const away = !!(c as Relayed).away || (c.id === 'a1' && majFlags.includes('loin'))
  const offered = !!s && (!account || account.rights.includes('admin')) && c.online === true && (s.update.available || s.update.installing) && !s.restarting
  const chip = offered && s!.update.available && !!s!.update.version
  if (!offered || sessions.length > 0) return { banner: { kind: 'none' }, chip }
  const u = s!.update
  if (u.installing) return { banner: { kind: 'installing' }, chip }
  const version = u.version ?? ''
  const later = snoozed.get(c.id)
  if (!version || (later && later.version === version && Date.now() / 1000 < later.at + 7 * 86400)) return { banner: { kind: 'none' }, chip }
  if (u.requires_setup) return { banner: { kind: 'setup', version }, chip }
  if (!u.installable) return { banner: { kind: 'none' }, chip }
  return { banner: { kind: away ? 'away' : 'update', version }, chip }
}
/** Les ordinateurs tels que l'app les envoie (copie JSON, avec leur `offer`). */
/** La puce « Micro » telle que le cœur la décrit (`MicUi::ui_json`), imitée : textes de l'imitation. */
const barMic = (on: boolean, state: BarMic['state']): BarMic => {
  const lit = on && state !== 'refused'
  const fr = i18n.lang === 'fr'
  const label = lit ? 'Micro' : fr ? 'Micro coupé' : 'Microphone off'
  const shortcut = fr ? 'Ctrl+Alt+Maj+V' : 'Ctrl+Alt+Shift+V'
  return { on, state, lit, sent: on && state === 'on', reason: null, label, tooltip: `${label} (${shortcut})`, tile: lit ? (state === 'on' ? 'Envoyé' : 'Envoi…') : 'Coupé' }
}

/** L'empreinte telle que l'app la met en forme (`console::short_fingerprint` du cœur : « 7C41 E09B AABB CCDD »). */
const shortFp = (f: string | null | undefined) => (f ?? '').replace(/^sha256:/i, '').replace(/[^0-9a-f]/gi, '').slice(0, 16).toUpperCase().match(/.{1,4}/g)?.join(' ') ?? ''
const computerViews = () =>
  structuredClone(computers).map((c) => ({
    ...c,
    fingerprint_short: shortFp(c.fingerprint),
    relay_label: (c as { account?: { relay: string } | null }).account?.relay.split('#')[0] ?? '',
    offer: offerOf(c),
  }))
/** Un appareil est en session sur Bureau-Salon (`?maj=session`) : refusé tant qu'on ne force pas. */
const stationInSession = (a: Record<string, unknown>) => majFlags.includes('session') && a.id === 'a1' && !a.force
let noticeTaken = false

/** L'accueil reçoit une copie (comme de l'app, qui envoie du JSON). */
const emitComputers = () => {
  emit('computers', computerViews())
  emit('bar-computers', barComputers())
}

/** « Passer en HEVC » choisi (bandeau de PyroWave) : les mesures redisent HEVC. */
let pyroHevc = false

// Profil Auto (docs/profil-auto.md) : `?auto` (Bureau-Salon a un test gardé, profil Auto), `?auto=reseau` (en plus,
// `/bar.html` montre le bandeau « Autre réseau que celui du test »), `?auto=echec` (le test ne joint pas l'ordinateur :
// « Réessayer »), `?auto=ancienne` (Station d'avant : l'étape du débit est passée). Sans `?auto`, choisir Auto ouvre le
// panneau du test, qui dure 4 s ici (une dizaine dans l'app).
const autoFlag = flags.has('auto') ? flags.get('auto') || 'garde' : null
const fr = () => i18n.lang === 'fr'
function autoResult(): AutoResult {
  return {
    at: Math.floor(Date.now() / 1000) - 86400 * 2,
    network: 'Ethernet',
    measures: { link: 'cable', latency: { min: 1.6, median: 2.1, jitter: 0.4 }, bandwidth: { mbps: 940, loaded_delta: 3 }, decode: [] },
    codec: 'hevc',
    yuv444: true,
    width: 1920,
    height: 1200,
    refresh: 60,
    mbps: 420,
    mode_mbps: 120,
    reliable: false,
    reasons: { codec: 'best_format', mode: 'matches_display', yuv444: 'yuv444_allowed', bitrate: 'measured', transport: 'steady' },
    station: 'BUREAU-SALON',
  }
}
let autoCancelled = false
/** Les étapes du test imité (titres et mesures du cœur, en français ou en anglais). */
function autoSteps(at: number): AutoStep[] {
  const old = autoFlag === 'ancienne'
  const all: [AutoStep['step'], string, string, boolean][] = [
    ['link', fr() ? 'Liaison' : 'Connection', fr() ? 'Câble' : 'Cable', false],
    ['latency', fr() ? 'Latence au repos' : 'Latency at rest', fr() ? '2,1 ms · gigue 0,4 ms' : '2.1 ms · jitter 0.4 ms', false],
    [
      'bandwidth',
      fr() ? 'Débit vers cet appareil' : 'Bitrate to this device',
      old ? (fr() ? 'Mettez à jour BUREAU-SALON pour mesurer le débit.' : 'Update BUREAU-SALON to measure the bitrate.') : fr() ? '940 Mbit/s · latence en charge +3 ms' : '940 Mbit/s · latency under load +3 ms',
      old,
    ],
    ['decode', fr() ? 'Décodage de cet appareil' : 'Decoding on this device', fr() ? 'HEVC 4K : 240 i/s · AV1 4K : 120 i/s' : 'HEVC 4K: 240 fps · AV1 4K: 120 fps', false],
    [
      'encoder',
      fr() ? "Encodeur de l'ordinateur" : "The computer's encoder",
      old ? (fr() ? 'Mettez à jour BUREAU-SALON pour essayer son encodeur.' : 'Update BUREAU-SALON to try its encoder.') : fr() ? "jusqu'à 250 Mbit/s en HEVC" : 'up to 250 Mbit/s in HEVC',
      old,
    ],
  ]
  return all.map(([step, title, text, skipped], i) =>
    i < at ? { step, title, state: skipped ? 'skipped' : 'done', text } : i === at ? { step, title, state: 'running' } : { step, title, state: 'waiting' },
  )
}
const autoChoices = (): AutoChoice[] =>
  fr()
    ? [
        { key: 'codec', title: 'Format vidéo', value: 'HEVC', reason: 'le meilleur format que cet appareil décode' },
        { key: 'mode', title: 'Définition', value: '1920 × 1200 à 60 Hz', reason: 'comme cet écran' },
        { key: 'yuv444', title: 'Texte net (4:4:4)', value: 'Oui', reason: 'le réseau et le décodage le permettent' },
        { key: 'bitrate', title: 'Débit', value: '120 Mbit/s', reason: '940 Mbit/s mesurés, avec une marge' },
        { key: 'transport', title: "Transport de l'image", value: 'Rapide', reason: 'réseau stable' },
        { key: 'adaptive', title: 'Débit adaptatif', value: 'Oui', reason: 'suit le réseau pendant la session' },
      ]
    : [
        { key: 'codec', title: 'Video format', value: 'HEVC', reason: 'the best format this device decodes' },
        { key: 'mode', title: 'Resolution', value: '1920 × 1200 at 60 Hz', reason: 'matches this display' },
        { key: 'yuv444', title: 'Sharp text (4:4:4)', value: 'On', reason: 'the network and decoding allow it' },
        { key: 'bitrate', title: 'Bitrate', value: '120 Mbit/s', reason: '940 Mbit/s measured, with a margin' },
        { key: 'transport', title: 'Picture transport', value: 'Fast', reason: 'steady network' },
        { key: 'adaptive', title: 'Adaptive bitrate', value: 'On', reason: 'follows the network during the session' },
      ]

const thisPc = {
  name: 'PORTABLE-BUREAU',
  gpu: 'Intel(R) Iris(R) Xe Graphics',
  h264: true,
  hevc: true,
  hevc444: true,
  av1: true,
  hdr: true,
  pyrowave: !!pyroFlag && pyroFlag !== 'pc',
  width: 1920,
  height: 1200,
  refresh: 60,
  audio_channels: 2,
  os: 'Windows 11 Famille 24H2',
  on_battery: false,
}

const discovered = [{ id: 'son', name: 'Studio-Son', host: '192.168.1.77', port: 31544, fingerprint: fp('studio-son') }]

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

// Comptes KyberNow (docs/link-ui.md « Comptes KyberNow ») : `?comptes` (Léa est connectée sur share.kybernow.app :
// deux ordinateurs de son compte arrivent à l'accueil, avec la pastille « Compte », l'un sur le réseau, l'autre hors de
// chez vous ; les ordinateurs appairés disent « Appairé ») ; `?comptes=2` (un second compte, sur
// relais.exemple.org:9443 : la pastille dit le relais). « Se connecter à un compte KyberNow » ouvre la page imitée, qui
// revient au bout de 2 s : « Connecté à Léa Martin. » (avec `?comptes=annule` : « Annuler » sur la page, rien ;
// `?comptes=refus` : « La connexion n'a pas abouti : réessayez. »). « Un autre relais… » : une adresse avec une espace
// est illisible, une autre connecte Tom. `?comptes=ferme` : la session de Léa est fermée ailleurs 4 s après
// l'ouverture (« Ce compte a été déconnecté de cet appareil : reconnectez-vous. »).
type Kept = (typeof computers)[number] & { account?: AccountComputer | null; origin?: string | null; away?: boolean }
const accountsFlag = flags.has('comptes') ? flags.get('comptes') || 'oui' : null
const lea: AccountView = { key: 'share.kybernow.app|acc_lea', name: t('mock.account_lea'), email: 'lea@exemple.fr', relay: 'share.kybernow.app' }
const tom: AccountView = { key: 'relais.exemple.org:9443|acc_tom', name: 'Tom', email: 'tom@exemple.org', relay: 'relais.exemple.org:9443' }
let accountList: AccountView[] = []

/** Un ordinateur d'un compte à l'accueil : `away`, seulement par le relais (pas d'adresse sur le réseau). */
function accountEntry(a: AccountView, id: string, name: string, away: boolean): Kept {
  const ac: AccountComputer = { account: a.key, relay: a.relay, station: id, name, rights: ['session', 'lend', 'share', 'admin'], owner: { kind: 'person', id: a.key, name: a.name }, mine: true, online: true, admin_fp: fp(id), attached_at: now }
  const c = {
    id,
    name,
    host: away ? '' : '192.168.1.130',
    port: away ? 0 : 9443,
    user: '',
    by_code: false,
    wake: [] as WakeTarget[],
    fingerprint: fp(id),
    os: 'Windows 11 Pro 24H2',
    gpu: 'NVIDIA GeForce RTX 4060',
    settings: { ...settings },
    last: null as LastSession | null,
    online: true as boolean | null,
    info: (away ? null : { hostname: name, os: 'Windows 11 Pro 24H2', gpu: 'NVIDIA GeForce RTX 4060', av1: true, admin_version: '0.9.0', virtual_display: { available: true, max_screens: 2 }, station: station('0.9.0'), wake: [] }) as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  }
  return Object.assign(c, { account: ac, away, via_relay: away })
}
/** Les pastilles d'origine, comme `account::origins` de l'app (seulement avec un compte connecté). */
function markOrigins() {
  const several = new Set(accountList.map((a) => a.relay)).size > 1
  for (const c of computers as Kept[]) {
    c.origin = accountList.length === 0 ? null : c.account ? (several ? `${t('mock.origin_account')} · ${c.account.relay}` : t('mock.origin_account')) : t('mock.origin_paired')
  }
}

/** Un compte connecté : ses ordinateurs arrivent à l'accueil. */
function signedIn(a: AccountView) {
  accountList = [...accountList.filter((x) => x.key !== a.key), a]
  const mine = a.key === lea.key ? [accountEntry(a, 'accatelier', t('mock.account_atelier'), false), accountEntry(a, 'accchalet', 'Chalet', true)] : [accountEntry(a, 'accstudio', t('mock.account_studio'), true)]
  for (const c of mine) if (!computers.some((x) => x.id === c.id)) computers.push(c)
  markOrigins()
}
/** Un compte oublié : ses ordinateurs quittent l'accueil, les appairés restent. */
function signedOut(key: string) {
  accountList = accountList.filter((a) => a.key !== key)
  computers = computers.filter((c) => (c as Kept).account?.account !== key)
  markOrigins()
}

if (accountsFlag && accountsFlag !== 'annule' && accountsFlag !== 'refus') {
  signedIn(lea)
  if (accountsFlag === '2') signedIn(tom)
}
if (accountsFlag === 'ferme') {
  setTimeout(() => {
    signedOut(lea.key)
    emit('accounts', structuredClone(accountList))
    emitComputers()
    emit('account-signed-out', t('mock.account_signed_out'))
  }, 4000)
}

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
let syncSettings = true
/** Fin du suivi de la Station, comme `follow_loop` de l'app. */
const outcome = (o: StationOutcome) => {
  change(o.id, (c) => (c.station_pending = null))
  emit('station-outcome', o)
}
const update = () => ({ version: '0.5.1', channel: 'stable', notes: notesFor(i18n.lang), page: '', file: 'KyberNow-Link-Setup.exe', size: 1 })

/**
 * Les valeurs communes du cœur (`plan::values_json`), imitées : celles de l'app viennent de Rust
 * (`common_values`).
 */
const commonValues: Values = {
  manual_mbps: { min: 5, step: 5, max: 500, max_pyrowave: 2500, lg_tv_max: 95 },
  background_bitrate: { computer: 20_000_000 },
  max_guests: 4,
  max_sessions: 4,
  relay_link_wait_secs: 10,
  admin_port: 9443,
  address_example: '192.168.1.20:9443',
  default_relay: 'share.kybernow.app',
}

/**
 * Le plan d'une session imitée (`plan::plan` du cœur, à grands traits : profils, au plus 4K, H.264, sur
 * batterie, définition sur mesure) ; dans l'app, `displays_info` le donne.
 */
const planFor = (s: Settings, auto: AutoResult | null | undefined): SessionPlan => {
  const codec = (thisPc.hevc ? 'hevc' : 'h264') as NonNullable<Settings['codec']>
  const profile = s.profile === 'auto' && !auto ? 'jeu' : s.profile
  let [yuv444, mbps, reliable] = { jeu: [false, 60, false], bureau: [true, 90, true], max: [true, 300, false], auto: [false, 60, false] }[profile] as [boolean, number, boolean]
  if (yuv444 && !thisPc.hevc444) [yuv444, mbps] = [false, mbps + Math.floor(mbps / 3)]
  const base = profile === 'auto' && auto ? { codec: auto.codec, yuv444: auto.yuv444, mbps: auto.mbps, reliable: auto.reliable } : { codec, yuv444, mbps, reliable }
  const custom = (s.codec != null && s.codec !== base.codec) || s.yuv444 != null || s.mbps != null || s.reliable != null
  let mode = s.custom ?? { width: thisPc.width, height: thisPc.height, refresh: thisPc.refresh }
  const fit = (w: number, h: number) => {
    const k = Math.min(1, w / mode.width, h / mode.height)
    mode = { width: Math.floor(mode.width * k) & ~1, height: Math.floor(mode.height * k) & ~1, refresh: mode.refresh }
  }
  if (s.max_4k && !s.custom) fit(3840, 2160)
  if (battery === 'alleger' && thisPc.on_battery) mode = { ...mode, refresh: Math.min(30, mode.refresh) }
  const chosen = s.codec ?? base.codec
  if (chosen === 'h264') fit(4096, 4096)
  const from = chosen === 'pyrowave' ? ((s.yuv444 ?? base.yuv444) ? 1200 : 800) : base.mbps
  const scale = Math.min(3, Math.max(0.4, Math.pow((mode.width * mode.height * Math.max(1, mode.refresh)) / (3840 * 2160 * 60), 0.75)))
  const session = s.mbps ?? Math.max(5, Math.round(from * scale))
  const names: Record<string, string> = { jeu: 'Jeu', bureau: 'Bureau', max: 'Max', auto: 'Auto' }
  const label = `${custom ? 'Personnalisé' : names[profile]} · ${chosen === 'pyrowave' ? 'PyroWave' : chosen.toUpperCase()}`
  return { custom, base, ...mode, mbps: session, label }
}

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
/** « Laisser les invités parler » : coupé à chaque partage. */
let guestsSpeak = false
/** Micro : celui choisi dans les Réglages de l'app, et le traitement voix. */
let micDevice: string | null = null
let micVoice = true
/** Le lien du relais en préparation (`?relais=preparation` 4 s, `?relais=lent` sans fin). */
let sharePreparing = false
let shareJoinedLater = false
const prepareRelay = () => {
  if (relayFlag !== 'preparation' && relayFlag !== 'lent') return
  sharePreparing = true
  shareJoinedLater = false
  if (relayFlag === 'preparation')
    setTimeout(() => {
      sharePreparing = false
      shareJoinedLater = true
      emitShare()
    }, 4000)
}
/** Le relais tient le code du partage (lien du relais dans `link` et `web`). */
const shareRelayJoined = () => relayJoined || shareJoinedLater
if (shareOpen) prepareRelay()
const shareAvailability = (): Share['availability'] => (shareFlag === 'ancienne' ? 'unsupported' : shareFlag === 'coupe' ? 'disabled' : 'available')
const shareText = () => (shareFlag === 'ancienne' ? t('mock.share_unsupported') : shareFlag === 'coupe' ? t('mock.share_disabled') : null)
/** Le panneau de l'hôte, comme `ShareView` de l'app. */
const shareView = (): Share => ({
  availability: shareAvailability(),
  text: shareText(),
  open: shareOpen,
  code: shareOpen ? shareCode : '',
  link: shareOpen ? `kybernow://join?host=192.168.1.100&port=9443&fp=sha256:aa&code=482913${shareRelayJoined() ? '&relay=share.kybernow.app&sid=abcdefghijklmnopqrstuvwxyz' : ''}` : '',
  // Relais joint : le lien du client web est celui du relais.
  web: shareOpen ? (shareRelayJoined() ? 'https://share.kybernow.app/j/482913' : 'https://192.168.1.100:9443/link/?join=482913') : '',
  display: shareOpen ? 'Écran 1' : null,
  guests: structuredClone(sharedGuests),
  requests: structuredClone(shareRequests),
  // Relais joint : « Sur le même réseau ou ailleurs… » ; injoignable : « Pas encore joignable hors de chez vous. » ;
  // sans `?relais`, le relais n'est pas réglé.
  // En préparation : `connecting`, le lien du réseau dans `web` et `link`.
  relay: relayFlag === null ? null : relayFlag === 'injoignable' ? 'unreachable' : sharePreparing ? 'connecting' : 'joined',
  full: sharedGuests.length >= 4,
  qr: shareOpen ? fakeQr() : null,
  guests_speak: guestsSpeak,
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
const relayCodes = ['424242', '515151', '626262', '353535']
/** Ce que renvoie `share_join` quand l'invité entre. */
const joinedAs = (): ShareJoined & { kind: 'joined' } => ({ kind: 'joined', id: `guest-${++guestSeq}`, title: t('mock.guest_on'), host: 'Léa', role: 'spectator', fingerprint: fp('atelier') })

// MARK: - Prêt de l'ordinateur

const lendFlag = flags.has('pret') ? flags.get('pret') || 'oui' : null
/** Mercredi prochain, de 14 h à 18 h (heure de ce PC). */
const nextWednesday = (() => {
  const d = new Date()
  d.setDate(d.getDate() + ((3 - d.getDay() + 7) % 7 || 7))
  d.setHours(14, 0, 0, 0)
  return Math.floor(d.getTime() / 1000)
})()
let friends: Friend[] =
  lendFlag !== null && !['prete', 'reprise', 'code', 'demandes', 'creneau', 'ancienne', 'impossible', 'coupe'].includes(lendFlag)
    ? [
        { id: 'f1', owner: 'Matthieu', computer: 'PC-SALON', login: 'lea', remember: lendFlag !== 'mdp', state: lendFlag === 'mdp' ? 'password' : 'free', slot: { now: false, next: { start: nextWednesday, end: nextWednesday + 4 * 3600 } }, relay: true },
        { id: 'f2', owner: 'Inès', computer: 'TOUR-INES', login: 'lea', remember: true, state: 'busy', slot: null, relay: false },
      ]
    : []
const emitFriends = () => emit('lend-friends', structuredClone(friends))
// L'hôte : Bureau-Salon prêté à Léa (`?pret=prete`, `?pret=reprise`).
if (lendFlag === 'prete' || lendFlag === 'reprise') {
  const lent: HomeLend = { state: 'lent', name: 'Léa', until: now + 80 * 60, notice_at: lendFlag === 'reprise' ? now + 102 : null, notice: lendFlag === 'reprise' ? 'taken_back' : null }
  ;(computers[0] as Computer).lend = lent
}
/** Le prêt de l'hôte dans la barre (`/bar.html?pret=…`). */
let lendCodeOn = lendFlag === 'code'
let lendRequests: LendRequest[] =
  lendFlag === 'demandes'
    ? [
        { id: 'q1', name: 'Inès', kind: 'friend', app: 'ios', away: true, since: now - 30 },
        { id: 'q2', name: 'Hugo', kind: 'code', app: 'web', away: false, since: now - 5 },
      ]
    : lendFlag === 'creneau'
      ? [{ id: 'q3', name: 'Léa', kind: 'slot', app: 'android', away: false, since: now - 10 }]
      : []
/** Le relais du code de prêt, comme celui du partage (`&relais=preparation` : 4 s de préparation, `&relais=lent` : sans fin). */
let lendRelayPreparing = false
let lendRelayLater = false
const prepareLendRelay = () => {
  if (relayFlag !== 'preparation' && relayFlag !== 'lent') return
  lendRelayPreparing = true
  lendRelayLater = false
  if (relayFlag === 'preparation')
    setTimeout(() => {
      lendRelayPreparing = false
      lendRelayLater = true
      emitLend()
    }, 4000)
}
if (lendCodeOn) prepareLendRelay()
const lendRelayJoined = () => relayJoined || lendRelayLater
const lendView = (): LendPanel => ({
  availability: lendFlag === 'ancienne' ? 'unsupported' : lendFlag === 'impossible' ? 'unavailable' : lendFlag === 'coupe' ? 'disabled' : 'available',
  state: 'free',
  borrower: null,
  notice: lendFlag === 'creneau' ? { reason: 'slot_start', at: now + 120, text: t('lend.slot_notice', { name: 'Léa' }) } : null,
  requests: structuredClone(lendRequests),
  code: lendCodeOn
    ? {
        code: '205 718',
        // Relais joint : les liens sont ceux du relais ; en préparation, ceux du réseau.
        link: `kybernow://join?host=192.168.1.100&port=9443&code=205718${lendRelayJoined() ? '&relay=share.kybernow.app&sid=abcdefghijklmnopqrstuvwxyz' : ''}`,
        web: lendRelayJoined() ? 'https://share.kybernow.app/j/205718' : 'https://192.168.1.100:9443/link/?join=205718',
        duration: 7200,
        steam: 'friend',
        expires: now + 86400,
        relay: relayFlag === null ? null : relayFlag === 'injoignable' ? 'unreachable' : lendRelayPreparing ? 'connecting' : 'joined',
        qr: fakeQr(),
      }
    : null,
})
const emitLend = () => emit('lend-state', { id: sessions[0] ?? 'a1', lend: lendView() })
/** La barre de l'ami qui emprunte (`/bar.html?emprunt`). */
const borrowFlag = flags.has('emprunt')
const borrowBar = (): BorrowBar => ({
  host: 'Matthieu',
  until: flags.has('sansfin') ? null : now + 80 * 60,
  notice: flags.has('preavis') ? { reason: 'taken_back', at: Math.floor(Date.now() / 1000) + 120, text: t('mock.lend_notice_taken_back') } : null,
})
/** Les demandes d'emprunt qui attendent l'hôte : accepté au bout de 4 s (787878, Inès), refusé au bout de 3 s (797979). */
const lendWaits = new Map<number, { kind: 'accept' | 'refuse'; at: number; cancelled: boolean }>()
let lendSeq = 0
let lendTicket: { id: number; code: string } | null = null
/** Le code d'invitation trouvé par « Rejoindre » (246810 : avec le nom de l'ordinateur, 135790 : sans). */
let inviteTicket: { id: number; named: boolean } | null = null
let inviteSeq = 0
let invitation = flags.has('invitation')
const invitationFlag = flags.get('invitation')
const loanReady = (): BorrowOutcome & { kind: 'ready' } => ({ kind: 'ready', id: `lend-${++lendSeq}`, title: t('mock.lend_computer_of'), host: 'Matthieu', fingerprint: fp('pc-salon') })

/** Les textes du cœur (`recording` de apps/core/src/i18n), comme `RecordingState::ui_json` les donne. */
const recordingTexts = () =>
  i18n.lang === 'en'
    ? { record: 'Record', pill: 'Recording · {time}', record_tooltip: 'Record the session on the computer ({shortcut})', stop_tooltip: 'Stop recording ({shortcut})', stop: 'Stop recording', on_computer: 'On the computer', record_menu: 'Record the session', stop_menu: 'Stop recording ({time})', guest_pill: 'Recording', guest_tooltip: 'Atelier is recording the session.', disabled: 'Recording is turned off on this computer (Admin › Recordings).' }
    : { record: 'Enregistrer', pill: 'Enregistrement · {time}', record_tooltip: "Enregistrer la session sur l'ordinateur ({shortcut})", stop_tooltip: "Arrêter l'enregistrement ({shortcut})", stop: "Arrêter l'enregistrement", on_computer: "Sur l'ordinateur", record_menu: 'Enregistrer la session', stop_menu: "Arrêter l'enregistrement ({time})", guest_pill: 'Enregistrement', guest_tooltip: 'Atelier enregistre la session.', disabled: "L'enregistrement est coupé sur cet ordinateur (Admin › Enregistrements)." }
const recordingFlag = flags.has('enregistrement') ? flags.get('enregistrement') || 'oui' : null
/** L'enregistrement imité : commencé à `recordingSince` (ms), ou rien. */
let recordingSince: number | null = recordingFlag === 'encours' ? Date.now() - 130_000 : recordingFlag === 'heure' ? Date.now() - 3_595_000 : null
const recordingView = () =>
  recordingFlag === null
    ? null
    : { available: true, allowed: recordingFlag !== 'coupe', active: recordingSince !== null, elapsed: recordingSince === null ? 0 : Math.floor((Date.now() - recordingSince) / 1000), texts: recordingTexts() }

const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
  session_record: async (a) => {
    await new Promise((r) => setTimeout(r, 400))
    if (a.record && recordingFlag === 'codec') throw i18n.lang === 'en' ? "PyroWave and AV1 can't be recorded: switch to HEVC or H.264." : "PyroWave et AV1 ne s'enregistrent pas : passez en HEVC ou H.264."
    recordingSince = a.record ? Date.now() : null
    return recordingView()
  },
  lend_friends: () => structuredClone(friends),
  // Le lien d'invitation : avec le prénom de l'hôte (`name=`), ou un lien d'avant sans lui (`?invitation=ancien`).
  lend_invite: async () => {
    await sleep(300)
    if (invitationFlag === 'illisible') throw t('mock.lend_link')
    return { login: 'lea', host: invitationFlag === 'ancien' ? null : 'Matthieu', relay: true }
  },
  lend_add: async (a) => {
    await sleep(700)
    if (a.password === 'faux') throw t('mock.lend_bad_credentials')
    const id = `f${friends.length + 1}`
    friends = [...friends, { id, owner: 'Matthieu', computer: 'PC-SALON', login: String(a.login), remember: Boolean(a.remember), state: 'free', slot: null, relay: !!a.link }]
    emitFriends()
    return id
  },
  lend_remove: (a) => {
    friends = friends.filter((f) => f.id !== a.id)
    emitFriends()
    return null
  },
  // « Emprunter » : la Station passe en mode console (l'imitation : 2,5 s), ou l'hôte doit accepter.
  lend_borrow: async (a) => {
    if (a.ticket != null) {
      if (!lendTicket || lendTicket.id !== a.ticket) throw t('mock.no_session')
      const code = lendTicket.code
      lendTicket = null
      if (code === '787878' || code === '797979') {
        await sleep(500)
        lendWaits.set(++lendSeq, { kind: code === '787878' ? 'accept' : 'refuse', at: Date.now(), cancelled: false })
        return { kind: 'pending', wait: lendSeq, host: 'Matthieu' } satisfies BorrowOutcome
      }
      await sleep(2500)
      return loanReady()
    }
    const f = friends.find((x) => x.id === a.friend)
    if (!f) throw t('mock.no_session')
    if (a.password === 'faux') {
      await sleep(400)
      throw t('mock.lend_bad_credentials')
    }
    if (a.password) {
      f.state = 'free'
      f.remember = Boolean(a.remember)
      emitFriends()
    }
    if (f.state === 'busy') {
      await sleep(500)
      lendWaits.set(++lendSeq, { kind: 'accept', at: Date.now(), cancelled: false })
      return { kind: 'pending', wait: lendSeq, host: f.owner } satisfies BorrowOutcome
    }
    await sleep(2500)
    return loanReady()
  },
  lend_wait: async (a) => {
    const w = lendWaits.get(Number(a.wait))
    if (!w || w.cancelled) return { kind: 'cancelled' } satisfies LendWaitOutcome
    await sleep(1500)
    if (w.cancelled) return { kind: 'cancelled' } satisfies LendWaitOutcome
    const waited = Date.now() - w.at
    if (w.kind === 'accept' && waited >= 4000) {
      lendWaits.delete(Number(a.wait))
      return loanReady() satisfies LendWaitOutcome
    }
    if (w.kind === 'refuse' && waited >= 3000) {
      lendWaits.delete(Number(a.wait))
      throw t('mock.lend_refused')
    }
    return { kind: 'waiting' } satisfies LendWaitOutcome
  },
  lend_cancel: (a) => {
    const w = lendWaits.get(Number(a.wait))
    if (w) w.cancelled = true
    lendWaits.delete(Number(a.wait))
    console.info(`lend_cancel : la demande n°${a.wait} est retirée chez l'hôte`)
    return null
  },
  lend_come_back: async () => {
    await sleep(1500)
    return loanReady()
  },
  // La session d'emprunt : comme celle d'un invité ; `?pret=fin` et `?pret=retour` la finissent au bout de 5 s.
  lend_connect: (a) => {
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
        await sleep(900)
      }
      emit('session-status', { id, phase: 'streaming', detail: null })
      await sleep(500)
      emit('session-shown', id)
      const ends: Record<string, LendEnd> = {
        fin: { host: 'Matthieu', text: t('mock.lend_taken_back'), come_back: false, left: false },
        arrete: { host: 'Matthieu', text: t('mock.lend_stopped'), come_back: false, left: false },
        rendu: { host: 'Matthieu', text: t('mock.lend_given_back'), come_back: false, left: true },
        retour: { host: 'Matthieu', text: null, come_back: true, left: false },
      }
      const lend = lendFlag ? ends[lendFlag] : undefined
      if (!lend) return
      await sleep(5000)
      sessions = sessions.filter((s) => s !== id)
      emit('session-ended', { id, summary: { error: lend.left ? null : lend.text, median_ms: 9, p95_ms: 13, seconds: 5 }, lend })
      emitSessions()
    })()
    return { screens: 1, windowed: false } satisfies ConnectInfo
  },
  lend_give_back: () => {
    console.info("lend_give_back : l'ordinateur est rendu")
    return null
  },
  lend_state: async () => {
    await sleep(300)
    return lendView()
  },
  lend_code: async (a) => {
    await sleep(500)
    console.info(`lend_code : ${a.duration ?? 'sans fin'} s, Steam ${a.steam}`)
    lendCodeOn = true
    prepareLendRelay()
    emitLend()
    return lendView()
  },
  lend_cancel_code: () => {
    lendCodeOn = false
    emitLend()
    return lendView()
  },
  lend_answer: (a) => {
    lendRequests = lendRequests.filter((r) => r.id !== a.request)
    console.info(`lend_answer : ${a.request} ${a.accept ? 'acceptée' : 'refusée'}`)
    emitLend()
    return lendView()
  },
  // « Reprendre l'ordinateur » : le préavis de 2 min ; « Reprendre tout de suite » : le prêt finit.
  lend_take_back: (a) => {
    change(a.id, (c) => {
      const l = (c as Computer).lend
      if (!l) return
      ;(c as Computer).lend = a.now ? null : { ...l, notice_at: Math.floor(Date.now() / 1000) + 120, notice: 'taken_back' }
    })
    return lendView()
  },
  // « Ajouter l'ordinateur » (invitation) : 1,5 s d'appel ; le ticket reste après un refus.
  invite_redeem: async (a) => {
    if (!inviteTicket || inviteTicket.id !== a.ticket) throw { kind: 'other', message: t('mock.no_session') } satisfies InviteRefusal
    await sleep(1500)
    const refusal = (kind: InviteRefusal['kind']): InviteRefusal => ({ kind, message: t(`mock.invite_${kind}`) })
    const password = String(a.password).trim().toLowerCase()
    if (password === 'faux') throw refusal('bad_password')
    if (password === 'vieux') throw refusal('no_invite')
    if (password === 'plein') throw refusal('full')
    if (password === 'essais') throw refusal('too_many')
    if (password === 'coupe') throw refusal('access_off')
    const named = inviteTicket.named
    inviteTicket = null
    // Le même Admin (même empreinte) prend le nouvel accès au lieu d'être ajouté deux fois.
    const name = named ? t('mock.invite_computer') : '192.168.50.20'
    const old = computers.find((c) => c.id === 'inv1')
    if (old) {
      old.user = 'PORTABLE-BUREAU-ab12'
      old.online = true
    } else {
      computers.push({
        ...structuredClone(computers.find((c) => c.id === 'c3') ?? computers[0]),
        id: 'inv1',
        name,
        host: '192.168.50.20',
        port: 9443,
        user: 'PORTABLE-BUREAU-ab12',
        by_code: true,
        wake: [],
        fingerprint: fp('pc-du-salon'),
        settings: { ...settings },
        last: null,
        online: true,
        info: null,
        preview: null,
        preview_source: null,
        waking: false,
        station_pending: null,
      })
    }
    emitComputers()
    return { id: 'inv1', name: old?.name ?? name } satisfies InviteAdded
  },
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
    // Un code d'invitation : l'app demande le mot de passe, puis ajoute l'ordinateur avec le ticket.
    if (code === '246810' || code === '135790') {
      inviteTicket = { id: ++inviteSeq, named: code === '246810' }
      return { kind: 'invite', ticket: inviteSeq, computer: code === '246810' ? t('mock.invite_computer') : '' } satisfies ShareJoinOutcome
    }
    // Un partage ou un prêt ouvre une session : avec quatre sessions ouvertes (`?ordinateurs=4`), refusé après la
    // réponse ; une invitation, elle, passe toujours.
    const full = () => {
      if (sessions.length >= 4) throw t('common.four_at_most')
    }
    // Un code de prêt : l'app emprunte ensuite avec le ticket.
    if (code === '777777' || code === '787878' || code === '797979') {
      full()
      lendTicket = { id: ++lendSeq, code }
      return { kind: 'lend', ticket: lendSeq } satisfies ShareJoinOutcome
    }
    // 111111 : l'hôte envoie un format que ce PC ne lit pas, on entre quand même (image à part).
    const relayed = relayCodes.includes(code)
    if (!relayed && code !== '123456' && code !== '482913' && code !== '111111') throw t('mock.no_share')
    full()
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
    // L'hôte arrête son partage pendant l'attente : le cœur dit `stopped`.
    if (w.code === '353535' && waited >= 3000) {
      waits.delete(Number(a.wait))
      throw t('mock.share_stopped_waiting')
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
    prepareRelay()
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
  share_speak: (a) => {
    guestsSpeak = Boolean(a.on)
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
  snapshot: () => ({ computers: computerViews(), discovered: flags.has('vide') ? [] : discovered, this_pc: thisPc, sessions: [...sessions], version: 'dev' }),
  poll_now: async () => {
    await sleep(700)
    emitComputers()
  },
  probe_address: async (a) => {
    await sleep(500)
    const fingerprint = 'sha256:9f:3a:c2:71:0d:5e:88:b4'
    return { host: a.host, port: a.port, name: null, fingerprint, fingerprint_short: shortFp(fingerprint) }
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
  session_microphone: (a) => {
    const on = Boolean(a.on)
    emit('bar-mic', { id: a.id as string, mic: barMic(on, on ? 'waiting' : 'off') })
    if (on) setTimeout(() => emit('bar-mic', { id: a.id as string, mic: barMic(on, 'on') }), 600)
    return null
  },
  mic_settings: () => ({
    devices: [
      { id: 'mic-1', name: 'Microphone (Realtek Audio)' },
      { id: 'mic-2', name: 'Casque (USB Audio)' },
    ],
    device: micDevice,
    voice: micVoice,
  }),
  set_mic_device: (a) => {
    micDevice = (a.device as string | null) ?? null
    return null
  },
  set_mic_voice: (a) => {
    micVoice = Boolean(a.on)
    return null
  },
  session_mouse: (a) => {
    console.info(`session_mouse : souris ${a.mode} pour ${a.id}`)
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
    // HDR (`?hdr` : l'écran de ce PC est HDR ; `?hdr=windows` : la Station ne le peut pas) : la puce du cœur.
    const hdrShown = flags.has('hdr')
    const hdrReason = flags.get('hdr') === 'windows' ? t('mock.hdr_windows') : s.codec === 'h264' ? t('mock.hdr_codec') : null
    const hdr = { shown: hdrShown, available: !hdrReason, reason: hdrReason, kind: null }
    const plan = planFor(s as unknown as Settings, (c as { auto?: AutoResult } | undefined)?.auto)
    return { enabled, shown, count: all ? Math.max(1, Math.min(deviceScreens, max, 4)) : 1, virtual_display, hdr, hdr_on: hdrShown && !hdrReason && ((s as Partial<Settings>).hdr ?? true), plan } satisfies DisplaysInfo
  },
  recording_clock: (a) => {
    const total = Math.max(0, Math.floor(Number(a.seconds)))
    const two = (n: number) => String(n).padStart(2, '0')
    const ms = `${two(Math.floor(total / 60) % 60)}:${two(total % 60)}`
    return total >= 3600 ? `${Math.floor(total / 3600)}:${ms}` : ms
  },
  home_hdr: () => flags.has('hdr'),
  common_values: () => commonValues,
  ps_set_hdr: (a) => {
    const c = psConsole(String(a.id))
    if (c) c.hdr = !!a.on
    emitPs()
    return null
  },
  session_hdr: () => null,
  computer_wired: () => pyroFlag !== 'wifi',
  session_hevc: () => {
    pyroHevc = true
    return null
  },
  fond_state: () => ({ not_created: flags.has('sanscreation') }) satisfies FondState,
  session_command: () => null,
  bar_hide: () => null,
  // `?rejoindre` : « Rejoindre avec un code » demandé à l'ouverture (`=123456` : par un lien, code rempli).
  take_action: () => {
    if (invitation) {
      invitation = false
      return { kind: 'borrow', link: `kybernow://borrow?host=192.168.1.100&port=9443&fp=sha256:aa&login=lea&relay=share.kybernow.app&sid=abcdefghijklmnopqrstuvwxyz${invitationFlag === 'ancien' ? '' : '&name=Matthieu'}` }
    }
    if (!joinAction) return null
    joinAction = false
    const code = flags.get('rejoindre')
    return { kind: 'join', link: code ? `kybernow://join?host=192.168.1.77&port=31544&code=${code}` : null }
  },
  share_log: async () => {
    await sleep(800)
    console.info('share_log : la fenêtre « Partager » de Windows s’ouvrirait')
    return null
  },
  app_info: () => ({
    version: '0.5.0',
    channel: 'stable',
    language,
    lang: i18n.lang,
    update: update(),
    battery,
    open_at_login: openAtLogin,
    sync_settings: syncSettings,
    sync_settings_title: i18n.lang === 'fr' ? 'Synchroniser les réglages des ordinateurs' : 'Sync computer settings',
    sync_settings_help:
      i18n.lang === 'fr'
        ? 'Le profil, le format, le débit, le HDR et le mode immersif de chaque ordinateur vous suivent sur vos appareils.'
        : "Each computer's profile, format, bitrate, HDR and immersive mode follow you on your devices.",
  }),
  set_sync_settings: (a) => (syncSettings = Boolean(a.on)),
  set_open_at_login: (a) => (openAtLogin = Boolean(a.on)),
  set_profile: (a) => {
    // Auto sans test gardé : le panneau du test (comme l'app, `autotest::choose`).
    const kept = computers.find((c) => c.id === a.id) as (Computer & { auto?: AutoResult | null }) | undefined
    if (a.profile === 'auto' && !kept?.auto) return 'test'
    computers = computers.map((c) => (c.id === a.id ? { ...c, settings: { ...c.settings, profile: a.profile as string, codec: null, yuv444: null, mbps: null, reliable: null } } : c))
    emitComputers()
    return 'applied'
  },
  // Le test imité : les cinq étapes en 5 s, « Annuler » l'arrête.
  autotest_run: async (a) => {
    autoCancelled = false
    const count = autoSteps(0).length
    for (let i = 0; i <= count; i++) {
      emit('autotest-progress', { id: a.id, progress: { steps: autoSteps(i) } })
      if (i === count) break
      await sleep(i === 3 ? 1500 : 800)
      if (autoCancelled) return { result: null, choices: [], error: null, cancelled: true }
      if (autoFlag === 'echec' && i === 0) {
        return { result: null, choices: [], error: fr() ? "Le test n'a pas pu joindre Bureau-Salon." : "The test couldn't reach Bureau-Salon.", cancelled: false }
      }
    }
    return { result: autoResult(), choices: autoChoices(), error: null, cancelled: false }
  },
  autotest_cancel: () => {
    autoCancelled = true
    return null
  },
  autotest_apply: (a) => {
    const result = a.result as AutoResult
    let out: Settings | null = null
    computers = computers.map((c) => {
      if (c.id !== a.id) return c
      const s = { ...c.settings, profile: 'auto', codec: null, yuv444: null, mbps: null, reliable: null }
      const match = result.width === thisPc.width && result.height === thisPc.height && result.refresh === thisPc.refresh
      const next = { ...s, custom: match ? null : { width: result.width, height: result.height, refresh: result.refresh }, ...(match ? { max_4k: false } : {}) }
      out = next as unknown as Settings
      return { ...c, settings: next, auto: result } as unknown as typeof c
    })
    emitComputers()
    return out
  },
  autotest_later: () => null,
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
    // Auto sans test gardé : le panneau du test à la fin de la session (`later`, la barre le dit).
    const kept = computers.find((c) => c.id === a.id) as (Computer & { auto?: AutoResult | null }) | undefined
    if (a.profile === 'auto' && !kept?.auto) return 'later'
    // L'app écrit le profil en français, l'interface le traduit (localLabel).
    const names: Record<string, string> = { jeu: 'Jeu', bureau: 'Bureau', max: 'Max', auto: 'Auto' }
    emit('bar-label', { id: a.id, label: `${names[a.profile as string]} · HEVC`, profile: a.profile })
    return 'applied'
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
    if (stationInSession(a)) throw { kind: 'in_session', message: t('mock.station_in_session'), device: 'Apple TV du salon', seconds: null }
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
    const target = computers.find((c) => c.id === a.id)?.info?.station?.update.version ?? '0.5.1'
    setTimeout(() => outcome({ id: a.id as string, outcome: flags.has('echecmaj') ? 'update_failed' : 'updated', version: target }), 8500)
    return target
  },
  // Mise à jour des Stations : « Plus tard » (7 jours, pour cette version) et l'avis après une mise à jour de l'app.
  station_later: (a) => {
    snoozed.set(a.id as string, { version: a.version as string, at: Date.now() / 1000 })
    emitComputers()
    return null
  },
  take_station_notice: () => {
    if (!flags.has('avis') || noticeTaken) return null
    noticeTaken = true
    const stations = computers
      .filter((c) => c.online === true && c.info?.station?.update.available && c.info.station.update.installable && !c.info.station.update.requires_setup)
      .map((c) => ({ id: c.id, name: c.name, version: c.info!.station!.update.version ?? '' }))
    return stations.length ? { app: '0.11.3', stations } : null
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

// MARK: - Consoles PlayStation
//
// `?consoles` : deux comptes (« Matthieu_PSN », « Camille-92 ») et un compte d'une version précédente
// (sans pseudo) ; la « PS5 du salon », associée avec les deux premiers, allumée, « Astro Bot » en cours ;
// la « PS5 de Léa » en veille sur le réseau, pas gardée. `?consoles=vide` : aucun compte, les deux consoles
// sur le réseau ; `?consoles=releve` : la PS5 du salon ne répond pas sur le réseau, une Station la voit
// (« Hors de chez vous ») ; `?consoles=fin` : la partie s'arrête d'elle-même (« La console ne répond pas. »).
// Association : 12345678 passe, 00000000 est refusé, un autre code reste sans réponse. « Par adresse » :
// 192.168.1.111 est la PS5 de Léa. « Envoyer à un appareil » : une Apple TV attend (le code 000000 est
// refusé) ; « Recevoir d'un autre appareil » : le code 482 913, puis des comptes reçus au bout de 6 s
// (`&recevoir=expire` : le code expire). La barre d'une partie : `/bar.html?console` (souris, son, plein
// écran, mesures), `&manette` (une manette branchée, la barre pilotée par la manette : l'anneau se déplace).
const psFlag = flags.has('consoles') ? flags.get('consoles') || 'demo' : null
const salonId = 'A1B2C3D4E5F6'
const leaId = 'F6E5D4C3B2A1'
const psAccounts: PsAccount[] =
  psFlag && psFlag !== 'vide'
    ? [
        { id: 'acc-1', name: 'Matthieu_PSN', consoles: 1 },
        { id: 'acc-2', name: 'Camille-92', consoles: 1 },
        { id: 'acc-3', name: null, consoles: 0 },
      ]
    : []
const salonFound: PsFound = { host_id: salonId, name: t('mock.ps_living_room'), model: 'PS5', address: '192.168.1.110', state: 'ready', target: 'ps5_v1', running_app: 'Astro Bot' }
const leaFound: PsFound = { host_id: leaId, name: t('mock.ps_lea'), model: 'PS5', address: '192.168.1.111', state: 'standby', target: 'ps5_v1', running_app: null }
const away = psFlag === 'releve'
let psConsoles: PsConsole[] =
  psFlag && psFlag !== 'vide'
    ? [
        {
          id: `ps-${salonId}`,
          host_id: salonId,
          name: salonFound.name,
          model: 'PS5',
          address: salonFound.address,
          status: away ? 'away' : 'on',
          running_app: away ? null : 'Astro Bot',
          accounts: [
            { id: 'acc-1', name: 'Matthieu_PSN' },
            { id: 'acc-2', name: 'Camille-92' },
          ],
          chosen: 'acc-1',
          free_accounts: 1,
          custom: false,
          hdr: true,
          waking: false,
          reaching: false,
          in_session: false,
          found: away ? null : salonFound,
        },
      ]
    : []
let psFoundList: PsFound[] = psFlag ? [...(away ? [] : [salonFound]), leaFound] : []
const psSettingsOf = new Map<string, PsSettings>()
const psLayoutOf = new Map<string, PsLayout>()
const psDefaultSettings: PsSettings = { resolution: 'auto', fps: 60, bitrate_kbps: null, codec: 'auto', hdr: false, fullscreen: true, upscale: true, sharpness: 40, rumble: 100, triggers: 100 }
/** La disposition par défaut du cœur (keymouse.rs). */
const psDefaultLayout = (): PsLayout => ({
  bindings: [
    [{ key: 0x1a }, 'left_stick_up'], [{ key: 0x16 }, 'left_stick_down'], [{ key: 0x04 }, 'left_stick_left'], [{ key: 0x07 }, 'left_stick_right'],
    [{ key: 0x2c }, 'cross'], [{ key: 0x28 }, 'cross'], [{ key: 0x06 }, 'circle'], [{ key: 0x2a }, 'circle'], [{ key: 0x15 }, 'square'], [{ key: 0x09 }, 'triangle'],
    [{ key: 0x14 }, 'l1'], [{ key: 0x08 }, 'r1'], [{ mouse: 1 }, 'l2'], [{ mouse: 0 }, 'r2'], [{ key: 0xe1 }, 'l3'], [{ key: 0x19 }, 'r3'], [{ mouse: 2 }, 'r3'],
    [{ key: 0x52 }, 'dpad_up'], [{ key: 0x51 }, 'dpad_down'], [{ key: 0x50 }, 'dpad_left'], [{ key: 0x4f }, 'dpad_right'],
    [{ key: 0x29 }, 'options'], [{ key: 0x31 }, 'create'], [{ key: 0x35 }, 'ps'], [{ key: 0x2b }, 'touchpad'],
  ],
  mouse_stick: true,
  sensitivity: 1,
  curve: 0.8,
  deadzone: 0.2,
  invert_y: false,
})
/** Les noms des touches que Windows donnerait, sur un clavier AZERTY (la place de A est le Q). */
const psKeyNames = (): Record<string, string> => {
  const fr = i18n.lang === 'fr'
  const names: Record<string, string> = {}
  const azerty = 'QBCDEFGHIJKL,NOPARSTUVZXYW'
  for (let i = 0; i < 26; i++) names[String(0x04 + i)] = azerty[i]
  for (let i = 1; i <= 12; i++) names[String(0x39 + i)] = `F${i}`
  Object.assign(names, {
    [0x28]: fr ? 'Entrée' : 'Enter', [0x29]: fr ? 'Échap' : 'Esc', [0x2a]: fr ? 'Retour arrière' : 'Backspace', [0x2b]: 'Tab',
    [0x2c]: fr ? 'Espace' : 'Space', [0x31]: '*', [0x35]: '²', [0x33]: 'M',
    [0x52]: fr ? 'Haut' : 'Up', [0x51]: fr ? 'Bas' : 'Down', [0x50]: fr ? 'Gauche' : 'Left', [0x4f]: fr ? 'Droite' : 'Right',
    [0xe0]: fr ? 'Ctrl gauche' : 'Left Ctrl', [0xe1]: fr ? 'Maj gauche' : 'Left Shift', [0xe2]: 'Alt', [0xe4]: fr ? 'Ctrl droite' : 'Right Ctrl', [0xe5]: fr ? 'Maj droite' : 'Right Shift', [0xe6]: 'Alt Gr',
  })
  return names
}
const psState = (): PsState => {
  // Le nombre de consoles de chaque compte, et les comptes libres de chaque console, d'après les associations.
  for (const a of psAccounts) a.consoles = psConsoles.filter((c) => c.accounts.some((x) => x.id === a.id)).length
  for (const c of psConsoles) {
    c.free_accounts = psAccounts.filter((a) => !c.accounts.some((x) => x.id === a.id)).length
    c.accounts = c.accounts.map((x) => ({ id: x.id, name: psAccounts.find((a) => a.id === x.id)?.name ?? x.name }))
  }
  return structuredClone({ accounts: psAccounts, consoles: psConsoles, found: psFoundList, searched: true })
}
const emitPs = () => emit('ps-state', psState())
const psConsole = (id: unknown) => psConsoles.find((c) => c.id === id)
let psHandoffTimer: ReturnType<typeof setTimeout> | null = null
let psAccountSeq = 3
/** La barre d'une partie imitée (`/bar.html?console`). */
const psBarFlag = flags.has('console')
let psBarState: PsBar = { id: `ps-${salonId}`, name: t('mock.ps_living_room'), pad: flags.has('manette'), captured: false, muted: false, fullscreen: true, pad_mode: flags.has('manette') }

const psCommands: Record<string, (a: Record<string, unknown>) => unknown> = {
  ps_state: () => psState(),
  ps_search_now: async () => {
    await sleep(2000)
    return psState()
  },
  // La fenêtre de Sony : le compte est lu au bout de 1,5 s.
  ps_login: async () => {
    await sleep(1500)
    const account: PsAccount = { id: `acc-${++psAccountSeq}`, name: t('mock.ps_new_account'), consoles: 0 }
    psAccounts.push(account)
    emitPs()
    return { account, existed: false } satisfies PsKept
  },
  ps_account_manual: async (a) => {
    await sleep(300)
    const psnId = String(a.psnId)
    if (!/^[A-Za-z0-9+/]{11}=$/.test(psnId)) throw t('mock.ps_bad_id')
    const id = `manual-${psnId}`
    const known = psAccounts.find((x) => x.id === id)
    if (known) {
      if (a.name) known.name = String(a.name)
      emitPs()
      return { account: { ...known }, existed: true } satisfies PsKept
    }
    const account: PsAccount = { id, name: String(a.name || '') || null, consoles: 0 }
    psAccounts.push(account)
    emitPs()
    return { account, existed: false } satisfies PsKept
  },
  ps_account_rename: (a) => {
    const x = psAccounts.find((y) => y.id === a.account)
    if (x) x.name = String(a.name)
    emitPs()
    return null
  },
  ps_account_remove: (a) => {
    psAccounts.splice(psAccounts.findIndex((y) => y.id === a.account), 1)
    for (const c of psConsoles) c.accounts = c.accounts.filter((x) => x.id !== a.account)
    psConsoles = psConsoles.filter((c) => c.accounts.length > 0)
    emitPs()
    return null
  },
  ps_pair: async (a) => {
    await sleep(1200)
    if (a.pin === '00000000') throw t('mock.ps_wrong_code')
    if (a.pin !== '12345678') throw t('mock.ps_no_answer')
    const id = `ps-${a.hostId}`
    const found = psFoundList.find((f) => f.host_id === a.hostId)
    const account = psAccounts.find((x) => x.id === a.account)!
    const kept = psConsole(id)
    if (kept) {
      kept.accounts.push({ id: account.id, name: account.name })
    } else {
      psConsoles.push({
        id,
        host_id: String(a.hostId),
        name: found?.name ?? String(a.hostId),
        model: found?.model ?? 'PS5',
        address: String(a.address),
        status: found?.state === 'ready' ? 'on' : found?.state === 'standby' ? 'standby' : 'offline',
        running_app: found?.running_app ?? null,
        accounts: [{ id: account.id, name: account.name }],
        chosen: account.id,
        free_accounts: 0,
        custom: false,
        hdr: true,
        waking: false,
        reaching: false,
        in_session: false,
        found: found ?? null,
      })
    }
    emitPs()
    return id
  },
  // « Par adresse » : la PS5 de Léa répond à son adresse.
  ps_probe_address: async (a) => {
    await sleep(1500)
    return a.host === leaFound.address ? leaFound : null
  },
  ps_play: (a) => {
    const id = String(a.id)
    const c = psConsole(id)
    if (!c) throw t('mock.no_session')
    if (sessions.includes(id)) return { windowed: true }
    const windowed = sessions.length > 0
    sessions = [...sessions, id]
    emitSessions()
    void (async () => {
      if (c.status === 'standby') {
        c.waking = true
        emitPs()
        emit('session-status', { id, phase: 'preparing', detail: null })
        await sleep(2000)
        c.waking = false
        c.status = 'on'
      }
      c.in_session = true
      emitPs()
      emit('session-status', { id, phase: 'connecting', detail: null })
      emit('session-route', { id, route: c.status === 'away' ? routeView() : directRoute } satisfies RouteEvent)
      await sleep(1500)
      emit('session-status', { id, phase: 'streaming', detail: null })
      await sleep(600)
      emit('session-shown', id)
      await sleep(psFlag === 'fin' ? 3000 : 8000)
      sessions = sessions.filter((s) => s !== id)
      c.in_session = false
      emitPs()
      emit('session-status', { id, phase: 'ended', detail: null })
      emit('session-ended', { id, summary: { error: psFlag === 'fin' ? t('mock.ps_not_answering') : null, median_ms: null, p95_ms: null, seconds: 10 } })
      emitSessions()
    })()
    return { windowed }
  },
  ps_choose_account: (a) => {
    const c = psConsole(a.id)
    if (c) c.chosen = String(a.account)
    emitPs()
    return null
  },
  ps_settings: (a) => {
    const c = psConsole(a.id)
    if (!c) throw t('mock.no_session')
    return {
      id: c.id,
      name: c.name,
      model: c.model,
      settings: structuredClone(psSettingsOf.get(c.id) ?? psDefaultSettings),
      layout: structuredClone(psLayoutOf.get(c.id) ?? psDefaultLayout()),
      key_names: psKeyNames(),
    } satisfies PsSettingsView
  },
  ps_save_settings: (a) => {
    const s = a.settings as PsSettings
    psSettingsOf.set(String(a.id), s)
    const c = psConsole(a.id)
    if (c) c.custom = s.resolution !== 'auto' || s.fps !== 60 || s.bitrate_kbps != null || s.codec !== 'auto'
    emitPs()
    return null
  },
  ps_save_layout: (a) => {
    psLayoutOf.set(String(a.id), a.layout as PsLayout)
    return null
  },
  ps_default_layout: (a) => {
    psLayoutOf.delete(String(a.id))
    return psDefaultLayout()
  },
  ps_remove: (a) => {
    psConsoles = psConsoles.filter((c) => c.id !== a.id)
    emitPs()
    return null
  },
  // Une Apple TV attend un envoi (la recherche prend 1 s).
  ps_handoff_discover: async () => {
    await sleep(1000)
    return [{ name: t('mock.ps_tv'), platform: 'tvos', address: '192.168.1.50:41234' }] satisfies PsWaiting[]
  },
  ps_handoff_send: async (a) => {
    await sleep(1000)
    if (a.code === '000000') throw t('mock.ps_send_refused', { name: String(a.name) })
    return t('mock.ps_sent', { name: String(a.name) })
  },
  ps_handoff_receive: (a) => {
    if (psHandoffTimer) clearTimeout(psHandoffTimer)
    psHandoffTimer = null
    if (!a.on) return null
    psHandoffTimer = setTimeout(() => {
      psHandoffTimer = null
      emit('ps-handoff', (flags.get('recevoir') === 'expire' ? { event: 'expired' } : { event: 'received', text: t('mock.ps_received') }) satisfies PsHandoffEvent)
    }, 6000)
    return '482 913'
  },
  // La barre d'une partie : le son, la souris, le plein écran changent ; « Se déconnecter » et la veille la ferment.
  session_command: (a) => {
    if (typeof a.id !== 'string' || !a.id.startsWith('ps-')) return null
    const change = { mute: { muted: !psBarState.muted }, mouse: { captured: !psBarState.captured }, fullscreen: { fullscreen: !psBarState.fullscreen } }[a.command as 'mute']
    if (change) {
      psBarState = { ...psBarState, ...change }
      emit('ps-bar', psBarState)
    }
    console.info(`session_command ${a.command} : ${a.id}`)
    return null
  },
}
Object.assign(commands, psCommands)

/** Comptes KyberNow : la page du relais imitée revient au bout de 2 s. */
let signInTimer: ReturnType<typeof setTimeout> | null = null
const accountCommands: Record<string, (a: Record<string, unknown>) => unknown> = {
  account_list: () => structuredClone(accountList),
  account_sign_in: async (a) => {
    const relay = String(a.relay ?? '').trim()
    if (/\s/.test(relay)) throw t('mock.account_bad_relay')
    console.info(`account_sign_in : la page de ${relay || 'share.kybernow.app'} s'ouvre dans le navigateur`)
    if (signInTimer) clearTimeout(signInTimer)
    signInTimer = setTimeout(() => {
      signInTimer = null
      let r: AccountSignIn
      if (accountsFlag === 'annule') r = { kind: 'cancelled' }
      else if (accountsFlag === 'refus') r = { kind: 'failed', message: t('mock.account_bad_code') }
      else {
        const who = relay && relay !== 'share.kybernow.app' ? tom : lea
        signedIn(who)
        emit('accounts', structuredClone(accountList))
        emitComputers()
        r = { kind: 'signed_in', account: who.name }
      }
      emit('account-sign-in', r)
    }, 2000)
    return null
  },
  account_sign_in_cancel: () => {
    if (signInTimer) clearTimeout(signInTimer)
    signInTimer = null
    return null
  },
  account_manage: (a) => {
    console.info(`account_manage : la page du compte ${a.key} s'ouvre dans le navigateur`)
    return null
  },
  account_sign_out: async (a) => {
    await sleep(600)
    signedOut(String(a.key))
    emit('accounts', structuredClone(accountList))
    emitComputers()
    return null
  },
}
Object.assign(commands, accountCommands)

// Accueil au premier lancement et incitation au compte (docs/link-ui.md « Accueil et compte KyberNow ») : `?accueil`
// (l'accueil en trois écrans au lancement) ; sans compte, la carte du compte
// en fin de rangée (fermée, elle revient au rechargement au lieu de 30 jours ; `?carte=revenue` : revenue, sa croix dit « Ne plus
// afficher » ; `?carte=non` : fermée pour de bon) ; « Hors de chez vous ? » : « Réessayer » sur Atelier, qui reste hors ligne (`?sansreveil`), une fois par
// jour ; le bandeau après un appairage : l'ordinateur ajouté par une invitation (« Rejoindre » : 246810), une fois.
// « Créer un compte » et « www.kybernow.app » : la page s'ouvrirait dans le navigateur (console).
const welcomeFlag = flags.get('carte')
const welcomeMock = { seen: !flags.has('accueil'), closed: welcomeFlag === 'non' ? 2 : welcomeFlag === 'revenue' ? 1 : 0, paired: new Set<string>(), away: new Set<string>() }
const welcomeView = () => {
  const has = accountList.length > 0
  return { show: !has && !welcomeMock.seen, has_account: has, card: !has && welcomeMock.closed < 2, card_final: welcomeMock.closed >= 1, relay: 'share.kybernow.app' }
}
const welcomeCommands: Record<string, (a: Record<string, unknown>) => unknown> = {
  welcome_state: () => welcomeView(),
  welcome_done: () => {
    welcomeMock.seen = true
    return welcomeView()
  },
  welcome_close_card: () => {
    // Fermée une première fois : revenue d'un coup dans l'imitation (au lieu de 30 jours), pour voir « Ne plus afficher ».
    welcomeMock.closed = welcomeMock.closed === 0 ? 1 : 2
    const v = welcomeView()
    return welcomeMock.closed === 1 ? { ...v, card: false } : v
  },
  welcome_paired_hint: (a) => {
    if (accountList.length > 0 || welcomeMock.paired.has(String(a.id))) return false
    welcomeMock.paired.add(String(a.id))
    return true
  },
  welcome_away_hint: (a) => {
    if (accountList.length > 0 || welcomeMock.away.has(String(a.id))) return false
    welcomeMock.away.add(String(a.id))
    return true
  },
  welcome_sign_up: () => {
    console.info(`welcome_sign_up : https://share.kybernow.app/compte/inscription?lang=${i18n.lang} s'ouvre dans le navigateur`)
    return null
  },
  welcome_station_download: () => {
    console.info("welcome_station_download : https://www.kybernow.app/#telecharger s'ouvre dans le navigateur")
    return null
  },
}
Object.assign(commands, welcomeCommands)

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
        const served = psBarFlag ? psBarState.id : guestFlag !== null ? 'guest-1' : borrowFlag ? 'lend-1' : (sessions[0] ?? 'a1')
        // « Garder le son » à partir de deux sessions (`?ordinateurs=2`) ; sans session imitée, celle de la barre.
        if (name === 'bar-shown') {
          const guest: GuestBar | null = guestFlag !== null ? { role: guestRole, host: 'Léa', quality: guestQuality, image: imageOf(guestQuality) } : null
          // `?relais` : la session de la barre passe par le relais (puce « Par le relais »).
          const route = relayJoined ? routeView() : null
          const borrow = borrowFlag ? borrowBar() : null
          const host = !guest && !borrow
          // Le nom à côté du symbole ; `?largeur=900` : une session étroite (barre compacte) ; `?statistiques` : avec son panneau.
          const title = guest ? t('mock.guest_on') : (computers.find((c) => c.id === served)?.name ?? '')
          const room = Number(flags.get('largeur') ?? 1240) || 1240
          // `?mac` : une Station Mac (menu Touches du Mac).
          const shown = { id: served, label: 'Jeu · HEVC', profile: 'jeu', title, mouse: 'auto', clipboard: true, mac: flags.has('mac'), system: flags.has('mac') ? 'mac' : flags.has('linux') ? 'other' : 'windows', room, stats: flags.has('statistiques'), multi: deviceScreens > 1, keep_sound: keepSound.has(served), sessions: Math.max(1, sessions.length), guest, share: host ? shareView() : null, lend: host ? lendView() : null, borrow, recording: borrow ? null : recordingView(), route, hdr: flags.has('hdr') && !guest ? { on: true, available: flags.get('hdr') !== 'windows', reason: flags.get('hdr') === 'windows' ? t('mock.hdr_windows') : null } : null, mic: flags.has('sans-micro') ? null : barMic(false, 'off'), console: psBarFlag ? psBarState : null }
          // Un avis bref (`&avis`, `?arrivee`, `&preavis`) : sa capsule seule, sans la barre.
          const capsuleOnly = (guest && flags.has('avis')) || (!guest && flags.has('arrivee')) || !!borrow?.notice || (host && !!pyroFlag && (pyroFlag === 'wifi' || pyroFlag === 'sature' || !!relayJoined)) || (host && autoFlag === 'reseau')
          // `&manette` : la barre pilotée par la manette, l'anneau va et vient.
          if (psBarFlag && flags.has('manette')) {
            const keys = ['right', 'right', 'right', 'right', 'left', 'left', 'left', 'left'] as const
            let n = 0
            setInterval(() => emit('ps-bar-nav', { id: served, key: keys[n++ % keys.length] }), 1200)
          }
          if (!capsuleOnly) setTimeout(() => cb({ payload: { ...shown, wired: pyroFlag ? pyroFlag !== 'wifi' : null } }), 300)
          // `?pyrowave=wifi` (ou `&relais`) : le bandeau du début de session, « Passer en HEVC ».
          if (host && pyroFlag && (pyroFlag === 'wifi' || relayJoined)) {
            const kind = relayJoined ? 'pyrowave_relay' : 'pyrowave_wifi'
            setTimeout(() => cb({ payload: { ...shown, wired: pyroFlag !== 'wifi', pinned: true, notice: t('mock.pyrowave_notice'), notice_kind: kind } }), 2000)
          }
          // `?auto=reseau` : profil Auto sur un autre réseau que celui du test, son bandeau discret.
          if (host && autoFlag === 'reseau') {
            setTimeout(() => cb({ payload: { ...shown, label: 'Auto · HEVC 4:4:4', profile: 'auto', pinned: true, notice: t('autotest.other_network_text'), notice_kind: 'auto_network' } }), 2000)
          }
          // `?pyrowave=sature` : le réseau sature depuis 5 s, le même bandeau titré « Le réseau sature ».
          if (host && pyroFlag === 'sature' && !relayJoined) {
            setTimeout(() => cb({ payload: { ...shown, wired: true, pinned: true, notice: t('mock.pyrowave_notice'), notice_kind: 'pyrowave_strained' } }), 2000)
          }
          // `&avis` : le changement de rôle d'un invité, montré à la place des statistiques.
          if (guest && flags.has('avis')) {
            const kind = flags.get('avis') || (guestRole === 'spectator' ? 'watch' : guestRole === 'player' ? 'play' : 'kbm')
            // `image` : « Image allégée pour suivre. » ; `limite` : plus d'image à part possible, la qualité reçue est celle du texte.
            const text = kind === 'image' ? t('mock.notice_image_lag') : kind === 'limite' ? t('mock.notice_image_limit', { quality: '720p · 30 Hz' }) : t(`mock.notice_${kind}`)
            setTimeout(() => cb({ payload: { ...shown, pinned: true, notice: text } }), 2500)
          }
          // `?arrivee` (hôte) : une demande à rejoindre arrive, l'avis « Inès demande à rejoindre. » se choisit pour ouvrir le panneau.
          if (!guest && flags.has('arrivee')) {
            // `&arrivee=pret` : une demande d'emprunt, l'avis ouvre « Prêter l'ordinateur… ».
            const lending = flags.get('arrivee') === 'pret'
            const text = lending ? t('lend.wants_to_borrow', { name: 'Inès' }) : t('mock.notice_request', { name: 'Inès' })
            setTimeout(() => cb({ payload: { ...shown, pinned: true, notice: text, notice_opens_share: !lending, notice_opens_lend: lending } }), 2500)
          }
          // `&preavis` : le préavis de l'ami, montré comme un avis bref.
          if (borrow?.notice) setTimeout(() => cb({ payload: { ...shown, pinned: true, notice: borrow.notice!.text } }), 2000)
          // `&droits` : l'hôte donne et reprend les droits à tour de rôle (manettes, clavier et souris, un à la fois).
          if (guest && flags.has('droits')) {
            const steps: ShareRole[] = ['player', 'player_kbm', 'kbm', 'spectator', 'kbm', 'player_kbm']
            steps.forEach((role, i) => setTimeout(() => emit('share-role', { id: served, role } satisfies ShareRoleEvent), 3000 * (i + 1)))
          }
        }
        if (name === 'bar-computers') setTimeout(() => cb({ payload: barComputers() }), 300)
        // La partie d'une console : ses mesures à elle, une fois par seconde.
        if (name === 'ps-stats' && psBarFlag) {
          let lost = 0
          const timer = setInterval(() => {
            if (Math.random() < 0.2) lost++
            cb({ payload: { id: served, width: 1920, height: 1080, fps_asked: 60, hevc: true, hdr: false, target_kbps: 15000, fps: 58 + Math.random() * 2, mbps: 11.5 + Math.random() * 2, lost, recovered: Math.max(0, lost - 1) } })
          }, 1000)
          return () => clearInterval(timer)
        }
        if (name === 'stats' && psBarFlag) return () => {}
        if (name === 'stats') {
          const timer = setInterval(() => cb({ payload: { id: served, latency_ms: 8 + Math.random() * 3, fps: 60, width: 1920, height: 1200, decoder: `${pyroFlag && !pyroHevc ? 'PyroWave' : 'HEVC'} · ${t('mock.hardware')}`, relative: false, rtt_ms: 2.1, audio_ms: 18 + Math.random() * 4, audio_gaps: 0, screens: deviceScreens, chain: [0.4 + Math.random() * 0.3, 2.6 + Math.random() * 0.8, 0.3, 1.2 + Math.random(), 1.1 + Math.random() * 0.4, 2.4 + Math.random() * 1.5], hdr: false, refresh: 120, mbps: 70 + Math.random() * 15, target: { mbps: 80, text: '80 Mbit/s · Auto', label: i18n.lang === 'fr' ? 'Débit visé' : 'Target bitrate' }, lost_frames: Math.random() < 0.1 ? 1 : 0, lost_packets: Math.random() < 0.2 ? 3 : 0, keyframes: 0, measured_pct: 98, rejected: 0, audio_channels: 2, audio_buffer_ms: 14 + Math.random() * 3, audio_output_ms: 10 } }), 1000)
          return () => clearInterval(timer)
        }
        return () => {}
      },
    },
    window: { getCurrentWindow: () => ({ minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {} }) },
  }
}
