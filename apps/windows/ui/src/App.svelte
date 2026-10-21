<!-- L'app : accueil, ajout, réglages, passage vers la session. -->
<script lang="ts">
  import { onMount } from 'svelte'
  import TitleBar from './TitleBar.svelte'
  import Home from './Home.svelte'
  import AddSheet from './AddSheet.svelte'
  import Settings from './Settings.svelte'
  import Passage from './Passage.svelte'
  import AppSheet from './AppSheet.svelte'
  import type { Action, AppInfo, Candidate, Computer, Found, Phase, Snapshot, Summary, ThisPc, Update } from './lib/api'
  import { invoke, listen } from './lib/api'

  let computers = $state<Computer[]>([])
  let discovered = $state<Found[]>([])
  let thisPc = $state<ThisPc | null>(null)
  let selectedId = $state<string | null>(read('kybernow.selected'))
  let settingsId = $state<string | null>(null)
  let adding = $state<{ found: Found | null } | null>(null)
  let passage = $state<{ id: string; phase: Phase['phase']; opening: boolean; closing: boolean } | null>(null)
  let error = $state<string | null>(null)
  let appSheet = $state(false)
  let update = $state<Update | null>(null)
  let progress = $state<number | null>(null)
  let notice = $state<string | null>(null)

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
      listen<number>('update-progress', (p) => (progress = p)),
      listen<Phase>('session-status', (p) => {
        if (!passage || passage.id !== p.id) return
        // Connectée mais sans image encore : le passage attend la première,
        // il ne s'ouvre pas sur du noir.
        passage.phase = p.phase === 'streaming' && !passage.opening ? 'first_images' : p.phase
      }),
      listen<string>('session-shown', (id) => {
        if (passage && passage.id === id) passage.opening = true
      }),
      listen<{ id: string; summary: Summary }>('session-ended', ({ summary }) => {
        error = summary.error ? `La session s’est arrêtée : ${summary.error}` : null
        if (!passage) return
        passage.opening = false
        passage.closing = true
        setTimeout(() => (passage = null), 750)
      }),
    ]
    // L'état complet une fois les écoutes en place (rien ne passe entre deux).
    Promise.all(off).then(() =>
      invoke<Snapshot>('snapshot').then((s) => {
        computers = s.computers
        discovered = s.discovered
        thisPc = s.this_pc ?? thisPc
      })
        .then(() => invoke<AppInfo>('app_info').then((i) => (update = i.update)))
        .then(() => invoke<Action | null>('take_action'))
        .then((a) => a && handle(a)),
    )
    return () => off.forEach((p) => p.then((f) => f()))
  })

  /** Demande venue de l'extérieur : lien, barre des tâches, notification. */
  async function handle(a: Action) {
    settingsId = null
    if (a.kind === 'connect') {
      if (passage) return
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
      invoke<AppInfo>('app_info').then((i) => (notice = `KyberNow ${i.version} est installée.`))
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
    passage = { id, phase: 'preparing', opening: false, closing: false }
    try {
      await invoke('connect', { id })
    } catch (e) {
      passage = null
      error = String(e)
    }
  }

  async function setProfile(id: string, profile: Computer['settings']['profile']) {
    const c = computers.find((x) => x.id === id)
    if (!c) return
    await invoke('save_settings', { id, settings: { ...c.settings, profile, codec: null, yuv444: null, mbps: null, reliable: null } })
  }

  function added(id: string) {
    adding = null
    select(id)
  }
</script>

<TitleBar
  title={settingsFor ? 'KyberNow · Réglages' : 'KyberNow'}
  onback={settingsFor ? () => (settingsId = null) : undefined}
  onapp={() => (appSheet = true)}
  badge={update !== null}
/>

{#if (update || notice) && !settingsFor && !passage}
  <div class="banner" role="status">
    {#if update}
      <span>KyberNow {update.version} est prête.</span>
      {#if progress !== null}<span class="mono">{progress} %</span>{:else}<button onclick={installUpdate}>Installer et relancer</button>{/if}
      <button class="more" onclick={() => (appSheet = true)}>Nouveautés</button>
    {:else}
      <span>{notice}</span>
      <button class="more" onclick={() => (notice = null)}>OK</button>
    {/if}
  </div>
{/if}

{#if settingsFor}
  {#key settingsFor.id}
    <Settings computer={settingsFor} {thisPc} onremoved={() => (settingsId = null)} />
  {/key}
{:else}
  <Home
    {computers}
    {discovered}
    {thisPc}
    {selected}
    busy={passage !== null}
    {error}
    onselect={select}
    onconnect={connect}
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
  <AppSheet {update} {progress} onclose={() => (appSheet = false)} oninstall={installUpdate} />
{/if}

{#if passage && passageFor}
  <Passage
    computer={passageFor}
    phase={passage.phase}
    opening={passage.opening}
    closing={passage.closing}
    oncancel={() => invoke('session_command', { command: 'disconnect' })}
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
