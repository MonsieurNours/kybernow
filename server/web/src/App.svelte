<script lang="ts">
  import { onMount } from 'svelte'
  import { api, setUnauthorizedHandler } from './lib/api'
  import { app, refreshStatus, notify, errorText } from './lib/store.svelte'
  import Login from './pages/Login.svelte'
  import Dashboard from './pages/Dashboard.svelte'
  import Settings from './pages/Settings.svelte'
  import Users from './pages/Users.svelte'
  import Logs from './pages/Logs.svelte'
  import Advanced from './pages/Advanced.svelte'
  import VirtualDisplay from './pages/VirtualDisplay.svelte'
  import Mark from './lib/Mark.svelte'
  import SignalBackground from './lib/SignalBackground.svelte'

  // Icônes : tracés 18 × 18, en trait.
  const pages = [
    { id: 'dashboard', label: 'Tableau de bord', component: Dashboard, icon: 'M2.5 9.5h3l2-5 3 9 2-4h3' },
    { id: 'settings', label: 'Réglages', component: Settings, icon: 'M3 5h7M13 5h2M3 13h2M8 13h7M10 3v4M5 11v4' },
    { id: 'display', label: 'Écran virtuel', component: VirtualDisplay, icon: 'M2.5 4h13v8.5h-13zM6.5 15.5h5M9 12.5v3M5.5 7.5l2 2 3-3.5' },
    { id: 'users', label: 'Utilisateurs', component: Users, icon: 'M6.5 8.5a2.5 2.5 0 100-5 2.5 2.5 0 000 5zM2 15c.6-2.6 2.3-4 4.5-4s3.9 1.4 4.5 4M12 4.5a2.3 2.3 0 010 4.4M13.5 11c1.3.5 2.1 1.8 2.5 4' },
    { id: 'logs', label: 'Journaux', component: Logs, icon: 'M4 2.5h7l3 3v10H4zM6.5 8h5M6.5 11h5M6.5 5h2.5' },
    { id: 'advanced', label: 'Avancé', component: Advanced, icon: 'M6 5L2.5 9 6 13M12 5l3.5 4-3.5 4M10 3.5l-2 11' },
  ]

  let session = $state<{ configured: boolean; authenticated: boolean; setup_code_file: string } | null>(null)
  let route = $state(location.hash.slice(2) || 'dashboard')
  let restarting = $state(false)
  const current = $derived(pages.find((p) => p.id === route) ?? pages[0])

  async function loadSession() {
    session = await api.session()
  }

  onMount(() => {
    setUnauthorizedHandler(() => {
      if (session) session.authenticated = false
    })
    loadSession()
    const onHash = () => (route = location.hash.slice(2) || 'dashboard')
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  })

  // Rafraîchit l'état du serveur tant que l'on est connecté.
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
      notify('ok', 'Kyber a redémarré avec la nouvelle configuration.')
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
  <div class="loading muted">Chargement…</div>
{:else if !session.authenticated}
  <Login configured={session.configured} setupCodeFile={session.setup_code_file} onDone={loadSession} />
{:else}
  <div class="shell">
    <nav>
      <div class="brand">
        <Mark size={34} id="kn-nav" />
        <div class="brand-text">
          <span class="name">Kyber<span>Now</span></span>
          <span class="tag">Console</span>
        </div>
      </div>
      {#each pages as page (page.id)}
        <a href="#/{page.id}" class:active={page.id === current.id}>
          <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><path d={page.icon} fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
          {page.label}
        </a>
      {/each}
      <div class="spacer"></div>
      {#if app.status}
        <div class="host muted">
          {app.status.hostname ?? ''}<br />
          <span>KyberNow {app.status.admin_version}</span>
        </div>
      {/if}
      <button class="link" onclick={logout}>Se déconnecter</button>
    </nav>

    <main>
      <div class="backdrop" aria-hidden="true"><SignalBackground /></div>
      {#if app.status?.restart_pending}
        <div class="notice warn row restart">
          <span>Configuration modifiée : redémarrez Kyber pour l'appliquer. Les clients connectés seront déconnectés quelques secondes.</span>
          <span class="spacer"></span>
          <button class="primary" disabled={restarting} onclick={restartKyber}>
            {restarting ? 'Redémarrage…' : 'Redémarrer Kyber'}
          </button>
        </div>
      {/if}
      {#key current.id}
        <div class="page kn-enter">
          <current.component />
        </div>
      {/key}
    </main>
  </div>
{/if}

{#if app.toast}
  <div class="toast {app.toast.kind}" role="status">{app.toast.text}</div>
{/if}

<style>
  .loading {
    padding: 40px;
    text-align: center;
  }

  .shell {
    display: grid;
    grid-template-columns: 220px 1fr;
    min-height: 100vh;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 24px 14px 18px;
    border-right: 1px solid var(--border);
    background: var(--rail);
    position: sticky;
    top: 0;
    height: 100vh;
    z-index: 2;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 8px 22px;
  }

  .brand-text {
    display: flex;
    flex-direction: column;
    line-height: 1.15;
  }

  .name {
    font-family: var(--display);
    font-weight: 700;
    font-size: 21px;
    letter-spacing: -0.02em;
  }

  .name span {
    color: var(--accent);
  }

  .tag {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  nav a {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-radius: 10px;
    color: var(--muted);
    text-decoration: none;
    font-size: 0.93rem;
    transition: background-color 0.2s, color 0.2s;
  }

  nav a:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  nav a.active {
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
  }

  nav a.active svg {
    color: var(--accent);
  }

  .host {
    font-size: 0.8rem;
    padding: 0 10px 8px;
    word-break: break-all;
  }

  nav .link {
    align-self: flex-start;
    padding: 0 10px;
  }

  main {
    position: relative;
    padding: 32px 36px 60px;
    max-width: 1140px;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  /* Fond « flux de signal », plus discret que dans l'installateur. */
  .backdrop {
    position: fixed;
    inset: 0 0 0 220px;
    opacity: 0.45;
    pointer-events: none;
  }

  main > :global(*:not(.backdrop)) {
    position: relative;
  }

  .page {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .restart {
    flex-wrap: nowrap;
  }

  .toast {
    position: fixed;
    bottom: 20px;
    right: 20px;
    max-width: 420px;
    padding: 12px 16px;
    border-radius: var(--radius);
    background: var(--text);
    color: var(--bg);
    font-size: 0.92rem;
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.18);
  }

  .toast.error {
    background: var(--danger);
    color: #fff;
  }

  @media (max-width: 760px) {
    .shell {
      grid-template-columns: 1fr;
    }

    nav {
      position: static;
      height: auto;
      flex-direction: row;
      flex-wrap: wrap;
      border-right: none;
      border-bottom: 1px solid var(--border);
      padding: 12px 16px;
    }

    .brand {
      width: 100%;
      padding-bottom: 8px;
    }

    nav .spacer,
    .host {
      display: none;
    }

    main {
      padding: 20px 16px 48px;
    }

    .backdrop {
      inset: 0;
    }

    .restart {
      flex-wrap: wrap;
    }
  }
</style>
