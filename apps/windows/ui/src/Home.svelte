<!--
  Accueil (docs/link-ui.md) : l'ordinateur choisi en plein cadre (son image,
  sinon sa signature) avec, dans l'ordre, son état, son nom, son système,
  l'écran que la session demandera, le bouton principal (hors ligne :
  « Réveiller l'ordinateur » quand ses cartes réseau sont connues, sinon
  « Réessayer » ; déjà en session : « Afficher la session » ; quatre
  ordinateurs en session : « Se connecter » inactif, avec la raison), le
  profil et l'accès à ses réglages ; en bas les autres ordinateurs (nom et
  état), ceux annoncés sur le réseau et l'ajout.

  Hors de chez vous (docs/relais.md) : un ordinateur qui ne répond pas sur le réseau mais que le
  relais tient est « Hors de chez vous » à la place de « En ligne » (l'app le demande au relais) ;
  « Se connecter » passe alors par le relais.

  Prêt de l'ordinateur (docs/link-ui.md « Prêt de l'ordinateur ») : les ordinateurs empruntés à
  des amis ont leur carte « Ordinateur de {hôte} » après vos ordinateurs (son état : « Disponible »,
  « Occupé »…) ; choisie, elle montre son état, son créneau, « Emprunter » (occupé : « Demander à
  {hôte} ») et « Retirer ». Un de vos ordinateurs prêté à un ami dit « Prêté à {prénom} » (jusqu'à
  18 h) et propose « Reprendre l'ordinateur » ; pendant le préavis, « Reprise dans 1:42 » et
  « Reprendre tout de suite ».
-->
<script lang="ts">
  import Icon from './lib/Icon.svelte'
  import Mark from './lib/Mark.svelte'
  import { onMount } from 'svelte'
  import type { Battery, Computer, DisplaysInfo, Found, Friend, Settings, ThisPc } from './lib/api'
  import { MAX_SESSIONS, clock, countdown, friendStateText, hue, invoke, isCustom, listen, sessionPlan, slotText } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    computers,
    discovered,
    thisPc,
    battery,
    selected,
    friends = [],
    selectedFriend = null,
    sessions = [],
    busy,
    retrying,
    error,
    onselect,
    onconnect,
    onshow,
    onretry,
    onwake,
    onsettings,
    onprofile,
    onadd,
    onjoin,
    onborrow,
    onremovefriend,
    ontakeback,
  }: {
    computers: Computer[]
    discovered: Found[]
    thisPc: ThisPc | null
    battery: Battery
    selected: Computer | null
    /** Les ordinateurs empruntés à des amis (« Ordinateur de {hôte} »). */
    friends?: Friend[]
    /** L'ordinateur d'un ami choisi (à la place d'un de vos ordinateurs). */
    selectedFriend?: Friend | null
    /** Les ordinateurs en session (plusieurs ordinateurs, quatre au plus). */
    sessions?: string[]
    /** La connexion de la première session se fait (passage). */
    busy: boolean
    /** « Réessayer » en cours pour cet ordinateur. */
    retrying: boolean
    error: string | null
    onselect: (id: string) => void
    onconnect: (id: string) => void
    /** « Afficher la session » : sa fenêtre passe devant. */
    onshow: (id: string) => void
    onretry: (id: string) => void
    onwake: (id: string) => void
    onsettings: (id: string) => void
    onprofile: (id: string, p: Settings['profile']) => void
    onadd: (found?: Found) => void
    /** « Rejoindre avec un code » (KyberNow Share) : à côté de « Ajouter un ordinateur ». */
    onjoin: () => void
    /** « Emprunter » ou « Demander à {hôte} » : l'ordinateur d'un ami. */
    onborrow: (f: Friend) => void
    /** « Retirer » l'ordinateur d'un ami de cet appareil. */
    onremovefriend: (id: string) => void
    /** « Reprendre l'ordinateur » (`now` faux : préavis de 2 min) ou « Reprendre tout de suite ». */
    ontakeback: (id: string, now: boolean) => void
  } = $props()

  const fresh = $derived(discovered.filter((f) => !computers.some((c) => c.host === f.host && c.port === f.port)))

  const stateOf = (c: Computer, searching = false) =>
    c.waking && !c.online
      ? t('home.waking')
      : searching || c.online == null
        ? t('common.searching')
        : c.online
          ? t(c.away ? 'home.away' : 'home.online')
          : t('home.offline')

  const waking = $derived(!!selected?.waking && !selected.online)
  /** L'ordinateur choisi est en session : « Afficher la session ». */
  const inSession = $derived(!!selected && sessions.includes(selected.id))
  /** Quatre ordinateurs sont déjà en session : « Se connecter » inactif, avec la raison. */
  const full = $derived(sessions.length >= MAX_SESSIONS)
  /** Hors ligne, l'app sait le réveiller : ses cartes réseau, apprises de la Station. */
  const canWake = $derived(!!selected && (selected.wake?.length ?? 0) > 0)

  const status = $derived.by(() => {
    if (!selected) return null
    if (waking) return { text: t('home.waking'), tone: 'wait' }
    if (retrying || selected.online == null) return { text: t('common.searching'), tone: 'wait' }
    return selected.online ? { text: t(selected.away ? 'home.away' : 'home.online'), tone: 'ok' } : { text: t('home.offline'), tone: 'off' }
  })

  // Les écrans que la session prendra : le calcul du cœur, le même que la session (réglage « Écrans »,
  // profil, Station, écrans de ce PC). Revu quand l'ordinateur, ses réglages ou les écrans de ce PC changent.
  let screens = $state(1)
  // L'écran virtuel servira (demandé et utilisable sur la Station), d'après le cœur.
  let virtualDisplay = $state(true)
  let screensEpoch = $state(0)
  let asked = 0
  $effect(() => {
    void screensEpoch
    void battery
    void thisPc
    if (!selected) {
      screens = 1
      return
    }
    const id = selected.id
    const settings = $state.snapshot(selected.settings)
    void selected.info?.virtual_display?.max_screens
    void selected.info?.virtual_display?.available
    const n = ++asked
    invoke<DisplaysInfo>('displays_info', { id, settings })
      .then((i) => {
        if (n !== asked) return
        screens = i.count
        virtualDisplay = i.virtual_display
      })
      .catch(() => {})
  })
  onMount(() => {
    const off = listen<null>('displays', () => screensEpoch++)
    return () => off.then((f) => f())
  })

  /** L'écran que la session demandera (même calcul que plan.rs : au plus 4K, H.264, sur batterie), ou le nombre d'écrans. */
  const screen = $derived.by(() => {
    if (!selected || !thisPc) return null
    const s = selected.settings
    if (!virtualDisplay) return t('home.screens_as_is')
    if (screens > 1) return t('home.screens_many', { count: screens })
    const p = sessionPlan(s, thisPc, battery)
    if (!p) return null
    const params = { width: p.width, height: p.height, refresh: p.refresh }
    return !s.custom && p.width === thisPc.width && p.height === thisPc.height ? t('home.screen_match', params) : t('home.screen', params)
  })

  const custom = $derived(selected ? isCustom(selected.settings, thisPc) : false)
  const profiles: { value: Settings['profile']; label: string }[] = $derived([
    { value: 'jeu', label: t('profiles.jeu') },
    { value: 'bureau', label: t('profiles.bureau') },
    { value: 'max', label: t('profiles.max') },
  ])
  const sentence = $derived(selected ? (custom ? t('profiles.custom_sub') : t(`profiles.${selected.settings.profile}_sub`)) : '')

  // Prêt de l'ordinateur : l'heure, pour « Reprise dans 1:42 » (revue chaque seconde pendant un préavis).
  let now = $state(Date.now() / 1000)
  $effect(() => {
    if (!selected?.lend?.notice_at) return
    const timer = setInterval(() => (now = Date.now() / 1000), 1000)
    return () => clearInterval(timer)
  })
  /** Votre ordinateur prêté à un ami : « Prêté à Léa », « Prêté à Léa jusqu'à 18 h ». */
  const lentText = $derived(
    selected?.lend ? (selected.lend.until ? t('lend.lent_to_until', { name: selected.lend.name, time: clock(selected.lend.until) }) : t('lend.lent_to', { name: selected.lend.name })) : '',
  )
  /** « Reprise dans 1:42 » : le préavis de la reprise en cours. */
  const takingBack = $derived(selected?.lend?.notice_at && selected.lend.notice_at > now ? t('lend.taking_back', { time: countdown(selected.lend.notice_at, now) }) : null)
  /** « Retirer » l'ordinateur d'un ami : confirmé d'un second clic. */
  let confirmRemove = $state<string | null>(null)
  /** Ce que l'ami peut faire de l'ordinateur : l'emprunter, le demander à l'hôte, ou rien (prêté, verrouillé, coupé, hors ligne). */
  const canBorrow = (f: Friend) => ['free', 'busy', 'password', 'unknown', 'refused'].includes(f.state)
  const friendTone = (f: Friend) => (f.state === 'free' ? 'ok' : f.state === 'unknown' ? 'wait' : f.state === 'busy' ? 'busy' : 'off')

  const signature = (fp: string | null) =>
    `radial-gradient(900px 600px at 78% 22%, hsl(${hue(fp)} 90% 62% / 0.38), transparent 70%), radial-gradient(700px 520px at 18% 78%, hsl(${hue(fp, 7)} 70% 55% / 0.16), transparent 70%), #070a0e`
</script>

<main>
  <div class="backdrop" class:offline={selected?.online === false}>
    {#if selected?.preview}
      {#key selected.preview}<img src={selected.preview} alt="" />{/key}
    {:else}
      <div class="signature" style:background={signature(selected?.fingerprint ?? null)}></div>
      <div class="watermark"><Mark size={760} state="console" seam="transparent" /></div>
    {/if}
    <div class="shade"></div>
  </div>

  <div class="layout">
  <section class="hero">
    {#if selectedFriend}
      {@const f = selectedFriend}
      {@const slot = slotText(f.slot)}
      {#if friendStateText(f.state)}<div class="eyebrow status {friendTone(f)}"><span class="dot"></span>{friendStateText(f.state)}</div>{/if}
      <h1>{t('lend.computer_of', { host: f.owner })}</h1>
      {#if f.computer}<div class="system">{f.computer}</div>{/if}
      {#if error}<div class="error" role="alert">{error}</div>{/if}
      {#if slot}
        <div class="chip"><Icon name="restart" size={20} /><span>{slot}</span></div>
      {/if}
      <div class="actions">
        <button class="primary" disabled={busy || full || !canBorrow(f)} aria-describedby={full ? 'four-at-most' : undefined} onclick={() => onborrow(f)}>
          <Icon name="play" size={18} />{f.state === 'busy' ? t('lend.ask', { host: f.owner }) : t('lend.borrow')}
        </button>
        <!-- « Retirer cet ordinateur », confirmé : « Retirer l'ordinateur de {hôte} de cet appareil ? » -->
        {#if confirmRemove === f.id}
          <button class="gear danger" onclick={() => ((confirmRemove = null), onremovefriend(f.id))}>{t('station.remove')}</button>
          <button class="gear" onclick={() => (confirmRemove = null)}>{t('common.cancel')}</button>
        {:else}
          <button class="gear" onclick={() => (confirmRemove = f.id)}><Icon name="trash" size={18} stroke={1.6} />{t('lend.remove')}</button>
        {/if}
      </div>
      {#if confirmRemove === f.id}<p class="sentence">{t('lend.remove_confirm', { host: f.owner })}</p>{/if}
      {#if full}<p class="sentence limit" id="four-at-most">{t('common.four_at_most')}</p>{/if}
    {:else if selected}
      {#if selected.lend && selected.online !== false}
        <div class="eyebrow status lent"><span class="dot"></span>{lentText}</div>
      {:else}
        <div class="eyebrow status {status?.tone}"><span class="dot"></span>{status?.text}</div>
      {/if}
      <h1>{selected.name}</h1>
      {#if selected.info?.os || selected.os}<div class="system">{selected.info?.os || selected.os}</div>{/if}
      {#if error}<div class="error" role="alert">{error}</div>{/if}
      {#if screen}
        <div class="chip"><Icon name="monitor" size={20} /><span>{screen}</span></div>
      {/if}
      <div class="actions">
        {#if selected.lend && selected.online !== false}
          <!-- Prêté à un ami : « Reprendre l'ordinateur » ; pendant le préavis, « Reprise dans 1:42 » et « Reprendre tout de suite ». -->
          {#if takingBack}
            <span class="chip countdown" role="timer">{takingBack}</span>
            <button class="primary" onclick={() => ontakeback(selected!.id, true)}><Icon name="power" size={18} />{t('lend.take_back_now')}</button>
          {:else}
            <button class="primary" onclick={() => ontakeback(selected!.id, false)}><Icon name="power" size={18} />{t('lend.take_back')}</button>
          {/if}
        {:else if inSession}
          <button class="primary" onclick={() => onshow(selected!.id)}>
            <Icon name="monitor" size={18} />{t('home.show_session')}
          </button>
        {:else if selected.online === false && canWake}
          <button class="primary" disabled={busy || waking} onclick={() => onwake(selected!.id)}>
            <Icon name="power" size={18} />{waking ? t('home.waking') : t('home.wake')}
          </button>
        {:else if selected.online === false}
          <button class="primary" disabled={busy || retrying} onclick={() => onretry(selected!.id)}>
            <Icon name="restart" size={18} />{t('home.retry')}
          </button>
        {:else}
          <button class="primary" disabled={busy || full} aria-describedby={full ? 'four-at-most' : undefined} onclick={() => onconnect(selected!.id)}>
            <Icon name="play" size={18} />{t('home.connect')}
          </button>
        {/if}
        <div class="profiles" role="radiogroup" aria-label={t('home.profile')}>
          {#each profiles as p (p.value)}
            {@const on = !custom && selected.settings.profile === p.value}
            <button role="radio" aria-checked={on} class:on onclick={() => onprofile(selected!.id, p.value)}>{p.label}</button>
          {/each}
          {#if custom}<span class="on custom" role="radio" aria-checked="true" aria-disabled="true">{t('profiles.custom')}</span>{/if}
        </div>
        <button class="gear" aria-label={t('home.settings_for', { name: selected.name })} onclick={() => onsettings(selected!.id)}>
          <Icon name="gear" size={18} stroke={1.6} />{t('home.settings')}
        </button>
      </div>
      {#if full && !inSession && selected.online !== false}
        <p class="sentence limit" id="four-at-most">{t('common.four_at_most')}</p>
      {/if}
      <p class="sentence">{sentence}</p>
    {:else}
      <div class="eyebrow">{t('home.welcome')}</div>
      <h1>{t('home.welcome_title')}</h1>
      <p class="lead">{t('home.welcome_lead')}</p>
      <div class="actions">
        <button class="primary" onclick={() => onadd()}><Icon name="plus" size={18} />{t('home.add_computer')}</button>
        <button class="gear" onclick={onjoin}><Icon name="key" size={18} stroke={1.6} />{t('home.join')}</button>
      </div>
      {#if error}<div class="error" role="alert">{error}</div>{/if}
    {/if}
  </section>

  <section class="row">
    <div class="eyebrow">{t('home.your_computers')}</div>
    <div class="cards">
      {#each computers as c (c.id)}
        <button class="card" class:current={c.id === selected?.id} class:offline={c.online === false} onclick={() => onselect(c.id)}>
          {#if c.preview}<img src={c.preview} alt="" />{:else}<span class="sig" style:background={signature(c.fingerprint)}></span>{/if}
          <span class="label">
            <span class="name">{c.name}</span>
            <span class="state">{stateOf(c, retrying && c.id === selected?.id)}</span>
          </span>
        </button>
      {/each}
      {#each friends as f (f.id)}
        <button class="card friend" class:current={f.id === selectedFriend?.id} class:offline={!canBorrow(f)} onclick={() => onselect(f.id)}>
          <span class="sig" style:background={signature(f.id)}></span>
          <span class="label">
            <span class="name">{t('lend.computer_of', { host: f.owner })}</span>
            <span class="state">{friendStateText(f.state) || f.computer}</span>
          </span>
        </button>
      {/each}
      {#each fresh as f (f.id)}
        <button class="card found" aria-label={t('home.add_named', { name: f.name })} onclick={() => onadd(f)}>
          <span class="label">
            <span class="name">{f.name}</span>
            <span class="state found-state"><span class="pulse"></span>{t('home.on_network')}</span>
          </span>
        </button>
      {/each}
      <button class="card add" onclick={() => onadd()}>
        <Icon name="plus" size={24} />
        <span>{t('home.add_computer')}</span>
      </button>
      <!-- À côté de « Ajouter un ordinateur » : rejoindre le partage de quelqu'un, avec son code. -->
      <button class="card add" onclick={onjoin}>
        <Icon name="key" size={24} stroke={1.6} />
        <span>{t('home.join')}</span>
      </button>
    </div>
  </section>
  </div>
</main>

<style>
  main {
    position: absolute;
    inset: 0;
    overflow: hidden;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    transition: filter 0.6s;
  }

  .backdrop.offline {
    filter: grayscale(0.85) brightness(0.8);
  }

  .backdrop img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation:
      reveal 0.9s var(--ease-out),
      drift 60s linear infinite alternate;
  }

  @keyframes reveal {
    from {
      clip-path: polygon(0 0, 0 0, 0 0);
      opacity: 0.4;
    }
    to {
      clip-path: polygon(0 0, 200% 0, 0 200%);
      opacity: 1;
    }
  }

  @keyframes drift {
    from {
      transform: scale(1.02);
    }
    to {
      transform: scale(1.08);
    }
  }

  .signature {
    position: absolute;
    inset: 0;
  }

  .watermark {
    position: absolute;
    right: -120px;
    top: 40px;
    opacity: 0.07;
  }

  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgb(5 7 10 / 0.6) 0%, rgb(5 7 10 / 0.1) 22%, rgb(5 7 10 / 0.15) 50%, rgb(5 7 10 / 0.92) 78%, #05070a 100%),
      linear-gradient(90deg, rgb(5 7 10 / 0.55) 0%, transparent 60%);
  }

  /* Mise en page qui suit la fenêtre (960 × 600 au plus petit) : l'ordinateur
     choisi prend la place, la rangée des ordinateurs reste en bas ; trop
     petit, la page défile. */
  .layout {
    position: absolute;
    inset: 40px 0 0 0;
    display: flex;
    flex-direction: column;
    gap: 28px;
    padding: clamp(24px, 7vh, 72px) clamp(32px, 5vw, 72px) clamp(24px, 4vh, 40px);
    overflow-y: auto;
  }

  .hero {
    flex: 1 0 auto;
    max-width: 760px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: clamp(12px, 2.2vh, 18px);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--muted);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 4px;
    background: var(--ghost);
  }

  .status.ok .dot {
    background: var(--ok);
  }

  /* Prêté à un ami, ou l'ordinateur d'un ami occupé : ni libre ni hors ligne. */
  .status.lent .dot,
  .status.busy .dot {
    background: var(--warn);
  }

  .chip.countdown {
    height: 52px;
    padding: 0 18px;
    font-variant-numeric: tabular-nums;
  }

  .gear.danger {
    color: var(--bad, #ff6b6b);
    border-color: currentColor;
  }

  .status.wait .dot {
    background: var(--signal);
    animation: blink 1.2s ease-in-out infinite;
  }

  @keyframes blink {
    50% {
      opacity: 0.3;
    }
  }

  h1 {
    margin: 0;
    font-family: var(--display);
    font-size: clamp(44px, min(6vw, 9vh), 84px);
    line-height: 0.95;
    font-weight: 640;
    font-stretch: 125%;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }

  .system {
    font-size: 15px;
    color: var(--muted);
  }

  .lead {
    margin: 0;
    max-width: 520px;
    font-size: 17px;
    line-height: 1.5;
    color: var(--muted);
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 12px;
    width: fit-content;
    padding: 12px 16px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: rgb(9 13 18 / 0.7);
    font-size: 14px;
    color: var(--text);
  }

  .chip :global(svg) {
    color: var(--signal);
  }


  .actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 16px;
    margin-top: 14px;
  }

  .profiles {
    display: flex;
    padding: 4px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: rgb(9 13 18 / 0.75);
  }

  .profiles button {
    height: 42px;
    padding: 0 18px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--muted);
    font-size: 14px;
  }

  /* « Personnalisé » : l'état des réglages, pas un choix (un profil le remet). */
  .profiles .custom {
    display: flex;
    align-items: center;
    height: 42px;
    padding: 0 18px;
    font-size: 14px;
  }

  .profiles .on {
    border-radius: 7px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: var(--text);
    font-weight: 600;
  }

  .gear {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 52px;
    padding: 0 18px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: rgb(9 13 18 / 0.75);
    color: var(--muted);
    font-size: 14px;
  }

  .gear:hover {
    color: var(--text);
    border-color: var(--faint);
  }

  .sentence {
    margin: 0;
    font-size: 14px;
    color: var(--muted);
  }

  /* « Quatre ordinateurs au plus en même temps. » : la raison de « Se connecter » inactif. */
  .sentence.limit {
    color: var(--warn);
  }

  .row {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .cards {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(190px, 1fr);
    gap: 16px;
    overflow-x: auto;
    padding: 2px 2px 6px;
  }

  .card {
    position: relative;
    height: clamp(104px, 15vh, 132px);
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--ink-2);
    overflow: hidden;
    text-align: left;
    transition: border-color 0.2s, box-shadow 0.2s;
  }

  .card:hover {
    border-color: var(--line-2);
  }

  .card img,
  .card .sig {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0.55;
  }

  .card.offline img,
  .card.offline .sig {
    filter: grayscale(1);
    opacity: 0.3;
  }

  .card.current {
    border-color: var(--signal);
    box-shadow: 0 0 0 1px rgb(154 140 255 / 0.35), 0 0 22px rgb(109 91 255 / 0.25);
  }

  .card .label {
    position: absolute;
    left: 18px;
    right: 18px;
    bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-shadow: 0 1px 8px rgb(0 0 0 / 0.6);
  }

  .card .name {
    font-size: 17px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card .state {
    font-size: 12px;
    color: var(--muted);
  }

  .card.found {
    border-style: dashed;
    border-color: var(--line-2);
    background: var(--ink-1);
    animation: light 1.2s var(--ease-out);
  }

  @keyframes light {
    from {
      box-shadow: 0 0 0 1px var(--signal), 0 0 30px rgb(109 91 255 / 0.6);
    }
  }

  .found-state {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--signal);
  }

  .pulse {
    width: 6px;
    height: 6px;
    border-radius: 3px;
    background: var(--signal);
    box-shadow: 0 0 8px var(--signal);
  }

  .card.add {
    background: var(--ink-1);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--muted);
    font-size: 14px;
  }

  .card.add:hover {
    color: var(--text);
  }

  .error {
    max-width: 620px;
  }
</style>
