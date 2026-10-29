<!-- L'app : accueil, ajout, réglages, passage vers la session. -->
<script lang="ts">
  import { onMount } from 'svelte'
  import TitleBar from './TitleBar.svelte'
  import Home from './Home.svelte'
  import AddSheet from './AddSheet.svelte'
  import Settings from './Settings.svelte'
  import Passage from './Passage.svelte'
  import AppSheet from './AppSheet.svelte'
  import JoinSheet from './JoinSheet.svelte'
  import LendSheet from './LendSheet.svelte'
  import type { Action, AppInfo, Battery, BorrowOutcome, Candidate, Computer, ConnectInfo, Found, Friend, LendEnd, LendInvite, LendSheetKind, LoanReady, Phase, ScreensEvent, ShareJoined, Snapshot, StationRestarting, Summary, ThisPc, Update } from './lib/api'
  import { MAX_SESSIONS, guestComputer, invoke, listen } from './lib/api'
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
  } | null>(null)
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

  function read(k: string) {
    try {
      return localStorage.getItem(k)
    } catch {
      return null
    }
  }

  /** L'ordinateur d'un ami choisi sur l'accueil (prêt de l'ordinateur). */
  const selectedFriend = $derived(friends.find((f) => f.id === selectedId) ?? (computers.length === 0 ? (friends[0] ?? null) : null))
  const selected = $derived(selectedFriend ? null : (computers.find((c) => c.id === selectedId) ?? computers[0] ?? null))
  const settingsFor = $derived(computers.find((c) => c.id === settingsId) ?? null)
  /** Ce que le passage montre : l'ordinateur, ou « Invité sur {nom} » pour une session d'invité. */
  const passageFor = $derived(
    computers.find((c) => c.id === passage?.id) ??
      (passage?.guest ? guestComputer(passage.guest) : passage?.borrow ? guestComputer(passage.borrow) : null),
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
      listen<Update | null>('update', (u) => (update = u)),
      listen<string[]>('sessions', (s) => (sessions = s)),
      listen<string>('lang', setLang),
      listen<number>('update-progress', (p) => (progress = p)),
      listen<Phase>('session-status', (p) => {
        if (!passage || passage.id !== p.id) return
        // « La Station redémarre… » jusqu'à l'étape suivante (voir StationRestarting).
        passage.restarting = false
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
        // Une session d'emprunt : sa carte de fin (texte du prêt, ou « Revenir » pendant 15 min).
        if (lend) lendSheet = { kind: 'end', end: lend }
        // Seulement la session du passage : une session suivante montre son échec dans
        // sa propre fenêtre de connexion, ou par une notification.
        if (!passage || passage.id !== id) return
        // Avant la première image, c'est la connexion qui a échoué.
        const failed = !passage.opening
        // Un invité : la raison du partage, telle quelle (« Léa a arrêté le partage. »).
        error = summary.error
          ? passage.guest || passage.borrow
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
    Promise.all(off).then(() =>
      invoke<Snapshot>('snapshot').then((s) => {
        computers = s.computers
        discovered = s.discovered
        thisPc = s.this_pc ?? thisPc
        sessions = s.sessions ?? []
      })
        .then(() => invoke<Friend[]>('lend_friends').then((f) => (friends = f)))
        .then(() =>
          invoke<AppInfo>('app_info').then((i) => {
            update = i.update
            battery = i.battery ?? battery
            setLang(i.lang)
          }),
        )
        .then(() => invoke<Action | null>('take_action'))
        .then((a) => a && handle(a)),
    )
    return () => off.forEach((p) => p.then((f) => f()))
  })

  /** Demande venue de l'extérieur : lien, barre des tâches, notification, menu de l'icône. */
  async function handle(a: Action) {
    settingsId = null
    settingsRubric = null
    if (a.kind === 'show') {
      if (computers.some((c) => c.id === a.id)) select(a.id)
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
    } else if (a.kind === 'connect') {
      if (!computers.some((c) => c.id === a.id)) return
      select(a.id)
      connect(a.id)
    } else if (a.kind === 'add') {
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
  async function joined(j: ShareJoined) {
    joining = null
    error = null
    const following = sessions.length > 0 || passage !== null
    const fresh = () => ({ id: j.id, phase: 'preparing' as const, opening: false, closing: false, restarting: false, many: false, guest: j })
    if (!following) passage = fresh()
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

  // MARK: - Prêt de l'ordinateur, côté ami

  /**
   * « Emprunter » (l'ordinateur d'un ami) ou la suite de « Rejoindre avec un code » (un code de prêt) : le passage
   * s'ouvre sur « Préparation de l'ordinateur » pendant que la Station passe en mode console (30 s au plus), sauf
   * ordinateur occupé (l'hôte doit accepter : « En attente de {hôte}… »). Sans mot de passe retenu, il est demandé.
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
    try {
      const r = await invoke<BorrowOutcome>('lend_borrow', { friend: source.friend ?? null, ticket: source.ticket ?? null, password: source.password ?? null, remember: source.remember ?? null })
      if (r.kind === 'ready') return loanReady(r)
      if (passage?.id === pendingId) passage = null
      lendSheet = { kind: 'waiting', wait: r.wait, host: r.host }
    } catch (e) {
      if (passage?.id === pendingId) passage = null
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
    await invoke('lend_remove', { id })
    if (selectedId === id) selectedId = computers[0]?.id ?? null
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

  /** Comme le menu de l'icône : retouches effacées, appliqué tout de suite pendant sa session. */
  async function setProfile(id: string, profile: Computer['settings']['profile']) {
    try {
      await invoke('set_profile', { id, profile })
    } catch (e) {
      error = String(e)
    }
  }

  function added(id: string) {
    adding = null
    select(id)
  }
</script>

<TitleBar
  title={settingsFor ? t('app.title_settings') : 'KyberNow Link'}
  onback={settingsFor ? () => (settingsId = null) : undefined}
  onapp={() => ((appCheck = false), (appSheet = true))}
  badge={update !== null}
/>

{#if (update || notice) && !settingsFor && !passage}
  <div class="banner" role="status">
    {#if update}
      <span>{t('app.update_ready', { version: update.version })}</span>
      {#if progress !== null}<span class="mono">{progress} %</span>{:else}<button onclick={installUpdate}>{t('app.install_relaunch')}</button>{/if}
      <button class="more" onclick={() => ((appCheck = false), (appSheet = true))}>{t('app.whats_new')}</button>
    {:else}
      <span>{notice}</span>
      <button class="more" onclick={() => (notice = null)}>{t('common.ok')}</button>
    {/if}
  </div>
{/if}

{#if settingsFor}
  {#key settingsFor.id}
    <Settings computer={settingsFor} {thisPc} {battery} start={settingsRubric} onremoved={() => (settingsId = null)} />
  {/key}
{:else}
  <Home
    {computers}
    {discovered}
    {thisPc}
    {battery}
    {selected}
    {friends}
    {selectedFriend}
    {sessions}
    busy={passage !== null && !passage.opening}
    retrying={retrying !== null && retrying === selected?.id}
    {error}
    onselect={select}
    onconnect={connect}
    onshow={show}
    onretry={retry}
    onwake={wake}
    onsettings={(id) => (settingsId = id)}
    onprofile={setProfile}
    onadd={(found) => (adding = { found: found ?? null })}
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
    full={sessions.length >= MAX_SESSIONS}
    onclose={() => (joining = null)}
    onjoined={joined}
    onborrow={(ticket) => {
      joining = null
      borrow({ ticket }, '')
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
    deviceName={thisPc?.name ?? ''}
    initial={adding.found}
    onclose={() => (adding = null)}
    onadded={added}
  />
{/if}

{#if appSheet}
  <AppSheet {update} {progress} {thisPc} {battery} check={appCheck} onbattery={setBattery} onclose={() => (appSheet = false)} oninstall={installUpdate} />
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
    oncancel={() => passage && invoke('session_command', { id: passage.id, command: 'disconnect' })}
  />
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
