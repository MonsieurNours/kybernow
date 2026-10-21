<!--
  Barre de session : fenêtre transparente au-dessus de la session, montrée
  quand le pointeur touche le haut de l'écran, cachée quand il la quitte (le
  clavier revient alors à la session). Mesures en direct, profil, touches
  spéciales, souris, plein écran, fin de la session. Un menu ouvert fait
  grandir la fenêtre vers le bas le temps de son affichage.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Mark from './lib/Mark.svelte'
  import type { Stats } from './lib/api'
  import { invoke, listen, ms } from './lib/api'

  let stats = $state<Stats | null>(null)
  let label = $state('')
  let profile = $state('')
  let menu = $state<'profile' | 'keys' | null>(null)

  type Shown = { label: string; profile: string }
  const profiles = [
    { id: 'jeu', name: 'Jeu', hint: 'réactivité avant tout' },
    { id: 'bureau', name: 'Bureau', hint: 'texte net, transport fiable' },
    { id: 'max', name: 'Max', hint: 'la meilleure image' },
  ]
  const keys = [
    { id: 'ctrl-alt-del', name: 'Ctrl + Alt + Suppr', hint: 'verrouiller, changer' },
    { id: 'win', name: 'Touche ⊞ seule', hint: 'menu Démarrer' },
    { id: 'alt-tab', name: 'Alt + Tab', hint: "changer d'app" },
    { id: 'print', name: 'Impr. écran', hint: 'capture' },
  ]
  let history = $state<number[]>([])
  let hideTimer: ReturnType<typeof setTimeout> | null = null

  onMount(() => {
    const off = [
      listen<Stats>('stats', (s) => {
        stats = s
        if (s.latency_ms != null) history = [...history, s.latency_ms].slice(-40)
      }),
      listen<Shown>('bar-label', (s) => {
        label = s.label
        profile = s.profile
      }),
      listen<Shown>('bar-shown', (s) => {
        label = s.label
        profile = s.profile
        // Le pointeur n'y entre pas : elle repart d'elle-même.
        scheduleHide(1600)
      }),
    ]
    return () => off.forEach((p) => p.then((f) => f()))
  })

  const spark = $derived.by(() => {
    if (history.length < 2) return ''
    const max = Math.max(20, ...history)
    return history.map((v, i) => `${(i / (history.length - 1)) * 70},${18 - (v / max) * 16}`).join(' ')
  })

  function cancelHide() {
    if (hideTimer) clearTimeout(hideTimer)
    hideTimer = null
  }

  function scheduleHide(delay = 700) {
    cancelHide()
    if (menu) return
    hideTimer = setTimeout(() => invoke('bar_hide'), delay)
  }

  async function toggle(which: 'profile' | 'keys') {
    const next = menu === which ? null : which
    if (next && !menu) await invoke('bar_menu', { open: true })
    menu = next
    if (!next) await invoke('bar_menu', { open: false })
  }

  async function closeMenu() {
    menu = null
    await invoke('bar_menu', { open: false })
  }

  async function pickProfile(id: string) {
    await closeMenu()
    if (id !== profile) await invoke('session_profile', { profile: id })
  }

  async function pickKeys(id: string) {
    await closeMenu()
    await command(`keys:${id}`)
    // Le clavier revient à la session (et la barre s'en va).
    invoke('bar_hide')
  }

  const command = (c: string) => invoke('session_command', { command: c })
</script>

<!-- Clic ailleurs (la session) : le menu se referme. -->
<svelte:window onblur={() => menu && closeMenu().then(() => scheduleHide())} />

<div class="bar" role="toolbar" aria-label="Barre de session" tabindex="-1" onmouseleave={() => scheduleHide()} onmouseenter={cancelHide}>
  <Mark size={20} state="now" lit />
  <div class="lat">
    <span class="mono">{ms(stats?.latency_ms)} ms</span>
    <svg width="70" height="20" viewBox="0 0 70 20" aria-hidden="true"><polyline points={spark} fill="none" stroke="#9A8CFF" stroke-width="1.5" /></svg>
  </div>
  <span class="sep"></span>
  <span class="mono info">{stats ? `${stats.width} × ${stats.height} · ${stats.fps} i/s · ${stats.decoder}` : '—'}</span>
  <span class="sep"></span>
  <div class="anchor">
    <button class="chip" class:open={menu === 'profile'} aria-haspopup="menu" aria-expanded={menu === 'profile'} title="Profil de la session" onclick={() => toggle('profile')}>
      {label || 'Profil'}<span class="caret" aria-hidden="true">▾</span>
    </button>
    {#if menu === 'profile'}
      <div class="menu" role="menu" aria-label="Profil">
        {#each profiles as p (p.id)}
          <button role="menuitemradio" aria-checked={p.id === profile} onclick={() => pickProfile(p.id)}>
            <span class="tick">{#if p.id === profile}<Icon name="check" size={14} stroke={2.2} />{/if}</span>
            <span class="name">{p.name}</span><span class="hint">{p.hint}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
  <div class="anchor">
    <button class="icon" class:on={menu === 'keys'} aria-haspopup="menu" aria-expanded={menu === 'keys'} aria-label="Touches spéciales" title="Touches spéciales" onclick={() => toggle('keys')}>
      <Icon name="keyboard" size={20} stroke={1.6} />
    </button>
    {#if menu === 'keys'}
      <div class="menu" role="menu" aria-label="Touches">
        {#each keys as k (k.id)}
          <button role="menuitem" onclick={() => pickKeys(k.id)}>
            <span class="name">{k.name}</span><span class="hint">{k.hint}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
  <button class="icon" class:on={stats?.relative} aria-label={stats?.relative ? 'Souris des jeux (Ctrl+Alt+Maj+M)' : 'Souris du bureau (Ctrl+Alt+Maj+M)'} title={stats?.relative ? 'Souris des jeux' : 'Souris du bureau'} onclick={() => command('mouse')}>
    <Icon name="mouse" size={20} stroke={1.6} />
  </button>
  <button class="icon" aria-label="Plein écran ou fenêtre (Ctrl+Alt+Maj+F)" title="Plein écran ou fenêtre" onclick={() => command('fullscreen')}>
    <Icon name="fullscreen" size={20} stroke={1.6} />
  </button>
  <span class="sep"></span>
  <button class="quit" onclick={() => command('disconnect')}>Quitter</button>
</div>

<style>
  :global(html.bar #app) {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 6px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 56px;
    padding: 0 8px 0 16px;
    border: 1px solid rgb(43 55 66 / 0.9);
    border-radius: 14px;
    background: rgb(9 13 18 / 0.9);
    box-shadow: 0 16px 40px rgb(0 0 0 / 0.5);
    white-space: nowrap;
    animation: drop 0.25s var(--ease-out);
  }

  @keyframes drop {
    from {
      transform: translateY(-10px);
      opacity: 0;
    }
  }

  .lat {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px;
    font-size: 15px;
  }

  .sep {
    width: 1px;
    height: 24px;
    background: var(--line-2);
  }

  .info {
    padding: 0 8px;
    font-size: 12px;
    color: var(--muted);
  }

  .icon {
    width: 44px;
    height: 40px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon:hover {
    background: rgb(255 255 255 / 0.06);
    color: var(--text);
  }

  .icon.on {
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--signal);
    color: var(--text);
  }

  .anchor {
    position: relative;
  }

  .chip {
    height: 34px;
    padding: 0 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line-2);
    border-radius: 8px;
    background: none;
    color: var(--text);
    font-size: 13px;
  }

  .chip:hover,
  .chip.open {
    background: rgb(255 255 255 / 0.06);
  }

  .caret {
    font-size: 10px;
    color: var(--muted);
  }

  .menu {
    position: absolute;
    top: calc(100% + 12px);
    left: 50%;
    transform: translateX(-50%);
    min-width: 250px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    border: 1px solid rgb(43 55 66 / 0.9);
    border-radius: 12px;
    background: rgb(9 13 18 / 0.96);
    box-shadow: 0 16px 40px rgb(0 0 0 / 0.5);
    animation: drop 0.18s var(--ease-out);
  }

  .menu button {
    display: grid;
    grid-template-columns: 18px auto 1fr;
    align-items: center;
    gap: 8px;
    padding: 9px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text);
    text-align: left;
    font-size: 13px;
  }

  .menu[aria-label='Touches'] button {
    grid-template-columns: auto 1fr;
  }

  .menu button:hover {
    background: rgb(255 255 255 / 0.07);
  }

  .tick {
    color: var(--signal);
    display: flex;
  }

  .hint {
    justify-self: end;
    padding-left: 14px;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
  }

  .quit {
    height: 40px;
    padding: 0 14px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: #ff8a8a;
    font-size: 13px;
  }

  .quit:hover {
    background: rgb(255 107 107 / 0.12);
  }
</style>
