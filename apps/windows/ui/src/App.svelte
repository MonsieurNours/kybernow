<!-- L'app : accueil, ajout, réglages, passage vers la session. -->
<script lang="ts">
  import { onMount } from 'svelte'
  import TitleBar from './TitleBar.svelte'
  import Home from './Home.svelte'
  import AddSheet from './AddSheet.svelte'
  import Settings from './Settings.svelte'
  import Passage from './Passage.svelte'
  import type { Computer, Found, Phase, Snapshot, Summary, ThisPc } from './lib/api'
  import { invoke, listen } from './lib/api'

  let computers = $state<Computer[]>([])
  let discovered = $state<Found[]>([])
  let thisPc = $state<ThisPc | null>(null)
  let selectedId = $state<string | null>(read('kybernow.selected'))
  let settingsId = $state<string | null>(null)
  let adding = $state<{ found: Found | null } | null>(null)
  let passage = $state<{ id: string; phase: Phase['phase']; opening: boolean; closing: boolean } | null>(null)
  let error = $state<string | null>(null)

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
      listen<Phase>('session-status', (p) => {
        if (!passage || passage.id !== p.id) return
        passage.phase = p.phase
        if (p.phase === 'streaming') passage.opening = true
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
      }),
    )
    return () => off.forEach((p) => p.then((f) => f()))
  })

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

<TitleBar title={settingsFor ? 'KyberNow · Réglages' : 'KyberNow'} onback={settingsFor ? () => (settingsId = null) : undefined} />

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

{#if passage && passageFor}
  <Passage
    computer={passageFor}
    phase={passage.phase}
    opening={passage.opening}
    closing={passage.closing}
    oncancel={() => invoke('session_command', { command: 'disconnect' })}
  />
{/if}
