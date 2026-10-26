<!--
  Barre de session : fenêtre transparente au-dessus de la session, montrée
  quand le pointeur reste contre le bord haut (l'app décide : src/window.rs),
  cachée 0,4 s après qu'il l'a quittée, jamais pendant qu'un de ses menus est
  ouvert (le clavier revient alors à la session). Statistiques en direct,
  profil, touches, souris, plein écran, « Ordinateurs », « Garder le son »,
  « Se déconnecter ». Un menu ouvert fait grandir la fenêtre vers le bas le
  temps de son affichage. Elle dit sa largeur à l'app, qui en fait la zone du
  bord haut où le pointeur l'ouvre.

  Plusieurs ordinateurs (docs/plusieurs-ordinateurs.md) : une seule barre, qui
  sert la session de la fenêtre où elle s'ouvre (`id`) ; ses commandes et ses
  mesures sont celles de cette session. Le menu « Ordinateurs », présenté comme
  le menu « Touches » et absent sans autre ordinateur, liste les autres
  ordinateurs : un clic ouvre une session de plus (dans sa fenêtre), réveille
  un ordinateur hors ligne, ou montre la fenêtre d'un ordinateur déjà en
  session (coché, « Afficher »). « Garder le son » à partir de deux sessions.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Mark from './lib/Mark.svelte'
  import type { AppInfo, BarComputer, KeepSound, SessionStats, Stats } from './lib/api'
  import { invoke, listen, localLabel, ms } from './lib/api'
  import { setLang, t } from './lib/i18n/index.svelte'

  /** L'ordinateur de la session que la barre sert. */
  let id = $state('')
  let stats = $state<Stats | null>(null)
  let label = $state('')
  let profile = $state('')
  let menu = $state<'profile' | 'keys' | 'computers' | null>(null)
  /** Plusieurs écrans : « Plein écran » n'est pas proposé, les fenêtres y restent. */
  let multi = $state(false)
  /** « Garder le son » de cette session. */
  let keepSound = $state(false)
  /** Sessions ouvertes : le bouton « Garder le son » à partir de deux. */
  let sessionCount = $state(1)
  /** Le menu « Ordinateurs » : tous les ordinateurs (celui de la session est écarté à l'affichage). */
  let computers = $state<BarComputer[]>([])
  /** Un refus d'ouvrir une session (menu « Ordinateurs »), montré dans le menu. */
  let refusal = $state<string | null>(null)

  /**
   * `id` : la session servie. `pinned` : demandée par « Statistiques » (menu de l'icône), elle
   * reste jusqu'au passage du pointeur ; sinon, ouverte par le pointeur, qui est sur elle.
   * `multi` : plusieurs écrans. `keep_sound` : « Garder le son ». `sessions` : sessions ouvertes.
   */
  type Shown = { id: string; label: string; profile: string; pinned?: boolean; multi?: boolean; keep_sound?: boolean; sessions?: number }
  const profiles = $derived(
    ['jeu', 'bureau', 'max'].map((id) => ({ id, name: t(`profiles.${id}`), hint: t(`profiles.${id}_sub`) })),
  )
  const keys = $derived(
    [
      ['ctrl-alt-del', 'ctrl_alt_del'],
      ['win', 'win'],
      ['alt-tab', 'alt_tab'],
      ['print', 'print'],
    ].map(([id, k]) => ({ id, name: t(`bar.${k}`), hint: t(`bar.${k}_hint`) })),
  )
  let history = $state<number[]>([])
  /**
   * Les dernières mesures et la courbe de latence de chaque session, gardées même barre cachée :
   * la barre les montre dès son ouverture, sur la session qu'elle sert.
   */
  const seen = new Map<string, { stats: Stats; history: number[] }>()
  let hideTimer: ReturnType<typeof setTimeout> | null = null
  let bar = $state<HTMLElement | null>(null)
  /** Le pointeur est sur la barre, ou dans la bande au-dessus d'elle, contre le bord haut (là où il l'a ouverte). */
  let inside = false
  /** Demandée par « Statistiques » : rien ne la cache avant que le pointeur y soit passé. */
  let held = false
  /** Dernière largeur dite à l'app. */
  let sentWidth = 0

  onMount(() => {
    const off = [
      listen<SessionStats>('stats', (s) => {
        // Chaque session envoie les siennes : gardées par session, montrées pour celle servie.
        const before = seen.get(s.id)?.history ?? []
        const entry = { stats: s as Stats, history: s.latency_ms != null ? [...before, s.latency_ms].slice(-40) : before }
        seen.set(s.id, entry)
        if (s.id !== id) return
        stats = entry.stats
        history = entry.history
        multi = s.screens > 1
        reportWidth()
      }),
      listen<Shown>('bar-label', (s) => {
        if (s.id !== id) return
        label = s.label
        profile = s.profile
        reportWidth()
      }),
      listen<KeepSound>('session-keep-sound', (k) => {
        if (k.id === id) keepSound = k.on
      }),
      listen<BarComputer[]>('bar-computers', (c) => {
        computers = c
        reportWidth()
      }),
      listen<string[]>('sessions', (s) => {
        sessionCount = s.length
        // Les mesures des sessions finies s'oublient.
        for (const k of [...seen.keys()]) if (!s.includes(k)) seen.delete(k)
        reportWidth()
      }),
      listen<Shown>('bar-shown', (s) => {
        // Les mesures et la courbe de cette session, déjà reçues barre cachée.
        if (s.id !== id) {
          const kept = seen.get(s.id)
          stats = kept?.stats ?? null
          history = kept?.history ?? []
        }
        id = s.id
        label = s.label
        profile = s.profile
        multi = !!s.multi
        keepSound = !!s.keep_sound
        sessionCount = s.sessions ?? 1
        refusal = null
        // Montrée de nouveau : aucun menu d'une fois précédente.
        menu = null
        // Ouverte par le pointeur, immobile contre le bord haut : il est sur
        // elle. Demandée par « Statistiques » : elle reste, le menu de
        // l'icône la cache, ou le pointeur quand il y est passé puis l'a quittée.
        held = !!s.pinned
        inside = !s.pinned
        cancelHide()
        reportWidth()
      }),
      listen<string>('lang', (l) => {
        setLang(l)
        reportWidth()
      }),
    ]
    // Langue de l'app si la fenêtre a été rechargée depuis son ouverture.
    invoke<AppInfo>('app_info').then((i) => {
      setLang(i.lang)
      reportWidth()
    })
    // Largeur avec les vraies polices, avant même la première fois montrée.
    document.fonts?.ready.then(reportWidth)
    reportWidth()
    return () => off.forEach((p) => p.then((f) => f()))
  })

  /** Dit à l'app la largeur de la barre (en points) : le pointeur l'ouvre dans une zone du bord haut de cette largeur. */
  async function reportWidth() {
    await tick()
    const width = Math.round(bar?.getBoundingClientRect().width ?? 0)
    if (width > 0 && width !== sentWidth) {
      sentWidth = width
      invoke('bar_width', { width })
    }
  }

  /** « 3840 × 2160 · 160 i/s · HEVC » ; avec plusieurs écrans, leur nombre d'abord (les mesures sont celles de l'écran principal). */
  const infoLine = (s: Stats) => {
    const line = t('bar.info', { width: s.width, height: s.height, fps: s.fps, decoder: s.decoder })
    return s.screens > 1 ? `${t('stats.screens', { count: s.screens })} · ${line}` : line
  }

  const spark = $derived.by(() => {
    if (history.length < 2) return ''
    const max = Math.max(20, ...history)
    return history.map((v, i) => `${(i / (history.length - 1)) * 70},${18 - (v / max) * 16}`).join(' ')
  })

  function cancelHide() {
    if (hideTimer) clearTimeout(hideTimer)
    hideTimer = null
  }

  /** Se cache dans 0,4 s, sauf menu ouvert ou barre de « Statistiques » pas encore visitée. */
  function scheduleHide(delay = 400) {
    cancelHide()
    if (menu || held) return
    hideTimer = setTimeout(() => {
      hideTimer = null
      invoke('bar_hide', { id })
    }, delay)
  }

  /** Le pointeur bouge : sur la barre (et la bande au-dessus), elle reste ; ailleurs, le compte à rebours part (une fois : bouger dehors ne le prolonge pas). */
  function track(e: MouseEvent) {
    const r = bar?.getBoundingClientRect()
    inside = !!r && e.clientX >= r.left && e.clientX <= r.right && e.clientY <= r.bottom
    if (inside) {
      held = false
      cancelHide()
    } else if (!hideTimer) {
      scheduleHide()
    }
  }

  /** Le pointeur quitte la fenêtre de la barre. */
  function leave(e: MouseEvent) {
    if (e.relatedTarget) return
    inside = false
    scheduleHide()
  }

  /** Menu refermé par un choix ou un clic ailleurs : si le pointeur n'est plus sur la barre, elle repart. */
  function afterMenu() {
    if (!inside) scheduleHide()
  }

  async function toggle(which: 'profile' | 'keys' | 'computers') {
    const next = menu === which ? null : which
    refusal = null
    if (next === 'computers') computers = await invoke<BarComputer[]>('bar_computers')
    if (next && !menu) await invoke('bar_menu', { open: true })
    menu = next
    if (next) await growForMenu()
    else await invoke('bar_menu', { open: false })
  }

  /** La fenêtre grandit jusqu'au bas du menu ouvert (« Ordinateurs » : selon leur nombre). */
  async function growForMenu() {
    await tick()
    const bottom = bar?.querySelector('.menu')?.getBoundingClientRect().bottom ?? 0
    if (bottom > 0) await invoke('bar_menu', { open: true, height: Math.ceil(bottom + 24) })
  }

  async function closeMenu() {
    menu = null
    refusal = null
    await invoke('bar_menu', { open: false })
  }

  async function pickProfile(p: string) {
    await closeMenu()
    afterMenu()
    // Même profil : remis tel quel (utile après des réglages personnalisés).
    await invoke('session_profile', { id, profile: p })
  }

  async function pickKeys(k: string) {
    await closeMenu()
    await command(`keys:${k}`)
    // Le clavier revient à la session (et la barre s'en va).
    invoke('bar_hide', { id })
  }

  /** Les autres ordinateurs, dans l'ordre de l'accueil. */
  const others = $derived(computers.filter((c) => c.id !== id))
  const stateText = (c: BarComputer) => (c.state === 'online' ? t('home.online') : c.state === 'offline' ? t('home.offline') : t('common.searching'))
  /** Ce que dit la ligne à droite : « Afficher », « Réveiller l'ordinateur »… */
  const hintOf = (c: BarComputer) =>
    ({ show: t('bar.show'), wake: t('home.wake'), waking: t('home.waking'), retry: t('home.retry') })[c.pick as string] ?? ''
  const full = $derived(others.some((c) => c.pick === 'full'))

  /** Un ordinateur du menu « Ordinateurs » : comme l'accueil et le menu de l'icône. */
  async function pickComputer(c: BarComputer) {
    refusal = null
    try {
      if (c.pick === 'show') {
        await closeMenu()
        await invoke('show_session', { id: c.id })
        invoke('bar_hide', { id: c.id })
      } else if (c.pick === 'connect') {
        // Une session de plus, dans sa fenêtre ; sa connexion s'y montre (elle garde le clavier).
        await invoke('connect', { id: c.id })
        await closeMenu()
        invoke('bar_hide', { id, focus: false })
      } else if (c.pick === 'wake') {
        await invoke('wake', { id: c.id })
        computers = await invoke<BarComputer[]>('bar_computers')
      } else if (c.pick === 'retry') {
        await invoke('poll_now', { id: c.id })
        computers = await invoke<BarComputer[]>('bar_computers')
      }
    } catch (e) {
      refusal = String(e)
      await growForMenu()
    }
  }

  async function toggleKeepSound() {
    keepSound = !keepSound
    await invoke('session_keep_sound', { id, on: keepSound })
  }

  const command = (c: string) => invoke('session_command', { id, command: c })
</script>

<!-- Clic ailleurs (la session) : le menu se referme. -->
<svelte:window
  onblur={() => menu && closeMenu().then(afterMenu)}
  onmousemove={track}
  onmouseout={leave}
  onpointerdown={(e) => {
    // Clic dans la partie transparente sous la barre : le menu se referme.
    if (menu && !(e.target as Element).closest('.bar')) closeMenu().then(afterMenu)
  }}
/>

<div class="bar" role="toolbar" aria-label={t('bar.toolbar')} tabindex="-1" bind:this={bar}>
  <Mark size={20} state="now" lit />
  <div class="stats" role="group" aria-label={t('bar.statistics')} title={t('bar.statistics')}>
    <div class="lat" title={stats?.audio_ms != null ? t('bar.audio', { ms: ms(stats.audio_ms, 0), gaps: stats.audio_gaps ?? 0 }) : undefined}>
      <span class="mono">{ms(stats?.latency_ms)} ms</span>
      <svg width="70" height="20" viewBox="0 0 70 20" aria-hidden="true"><polyline points={spark} fill="none" stroke="#9A8CFF" stroke-width="1.5" /></svg>
    </div>
    <span class="sep"></span>
    <span class="mono info">{stats ? infoLine(stats) : '—'}</span>
  </div>
  <span class="sep"></span>
  <div class="anchor">
    <button class="chip" class:open={menu === 'profile'} aria-haspopup="menu" aria-expanded={menu === 'profile'} title={t('bar.session_profile')} onclick={() => toggle('profile')}>
      {label ? localLabel(label) : t('bar.profile')}<span class="caret" aria-hidden="true">▾</span>
    </button>
    {#if menu === 'profile'}
      <div class="menu" role="menu" aria-label={t('bar.profile')}>
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
    <button class="icon" class:on={menu === 'keys'} aria-haspopup="menu" aria-expanded={menu === 'keys'} aria-label={t('bar.keys')} title={t('bar.keys')} onclick={() => toggle('keys')}>
      <Icon name="keyboard" size={20} stroke={1.6} />
    </button>
    {#if menu === 'keys'}
      <div class="menu keys" role="menu" aria-label={t('bar.keys')}>
        {#each keys as k (k.id)}
          <button role="menuitem" onclick={() => pickKeys(k.id)}>
            <span class="name">{k.name}</span><span class="hint">{k.hint}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
  <button class="icon" class:on={stats?.relative} aria-label={(stats?.relative ? t('bar.mouse_game') : t('bar.mouse_desktop')) + t('bar.mouse_shortcut')} title={stats?.relative ? t('bar.mouse_game') : t('bar.mouse_desktop')} onclick={() => command('mouse')}>
    <Icon name="mouse" size={20} stroke={1.6} />
  </button>
  {#if !multi}
    <button class="icon" aria-label={t('bar.fullscreen') + t('bar.fullscreen_shortcut')} title={t('bar.fullscreen')} onclick={() => command('fullscreen')}>
      <Icon name="fullscreen" size={20} stroke={1.6} />
    </button>
  {/if}
  {#if sessionCount >= 2}
    <!-- À partir de deux sessions : la session reste audible même sans être active. -->
    <button class="icon" class:on={keepSound} aria-pressed={keepSound} aria-label={t('bar.keep_sound')} title={t('bar.keep_sound')} onclick={toggleKeepSound}>
      <Icon name="speaker" size={20} stroke={1.6} />
    </button>
  {/if}
  {#if others.length > 0}
  <!-- Présenté comme le menu Touches (icône seule, libellé accessible) ; absent sans autre ordinateur. -->
  <div class="anchor">
    <button class="icon" class:on={menu === 'computers'} aria-haspopup="menu" aria-expanded={menu === 'computers'} aria-label={t('bar.computers')} title={t('bar.computers')} onclick={() => toggle('computers')}>
      <Icon name="monitor" size={20} stroke={1.6} />
    </button>
    {#if menu === 'computers'}
      <div class="menu computers" role="menu" aria-label={t('bar.computers')}>
        {#each others as c (c.id)}
          {@const inert = c.pick === 'full' || c.pick === 'waking' || c.pick === 'busy'}
          <button
            role="menuitemcheckbox"
            aria-checked={c.pick === 'show'}
            aria-disabled={inert}
            class:inert
            onclick={() => !inert && pickComputer(c)}
          >
            <span class="tick">{#if c.pick === 'show'}<Icon name="check" size={14} stroke={2.2} />{/if}</span>
            <span class="name"><span class="dot {c.dot}" aria-hidden="true"></span>{t('bar.computer', { name: c.name, state: stateText(c) })}</span>
            <span class="hint">{hintOf(c)}</span>
          </button>
        {/each}
        {#if full}<p class="note" role="note">{t('common.four_at_most')}</p>{/if}
        {#if refusal}<p class="note refusal" role="alert">{refusal}</p>{/if}
      </div>
    {/if}
  </div>
  {/if}
  <span class="sep"></span>
  <button class="quit" title={t('bar.disconnect') + t('bar.disconnect_shortcut')} onclick={() => command('disconnect')}>{t('bar.disconnect')}</button>
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
    gap: 6px;
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

  .stats {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .lat {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 4px;
    font-size: 15px;
  }

  .sep {
    width: 1px;
    height: 24px;
    background: var(--line-2);
  }

  .info {
    padding: 0 4px;
    font-size: 12px;
    color: var(--muted);
  }

  .icon {
    width: 40px;
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

  .menu.keys button {
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
    font-size: 12px;
    color: var(--muted);
  }

  .menu.keys .hint {
    font-family: var(--mono);
    font-size: 11px;
  }

  /* Ordinateurs : à droite de la barre, le menu s'aligne sur son bouton. */
  .menu.computers {
    left: auto;
    right: -8px;
    transform: none;
    width: max-content;
    min-width: 300px;
    max-width: 480px;
  }

  .menu.computers .name {
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .menu button.inert {
    color: var(--ghost);
    cursor: default;
  }

  .menu button.inert:hover {
    background: none;
  }

  /* Point de couleur de l'état, comme dans le menu de l'icône (couleurs d'état de brand/v2). */
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 4px;
    background: var(--ghost);
  }

  .dot.ok {
    background: var(--ok);
  }

  .dot.bad {
    background: var(--bad);
  }

  .dot.warn {
    background: var(--warn);
  }

  .note {
    margin: 4px 10px 6px;
    font-size: 12px;
    line-height: 1.4;
    color: var(--warn);
    white-space: normal;
  }

  .note.refusal {
    color: var(--bad-text);
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
