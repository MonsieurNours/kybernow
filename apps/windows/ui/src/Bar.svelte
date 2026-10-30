<!--
  Barre de session : fenêtre transparente au-dessus de la session, montrée
  quand le pointeur reste contre le bord haut (l'app décide : src/window.rs),
  cachée 0,4 s après qu'il l'a quittée, jamais pendant qu'un de ses menus est
  ouvert (le clavier revient alors à la session). Statistiques en direct,
  profil, touches, souris, plein écran, « Ordinateurs », « Garder le son »,
  « Se déconnecter ». Un menu ouvert fait grandir la fenêtre vers le bas le
  temps de son affichage. Elle dit sa largeur à l'app, qui en fait la zone du
  bord haut où le pointeur l'ouvre.

  KyberNow Share (docs/link-ui.md « Partage ») : pour une session d'hôte,
  « Partager » (icône de partage, comme les autres menus) ouvre le partage et son
  panneau (code, « Copier le lien », QR code, invités et leurs actions, « Nouveau
  code », « Arrêter le partage »), et une pastille « 2 invités » le rouvre ; impossible
  (Station trop ancienne, partage coupé), il est estompé : à la souris sans effet, avec la
  raison en infobulle ; au toucher, il ouvre le panneau réduit à la raison. Pour une session d'invité,
  la barre est réduite : son rôle (« Spectateur », « Joueur »), les statistiques,
  « Touches » et la souris seulement avec le clavier et la souris, le plein écran,
  « Quitter le partage » ; ni profil ni « Ordinateurs ». Sa « Qualité de l'image »
  (à la place du profil) montre la qualité reçue (« 1080p · 60 Hz ») et se change :
  « Auto », « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz » (l'image de l'invité
  seulement, jamais celle de l'hôte). Un changement de rôle ou d'image (« Image allégée
  pour suivre. ») s'affiche quelques secondes à la place des statistiques (la barre se
  montre alors sans prendre le clavier à la session). Dans le panneau de l'hôte, la
  qualité d'un invité est sous son nom quand ce n'est pas celle de l'hôte (lecture seule).

  Hors de chez vous (docs/relais.md) : une session qui passe par le relais montre la puce « Par le
  relais », dans la partie toujours visible de la barre. Dans le panneau de l'hôte, les invités
  venus par le relais sont acceptés un à un : la liste « Demandes » (« {prénom} demande à
  rejoindre », « {app} · hors de chez vous », « Accepter », « Refuser ») est au-dessus des
  invités, et une pastille « 1 demande » à côté de « 2 invités » rouvre le panneau ; un invité
  venu par le relais le dit après son app (« hors de chez vous »). L'aide dit « Sur le même réseau ou
  ailleurs » quand le relais tient le code, et « Pas encore joignable hors de chez vous. » quand il
  ne répond pas.

  Plusieurs ordinateurs (docs/plusieurs-ordinateurs.md) : une seule barre, qui
  sert la session de la fenêtre où elle s'ouvre (`id`) ; ses commandes et ses
  mesures sont celles de cette session. Le menu « Ordinateurs », présenté comme
  le menu « Touches » et absent sans autre ordinateur, liste les autres
  ordinateurs : un clic ouvre une session de plus (dans sa fenêtre), réveille
  un ordinateur hors ligne, ou montre la fenêtre d'un ordinateur déjà en
  session (coché, « Afficher »). « Garder le son » à partir de deux sessions.

  Prêt de l'ordinateur (docs/link-ui.md « Prêt de l'ordinateur ») : pour une session d'hôte,
  « Prêter l'ordinateur… » (icône de manette, comme les autres menus) ouvre son panneau : la durée
  (« 1 heure », « 2 heures », « 4 heures », « Jusqu'à ce que je reprenne »), le compte Steam,
  « Créer un code », puis le code, « Copier le lien » et le QR code comme le partage, « Pour une
  séance, valable 24 h. », « Votre session se fermera quand votre ami arrivera. », « Annuler le
  code » ; l'avertissement de confiance la première fois ; les demandes (« {prénom} veut emprunter
  l'ordinateur. », « Accepter », « Refuser », « Votre session va se fermer. ») avec leur pastille
  et leur avis bref. Station trop ancienne ou prêt coupé dans Admin : estompé, avec la raison.
  Pour l'ami qui emprunte, la barre est réduite : « Prêté par {hôte} », le temps restant
  (« Encore 1 h 20 », « Jusqu'à ce que {hôte} reprenne »), les statistiques, le son, le plein écran,
  la souris et les touches, et « Rendre l'ordinateur » (confirmation : « Rendre l'ordinateur à
  {hôte} ? ») ; ni profil de la Station, ni partage, ni « Ordinateurs ». Le préavis de 2 minutes
  s'affiche comme un avis bref.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Mark from './lib/Mark.svelte'
  import Segmented from './lib/Segmented.svelte'
  import type { AppInfo, BarComputer, BorrowBar, BorrowEvent, GuestBar, KeepSound, LendPanel, LendRequest, LendStateEvent, RouteEvent, RouteView, SessionStats, Share, ShareGuest, ShareImageEvent, ShareQr, ShareQuality, ShareRequestEvent, ShareRole, ShareRoleEvent, ShareStateEvent, Stats } from './lib/api'
  import { invoke, leftText, lendDurations, listen, localLabel, ms, roleKbm, roleOf, rolePads, rolePlays, shareQualities, sinceText } from './lib/api'
  import { setLang, t } from './lib/i18n/index.svelte'
  import { relayPhase, type RelayPhase } from './lib/relay.svelte'

  /** L'ordinateur de la session que la barre sert (`guest-N` pour une session d'invité). */
  let id = $state('')
  let stats = $state<Stats | null>(null)
  let label = $state('')
  let profile = $state('')
  let menu = $state<Menu | null>(null)
  type Menu = 'profile' | 'keys' | 'computers' | 'share' | 'quality' | 'lend' | 'giveback'
  /** Session d'emprunt (prêt de l'ordinateur) : l'hôte, la fin, le préavis ; la barre est réduite. */
  let borrow = $state<BorrowBar | null>(null)
  /** Session d'hôte : « Prêter l'ordinateur… » et le prêt, tels que l'app les dit. */
  let lend = $state<LendPanel | null>(null)
  /** Le prêt de chaque session d'hôte, gardé même barre cachée (signe de vie de la session). */
  const lends = new Map<string, LendPanel>()
  /** Une action du prêt attend la réponse d'Admin. */
  let lendActing = $state(false)
  let lendError = $state<string | null>(null)
  /** « Durée » (en secondes, `none` : « Jusqu'à ce que je reprenne ») et « Compte Steam » du code à créer. */
  let lendDuration = $state('7200')
  let lendSteam = $state<'friend' | 'host'>('friend')
  /** « Copier le lien » du code de prêt. */
  let lendCopied = $state<'ok' | 'fail' | null>(null)
  /** L'avertissement de confiance, montré la première fois que le panneau s'ouvre (et toujours dans Admin). */
  let trustOnce = $state(false)
  /** Session d'invité (KyberNow Share) : rôle, hôte et image ; la barre est réduite. */
  let guest = $state<GuestBar | null>(null)
  /**
   * La qualité que l'invité vient de choisir, tant que la Station n'a pas répondu : un signe de vie
   * parti avant le changement porte encore l'ancienne et ne la remet pas dans le menu.
   */
  let asked: { id: string; quality: ShareQuality; until: number } | null = null
  /** Par où passe la session servie : la puce « Par le relais » (nul tant que le cœur ne l'a pas dit). */
  let route = $state<RouteView | null>(null)
  /** Le chemin de chaque session, gardé même barre cachée. */
  const routes = new Map<string, RouteView>()
  /** Session d'hôte : « Partager » et le partage ouvert, tels que l'app les dit. */
  let share = $state<Share | null>(null)
  /** Le partage de chaque session d'hôte, gardé même barre cachée (signe de vie de la session). */
  const shares = new Map<string, Share>()
  /** « Partager » ouvre le partage (appel à Admin en cours). */
  let opening = $state(false)
  /** Une action sur le partage (rôle d'un invité, retrait, nouveau code, arrêt) attend la réponse d'Admin. */
  let acting = $state(false)
  /** Le refus d'une action du partage (Admin injoignable…), montré dans le panneau. */
  let shareError = $state<string | null>(null)
  /** « Copier le lien » : `ok` (« Lien copié. ») ou `fail` (« Copie impossible… »). */
  let copied = $state<'ok' | 'fail' | null>(null)
  /** Avis bref d'un changement de rôle, montré à la place des statistiques. */
  let notice = $state<string | null>(null)
  /** L'avis est celui d'une demande (à rejoindre, à emprunter) : le choisir ouvre ce panneau. */
  let noticePanel = $state<'share' | 'lend' | null>(null)
  let noticeTimer: ReturnType<typeof setTimeout> | null = null
  /** L'heure, pour « depuis 3 min » dans la liste des invités (revue toutes les 30 s). */
  let now = $state(Date.now())
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
   * `guest` : session d'invité (barre réduite). `share` : session d'hôte, « Partager » et son partage.
   * `notice` : un avis bref à montrer (changement de rôle), la barre repart ensuite d'elle-même ;
   * `notice_opens_share` : c'est celui d'une demande à rejoindre, que l'on choisit pour ouvrir le panneau.
   */
  type Shown = {
    id: string
    label: string
    profile: string
    pinned?: boolean
    multi?: boolean
    keep_sound?: boolean
    sessions?: number
    guest?: GuestBar | null
    share?: Share | null
    notice?: string | null
    notice_opens_share?: boolean
    notice_opens_lend?: boolean
    route?: RouteView | null
    lend?: LendPanel | null
    borrow?: BorrowBar | null
  }
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
        for (const k of [...shares.keys()]) if (!s.includes(k)) shares.delete(k)
        for (const k of [...routes.keys()]) if (!s.includes(k)) routes.delete(k)
        for (const k of [...lends.keys()]) if (!s.includes(k)) lends.delete(k)
        reportWidth()
      }),
      // Par où passe la session change (au départ, repli TCP) : la puce « Par le relais » suit.
      listen<RouteEvent>('session-route', (e) => {
        routes.set(e.id, e.route)
        if (e.id !== id) return
        route = e.route
        reportWidth()
      }),
      // Le partage de l'hôte change (arrivée, départ, rôle d'un invité, nouveau code, fin) : le panneau suit.
      listen<ShareStateEvent>('share-state', (e) => {
        shares.set(e.id, e.share)
        if (e.id !== id || guest) return
        share = e.share
        reportWidth()
        if (menu === 'share') growForMenu()
      }),
      // Une demande à rejoindre arrive pendant que la barre est ouverte : l'avis « {prénom} demande à rejoindre. », sauf
      // panneau de « Partager » ouvert (il montre la demande, l'avis ne le ferme pas).
      listen<ShareRequestEvent>('share-request', (e) => {
        const panel = e.panel ?? 'share'
        if (e.id !== id || guest || borrow || menu === panel) return
        showNotice(e.text, panel)
      }),
      // Le prêt de l'hôte change (code, demande, prêt en cours) : le panneau suit.
      listen<LendStateEvent>('lend-state', (e) => {
        lends.set(e.id, e.lend)
        if (e.id !== id || guest || borrow) return
        lend = e.lend
        reportWidth()
        if (menu === 'lend') growForMenu()
      }),
      // L'ami qui emprunte : le préavis arrive (le temps restant suit).
      listen<BorrowEvent>('lend-borrow', (e) => {
        if (e.id !== id) return
        borrow = e.borrow
        reportWidth()
      }),
      // Le choix d'une notification de demande d'emprunt : le panneau « Prêter l'ordinateur… » s'ouvre.
      listen<string>('lend-open-panel', async (session) => {
        if (session !== id || guest || borrow) return
        clearNotice()
        if (menu !== 'lend') await toggleLend()
      }),
      // Le choix d'une notification de demande : la barre est montrée sur cette session, le panneau de « Partager » s'ouvre.
      listen<string>('share-open-panel', async (session) => {
        if (session !== id || guest || borrow) return
        clearNotice()
        if (menu !== 'share') await toggleShare()
      }),
      // Le rôle de l'invité change (par l'hôte) : l'étiquette, et le clavier et la souris.
      listen<ShareRoleEvent>('share-role', (e) => {
        if (e.id !== id || !guest) return
        guest = { ...guest, role: e.role }
        if (menu === 'keys' && !roleKbm(e.role)) closeMenu()
        reportWidth()
      }),
      // L'image que reçoit l'invité change (qualité changée, image allégée, Station qui l'apprend).
      listen<ShareImageEvent>('share-image', (e) => {
        if (e.id !== id || !guest) return
        // Un signe de vie parti avant le changement porte encore l'ancienne qualité : tant que la Station
        // n'a pas pris le choix de l'invité (3 s au plus), le menu garde ce choix.
        let quality = e.quality
        if (asked && asked.id === e.id) {
          if (e.quality === asked.quality || Date.now() >= asked.until) asked = null
          else quality = asked.quality
        }
        guest = { ...guest, quality, image: e.label }
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
        // Par où passe cette session (puce « Par le relais »).
        route = s.route ?? routes.get(s.id) ?? null
        if (s.route) routes.set(s.id, s.route)
        // KyberNow Share : l'invité (barre réduite) ou, pour un hôte, son partage.
        guest = s.guest ?? null
        asked = null
        share = s.share ?? shares.get(s.id) ?? null
        if (s.share) shares.set(s.id, s.share)
        shareError = null
        copied = null
        // Prêt de l'ordinateur : l'ami qui emprunte (barre réduite) ou, pour un hôte, « Prêter l'ordinateur… ».
        borrow = s.borrow ?? null
        lend = s.lend ?? lends.get(s.id) ?? null
        if (s.lend) lends.set(s.id, s.lend)
        lendError = null
        lendCopied = null
        // Montrée de nouveau : aucun menu d'une fois précédente.
        menu = null
        if (s.notice) {
          // Un avis bref : la barre se montre sans prendre le clavier, et repart d'elle-même.
          held = false
          inside = false
          showNotice(s.notice, s.notice_opens_share ? 'share' : s.notice_opens_lend ? 'lend' : null)
        } else {
          clearNotice()
          // Ouverte par le pointeur, immobile contre le bord haut : il est sur
          // elle. Demandée par « Statistiques » : elle reste, le menu de
          // l'icône la cache, ou le pointeur quand il y est passé puis l'a quittée.
          held = !!s.pinned
          inside = !s.pinned
        }
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
    const clock = setInterval(() => (now = Date.now()), 30_000)
    return () => {
      clearInterval(clock)
      off.forEach((p) => p.then((f) => f()))
    }
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

  async function toggle(which: Menu) {
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
  const stateText = (c: BarComputer) =>
    c.state === 'online' ? t('home.online') : c.state === 'away' ? t('home.away') : c.state === 'offline' ? t('home.offline') : t('common.searching')
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

  // MARK: - KyberNow Share

  /** Le nom d'un choix de « Qualité de l'image » : « Auto », « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz ». */
  const qualityName = (q: ShareQuality) => t(`share.quality_${q}`)
  /** Ce que la puce montre : la qualité reçue quand Kyber la connaît, sinon celle qui est demandée. */
  const shownQuality = $derived(guest ? (guest.image ?? qualityName(guest.quality)) : '')
  /** Sur la ligne cochée : la qualité reçue, quand elle dit autre chose que le choix (« Auto » : « 1080p · 60 Hz »). */
  const receivedFor = (q: ShareQuality) => (guest && q === guest.quality && guest.image && guest.image !== qualityName(q) ? guest.image : '')

  /**
   * Une autre « Qualité de l'image » : l'app la dit à la Station et la retient pour la fois suivante ;
   * l'image de l'invité repart seule, celle de l'hôte ne bouge pas. La puce montre la nouvelle qualité
   * reçue dès que la Station l'a dite (événement `share-image`).
   */
  async function pickQuality(q: ShareQuality) {
    await closeMenu()
    afterMenu()
    if (!guest || q === guest.quality) return
    const mine = id
    const before = guest.quality
    asked = { id: mine, quality: q, until: Date.now() + 3000 }
    guest = { ...guest, quality: q }
    try {
      await invoke('share_quality', { id: mine, quality: q })
    } catch (e) {
      // La session n'a pas pris le choix : le menu redit celui qui reste.
      if (id === mine && guest) {
        asked = null
        guest = { ...guest, quality: before }
        showNotice(String(e))
      }
    }
  }

  /** Un avis bref à la place des statistiques ; la barre repart ensuite d'elle-même (sauf pointeur dessus ou menu ouvert). */
  function showNotice(text: string, panel: 'share' | 'lend' | null = null) {
    clearNotice()
    notice = text
    noticePanel = panel
    noticeTimer = setTimeout(() => {
      noticeTimer = null
      notice = null
      noticePanel = null
      if (!inside && !menu) invoke('bar_hide', { id, focus: false })
    }, 4000)
  }

  function clearNotice() {
    if (noticeTimer) clearTimeout(noticeTimer)
    noticeTimer = null
    notice = null
    noticePanel = null
  }

  /** Choisir l'avis d'une demande à rejoindre : le panneau de « Partager » s'ouvre, où « Accepter » et « Refuser » attendent. */
  async function openShareFromNotice() {
    const panel = noticePanel
    clearNotice()
    // La barre reste le temps de répondre (le pointeur est dessus, ou le panneau ouvert la tient).
    held = true
    if (panel === 'lend') await toggleLend()
    else await toggleShare()
  }

  /** Le nom de l'ordinateur de la session (titre du panneau). */
  const computerName = $derived(computers.find((c) => c.id === id)?.name ?? '')
  /** Le partage est inactif : Station trop ancienne, ou partage coupé sur l'ordinateur. */
  const blocked = $derived(share?.availability === 'unsupported' || share?.availability === 'disabled')
  /** Le panneau, sans le texte déjà dit par un « Partager » inactif. */
  const shownError = $derived(shareError && shareError !== share?.text ? shareError : null)
  const guestCount = $derived(share?.open ? share.guests.length : 0)
  /** Les demandes venues par le relais qui attendent l'hôte : elles ont leur pastille, et ne comptent pas parmi les invités. */
  const requestCount = $derived(share?.open ? share.requests.length : 0)
  /**
   * Le relais du partage et celui du code de prêt (lib/relay.svelte.ts) : joint, injoignable, pas de relais, ou
   * `waiting` (lien du relais en préparation, 10 s au plus : copie et QR code retenus).
   */
  const shareRelay = relayPhase(() => ({ state: share?.open ? share.relay : null, key: `${id}:${share?.code}` }))
  const relay = $derived(shareRelay.value)
  const lendRelay = relayPhase(() => ({ state: lend?.code?.relay, key: `${id}:${lend?.code?.code}` }))
  /** « Par le relais » : le chemin en toutes lettres (« Par le relais (TCP) » pour le repli), dans l'infobulle de la puce. */
  const routeTitle = $derived(route ? t(`relay.path_${route.transport}`) : '')
  /** « Joueur » dès qu'il a un droit (manettes, ou clavier et souris), « Spectateur » sinon. */
  const roleName = (r: ShareRole) => t(rolePlays(r) ? 'share.role_player' : 'share.role_spectator')

  /** Le dernier appui sur « Partager » est un doigt ou un stylet (pas une souris ni le clavier). */
  let shareTouch = false

  /**
   * « Partager » : ouvre le partage s'il ne l'est pas (code, lien, QR code) puis son panneau ; une
   * pastille d'invités le rouvre. Inactif (Station trop ancienne, partage coupé) : à la souris et au
   * clavier, il ne fait rien (l'infobulle dit pourquoi) ; au toucher, qui n'a pas d'infobulle, il reste
   * sélectionnable et ouvre le panneau réduit à la raison (docs/link-ui.md, « Partage »).
   */
  async function toggleShare() {
    if (menu === 'share') return toggle('share')
    const touched = shareTouch
    shareTouch = false
    if (blocked && !touched) return
    shareError = null
    copied = null
    if (!blocked && !share?.open) {
      if (opening) return
      opening = true
      // La barre a pu passer à une autre session pendant l'appel : le partage ne se range que chez la sienne.
      const mine = id
      try {
        const view = await invoke<Share>('share_open', { id: mine })
        shares.set(mine, view)
        if (id === mine) share = view
      } catch (e) {
        if (id === mine) shareError = String(e)
      } finally {
        opening = false
      }
    }
    await toggle('share')
  }

  /** Une action sur le partage : Admin répond avec le partage à jour (celui de la session qui l'a demandée). */
  async function shareAction(call: () => Promise<Share>) {
    // Une action à la fois : les droits d'un invité se déduisent de ce que le panneau montre, qui
    // n'est à jour qu'à la réponse d'Admin (deux clics d'affilée sur deux droits se défairaient).
    if (acting) return
    acting = true
    shareError = null
    const mine = id
    try {
      const view = await call()
      shares.set(mine, view)
      if (id === mine) share = view
    } catch (e) {
      if (id === mine) shareError = String(e)
    } finally {
      acting = false
    }
    await growForMenu()
  }

  const setRole = (g: ShareGuest, role: ShareRole) => shareAction(() => invoke<Share>('share_guest', { id, guest: g.id, role }))
  const kick = (g: ShareGuest) => shareAction(() => invoke<Share>('share_kick', { id, guest: g.id }))
  /**
   * « Accepter » : la demande devient un invité spectateur (inactif quand les quatre places sont prises) ;
   * « Refuser » : elle disparaît (son appareil ne redemande pas avant 10 minutes).
   */
  const accept = (g: ShareGuest) => (share?.full ? undefined : shareAction(() => invoke<Share>('share_accept', { id, guest: g.id })))
  const refuse = (g: ShareGuest) => shareAction(() => invoke<Share>('share_refuse', { id, guest: g.id }))
  const newCode = () => shareAction(() => invoke<Share>('share_code', { id }))

  async function stopSharing() {
    await shareAction(() => invoke<Share>('share_close', { id }))
    // Plus de partage : le panneau se referme (la barre repart si le pointeur n'y est plus).
    if (!share?.open && !shareError) {
      await closeMenu()
      afterMenu()
    }
  }

  /**
   * « Copier le lien » (partage, code de prêt) : l'adresse du client web ; sinon l'adresse sélectionnable, à copier à
   * la main. Rien tant que le lien du relais se prépare.
   */
  async function copyText(text: string, phase: RelayPhase, done: (r: 'ok' | 'fail' | null) => void, current: () => 'ok' | 'fail' | null) {
    if (!text || phase === 'waiting') return
    done(null)
    let ok = true
    try {
      await navigator.clipboard.writeText(text)
    } catch {
      // Le presse-papiers de Windows, par l'app.
      ok = await invoke('copy_text', { text }).then(() => true, () => false)
    }
    done(ok ? 'ok' : 'fail')
    if (ok) setTimeout(() => current() === 'ok' && done(null), 3000)
    await growForMenu()
  }

  const copyLink = () =>
    copyText(share?.web || share?.link || '', relay, (r) => (copied = r), () => copied)

  /** Le lien se sélectionne en entier au premier clic (copie à la main). */
  const selectAll = (e: Event) => (e.currentTarget as HTMLInputElement).select()

  const apps = ['mac', 'windows', 'linux', 'ios', 'tvos', 'android', 'webos', 'web']
  /**
   * « iPhone ou iPad · hors de chez vous · 720p · 30 Hz · depuis 3 min » : l'app de l'invité, « hors de chez vous » s'il
   * est venu par le relais, sa qualité d'image si ce n'est pas celle de l'hôte, et son arrivée.
   */
  const guestMeta = (g: ShareGuest) =>
    [
      apps.includes(g.app) ? t(`share.app_${g.app}`) : '',
      g.away ? t('share.away') : '',
      g.image ?? '',
      g.since != null ? t('share.since', { duration: sinceText(g.since, now / 1000) }) : '',
    ]
      .filter(Boolean)
      .join(' · ')
  /** « iPhone ou iPad · hors de chez vous » : sous « {prénom} demande à rejoindre ». */
  const requestMeta = (g: ShareGuest) => [apps.includes(g.app) ? t(`share.app_${g.app}`) : '', t('share.away')].filter(Boolean).join(' · ')

  // MARK: - Prêt de l'ordinateur

  /** « Prêter l'ordinateur… » inactif : Station trop ancienne, prêt impossible sur cet ordinateur, ou coupé dans Admin (avec la raison). */
  const lendBlocked = $derived(lend?.availability === 'unsupported' || lend?.availability === 'unavailable' || lend?.availability === 'disabled')
  const lendText = $derived(
    lend?.availability === 'unsupported'
      ? t('lend.unsupported')
      : lend?.availability === 'unavailable'
        ? t('lend.unavailable')
        : lend?.availability === 'disabled'
          ? t('lend.disabled')
          : null,
  )
  /** Une demande pendant un créneau : son texte dit déjà le préavis du créneau. */
  const slotAsked = $derived(lend?.requests.some((r) => r.kind === 'slot') ?? false)
  const lendRequests = $derived(lend?.requests.length ?? 0)
  /** Le dernier appui sur « Prêter l'ordinateur… » est un doigt ou un stylet (comme « Partager »). */
  let lendTouch = false

  function trustSeen() {
    try {
      const seen = localStorage.getItem('kybernow.lend.trust') === '1'
      localStorage.setItem('kybernow.lend.trust', '1')
      return seen
    } catch {
      return false
    }
  }

  /**
   * « Prêter l'ordinateur… » : le panneau, avec le prêt tel qu'Admin le dit maintenant. Inactif : à la souris, rien
   * (l'infobulle dit pourquoi) ; au toucher, le panneau réduit à la raison.
   */
  async function toggleLend() {
    if (menu === 'lend') return toggle('lend')
    const touched = lendTouch
    lendTouch = false
    if (lendBlocked && !touched) return
    lendError = null
    lendCopied = null
    trustOnce = !trustSeen()
    const mine = id
    invoke<LendPanel>('lend_state', { id: mine })
      .then((view) => {
        lends.set(mine, view)
        if (id === mine) {
          lend = view
          if (menu === 'lend') growForMenu()
        }
      })
      .catch((e) => {
        if (id === mine) {
          lendError = String(e)
          if (menu === 'lend') growForMenu()
        }
      })
    await toggle('lend')
  }

  /** Une action du prêt : Admin répond avec le prêt à jour (celui de la session qui l'a demandée). */
  async function lendAction(call: () => Promise<LendPanel>) {
    if (lendActing) return
    lendActing = true
    lendError = null
    const mine = id
    try {
      const view = await call()
      lends.set(mine, view)
      if (id === mine) lend = view
    } catch (e) {
      if (id === mine) lendError = String(e)
    } finally {
      lendActing = false
    }
    await growForMenu()
  }

  const createCode = () =>
    lendAction(() => invoke<LendPanel>('lend_code', { id, duration: lendDuration === 'none' ? null : Number(lendDuration), steam: lendSteam }))
  const cancelCode = () => lendAction(() => invoke<LendPanel>('lend_cancel_code', { id }))
  /** « Accepter » (avec le compte Steam choisi) ou « Refuser » une demande d'emprunt. */
  const answer = (r: LendRequest, accept: boolean) => lendAction(() => invoke<LendPanel>('lend_answer', { id, request: r.id, accept, steam: lendSteam }))
  /** « iPhone ou iPad · hors de chez vous » : sous « {prénom} veut emprunter l'ordinateur. » */
  const lendRequestMeta = (r: LendRequest) => [apps.includes(r.app) ? t(`share.app_${r.app}`) : '', r.away ? t('share.away') : ''].filter(Boolean).join(' · ')

  /** « Copier le lien » du code de prêt : l'adresse du client web, comme le partage (et comme lui, après le lien du relais). */
  const copyLendLink = () =>
    copyText(lend?.code?.web || lend?.code?.link || '', lendRelay.value, (r) => (lendCopied = r), () => lendCopied)

  /** L'ami qui emprunte : « Encore 1 h 20 » (jusqu'à la fin du préavis s'il y en a un), ou « Jusqu'à ce que {hôte} reprenne ». */
  const borrowLeft = $derived.by(() => {
    if (!borrow) return ''
    const end = borrow.notice?.at ?? borrow.until
    return end ? leftText(end, now / 1000) : t('lend.until_taken', { host: borrow.host })
  })

  /** « Rendre l'ordinateur », confirmé : le prêt finit pour de bon, puis la session. */
  async function giveBack() {
    await closeMenu()
    try {
      await invoke('lend_give_back', { id })
    } catch (e) {
      showNotice(String(e))
    }
  }
</script>

<!--
  Le code et son lien, partagés par « Partager » et « Prêter l'ordinateur… » : « 123 456 », « Copier le lien », le QR code
  (un indicateur d'attente tant que le lien du relais se prépare) et, copie impossible, l'adresse à copier à la main.
-->
{#snippet codeTop(code: string, qr: ShareQr | null, address: string, phase: RelayPhase, done: 'ok' | 'fail' | null, copy: () => void)}
  <div class="top">
    <div class="codebox">
      <span class="eyebrow">{t('share.code')}</span>
      <span class="digits mono" aria-label={t('share.code') + ' ' + code}>{code}</span>
      <button class="ghost" onclick={copy} disabled={phase === 'waiting'}><Icon name="copy" size={16} stroke={1.6} />{t('share.copy_link')}</button>
      {#if done === 'ok'}<span class="copied" role="status">{t('share.link_copied')}</span>{/if}
    </div>
    {#if phase === 'waiting'}
      <div class="qr qr-wait" role="img" aria-label={t('share.relay_preparing')}><span class="spin big"></span></div>
    {:else if qr}
      <svg class="qr" role="img" aria-label={t('share.qr')} viewBox="-4 -4 {qr.size + 8} {qr.size + 8}" shape-rendering="crispEdges">
        <rect x="-4" y="-4" width={qr.size + 8} height={qr.size + 8} fill="#fff" />
        <path d={qr.path} fill="#000" />
      </svg>
    {/if}
  </div>
  {#if done === 'fail'}
    <!-- Copie impossible : l'adresse reste là, sélectionnée d'un clic. -->
    <p class="note refusal" role="alert">{t('share.copy_failed')}</p>
    <input class="field link mono" readonly value={address} onfocus={selectAll} onclick={selectAll} aria-label={t('share.copy_link')} />
  {/if}
{/snippet}

<!-- Sous l'aide du même réseau : « Préparation du lien hors de chez vous… », puis « Pas encore joignable hors de chez vous. ». -->
{#snippet relayLine(phase: RelayPhase)}
  {#if phase === 'waiting'}
    <p class="help" role="status"><span class="spin" aria-hidden="true"></span>{t('share.relay_preparing')}</p>
  {:else if phase === 'unreachable'}<p class="help">{t('share.relay_down')}</p>{/if}
{/snippet}

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
  <!-- Un avis bref (changement de rôle d'un invité) prend la place des statistiques, sans changer la largeur de la barre. -->
  <div class="slot">
    <div class="stats" class:hidden={!!notice} role="group" aria-label={t('bar.statistics')} title={t('bar.statistics')}>
      <div class="lat" title={stats?.audio_ms != null ? t('bar.audio', { ms: ms(stats.audio_ms, 0), gaps: stats.audio_gaps ?? 0 }) : undefined}>
        <span class="mono">{ms(stats?.latency_ms)} ms</span>
        <svg width="70" height="20" viewBox="0 0 70 20" aria-hidden="true"><polyline points={spark} fill="none" stroke="#9A8CFF" stroke-width="1.5" /></svg>
      </div>
      <span class="sep"></span>
      <span class="mono info">{stats ? infoLine(stats) : '—'}</span>
    </div>
    {#if notice && noticePanel}
      <!-- Une demande à rejoindre : choisir l'avis ouvre le panneau de « Partager ». -->
      <button class="notice choice" aria-live="polite" onclick={openShareFromNotice}>{notice}</button>
    {:else if notice}
      <div class="notice" role="status">{notice}</div>
    {/if}
  </div>
  <!-- Par le relais (hors de chez vous) : toujours visible, dans la barre. -->
  {#if route?.via_relay}
    <span class="relay" class:tcp={route.transport === 'relay_tcp'} title={routeTitle}><Icon name="globe" size={14} stroke={1.6} />{t('relay.via')}</span>
  {/if}
  <span class="sep"></span>
  {#if borrow}
    <!-- L'ami qui emprunte : « Prêté par {hôte} » et le temps restant, à la place du profil (rien n'est changé sur l'ordinateur). -->
    <span class="role play">{t('lend.lent_by', { host: borrow.host })}</span>
    <span class="left" class:soon={!!borrow.notice} role="timer">{borrowLeft}</span>
  {:else if guest}
    <!-- Invité d'un partage : son rôle, à la place du profil (ni profil ni réglages). -->
    <span class="role" class:play={rolePlays(guest.role)}>{roleName(guest.role)}</span>
    <!-- « Qualité de l'image » : montre la qualité reçue, et permet d'en changer. -->
    <div class="anchor">
      <button
        class="chip"
        class:open={menu === 'quality'}
        aria-haspopup="menu"
        aria-expanded={menu === 'quality'}
        aria-label={t('share.quality') + ' : ' + shownQuality}
        title={t('share.quality')}
        onclick={() => toggle('quality')}
      >
        {shownQuality}<span class="caret" aria-hidden="true">▾</span>
      </button>
      {#if menu === 'quality'}
        <div class="menu quality" role="menu" aria-label={t('share.quality')}>
          <div class="menu-title" aria-hidden="true">{t('share.quality')}</div>
          {#each shareQualities as q (q)}
            <button role="menuitemradio" aria-checked={q === guest.quality} onclick={() => pickQuality(q)}>
              <span class="tick">{#if q === guest.quality}<Icon name="check" size={14} stroke={2.2} />{/if}</span>
              <span class="name">{qualityName(q)}</span><span class="hint">{receivedFor(q)}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
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
  {/if}
  <!-- Un invité n'a le clavier et la souris que si l'hôte les lui a donnés. -->
  {#if !guest || roleKbm(guest.role)}
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
  {/if}
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
  {#if !guest && !borrow}
  <!-- KyberNow Share : « Partager », présenté comme les autres menus (icône seule, libellé accessible) ;
       inactif, il dit pourquoi dans son panneau ; une pastille d'invités rouvre le panneau. -->
  <div class="anchor" class:withpill={guestCount > 0 || requestCount > 0}>
    <button
      class="icon"
      class:on={menu === 'share' || !!share?.open}
      class:inert={blocked}
      aria-haspopup="dialog"
      aria-expanded={menu === 'share'}
      aria-disabled={blocked}
      aria-label={t('share.share')}
      title={blocked && share?.text ? share.text : t('share.share')}
      disabled={opening}
      onpointerdown={(e) => (shareTouch = e.pointerType === 'touch' || e.pointerType === 'pen')}
      onclick={toggleShare}
    >
      <Icon name="share" size={20} stroke={1.6} />
    </button>
    {#if guestCount > 0}
      <button class="pill" aria-haspopup="dialog" aria-expanded={menu === 'share'} onclick={() => toggle('share')}>{t(guestCount === 1 ? 'share.guests_one' : 'share.guests_other', { count: guestCount })}</button>
    {/if}
    {#if requestCount > 0}
      <!-- « 1 demande » : à côté des invités, elle rouvre le panneau ; une demande n'est pas un invité. -->
      <button class="pill ask" aria-haspopup="dialog" aria-expanded={menu === 'share'} onclick={() => toggle('share')}>{t(requestCount === 1 ? 'share.requests_one' : 'share.requests_other', { count: requestCount })}</button>
    {/if}
    {#if menu === 'share'}
      <div class="menu share" role="dialog" aria-label={t('share.title', { name: computerName })}>
        {#if share?.open}
          <h3>{t('share.title', { name: computerName })}</h3>
          {@render codeTop(share.code, share.qr, share.web || share.link, relay, copied, copyLink)}
          <!-- Relais joint : le partage vaut aussi hors de chez vous, et l'hôte accepte chaque invité venu de loin. -->
          <p class="help">{t(relay === 'joined' ? 'share.help_relay' : 'share.help')}</p>
          {@render relayLine(relay)}
          {#if share.requests.length > 0}
            <div class="eyebrow list-title">{t('share.requests')}</div>
            <ul class="guests requests">
              {#each share.requests as g (g.id)}
                <li>
                  <div class="who">
                    <span class="gname">{t('share.wants_to_join', { name: g.name })}</span>
                    <span class="gmeta">{requestMeta(g)}</span>
                  </div>
                  <div class="acts">
                    <!-- Quatre invités ont leur place : « Accepter » est inactif (la raison est dessous : « Quatre invités au plus. »), « Refuser » reste. -->
                    <button class="accept" aria-disabled={acting || share.full} onclick={() => accept(g)}>{t('share.accept')}</button>
                    <button class="danger" aria-disabled={acting} onclick={() => refuse(g)}>{t('share.decline')}</button>
                  </div>
                </li>
              {/each}
            </ul>
          {/if}
          <div class="eyebrow list-title">{t('share.guests')}</div>
          {#if share.guests.length === 0}
            <p class="nobody">{t('share.nobody')}</p>
          {:else}
            <ul class="guests">
              {#each share.guests as g (g.id)}
                <li>
                  <div class="who">
                    <span class="gname">{t(`share.guest_${g.role}`, { name: g.name })}</span>
                    <span class="gmeta">{guestMeta(g)}</span>
                  </div>
                  <div class="acts">
                    <!-- Deux droits indépendants, chacun donné ou repris sans toucher à l'autre : les manettes, puis le
                         clavier et la souris. Pas d'autre clic tant qu'Admin n'a pas répondu : le rôle qui part se
                         déduit de celui qui est affiché. -->
                    <button aria-disabled={acting} onclick={() => setRole(g, roleOf(!rolePads(g.role), roleKbm(g.role)))}>{t(rolePads(g.role) ? 'share.take_pads' : 'share.give_pads')}</button>
                    <button aria-disabled={acting} onclick={() => setRole(g, roleOf(rolePads(g.role), !roleKbm(g.role)))}>{t(roleKbm(g.role) ? 'share.take_kbm' : 'share.give_kbm')}</button>
                    <button class="danger" aria-disabled={acting} onclick={() => kick(g)}>{t('share.remove')}</button>
                  </div>
                </li>
              {/each}
            </ul>
          {/if}
          {#if share.full}<p class="note" role="note">{t('share.limit')}</p>{/if}
          <div class="foot">
            <button class="ghost" onclick={newCode}>{t('share.new_code')}</button>
            <button class="ghost danger" onclick={stopSharing}>{t('share.stop')}</button>
          </div>
        {:else if share?.text}
          <!-- « Partager » inactif : Station trop ancienne, partage coupé sur l'ordinateur. -->
          <p class="note" role="note">{share.text}</p>
        {/if}
        {#if shownError}<p class="note refusal" role="alert">{shownError}</p>{/if}
      </div>
    {/if}
  </div>
  {/if}
  {#if !guest && !borrow}
  <!-- Prêt de l'ordinateur : « Prêter l'ordinateur… », présenté comme « Partager » ; une pastille rouvre le panneau des demandes. -->
  <div class="anchor" class:withpill={lendRequests > 0}>
    <button
      class="icon"
      class:on={menu === 'lend' || !!lend?.code}
      class:inert={lendBlocked}
      aria-haspopup="dialog"
      aria-expanded={menu === 'lend'}
      aria-disabled={lendBlocked}
      aria-label={t('lend.lend')}
      title={lendBlocked && lendText ? lendText : t('lend.lend')}
      onpointerdown={(e) => (lendTouch = e.pointerType === 'touch' || e.pointerType === 'pen')}
      onclick={toggleLend}
    >
      <Icon name="gamepad" size={20} stroke={1.6} />
    </button>
    {#if lendRequests > 0}
      <button class="pill ask" aria-haspopup="dialog" aria-expanded={menu === 'lend'} onclick={toggleLend}>{t(lendRequests === 1 ? 'share.requests_one' : 'share.requests_other', { count: lendRequests })}</button>
    {/if}
    {#if menu === 'lend'}
      <div class="menu share lend" role="dialog" aria-label={t('lend.panel_title', { name: computerName })}>
        {#if lendText}
          <p class="note" role="note">{lendText}</p>
        {:else}
          <h3>{t('lend.panel_title', { name: computerName })}</h3>
          {#if lend?.notice?.reason === 'slot_start' && lend.notice.text && !slotAsked}<p class="help" role="status">{lend.notice.text}</p>{/if}
          {#if lend && lend.requests.length > 0}
            <div class="eyebrow list-title">{t('share.requests')}</div>
            <ul class="guests requests">
              {#each lend.requests as r (r.id)}
                <li>
                  <div class="who">
                    <!-- Pendant son créneau, l'ami passe avant : le texte du créneau, et « Accepter » seul (sans « Refuser »). -->
                    <span class="gname">{r.kind === 'slot' ? t('lend.slot_notice', { name: r.name }) : t('lend.wants_to_borrow', { name: r.name })}</span>
                    {#if lendRequestMeta(r)}<span class="gmeta">{lendRequestMeta(r)}</span>{/if}
                  </div>
                  <div class="acts">
                    <button class="accept" aria-disabled={lendActing} onclick={() => answer(r, true)}>{t('share.accept')}</button>
                    {#if r.kind !== 'slot'}<button class="danger" aria-disabled={lendActing} onclick={() => answer(r, false)}>{t('share.decline')}</button>{/if}
                  </div>
                </li>
              {/each}
            </ul>
            <p class="help">{t('lend.accept_closes')}</p>
          {/if}
          {#if lend?.code}
            {@render codeTop(lend.code.code, lend.code.qr, lend.code.web || lend.code.link, lendRelay.value, lendCopied, copyLendLink)}
            <p class="help">{t('lend.code_valid')}</p>
            <p class="help">{t('lend.code_help')}</p>
            {@render relayLine(lendRelay.value)}
            <p class="help strong">{t('lend.session_closes')}</p>
            <div class="foot">
              <button class="ghost danger" aria-disabled={lendActing} onclick={cancelCode}>{t('lend.cancel_code')}</button>
            </div>
          {:else if !lend?.borrower}
            <div class="col">
              <span class="eyebrow">{t('lend.duration')}</span>
              <Segmented
                label={t('lend.duration')}
                options={lendDurations.map((d) => ({ value: d === null ? 'none' : String(d), label: t(`lend.duration_${d ?? 'none'}`) }))}
                value={lendDuration}
                disabled={lendActing}
                onchange={(v) => (lendDuration = v)}
              />
            </div>
            <div class="col">
              <span class="eyebrow">{t('lend.steam')}</span>
              <Segmented
                label={t('lend.steam')}
                options={[
                  { value: 'friend' as const, label: t('lend.steam_friend') },
                  { value: 'host' as const, label: t('lend.steam_host') },
                ]}
                value={lendSteam}
                disabled={lendActing}
                onchange={(v) => (lendSteam = v)}
              />
            </div>
            {#if trustOnce}<p class="note" role="note">{t('lend.trust')}</p>{/if}
            <div class="foot">
              <button class="primary" disabled={lendActing} onclick={createCode}>{t('lend.create_code')}</button>
            </div>
          {/if}
        {/if}
        {#if lendError && lendError !== lendText}<p class="note refusal" role="alert">{lendError}</p>{/if}
      </div>
    {/if}
  </div>
  {/if}
  {#if !guest && !borrow && others.length > 0}
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
  {#if borrow}
    <!-- « Se déconnecter » ferme la session sans rendre l'ordinateur (l'ami peut revenir pendant 15 min) ;
         « Rendre l'ordinateur », confirmé, le rend. -->
    <button class="quit soft" title={t('bar.disconnect') + t('bar.disconnect_shortcut')} onclick={() => command('disconnect')}>{t('bar.disconnect')}</button>
    <div class="anchor">
      <button class="quit" aria-haspopup="dialog" aria-expanded={menu === 'giveback'} onclick={() => toggle('giveback')}>{t('lend.give_back')}</button>
      {#if menu === 'giveback'}
        <div class="menu giveback" role="alertdialog" aria-label={t('lend.give_back_confirm', { host: borrow.host })}>
          <p class="confirm">{t('lend.give_back_confirm', { host: borrow.host })}</p>
          <div class="foot">
            <button class="danger-solid" onclick={giveBack}>{t('lend.give_back')}</button>
            <button class="ghost" onclick={() => closeMenu().then(afterMenu)}>{t('common.cancel')}</button>
          </div>
        </div>
      {/if}
    </div>
  {:else}
    <button class="quit" title={(guest ? t('share.leave') : t('bar.disconnect')) + t('bar.disconnect_shortcut')} onclick={() => command('disconnect')}>{guest ? t('share.leave') : t('bar.disconnect')}</button>
  {/if}
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
    /* La fenêtre de la barre a une largeur fixe : trop de choses à la fois (puce du relais, pastilles
       d'invités et de demandes) ne rognent jamais les boutons, c'est la ligne des mesures qui se raccourcit. */
    min-width: 0;
    max-width: 100%;
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
    min-width: 0;
  }

  .lat {
    display: flex;
    flex: none;
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
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
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

  /* Les statistiques et, à leur place pour quelques secondes, l'avis d'un changement de rôle. */
  .slot {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    align-items: center;
    min-width: 0;
  }

  .slot > * {
    grid-area: 1 / 1;
  }

  .stats.hidden {
    visibility: hidden;
  }

  .notice {
    padding: 0 8px;
    font-size: 14px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    animation: drop 0.25s var(--ease-out);
  }

  /* L'avis d'une demande à rejoindre se choisit : il ouvre le panneau de « Partager ». */
  .notice.choice {
    border: 0;
    background: none;
    font: inherit;
    font-size: 14px;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }

  .notice.choice:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  /* « Qualité de l'image » de l'invité : le menu s'aligne sur sa puce, avec son titre. */
  .menu.quality {
    left: 0;
    transform: none;
    min-width: 260px;
  }

  .menu-title {
    padding: 6px 10px 8px;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--faint);
  }

  /* Rôle de l'invité, à la place du profil : « Spectateur », « Joueur ». */
  .role {
    height: 34px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    border: 1px solid var(--line-2);
    border-radius: 8px;
    font-size: 13px;
    color: var(--muted);
  }

  .role.play {
    border-color: var(--signal);
    background: var(--ink-3);
    color: var(--text);
  }

  /* « Partager » inactif : estompé ; à la souris sans effet (l'infobulle dit pourquoi), au toucher il ouvre la raison. */
  .icon.inert,
  .icon.inert:hover {
    background: none;
    color: var(--ghost);
  }

  /* « Par le relais » : la session passe par internet (hors de chez vous) ; le repli TCP s'ambre. */
  .relay {
    height: 26px;
    padding: 0 10px 0 8px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--line-2);
    border-radius: 999px;
    font-size: 12px;
    color: var(--muted);
  }

  .relay.tcp {
    border-color: var(--warn);
    color: var(--warn);
  }

  /* Pastille d'invités (« 2 invités »), à droite de « Partager » : rouvre le panneau. */
  .anchor.withpill {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .pill {
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--signal);
    border-radius: 999px;
    background: var(--signal-soft);
    color: var(--text);
    font-size: 12px;
  }

  .pill:hover {
    background: var(--ink-3);
  }

  /* « 1 demande » : à attendre de l'hôte, d'un autre ton que les invités. */
  .pill.ask {
    border-color: var(--warn);
    background: none;
  }

  .pill.ask:hover {
    background: var(--ink-3);
  }

  /* Panneau de l'hôte : à droite de la barre, aligné sur son bouton, comme « Ordinateurs ». */
  .menu.share {
    left: auto;
    right: -8px;
    transform: none;
    width: 400px;
    padding: 16px;
    gap: 12px;
    white-space: normal;
    /* Un petit écran : le panneau défile dans la fenêtre de la barre (qui ne dépasse pas l'écran). */
    max-height: calc(100vh - 90px);
    overflow-y: auto;
  }

  .menu.share h3 {
    margin: 0;
    font-size: 17px;
    font-weight: 640;
  }

  .menu.share .top {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
  }

  .menu.share .codebox {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .menu.share .digits {
    font-size: 32px;
    letter-spacing: 0.08em;
    user-select: text;
  }

  .menu.share .qr {
    flex: none;
    width: 150px;
    height: 150px;
    border-radius: 8px;
  }

  .menu.share .ghost {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border: 1px solid var(--line-2);
    border-radius: 8px;
    background: none;
    color: var(--muted);
    font-size: 13px;
    grid-template-columns: none;
  }

  .menu.share .ghost:hover {
    color: var(--text);
    border-color: var(--faint);
    background: none;
  }

  /* « Copier le lien » attend le lien du relais. */
  .menu.share .ghost:disabled {
    opacity: 0.5;
    cursor: default;
    pointer-events: none;
  }

  /* Lien du relais en préparation : un indicateur d'attente, à la place du QR code et devant le message. */
  .menu.share .qr-wait {
    display: grid;
    place-items: center;
  }

  .spin {
    display: inline-block;
    width: 12px;
    height: 12px;
    margin-right: 8px;
    vertical-align: -2px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, currentColor 20%, transparent);
    border-top-color: var(--signal, currentColor);
    animation: spin 0.8s linear infinite;
  }

  .spin.big {
    width: 32px;
    height: 32px;
    margin: 0;
    border-width: 3px;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation-duration: 2.4s;
    }
  }

  .menu.share .ghost.danger,
  .menu.share .danger {
    color: #ff8a8a;
  }

  .copied {
    font-size: 12px;
    color: var(--ok);
  }

  .menu.share .field.link {
    height: 36px;
    font-size: 12px;
  }

  .menu.share .help,
  .menu.share .nobody {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }

  .menu.share .list-title {
    margin-top: 2px;
  }

  .menu.share .guests {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .menu.share .guests li {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--ink-2);
  }

  .menu.share .who {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .menu.share .gname {
    font-size: 13px;
    font-weight: 600;
  }

  .menu.share .gmeta {
    font-size: 12px;
    color: var(--faint);
  }

  .menu.share .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  /* Les actions sur un invité : de petits boutons, pas des lignes de menu. */
  .menu.share .acts button {
    display: inline-flex;
    padding: 5px 10px;
    border: 1px solid var(--line-2);
    border-radius: 7px;
    font-size: 12px;
    grid-template-columns: none;
  }

  .menu.share .acts button:hover {
    border-color: var(--faint);
  }

  /* « Accepter » : l'action que l'on attend d'une demande. */
  .menu.share .acts button.accept {
    border-color: var(--signal);
    background: var(--signal-soft);
  }

  /* Une action attend la réponse d'Admin : les autres patientent. */
  .menu.share .acts button[aria-disabled="true"] {
    opacity: 0.5;
    pointer-events: none;
  }

  .menu.share .foot {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding-top: 4px;
    border-top: 1px solid var(--line);
  }

  .menu.share .foot .ghost {
    margin-top: 8px;
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

  /* L'ami : « Se déconnecter » (on peut revenir) reste discret à côté de « Rendre l'ordinateur ». */
  .quit.soft {
    color: var(--muted);
  }

  .quit.soft:hover {
    color: var(--text);
    background: rgb(255 255 255 / 0.06);
  }

  /* Prêt de l'ordinateur : le temps restant de l'ami, et le panneau de l'hôte (les styles du partage, en plus). */
  .left {
    padding: 0 6px;
    font-size: 13px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .left.soon {
    color: var(--warn);
  }

  .menu.lend .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .menu.lend .help.strong {
    color: var(--text);
  }

  .menu.lend .foot .primary {
    height: 40px;
    margin-top: 8px;
    padding: 0 18px;
    background: var(--signal-strong);
    color: #fff;
    font-weight: 600;
  }

  .menu.giveback {
    right: 0;
    left: auto;
    transform: none;
    width: 320px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .menu.giveback .confirm {
    margin: 0;
    font-size: 14px;
    white-space: normal;
  }

  .menu.giveback .foot {
    display: flex;
    gap: 8px;
  }

  .menu.giveback button,
  .menu.lend .foot .primary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: auto;
    height: 36px;
    padding: 0 14px;
    border-radius: 8px;
  }

  .menu.giveback .danger-solid {
    background: var(--bad);
    color: #fff;
    font-weight: 600;
  }
</style>
