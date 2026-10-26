<!-- L'app : accueil, ajout, réglages, passage vers la session. -->
<script lang="ts">
  import { onMount } from 'svelte'
  import TitleBar from './TitleBar.svelte'
  import Home from './Home.svelte'
  import AddSheet from './AddSheet.svelte'
  import Settings from './Settings.svelte'
  import Passage from './Passage.svelte'
  import AppSheet from './AppSheet.svelte'
  import type { Action, AppInfo, Battery, Candidate, Computer, ConnectInfo, Found, Phase, ScreensEvent, Snapshot, StationRestarting, Summary, ThisPc, Update } from './lib/api'
  import { invoke, listen } from './lib/api'
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
  let passage = $state<{ id: string; phase: Phase['phase']; opening: boolean; closing: boolean; restarting: boolean; many: boolean } | null>(null)
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

  const selected = $derived(computers.find((c) => c.id === selectedId) ?? computers[0] ?? null)
  const settingsFor = $derived(computers.find((c) => c.id === settingsId) ?? null)
  const passageFor = $derived(computers.find((c) => c.id === passage?.id) ?? null)

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
      listen<{ id: string; summary: Summary }>('session-ended', ({ id, summary }) => {
        // Seulement la session du passage : une session suivante montre son échec dans
        // sa propre fenêtre de connexion, ou par une notification.
        if (!passage || passage.id !== id) return
        // Avant la première image, c'est la connexion qui a échoué.
        const failed = !passage.opening
        error = summary.error ? t(failed ? 'app.connect_failed' : 'app.session_stopped', { error: summary.error }) : null
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
  />
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
