<!--
  L'app : accueil, ajout, réglages, passage vers la session. Les consoles PlayStation (docs/link-ui.md
  « Consoles PlayStation ») : leur état (`ps_state`, événement `ps-state`), leur association
  (PairSheet), leurs réglages (ConsoleSettings) et leur partie, ouverte comme une session d'ordinateur.

  Comptes KyberNow (docs/link-ui.md « Comptes KyberNow ») : les comptes connectés (`account_list`,
  événement `accounts`), la connexion par la page du relais dans le navigateur (`account_sign_in`,
  « Connexion au compte… » jusqu'à l'événement `account-sign-in` : l'avis « Connecté à {compte}. ») ;
  leurs ordinateurs arrivent avec les autres (`computers`), avec leur pastille d'origine. Une session
  d'appareil fermée ailleurs : l'avis « Ce compte a été déconnecté de cet appareil : reconnectez-vous. »
  (événement `account-signed-out`).

  Mise à jour des Stations (docs/link-ui.md) : au premier lancement après une mise à jour de l'app, l'avis
  « … a aussi une mise à jour » (StationNotice, `take_station_notice` après l'événement `station-notice`), montré
  seulement sur l'accueil, hors session : il attend la fin d'une session ouverte au lancement.

  Accueil et compte KyberNow (docs/link-ui.md) : l'accueil en trois écrans au premier lancement (Welcome,
  `welcome_state`, « Revoir l'accueil » dans les réglages de l'app), fermé par un compte connecté ; sans compte, le
  bouton « Connecter un compte » de la barre de titre, la carte du compte, le bandeau après un appairage
  (`welcome_paired_hint`) et « Hors de chez vous ? » (AwayPrompt, `welcome_away_hint`) quand « Réessayer » ou
  « Se connecter » laisse hors ligne un ordinateur appairé que le relais ne tient pas.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import TitleBar from './TitleBar.svelte'
  import Home from './Home.svelte'
  import AddSheet from './AddSheet.svelte'
  import Settings from './Settings.svelte'
  import Passage from './Passage.svelte'
  import AppSheet from './AppSheet.svelte'
  import AutoTest from './AutoTest.svelte'
  import JoinSheet from './JoinSheet.svelte'
  import InviteSheet from './InviteSheet.svelte'
  import LendSheet from './LendSheet.svelte'
  import PairSheet from './PairSheet.svelte'
  import ConsoleSettings from './ConsoleSettings.svelte'
  import StationNotice from './StationNotice.svelte'
  import Welcome from './Welcome.svelte'
  import AwayPrompt from './AwayPrompt.svelte'
  import type { AccountSignIn, AccountView, Action, AppInfo, Battery, BorrowOutcome, Candidate, Chosen, Computer, ConnectInfo, Settings as ComputerSettings, Found, Friend, LendEnd, LendInvite, LendSheetKind, LoanReady, Phase, ScreensEvent, ShareJoined, Snapshot, StationRestarting, Summary, ThisPc, Update, WatchWords } from './lib/api'
  import type { PsFound, PsState, StationNotice as StationNoticeData } from './lib/api'
  import type { WelcomeView } from './lib/api'
  import { guestComputer, invoke, isConsoleId, listen, stationBannerFor } from './lib/api'
  import { setLang, t } from './lib/i18n/index.svelte'

  let computers = $state<Computer[]>([])
  let discovered = $state<Found[]>([])
  let thisPc = $state<ThisPc | null>(null)
  let selectedId = $state<string | null>(read('kybernow.selected'))
  let settingsId = $state<string | null>(null)
  /** Rubrique des réglages ouverte par une notification ou le menu de l'icône (`station`). */
  let settingsRubric = $state<string | null>(null)
  let adding = $state<{ found: Found | null } | null>(null)
  /**
   * Le passage de la première session (l'accueil coupé en deux), gardé ouvert pendant toute la
   * session. `restarting` : dernier recours de la connexion, la Station redémarre ; `many` :
   * plusieurs écrans. Les sessions suivantes (plusieurs ordinateurs) ont leur propre fenêtre de
   * connexion, sans passage sur l'accueil.
   */
  let passage = $state<{
    id: string
    phase: Phase['phase']
    opening: boolean
    closing: boolean
    restarting: boolean
    many: boolean
    guest?: ShareJoined
    /** Session d'emprunt (prêt de l'ordinateur) : « Ordinateur de {hôte} », « Préparation de l'ordinateur ». */
    borrow?: { title: string; fingerprint: string | null }
    /** L'emprunt lui-même (la Station passe en mode console) : rien à annuler encore. */
    borrowing?: boolean
    /** Partie sur une console PlayStation : son nom ; `wakes` : « Réveil de la console » en première étape. */
    console?: { name: string; wakes: boolean }
  } | null>(null)
  /** Les consoles PlayStation : comptes, consoles gardées, consoles qui répondent. */
  let ps = $state<PsState>({ accounts: [], consoles: [], found: [], searched: false })
  /** Les réglages d'une console (à la place de l'accueil, comme ceux d'un ordinateur). */
  let consoleSettingsId = $state<string | null>(null)
  /** La feuille de l'association : une console (`found`), ou un compte seul (« Ajouter un compte », `found` nul). */
  let pairing = $state<{ found: PsFound | null } | null>(null)
  /** « Ce compte est déjà là. » sur l'onglet PlayStation de l'ajout, après « Ajouter un compte ». */
  let psAlready = $state(false)
  /** « Réessayer » d'une console hors ligne en cours. */
  let psSearching = $state(false)
  /** L'ajout s'ouvre sur l'onglet PlayStation. */
  let addingWay = $state<'playstation' | null>(null)
  /** Les ordinateurs empruntés (« Ordinateur de {hôte} »), à part des ordinateurs appairés. */
  let friends = $state<Friend[]>([])
  /** Une feuille du prêt côté ami : lien d'invitation, mot de passe, attente de l'hôte, carte de fin. */
  let lendSheet = $state<LendSheetKind | null>(null)
  /**
   * « Rejoindre avec un code » (KyberNow Share) : l'écran du code et du prénom. `code` et `link` :
   * ceux d'un lien `kybernow://join?…`, déjà remplis. Un partage rejoint ouvre une session d'invité
   * (passage ou fenêtre, comme un ordinateur) sans rien ajouter à la liste des ordinateurs.
   */
  let joining = $state<{ code: string; link: string | null } | null>(null)
  /**
   * « Rejoindre avec un code » est tombé sur un code d'invitation (docs/invitations.md) : la feuille « Ajouter
   * {ordinateur} » et son mot de passe. L'ordinateur ajouté est sélectionné sur l'accueil, avec l'avis
   * « {ordinateur} est ajouté. ».
   */
  let inviting = $state<{ ticket: number; computer: string } | null>(null)
  /** Les ordinateurs en session, dans l'ordre d'ouverture (plusieurs ordinateurs, quatre au plus). */
  let sessions = $state<string[]>([])
  let error = $state<string | null>(null)
  let appSheet = $state(false)
  /** Réglages de l'app ouverts par « Rechercher des mises à jour » (menu de l'icône). */
  let appCheck = $state(false)
  let update = $state<Update | null>(null)
  let progress = $state<number | null>(null)
  let notice = $state<string | null>(null)
  /** « Sur batterie » des réglages de l'app : l'écran que l'accueil annonce en dépend. */
  let battery = $state<Battery>('alleger')
  /** « Réessayer » en cours (accueil). */
  let retrying = $state<string | null>(null)
  /** Les comptes KyberNow connectés sur ce PC (réglages de l'app, pastilles d'origine). */
  let accounts = $state<AccountView[]>([])
  /** « Connexion au compte… » : la page du relais est ouverte dans le navigateur, l'app attend son retour. */
  let signingIn = $state(false)
  /** Le refus de la dernière connexion à un compte, montré aussi dans les réglages de l'app. */
  let signInError = $state<string | null>(null)
  /** L'incitation au compte : accueil du premier lancement, carte du compte (`welcome_state`). */
  let welcome = $state<WelcomeView | null>(null)
  /** L'accueil en trois écrans est ouvert (premier lancement, ou « Revoir l'accueil »). */
  let welcomeOpen = $state(false)
  /** L'ordinateur dont le bandeau « après un appairage » se montre (sans compte). */
  let pairedHint = $state<string | null>(null)
  /** L'ordinateur qui vient d'être ajouté : son bandeau attend que celui de la mise à jour de sa Station soit parti. */
  let pairedPending = $state<string | null>(null)
  /** « Hors de chez vous ? » : l'ordinateur appairé absent de ce réseau. */
  let awayPrompt = $state<{ id: string; name: string } | null>(null)
  /** L'avis après une mise à jour de l'app : les Stations à mettre à jour aussi. */
  let stationNotice = $state<StationNoticeData | null>(null)
  /** Profil Auto : l'ordinateur dont le panneau du test est ouvert. */
  let autotestId = $state<string | null>(null)
  /** Profil Auto appliqué : les réglages enregistrés, que l'écran des réglages ouvert reprend. */
  let autoApplied = $state<{ id: string; settings: ComputerSettings } | null>(null)
  // Un autre écran des réglages (ou aucun) : il part des réglages enregistrés, déjà à jour.
  $effect(() => {
    void settingsId
    autoApplied = null
  })

  function read(k: string) {
    try {
      return localStorage.getItem(k)
    } catch {
      return null
    }
  }

  /** La console choisie sur l'accueil ; sans ordinateur ni ordinateur d'ami, la première. */
  const selectedConsole = $derived(
    ps.consoles.find((c) => c.id === selectedId) ?? (computers.length === 0 && friends.length === 0 ? (ps.consoles[0] ?? null) : null),
  )
  /** L'ordinateur d'un ami choisi sur l'accueil (prêt de l'ordinateur). */
  const selectedFriend = $derived(selectedConsole ? null : (friends.find((f) => f.id === selectedId) ?? (computers.length === 0 ? (friends[0] ?? null) : null)))
  const selected = $derived(selectedFriend || selectedConsole ? null : (computers.find((c) => c.id === selectedId) ?? computers[0] ?? null))
  const consoleSettingsFor = $derived(ps.consoles.find((c) => c.id === consoleSettingsId) ?? null)
  const settingsFor = $derived(computers.find((c) => c.id === settingsId) ?? null)
  /** Ce que le passage montre : l'ordinateur, ou « Invité sur {nom} » pour une session d'invité. */
  const passageFor = $derived(
    computers.find((c) => c.id === passage?.id) ??
      (passage?.guest
        ? guestComputer(passage.guest)
        : passage?.borrow
          ? guestComputer(passage.borrow)
          : passage?.console
            ? // La signature d'une console vient de son nom (comme sur Mac).
              { name: passage.console.name, preview: null, fingerprint: passage.console.name }
            : null),
  )

  function select(id: string) {
    selectedId = id
    error = null
    try {
      localStorage.setItem('kybernow.selected', id)
    } catch {}
  }

  onMount(() => {
    const off = [
      listen<Computer[]>('computers', (c) => (computers = c)),
      listen<Friend[]>('lend-friends', (f) => (friends = f)),
      listen<Found[]>('discovered', (d) => (discovered = d)),
      listen<ThisPc>('this-pc', (p) => (thisPc = p)),
      // Demande d'une autre ouverture, gardée par l'app jusqu'à ce qu'on la prenne.
      listen<null>('action-ready', () => invoke<Action | null>('take_action').then((a) => a && handle(a))),
      // L'avis après une mise à jour de l'app, prêt une fois les Stations vérifiées.
      listen<null>('station-notice', takeStationNotice),
      listen<Update | null>('update', (u) => (update = u)),
      listen<string[]>('sessions', (s) => (sessions = s)),
      listen<PsState>('ps-state', (s) => (ps = s)),
      listen<string>('lang', setLang),
      listen<number>('update-progress', (p) => (progress = p)),
      listen<AccountView[]>('accounts', (a) => {
        accounts = a
        refreshWelcome()
      }),
      listen<AccountSignIn>('account-sign-in', (r) => {
        signingIn = false
        if (r.kind === 'signed_in') {
          signInError = null
          // Connecté depuis l'accueil du premier lancement : l'écran 3 est passé, les ordinateurs du compte arrivent.
          if (welcomeOpen) finishWelcome()
          pairedHint = null
          awayPrompt = null
          notice = t('account.signed_in', { account: r.account })
        } else if (r.kind === 'failed') {
          signInError = r.message
          error = r.message
        }
      }),
      // La session de l'appareil a été fermée ailleurs (page du compte, nouveau mot de passe) : ses ordinateurs sont partis.
      listen<string>('account-signed-out', (m) => (notice = m)),
      listen<Phase>('session-status', (p) => {
        if (!passage || passage.id !== p.id) return
        // « La Station redémarre… » jusqu'à l'étape suivante (voir StationRestarting).
        passage.restarting = false
        // Une console qui dort : son réveil d'abord (« Réveil de la console »).
        if (passage.console && p.phase === 'preparing') passage.console.wakes = true
        // Connectée mais sans image encore : le passage attend la première,
        // il ne s'ouvre pas sur du noir.
        passage.phase = p.phase === 'streaming' && !passage.opening ? 'first_images' : p.phase
      }),
      listen<StationRestarting>('session-station-restarting', ({ id }) => {
        if (passage && passage.id === id) passage.restarting = true
      }),
      // Les écrans de la session : « Préparation des écrans » avec plusieurs.
      listen<ScreensEvent>('session-screens', ({ id, screens }) => {
        if (passage && passage.id === id) passage.many = screens > 1
      }),
      // La connexion reprend en session (écrans de ce PC ou profil changés) : ses étapes
      // reviennent sur l'écran principal, l'accueil les montre de nouveau.
      listen<ScreensEvent>('session-replan', ({ id, screens }) => {
        if (!passage || passage.id !== id) return
        passage.restarting = false
        passage.closing = false
        passage.opening = false
        passage.phase = 'preparing'
        passage.many = screens > 1
      }),
      listen<string>('session-shown', (id) => {
        if (passage && passage.id === id) passage.opening = true
      }),
      listen<{ id: string; summary: Summary; lend?: LendEnd | null }>('session-ended', ({ id, summary, lend }) => {
        // Une session d'emprunt : sa carte de fin (texte du prêt, ou « Revenir » pendant 15 min) ;
        // l'ordinateur rendu : l'avis « Vous avez rendu l'ordinateur. », sans carte.
        if (lend?.left) notice = lend.text
        else if (lend) lendSheet = { kind: 'end', end: lend }
        // Seulement la session du passage : une session suivante montre son échec dans
        // sa propre fenêtre de connexion, ou par une notification.
        if (!passage || passage.id !== id) return
        // Avant la première image, c'est la connexion qui a échoué.
        const failed = !passage.opening
        // Un invité : la raison du partage, telle quelle (« Léa a arrêté le partage. »).
        error = summary.error
          ? passage.guest || passage.borrow || isConsoleId(id)
            ? lend
              ? null
              : summary.error
            : t(failed ? 'app.connect_failed' : 'app.session_stopped', { error: summary.error })
          : null
        passage.opening = false
        passage.closing = true
        setTimeout(() => {
          if (passage?.id === id && passage.closing) passage = null
        }, 750)
      }),
    ]
    // L'état complet une fois les écoutes en place (rien ne passe entre deux).
    // Lectures indépendantes, en même temps ; une qui échoue (noté au journal
    // de la page) n'empêche ni les autres, ni le lien `kybernow://` reçu au
    // lancement (`take_action`), ni l'avis des Stations.
    const logged = (what: string) => (e: unknown) => console.error(`${what} :`, e)
    Promise.all(off).then(() =>
      Promise.allSettled([
        invoke<Snapshot>('snapshot').then((s) => {
          computers = s.computers
          discovered = s.discovered
          thisPc = s.this_pc ?? thisPc
          sessions = s.sessions ?? []
        }, logged('snapshot')),
        invoke<Friend[]>('lend_friends').then((f) => (friends = f), logged('lend_friends')),
        invoke<PsState>('ps_state').then((s) => (ps = s), logged('ps_state')),
        invoke<AccountView[]>('account_list').then((a) => (accounts = a), logged('account_list')),
        invoke<WelcomeView>('welcome_state').then((w) => {
          welcome = w
          if (w.show) welcomeOpen = true
        }, logged('welcome_state')),
        invoke<AppInfo>('app_info').then((i) => {
          update = i.update
          battery = i.battery ?? battery
          setLang(i.lang)
        }, logged('app_info')),
      ])
        .then(() => invoke<Action | null>('take_action'))
        .then((a) => a && handle(a))
        .catch(logged('take_action'))
        .finally(takeStationNotice),
    )
    return () => off.forEach((p) => p.then((f) => f()))
  })

  function takeStationNotice() {
    invoke<StationNoticeData | null>('take_station_notice')
      .then((n) => n && n.stations.length > 0 && (stationNotice = n))
      .catch(() => {})
  }

  /** Demande venue de l'extérieur : lien, barre des tâches, notification, menu de l'icône. */
  async function handle(a: Action) {
    // Une demande venue d'ailleurs (lien, menu de l'icône…) passe avant l'accueil du premier lancement, qui revient au
    // prochain lancement (il n'est pas marqué vu).
    if (a.kind !== 'updated' && a.kind !== 'show') welcomeOpen = false
    settingsId = null
    settingsRubric = null
    consoleSettingsId = null
    if (a.kind === 'show') {
      if (computers.some((c) => c.id === a.id) || ps.consoles.some((c) => c.id === a.id)) select(a.id)
    } else if (a.kind === 'ps_settings') {
      // « Réglages de {nom} » depuis la barre d'une partie.
      if (!ps.consoles.some((c) => c.id === a.id)) return
      adding = null
      appSheet = false
      pairing = null
      select(a.id)
      consoleSettingsId = a.id
    } else if (a.kind === 'add_console') {
      addingWay = 'playstation'
      adding = { found: null }
    } else if (a.kind === 'settings') {
      if (!computers.some((c) => c.id === a.id)) return
      adding = null
      appSheet = false
      select(a.id)
      settingsRubric = a.rubric
      settingsId = a.id
    } else if (a.kind === 'app') {
      adding = null
      appCheck = a.check
      appSheet = true
    } else if (a.kind === 'autotest') {
      // Le panneau du test du profil Auto (menu de l'icône, ou demandé pendant la session).
      if (!computers.some((c) => c.id === a.id)) return
      adding = null
      appSheet = false
      select(a.id)
      autotestId = a.id
    } else if (a.kind === 'connect') {
      if (!computers.some((c) => c.id === a.id)) return
      select(a.id)
      connect(a.id)
    } else if (a.kind === 'watch') {
      if (!computers.some((c) => c.id === a.id)) return
      select(a.id)
      watch(a.id)
    } else if (a.kind === 'add') {
      addingWay = null
      adding = { found: null }
    } else if (a.kind === 'join') {
      adding = null
      appSheet = false
      join(a.link)
    } else if (a.kind === 'borrow') {
      adding = null
      appSheet = false
      try {
        lendSheet = { kind: 'invite', link: a.link, invite: await invoke<LendInvite>('lend_invite', { link: a.link }) }
      } catch (e) {
        error = String(e)
      }
    } else if (a.kind === 'link') {
      try {
        const c = await invoke<Candidate>('parse_link', { link: a.link })
        adding = { found: { id: 'lien', name: c.name ?? c.host, host: c.host, port: c.port, fingerprint: c.fingerprint } }
      } catch (e) {
        error = String(e)
      }
    } else if (a.kind === 'updated') {
      invoke<AppInfo>('app_info').then((i) => (notice = t('app.installed', { version: i.version })))
    }
  }

  /** « Rejoindre avec un code », avec le code d'un lien (`kybernow://join?…`) s'il y en a un. */
  async function join(link: string | null = null) {
    error = null
    inviting = null
    if (!link) {
      joining = { code: '', link: null }
      return
    }
    try {
      joining = { code: await invoke<string>('share_link_code', { link }), link }
    } catch (e) {
      joining = null
      error = String(e)
    }
  }

  /**
   * Le partage est rejoint : sa session s'ouvre comme celle d'un ordinateur — la première avec le passage
   * de l'accueil (titré « Invité sur {nom} »), les suivantes dans une fenêtre avec leur propre fenêtre de
   * connexion. La fin se lit telle que le cœur la dit.
   */
  async function joined(j: ShareJoined, replacing: string | null = null) {
    joining = null
    error = null
    // `replacing` : le passage d'attente de « Regarder », que celui de la session remplace.
    const following = sessions.length > 0 || (passage !== null && passage.id !== replacing)
    const fresh = () => ({ id: j.id, phase: 'preparing' as const, opening: false, closing: false, restarting: false, many: false, guest: j })
    if (!following) passage = fresh()
    else if (replacing && passage?.id === replacing) passage = null
    try {
      const started = await invoke<ConnectInfo>('share_connect', { id: j.id })
      if (started.windowed) {
        if (passage?.id === j.id && !passage.opening) passage = null
      } else if (!(passage?.id === j.id && !passage.closing)) {
        passage = fresh()
      }
    } catch (e) {
      if (passage?.id === j.id && !passage.opening) passage = null
      error = String(e)
    }
  }

  // MARK: - Regarder (droit « Regarder » d'un compte KyberNow)

  /**
   * « Regarder » un ordinateur d'un compte : la Station fait entrer ce PC en spectateur (10 s au plus), puis la
   * session s'ouvre comme celle d'un invité, titrée « Regarder {nom} » (mot du cœur). Sans autre session, le
   * passage s'ouvre tout de suite, sans « Annuler » tant que la Station n'a pas répondu. Refus : le texte du cœur.
   */
  async function watch(id: string) {
    error = null
    const c = computers.find((x) => x.id === id)
    if (!c) return
    const pendingId = 'watch-preparing'
    if (sessions.length === 0 && passage === null) {
      const w = await invoke<WatchWords>('share_watch_words', { name: c.name }).catch(() => null)
      if (w && passage === null)
        passage = { id: pendingId, phase: 'preparing', opening: false, closing: false, restarting: false, many: false, guest: { id: pendingId, title: w.title, host: '', role: 'spectator', fingerprint: c.fingerprint }, borrowing: true }
    }
    try {
      await joined(await invoke<ShareJoined>('share_watch', { id }), pendingId)
    } catch (e) {
      if (passage?.id === pendingId) passage = null
      error = String(e)
    }
  }

  // MARK: - Prêt de l'ordinateur, côté ami

  /**
   * « Emprunter » (l'ordinateur d'un ami) ou la suite de « Rejoindre avec un code » (un code de prêt) : le passage
   * s'ouvre sur « Préparation de l'ordinateur » pendant que la Station passe en mode console (30 s au plus), sauf
   * ordinateur occupé (l'hôte doit accepter : « En attente de {hôte}… »). Pendant une autre session, la feuille
   * `preparing` montre les mêmes étapes jusqu'à la fenêtre de connexion. Sans mot de passe retenu, il est demandé.
   */
  async function borrow(source: { friend?: string; ticket?: number; password?: string; remember?: boolean }, host: string, busy = false) {
    error = null
    const friend = friends.find((f) => f.id === source.friend)
    if (friend && friend.state === 'password' && !source.password) {
      lendSheet = { kind: 'password', friend }
      return
    }
    lendSheet = null
    const following = sessions.length > 0 || passage !== null
    const pendingId = 'lend-preparing'
    if (!following && !busy)
      passage = { id: pendingId, phase: 'preparing', opening: false, closing: false, restarting: false, many: false, borrow: { title: host ? t('lend.computer_of', { host }) : '', fingerprint: null }, borrowing: true }
    else if (!busy) lendSheet = { kind: 'preparing', host }
    const preparing = lendSheet
    try {
      const r = await invoke<BorrowOutcome>('lend_borrow', { friend: source.friend ?? null, ticket: source.ticket ?? null, password: source.password ?? null, remember: source.remember ?? null })
      if (r.kind === 'ready') return loanReady(r)
      if (passage?.id === pendingId) passage = null
      lendSheet = { kind: 'waiting', wait: r.wait, host: r.host }
    } catch (e) {
      if (passage?.id === pendingId) passage = null
      if (preparing && lendSheet === preparing) lendSheet = null
      error = String(e)
    }
  }

  /** Le prêt est accordé : sa session s'ouvre comme celle d'un ordinateur (passage, ou fenêtre pendant une autre session). */
  async function loanReady(j: LoanReady) {
    lendSheet = null
    error = null
    const following = sessions.length > 0 || (passage !== null && passage.id !== 'lend-preparing')
    const fresh = () => ({ id: j.id, phase: 'preparing' as const, opening: false, closing: false, restarting: false, many: false, borrow: { title: j.title, fingerprint: j.fingerprint } })
    if (!following) passage = fresh()
    else if (passage?.id === 'lend-preparing') passage = null
    try {
      const started = await invoke<ConnectInfo>('lend_connect', { id: j.id })
      if (started.windowed) {
        if (passage?.id === j.id && !passage.opening) passage = null
      } else if (!(passage?.id === j.id && !passage.closing)) {
        passage = fresh()
      }
    } catch (e) {
      if (passage?.id === j.id && !passage.opening) passage = null
      error = String(e)
    }
  }

  /** « Revenir » (carte de fin) : la même demande, pendant 15 min. */
  async function comeBack() {
    const host = lendSheet?.kind === 'end' ? lendSheet.end.host : ''
    lendSheet = null
    error = null
    try {
      const r = await invoke<BorrowOutcome>('lend_come_back')
      if (r.kind === 'ready') loanReady(r)
      else lendSheet = { kind: 'waiting', wait: r.wait, host: r.host || host }
    } catch (e) {
      error = String(e)
    }
  }

  /** « Retirer » l'ordinateur d'un ami de cet appareil. */
  async function removeFriend(id: string) {
    try {
      await invoke('lend_remove', { id })
      if (selectedId === id) selectedId = computers[0]?.id ?? null
    } catch (e) {
      error = String(e)
    }
  }

  /** « Reprendre l'ordinateur » (préavis de 2 min) ou « Reprendre tout de suite », depuis l'accueil de l'hôte. */
  async function takeBack(id: string, now: boolean) {
    error = null
    try {
      await invoke('lend_take_back', { id, now })
    } catch (e) {
      error = String(e)
    }
  }

  // MARK: - Accueil et compte KyberNow

  function refreshWelcome() {
    invoke<WelcomeView>('welcome_state')
      .then((w) => (welcome = w))
      .catch(() => {})
  }

  /** « Passer », « Plus tard », un compte connecté, « Ajouter » ou « Rejoindre avec un code » : l'accueil est vu. */
  function finishWelcome() {
    welcomeOpen = false
    invoke<WelcomeView>('welcome_done')
      .then((w) => (welcome = w))
      .catch(() => {})
  }

  /** « Créer un compte » (carte, « Hors de chez vous ? ») : la page d'inscription dans le navigateur, puis l'avis. */
  async function signUp() {
    error = null
    try {
      await invoke('welcome_sign_up')
      notice = t('welcome.sign_up_notice')
    } catch (e) {
      error = String(e)
    }
  }

  /** « Connecter un compte » (barre de titre, carte, bandeaux, « Hors de chez vous ? ») : le relais public. */
  async function connectAccount() {
    awayPrompt = null
    const e = await signIn()
    if (e) error = e
  }

  async function closeCard() {
    try {
      welcome = await invoke<WelcomeView>('welcome_close_card')
    } catch {}
  }

  /** Un ordinateur ajouté (appairage, invitation) : le bandeau, une fois, sans compte (voir l'effet ci-dessous). */
  function pairedOnce(id: string) {
    if (accounts.length === 0) pairedPending = id
  }

  /**
   * Un seul bandeau à la fois sous la rangée « Se connecter » : la mise à jour de la Station passe d'abord. Le bandeau
   * après un appairage n'est demandé (et compté comme montré) que quand il peut vraiment se montrer : l'ordinateur
   * ajouté choisi sur l'accueil, sans bandeau de sa Station.
   */
  $effect(() => {
    const id = pairedPending
    if (!id || accounts.length > 0 || welcomeOpen || selected?.id !== id || stationBannerFor(selected, sessions)) return
    pairedPending = null
    invoke<boolean>('welcome_paired_hint', { id, stationBanner: false })
      .then((show) => show && (pairedHint = id))
      .catch(() => {})
  })

  /**
   * « Réessayer » ou « Se connecter » laisse hors ligne un ordinateur appairé que le relais ne tient pas (pas « Hors de
   * chez vous ») : « Hors de chez vous ? », une fois par jour au plus, sans compte.
   */
  function awayOnce(id: string) {
    const c = computers.find((x) => x.id === id)
    if (!c || c.online !== false || c.away || c.account || accounts.length > 0) return
    invoke<boolean>('welcome_away_hint', { id })
      .then((show) => show && (awayPrompt = { id, name: c.name }))
      .catch(() => {})
  }

  // MARK: - Comptes KyberNow

  /**
   * « Se connecter à un compte KyberNow » (accueil : le relais public) ou « Ajouter un compte » (le relais choisi) :
   * la page du relais s'ouvre dans le navigateur, « Connexion au compte… » jusqu'à son retour. Rend le refus (adresse du
   * relais illisible, navigateur qui ne s'ouvre pas), ou `null`.
   */
  async function signIn(relay: string | null = null): Promise<string | null> {
    error = null
    signInError = null
    try {
      await invoke('account_sign_in', { relay })
      signingIn = true
      return null
    } catch (e) {
      signingIn = false
      return String(e)
    }
  }

  /** « Annuler » pendant « Connexion au compte… » (la page fermée sans revenir). */
  function cancelSignIn() {
    signingIn = false
    invoke('account_sign_in_cancel').catch(() => {})
  }

  async function installUpdate() {
    progress = 0
    try {
      await invoke('install_update')
    } catch (e) {
      progress = null
      error = String(e)
    }
  }

  async function connect(id: string) {
    error = null
    // Pendant une session, l'ordinateur s'ouvre dans sa propre fenêtre (plusieurs ordinateurs) :
    // pas de passage sur l'accueil. L'app tranche (`windowed`).
    const following = sessions.length > 0 || passage !== null
    const fresh = () => ({ id, phase: 'preparing' as const, opening: false, closing: false, restarting: false, many: false })
    if (!following) passage = fresh()
    try {
      const started = await invoke<ConnectInfo>('connect', { id })
      if (started.windowed) {
        if (passage?.id === id && !passage.opening) passage = null
      } else if (passage?.id === id && !passage.closing) {
        passage.many = started.screens > 1
      } else {
        // La première session après tout (l'accueil croyait une session ouverte, ou le passage
        // précédent se refermait encore) : son passage.
        passage = { ...fresh(), many: started.screens > 1 }
      }
    } catch (e) {
      if (passage?.id === id && !passage.opening) passage = null
      error = t('app.connect_failed', { error: String(e) })
      awayOnce(id)
    }
  }

  /** « Afficher la session » : la fenêtre de cet ordinateur passe devant (plusieurs ordinateurs). */
  async function show(id: string) {
    try {
      await invoke('show_session', { id })
    } catch (e) {
      error = String(e)
    }
  }

  /** Hors ligne : « Réessayer » interroge l'ordinateur tout de suite. */
  async function retry(id: string) {
    error = null
    retrying = id
    try {
      await invoke('poll_now', { id })
      awayOnce(id)
    } catch (e) {
      error = String(e)
    } finally {
      retrying = null
    }
  }

  /**
   * Hors ligne, cartes réseau connues : le paquet de réveil. L'app sonde ensuite l'ordinateur
   * jusqu'à son retour (2 min au plus) et le dit par `waking`, fenêtre ouverte ou non.
   */
  async function wake(id: string) {
    error = null
    try {
      await invoke('wake', { id })
    } catch (e) {
      error = String(e)
    }
  }

  async function setBattery(b: Battery) {
    battery = b
    battery = await invoke<Battery>('set_battery', { battery: b })
  }

  /**
   * Comme le menu de l'icône : retouches effacées, appliqué tout de suite pendant sa session. Auto sans test gardé :
   * le panneau du test (ou à la fin de la session de cet ordinateur).
   */
  async function setProfile(id: string, profile: Computer['settings']['profile']) {
    try {
      const chosen = await invoke<Chosen>('set_profile', { id, profile })
      if (chosen === 'test') autotestId = id
    } catch (e) {
      error = String(e)
    }
  }

  /** « Refaire le test » : le panneau, ou à la fin de la session si cet ordinateur est en session. */
  function runAutotest(id: string) {
    // En session : l'avis « Le test se fera à la fin de la session. » (barre de la session, sinon notification).
    if (sessions.includes(id)) invoke('autotest_later', { id, told: true }).catch(() => {})
    else autotestId = id
  }

  function added(id: string) {
    adding = null
    select(id)
    pairedOnce(id)
  }

  // MARK: - Consoles PlayStation

  /**
   * « Jouer », « Réveiller et jouer » : la partie s'ouvre comme une session d'ordinateur — la première avec le
   * passage de l'accueil, les suivantes dans leur fenêtre (l'app tranche, `windowed`). Une console qui dort
   * commence par son réveil.
   */
  async function play(id: string) {
    error = null
    const c = ps.consoles.find((x) => x.id === id)
    const following = sessions.length > 0 || passage !== null
    const fresh = () => ({
      id,
      phase: (c?.status === 'standby' ? 'preparing' : 'connecting') as Phase['phase'],
      opening: false,
      closing: false,
      restarting: false,
      many: false,
      console: { name: c?.name ?? '', wakes: c?.status === 'standby' },
    })
    if (!following) passage = fresh()
    try {
      const started = await invoke<{ windowed: boolean }>('ps_play', { id })
      if (started.windowed) {
        if (passage?.id === id && !passage.opening) passage = null
      } else if (!(passage?.id === id && !passage.closing)) {
        passage = fresh()
      }
    } catch (e) {
      if (passage?.id === id && !passage.opening) passage = null
      error = String(e)
    }
  }

  /** « Associer » : la feuille de l'association ; depuis l'ajout, l'ajout se ferme d'abord (comme sur Mac). */
  function pair(found: PsFound) {
    error = null
    adding = null
    pairing = { found }
  }

  /** « Réessayer » (console hors ligne) : une recherche tout de suite. */
  async function psSearch() {
    error = null
    psSearching = true
    try {
      ps = await invoke<PsState>('ps_search_now')
    } catch (e) {
      error = String(e)
    } finally {
      psSearching = false
    }
  }

  async function chooseAccount(id: string, account: string) {
    try {
      await invoke('ps_choose_account', { id, account })
    } catch (e) {
      error = String(e)
    }
  }
</script>

<TitleBar
  title={settingsFor || consoleSettingsFor ? t('app.title_settings') : 'KyberNow Link'}
  onback={settingsFor ? () => (settingsId = null) : consoleSettingsFor ? () => (consoleSettingsId = null) : undefined}
  onapp={() => ((appCheck = false), (appSheet = true))}
  badge={update !== null}
  onaccount={accounts.length === 0 && welcome && !welcomeOpen && !notice && !settingsFor && !consoleSettingsFor ? connectAccount : undefined}
  {signingIn}
  crowded={update !== null && !passage}
/>

{#if (update || notice) && !settingsFor && !consoleSettingsFor && !passage}
  <div class="banner" role="status">
    <!-- Un avis bref (« Vous avez rendu l'ordinateur. ») passe avant la mise à jour, qui revient après « OK ». -->
    {#if notice}
      <span>{notice}</span>
      <button class="more" onclick={() => (notice = null)}>{t('common.ok')}</button>
    {:else if update}
      <span>{t('app.update_ready', { version: update.version })}</span>
      {#if progress !== null}<span class="mono">{progress} %</span>{:else}<button onclick={installUpdate}>{t('app.install_relaunch')}</button>{/if}
      <button class="more" onclick={() => ((appCheck = false), (appSheet = true))}>{t('app.whats_new')}</button>
    {/if}
  </div>
{/if}

{#if settingsFor}
  {#key settingsFor.id}
    <Settings
      computer={settingsFor}
      {thisPc}
      {battery}
      start={settingsRubric}
      accountName={settingsFor.account ? (accounts.find((a) => a.key === settingsFor!.account!.account)?.name ?? null) : null}
      applied={autoApplied?.id === settingsFor.id ? autoApplied.settings : null}
      inSession={sessions.includes(settingsFor.id)}
      onprofile={setProfile}
      onautotest={runAutotest}
      onremoved={() => (settingsId = null)}
    />
  {/key}
{:else if consoleSettingsFor}
  {#key consoleSettingsFor.id}
    <ConsoleSettings console={consoleSettingsFor} onremoved={() => ((consoleSettingsId = null), (selectedId = null))} />
  {/key}
{:else}
  <Home
    {computers}
    {discovered}
    {thisPc}
    {battery}
    {selected}
    {selectedConsole}
    consoles={ps.consoles}
    psFound={ps.found}
    {psSearching}
    onplay={play}
    onpair={pair}
    onconsolesettings={(id) => (consoleSettingsId = id)}
    onchooseaccount={chooseAccount}
    onpssearch={psSearch}
    {friends}
    {selectedFriend}
    {sessions}
    {accounts}
    {signingIn}
    onsignin={async () => {
      const e = await signIn()
      if (e) error = e
    }}
    oncancelsignin={cancelSignIn}
    {welcome}
    {pairedHint}
    onsignup={signUp}
    onclosecard={closeCard}
    onpairedlater={() => (pairedHint = null)}
    busy={passage !== null && !passage.opening}
    retrying={retrying !== null && retrying === selected?.id}
    {error}
    onselect={select}
    onconnect={connect}
    onwatch={watch}
    onshow={show}
    onretry={retry}
    onwake={wake}
    onsettings={(id) => (settingsId = id)}
    onprofile={setProfile}
    onadd={(found) => ((addingWay = null), (adding = { found: found ?? null }))}
    onjoin={() => join()}
    onborrow={(f) => borrow({ friend: f.id }, f.owner, f.state === 'busy')}
    onremovefriend={removeFriend}
    ontakeback={takeBack}
  />
{/if}

{#if joining}
  <JoinSheet
    code={joining.code}
    link={joining.link}
    onclose={() => (joining = null)}
    onjoined={joined}
    onborrow={(ticket) => {
      joining = null
      borrow({ ticket }, '')
    }}
    oninvite={(ticket, computer) => {
      joining = null
      inviting = { ticket, computer }
    }}
  />
{/if}

{#if inviting}
  <InviteSheet
    ticket={inviting.ticket}
    computer={inviting.computer}
    onclose={() => (inviting = null)}
    onadded={(added) => {
      inviting = null
      settingsId = null
      consoleSettingsId = null
      select(added.id)
      notice = t('invite.added', { name: added.name })
      pairedOnce(added.id)
    }}
  />
{/if}

{#if lendSheet}
  {#key lendSheet}
    <LendSheet
      sheet={lendSheet}
      onclose={() => (lendSheet = null)}
      onadded={(id) => {
        lendSheet = null
        select(id)
      }}
      onpassword={(f, password, remember) => borrow({ friend: f.id, password, remember }, f.owner, f.state === 'busy')}
      onready={loanReady}
      oncomeback={comeBack}
    />
  {/key}
{/if}

{#if adding}
  <AddSheet
    {discovered}
    {computers}
    {ps}
    deviceName={thisPc?.name ?? ''}
    initial={adding.found}
    initialWay={addingWay}
    already={psAlready}
    onclose={() => (adding = null)}
    onadded={added}
    onpair={pair}
    onaddaccount={() => ((psAlready = false), (pairing = { found: null }))}
  />
{/if}

{#if pairing}
  <PairSheet
    target={pairing.found}
    {ps}
    onclose={() => (pairing = null)}
    onaccount={(existed) => (psAlready = existed)}
    onpaired={(id) => {
      pairing = null
      select(id)
    }}
  />
{/if}

{#if autotestId}
  {@const target = computers.find((c) => c.id === autotestId)}
  {#if target}
    {#key target.id}
      <AutoTest
        computer={target}
        onclose={() => (autotestId = null)}
        onapplied={(settings) => (autoApplied = { id: target.id, settings })}
      />
    {/key}
  {/if}
{/if}

{#if appSheet}
  <AppSheet
    {update}
    {progress}
    {thisPc}
    {battery}
    check={appCheck}
    {accounts}
    {signingIn}
    {signInError}
    onsignin={signIn}
    oncancelsignin={cancelSignIn}
    onbattery={setBattery}
    onclose={() => (appSheet = false)}
    oninstall={installUpdate}
    onwelcome={() => ((appSheet = false), (error = null), (welcomeOpen = true))}
  />
{/if}

{#if welcomeOpen && !passage}
  <Welcome
    {computers}
    {discovered}
    relay={welcome?.relay ?? 'share.kybernow.app'}
    {signingIn}
    error={signInError}
    onsignin={connectAccount}
    oncancelsignin={cancelSignIn}
    onadd={(found) => {
      finishWelcome()
      addingWay = null
      adding = { found }
    }}
    onjoin={() => {
      finishWelcome()
      join()
    }}
    ondone={finishWelcome}
  />
{/if}

{#if awayPrompt}
  <AwayPrompt name={awayPrompt.name} onsignin={connectAccount} onsignup={() => ((awayPrompt = null), signUp())} onclose={() => (awayPrompt = null)} />
{/if}

{#if passage && passageFor}
  <Passage
    computer={passageFor}
    phase={passage.phase}
    opening={passage.opening}
    closing={passage.closing}
    restarting={passage.restarting}
    many={passage.many}
    guest={!!passage.guest}
    borrow={!!passage.borrow}
    cancellable={!passage.borrowing}
    ps={!!passage.console}
    wakes={passage.console?.wakes ?? false}
    oncancel={() => passage && invoke('session_command', { id: passage.id, command: 'disconnect' })}
  />
{/if}

<!-- L'avis après une mise à jour de l'app : sur l'accueil, jamais en session (il attend la fin de celle-ci). -->
{#if stationNotice && sessions.length === 0 && !passage}
  <StationNotice notice={stationNotice} onclose={() => (stationNotice = null)} />
{/if}

<style>
  /* Dans la barre de titre, au milieu : rien ne recouvre l'accueil. */
  .banner {
    position: absolute;
    top: 3px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 25;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 3px 3px 3px 16px;
    border: 1px solid var(--signal);
    border-radius: 999px;
    background: rgb(9 13 18 / 0.92);
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.4), 0 0 18px rgb(109 91 255 / 0.25);
    font-size: 13px;
    white-space: nowrap;
  }

  .banner button {
    height: 28px;
    padding: 0 14px;
    border: 0;
    border-radius: 999px;
    background: var(--signal-strong);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
  }

  .banner .more {
    background: none;
    color: var(--muted);
    font-weight: 400;
  }
</style>
