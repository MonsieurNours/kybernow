<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { api, setUnauthorizedHandler } from './lib/api'
  import { app, refreshStatus, notify, errorText } from './lib/store.svelte'
  import Login from './pages/Login.svelte'
  import Overview from './pages/Overview.svelte'
  import ImageSound from './pages/ImageSound.svelte'
  import Access from './pages/Access.svelte'
  import System from './pages/System.svelte'
  import Mark from './lib/Mark.svelte'
  import Icon from './lib/Icon.svelte'

  // Quatre espaces, par intention.
  const spaces = [
    { id: 'ensemble', label: "Vue d'ensemble", short: 'Ensemble', icon: 'overview', component: Overview },
    { id: 'image-son', label: 'Image et son', short: 'Image et son', icon: 'image', component: ImageSound },
    { id: 'acces', label: 'Accès', short: 'Accès', icon: 'key', component: Access },
    { id: 'systeme', label: 'Système', short: 'Système', icon: 'gear', component: System },
  ]
  // Anciennes adresses (console 0.2) : vers l'espace qui les reprend.
  const legacy: Record<string, string> = {
    dashboard: 'ensemble',
    display: 'image-son',
    audio: 'image-son',
    settings: 'systeme/avance',
    devices: 'acces',
    users: 'acces/comptes',
    updates: 'systeme',
    logs: 'systeme/journaux',
    advanced: 'systeme/avance',
  }

  let session = $state<{ configured: boolean; authenticated: boolean; setup_code_file: string; hostname?: string | null } | null>(null)
  let hash = $state(location.hash.slice(2))
  let restarting = $state(false)

  const parts = $derived((legacy[hash] ?? hash).split('/'))
  const current = $derived(spaces.find((s) => s.id === parts[0]) ?? spaces[0])
  const section = $derived(parts[1] ?? '')

  const status = $derived(app.status)
  const live = $derived(status?.activity?.live ?? null)
  const serviceState = $derived(status?.service.state)
  const hostLine = $derived(
    !status
      ? ''
      : serviceState === 'running'
        ? live
          ? `En session · ${live.name}`
          : 'En marche · prêt à recevoir'
        : serviceState === 'starting'
          ? 'Démarrage…'
          : serviceState === 'stopped'
            ? 'Serveur arrêté'
            : 'Serveur introuvable',
  )
  const hostDot = $derived(serviceState === 'running' ? (live ? 'live' : 'ok') : serviceState === 'starting' ? 'warn' : 'bad')
  const updateBadge = $derived(status?.update_available?.split('.').slice(0, 2).join('.') ?? null)
  // Un appareil demande l'accès par code (Apple TV) : signalé partout.
  const pairRequests = $derived(status?.pair_requests ?? [])

  async function loadSession() {
    session = await api.session()
  }

  onMount(() => {
    setUnauthorizedHandler(() => {
      if (session) session.authenticated = false
    })
    loadSession()
    const onHash = () => (hash = location.hash.slice(2))
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  })

  // Une section demandée (#/systeme/journaux) : on la montre.
  $effect(() => {
    const id = section
    app.section = id
    const space = current.id
    tick().then(() => {
      if (id) document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
      else window.scrollTo({ top: 0 })
    })
    void space
  })

  // État du serveur tant que l'on est connecté.
  $effect(() => {
    if (!session?.authenticated) return
    refreshStatus()
    const timer = setInterval(refreshStatus, 3000)
    return () => clearInterval(timer)
  })

  async function restartKyber() {
    restarting = true
    try {
      await api.service('restart')
      notify('ok', 'Le serveur a redémarré avec les nouveaux réglages.')
      await refreshStatus()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      restarting = false
    }
  }

  async function logout() {
    await api.logout().catch(() => {})
    await loadSession()
  }
</script>

{#if !session}
  <div class="loading"><Mark size={34} lit breathe /></div>
{:else if !session.authenticated}
  <Login configured={session.configured} setupCodeFile={session.setup_code_file} hostname={session.hostname ?? null} onDone={loadSession} />
{:else}
  <div class="shell">
    <aside>
      <div class="brand">
        <Mark size={26} lit={!!live} breathe={!!live} />
        <span class="disp wordmark"><span>Kyber</span><b>Now</b></span>
      </div>

      <div class="host">
        <span class="name">{status?.hostname ?? 'Cet ordinateur'}</span>
        <span class="state"><span class="dot {hostDot}"></span>{hostLine}</span>
      </div>

      <nav aria-label="Espaces">
        {#each spaces as s (s.id)}
          <a href="#/{s.id}" class:current={s.id === current.id} aria-current={s.id === current.id ? 'page' : undefined}>
            <Icon name={s.icon} />
            {s.label}
            {#if s.id === 'systeme' && updateBadge}<span class="badge">{updateBadge}</span>{/if}
            {#if s.id === 'acces' && pairRequests.length}<span class="badge" aria-label="{pairRequests.length} demande{pairRequests.length > 1 ? 's' : ''} d'accès">{pairRequests.length}</span>{/if}
          </a>
        {/each}
      </nav>

      <div class="who">
        <span class="avatar" aria-hidden="true">A</span>
        Administrateur
        <button type="button" class="link" onclick={logout}>Se déconnecter</button>
      </div>
    </aside>

    <header class="mobile-top">
      <Mark size={22} lit={!!live} />
      <span class="name">{status?.hostname ?? ''}</span>
      <span class="state"><span class="dot {hostDot}"></span>{serviceState === 'running' ? (live ? 'En session' : 'En marche') : hostLine}</span>
    </header>

    <main>
      {#if status?.restart_pending}
        <div class="restart notice warn" role="status">
          <Icon name="restart" size={18} />
          <span>Réglages modifiés : redémarrez le serveur pour les appliquer. Un appareil connecté sera déconnecté quelques secondes.</span>
          <button type="button" class="ghost small" disabled={restarting} onclick={restartKyber}>
            {restarting ? 'Redémarrage…' : 'Redémarrer le serveur'}
          </button>
        </div>
      {/if}
      {#if pairRequests.length && current.id !== 'acces'}
        <div class="restart notice pair" role="status">
          <Icon name={pairRequests[0].kind === 'tv' ? 'tv' : 'key'} size={18} />
          <span>
            {pairRequests[0].name} demande l'accès à cet ordinateur, avec le code
            <b class="mono">{pairRequests[0].code.slice(0, 3)} {pairRequests[0].code.slice(3)}</b>.{pairRequests.length > 1 ? ` Et ${pairRequests.length - 1} autre${pairRequests.length > 2 ? 's' : ''}.` : ''}
          </span>
          <a class="ghost small" href="#/acces/appareils">Voir la demande</a>
        </div>
      {/if}
      {#key current.id}
        <current.component />
      {/key}
    </main>

    <nav class="tabbar" aria-label="Espaces">
      {#each spaces as s (s.id)}
        <a href="#/{s.id}" class:current={s.id === current.id} aria-current={s.id === current.id ? 'page' : undefined}>
          <Icon name={s.icon} size={22} />
          {s.short}
          {#if s.id === 'systeme' && updateBadge}<span class="pip" aria-label="Mise à jour disponible"></span>{/if}
          {#if s.id === 'acces' && pairRequests.length}<span class="pip" aria-label="Demande d'accès"></span>{/if}
        </a>
      {/each}
    </nav>
  </div>
{/if}

{#if app.toast}
  <div class="toast {app.toast.kind}" role="status">{app.toast.text}</div>
{/if}

<style>
  .loading {
    min-height: 100vh;
    display: grid;
    place-items: center;
  }

  .shell {
    display: grid;
    grid-template-columns: 264px minmax(0, 1fr);
    min-height: 100vh;
  }

  aside {
    position: sticky;
    top: 0;
    height: 100vh;
    padding: 22px 16px;
    border-right: 1px solid var(--line);
    background: var(--ink-1);
    display: flex;
    flex-direction: column;
    gap: 26px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 0 8px;
  }

  .wordmark {
    font-size: 16px;
    letter-spacing: -0.01em;
  }

  .wordmark span {
    font-weight: 420;
  }

  .wordmark b {
    font-weight: 720;
  }

  .host {
    padding: 14px;
    border-radius: 12px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--line);
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .host .name {
    font-size: 15px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .state {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-2);
    min-width: 0;
  }

  nav[aria-label='Espaces']:not(.tabbar) {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  aside nav a {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 9px;
    font-size: 14px;
    color: var(--muted);
    text-decoration: none;
    transition: background-color 0.2s, color 0.2s;
  }

  aside nav a:hover {
    background: rgb(255 255 255 / 0.04);
    color: var(--text);
  }

  aside nav a.current {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.4);
    color: #fff;
  }

  aside nav a.current :global(svg) {
    color: var(--signal-light);
  }

  .badge {
    margin-left: auto;
    padding: 2px 7px;
    border-radius: 999px;
    background: rgb(154 140 255 / 0.16);
    color: var(--signal-light);
    font-size: 11px;
    font-weight: 600;
  }

  .who {
    margin-top: auto;
    padding: 0 8px;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--muted);
  }

  .avatar {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    display: grid;
    place-items: center;
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
  }

  .link {
    margin-left: auto;
    white-space: nowrap;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 12.5px;
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .link:hover {
    color: var(--text);
  }

  main {
    min-width: 0;
    position: relative;
  }

  .restart {
    margin: 16px 40px 0;
    max-width: 1040px;
  }

  .notice.pair {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--signal-light);
  }

  .notice.pair b {
    color: var(--neon-core);
    letter-spacing: 0.06em;
  }

  .notice.pair a {
    display: inline-flex;
    align-items: center;
    text-decoration: none;
  }

  .mobile-top,
  .tabbar {
    display: none;
  }

  .toast {
    position: fixed;
    bottom: 24px;
    right: 24px;
    max-width: 420px;
    padding: 12px 16px;
    border-radius: 12px;
    background: rgb(14 20 26 / 0.92);
    backdrop-filter: blur(20px);
    box-shadow: inset 0 0 0 1px var(--line-2), 0 18px 40px rgb(0 0 0 / 0.5);
    font-size: 13.5px;
    color: var(--text);
    z-index: 20;
    animation: rise 0.35s var(--ease-out) both;
  }

  .toast.error {
    box-shadow: inset 0 0 0 1px var(--bad-line), 0 18px 40px rgb(0 0 0 / 0.5);
    color: var(--bad-text);
  }

  @media (max-width: 760px) {
    .shell {
      grid-template-columns: 1fr;
    }

    aside {
      display: none;
    }

    .mobile-top {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 16px 20px 6px;
    }

    .mobile-top .name {
      font-size: 15px;
      font-weight: 600;
    }

    .mobile-top .state {
      margin-left: auto;
      font-size: 12px;
    }

    .restart {
      margin: 12px 16px 0;
    }

    .tabbar {
      display: flex;
      position: fixed;
      left: 0;
      right: 0;
      bottom: 0;
      z-index: 10;
      padding: 6px 8px calc(10px + env(safe-area-inset-bottom));
      border-top: 1px solid rgb(255 255 255 / 0.08);
      background: rgb(9 13 18 / 0.92);
      backdrop-filter: blur(20px);
    }

    .tabbar a {
      position: relative;
      flex: 1;
      min-height: 56px;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 4px;
      font-size: 10.5px;
      color: var(--faint);
      text-decoration: none;
    }

    .tabbar a.current {
      color: #fff;
    }

    .tabbar a.current :global(svg) {
      color: var(--signal-light);
      filter: drop-shadow(0 0 4px var(--signal));
    }

    .pip {
      position: absolute;
      top: 8px;
      left: 56%;
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--signal);
      box-shadow: 0 0 6px var(--signal);
    }

    .toast {
      left: 16px;
      right: 16px;
      bottom: 96px;
    }
  }
</style>
