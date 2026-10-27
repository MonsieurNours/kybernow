<!--
  Appareils Link : ajouter un appareil (QR code, lien), le partage en cours
  (l'hôte, les invités et leurs droits), les demandes d'accès par code
  (Apple TV), et les appareils qui se sont connectés, chacun sur une carte
  qui s'ouvre : l'app au début de la session et ses options, l'accès, les
  sessions. Puis « Hors de chez vous » : le relais et l'accès par le relais
  (docs/relais.md). Rien de caché dans un menu.
-->
<script lang="ts">
  import { api, canCloseLaunch, joinedGuests, type Device, type Launch, type PairRequest, type Pairing, type RelayStatus } from '../lib/api'
  import { app, errorText, field, kyber, loadKyber, markRestartPending, notify, refreshStatus, setValues, value } from '../lib/store.svelte'
  import { ago, capital, deviceIcon, duration, mode, ms, when } from '../lib/format'
  import Icon from '../lib/Icon.svelte'
  import LaunchPicker from '../lib/LaunchPicker.svelte'
  import Qr from '../lib/Qr.svelte'
  import SharePanel from '../lib/SharePanel.svelte'
  import Switch from '../lib/Switch.svelte'
  import { t, tn } from '../lib/i18n/index.svelte'

  let pairing = $state<Pairing | null>(null)
  let devices = $state<Device[]>([])
  /** Carte ouverte ; tant qu'on n'a rien choisi, celle de l'appareil en session. */
  let open = $state<string | null | undefined>(undefined)
  /** Appareil dont on choisit l'app ouverte au début de la session. */
  let picking = $state<Device | null>(null)
  /** Appareil dont une option est en cours d'enregistrement. */
  let saving = $state<string | null>(null)
  let ending = $state(false)

  const live = $derived(app.status?.activity?.live ?? null)
  // Partage en cours (d'un appareil ou du PC) : ses participants, avec leurs droits.
  const share = $derived(app.status?.share ?? null)
  const guests = $derived(joinedGuests(share).length)
  const opened = $derived(open === undefined ? (live?.device_id ?? null) : open)
  // Demandes d'accès par code, relues avec l'état du serveur (toutes les 3 s).
  const requests = $derived(app.status?.pair_requests ?? [])
  let answering = $state<string | null>(null)
  let now = $state(Date.now() / 1000)

  $effect(() => {
    const timer = setInterval(() => (now = Date.now() / 1000), 1000)
    return () => clearInterval(timer)
  })

  function left(r: PairRequest) {
    const s = Math.max(0, Math.round(r.expires_at - now))
    return t('devices.timeLeft', { m: Math.floor(s / 60), s: String(s % 60).padStart(2, '0') })
  }

  async function accept(r: PairRequest) {
    answering = r.id
    try {
      const res = await api.pairAccept(r.id)
      devices = res.devices
      if (app.status) {
        app.status.pair_requests = res.pair_requests
        app.status.pending_access = res.pending_access
      }
      if (res.restart_pending) markRestartPending()
      const waiting = res.pending_access.some((p) => p.grant && p.account === res.username)
      notify(
        'ok',
        res.restarted
          ? t('devices.acceptedRestarting', { name: r.name })
          : waiting
            ? t('devices.acceptedWaiting', { name: r.name })
            : t('devices.accepted', { name: r.name }),
      )
    } catch (e) {
      notify('error', errorText(e))
      refreshStatus()
    } finally {
      answering = null
    }
  }

  async function refuse(r: PairRequest) {
    answering = r.id
    try {
      const res = await api.pairRefuse(r.id)
      if (app.status) app.status.pair_requests = res.pair_requests
      notify('ok', t('devices.refused', { name: r.name }))
    } catch (e) {
      notify('error', errorText(e))
      refreshStatus()
    } finally {
      answering = null
    }
  }

  async function loadDevices() {
    try {
      devices = (await api.devices()).devices
    } catch {
      // 401 gérée globalement
    }
  }

  $effect(() => {
    api.pairing().then((p) => (pairing = p)).catch((e) => notify('error', errorText(e)))
    loadDevices()
    loadKyber()
    loadRelay()
    const timer = setInterval(loadDevices, 8000)
    // L'état du relais change vite après un enregistrement (connexion, puis joint ou injoignable).
    const relayTimer = setInterval(loadRelay, 3000)
    return () => {
      clearInterval(timer)
      clearInterval(relayTimer)
    }
  })

  // « Hors de chez vous » : le relais (`GET /api/relay`), relu tant que la page est ouverte.
  let relay = $state<RelayStatus | null>(null)
  /** L'adresse du relais en cours de saisie (`null` : pas encore lue). */
  let relayServer = $state<string | null>(null)
  let relaySaving = $state(false)
  const relayLocked = $derived(!!relay?.locked_until && relay.locked_until > now)
  const relayEdited = $derived(relay !== null && relayServer !== null && relayServer.trim() !== relay.server)

  async function loadRelay() {
    try {
      relay = await api.relay()
      if (relayServer === null) relayServer = relay.server
    } catch {
      // 401 gérée globalement ; une Station sans relais n'a pas la section.
    }
  }

  async function saveRelay(changes: { server?: string; remote_access?: boolean }) {
    relaySaving = true
    try {
      relay = await api.setRelay(changes)
      if (changes.server !== undefined) relayServer = relay.server
      notify('ok', t('devices.away.saved'))
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      relaySaving = false
    }
  }

  // Empreinte groupée par quatre caractères : les quatre premiers groupes suffisent à comparer.
  const fingerprint = $derived(
    ((pairing?.fingerprint ?? '').replace(/^sha256:/, '').replaceAll(':', '').toUpperCase().match(/.{1,4}/g) ?? []).slice(0, 4).join(' '),
  )

  async function copy() {
    if (!pairing) return
    try {
      await navigator.clipboard.writeText(pairing.link)
      notify('ok', t('devices.linkCopied'))
    } catch {
      notify('error', t('devices.copyFailed'))
    }
  }

  function toggle(d: Device) {
    open = opened === d.id ? null : d.id
  }

  /** La ligne d'une carte fermée : système, compte, app au début, sessions. */
  function summary(d: Device): string {
    const launch = !d.launch
      ? t('devices.list.noLaunch')
      : d.launch.skip_desktop
        ? t('devices.list.launchesNotDesktop', { app: d.launch.name })
        : t('devices.list.launches', { app: d.launch.name })
    return [d.os, d.own_account ? t('devices.list.ownAccess') : t('devices.list.account', { user: d.user }), launch, tn('devices.list.sessions', d.sessions)]
      .filter(Boolean)
      .join(' · ')
  }

  /** Écran, profil et latence de la session en cours, ou de la dernière. */
  function details(d: Device, isLive: boolean): string {
    const m = isLive ? live?.mode : d.last_mode
    const latency = isLive ? live?.latency_ms : d.latency_ms
    return [
      m ? t('devices.card.screen', { mode: mode(m) }) : null,
      isLive && live?.profile ? t('devices.card.profile', { profile: live.profile }) : null,
      latency ? t('devices.card.latency', { latency: ms(latency) }) : null,
    ]
      .filter(Boolean)
      .join(', ')
  }

  function sessionsText(d: Device, isLive: boolean): string {
    const text = isLive
      ? t('devices.card.live', { details: details(d, true) })
      : d.sessions === 0
        ? t('devices.card.never')
        : details(d, false)
          ? t('devices.card.last', { ago: ago(d.last_seen), details: details(d, false) })
          : t('devices.card.lastPlain', { ago: ago(d.last_seen) })
    return d.sessions > 0 ? `${text} ${tn('devices.card.history', d.sessions, { since: when(d.first_seen) })}.` : text
  }

  async function setOption(d: Device, patch: Partial<Launch>) {
    if (!d.launch) return
    saving = d.id
    try {
      devices = (await api.setLaunch(d.id, { ...d.launch, ...patch })).devices
      notify('ok', t('devices.card.optionSaved', { name: d.name }))
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      saving = null
    }
  }

  async function endSession(d: Device) {
    if (!confirm(t('overview.confirmEnd', { who: d.name }))) return
    ending = true
    try {
      const r = await api.endSession()
      notify('ok', r.method === 'app' ? t('overview.willDisconnect', { who: d.name }) : r.method === 'restart' ? t('overview.cut') : t('overview.noSession'))
      await refreshStatus()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      ending = false
    }
  }

  async function forget(d: Device) {
    const question = d.own_account
      ? t('devices.confirmRevoke', { name: d.name, user: d.user })
      : t('devices.confirmForget', { name: d.name })
    if (!confirm(question)) return
    try {
      const res = await api.forgetDevice(d.id)
      devices = res.devices
      if (res.revoked) {
        if (res.restart_pending) markRestartPending()
        if (app.status) app.status.pending_access = res.pending_access
        notify(
          'ok',
          res.pending_access.some((p) => !p.grant && p.account === d.user)
            ? t('devices.revokedLater', { name: d.name })
            : t('devices.revoked', { name: d.name }),
        )
      } else {
        notify('ok', t('devices.forgotten', { name: d.name }))
      }
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  const multi = $derived(field('network.multi_client'))
</script>

<header class="space-head">
  <div>
    <span class="lbl">{t('devices.eyebrow')}</span>
    <h1>{t('devices.title')}</h1>
    <p>{t('devices.intro')} <a href="#/utilisateurs">{t('devices.introLink')}</a>.</p>
  </div>
</header>

<div class="space-body">
  <!-- Ajouter un appareil -->
  <section aria-labelledby="t-add" class="card feature add">
    {#if pairing}<Qr text={pairing.link} label={t('devices.add.qrLabel')} />{:else}<div class="qr-wait"></div>{/if}
    <div class="add-text">
      <h2 id="t-add" class="disp">{t('devices.add.title')}</h2>
      <p>{t('devices.add.text')}</p>
      <div class="add-actions">
        <button type="button" class="primary" disabled={!pairing} onclick={copy}><Icon name="copy" size={15} stroke={2} />{t('devices.add.copy')}</button>
        {#if pairing}<span class="mono link">{pairing.link.replace(/\?.*/, '')}</span>{/if}
      </div>
      <details>
        <summary>{t('devices.add.contents')}</summary>
        <p>{t('devices.add.explain')}{#if fingerprint} {t('devices.add.fingerprint')} <span class="mono">{fingerprint}</span>{/if}</p>
      </details>
    </div>
  </section>

  <!-- Partage en cours -->
  {#if share}
    <section id="partage" class="card share">
      <SharePanel
        {share}
        computer={app.status?.hostname ?? t('overview.thisComputer')}
        host={share.origin === 'host' && live ? { name: live.name, kind: live.kind, since: live.started_at } : null}
      />
    </section>
  {/if}

  <!-- Vos appareils -->
  <section id="appareils" aria-labelledby="t-dev" class="card devices">
    <div class="card-head">
      <div>
        <h2 id="t-dev">{t('devices.list.title')}</h2>
        <span class="sub">{t('devices.list.sub')}</span>
      </div>
      {#if devices.length}
        <span class="mono count">{tn('devices.list.count', devices.length)} · {value('network.multi_client') ? t('devices.list.together') : t('devices.list.oneAtATime')}</span>
      {/if}
    </div>

    {#each requests as r (r.id)}
      <div class="request" role="group" aria-labelledby="req-{r.id}">
        <span class="device-icon live"><Icon name={deviceIcon(r.kind)} size={20} stroke={1.6} /></span>
        <div class="who">
          <span class="name" id="req-{r.id}">{t('devices.list.asks', { name: r.name })}</span>
          <span class="mono meta">{[r.os, r.address, t('devices.remaining', { time: left(r) })].filter(Boolean).join(' · ')}</span>
        </div>
        <div class="code">
          <span class="lbl">{t('devices.list.codeLabel')}</span>
          <strong class="mono" aria-label={t('devices.list.codeAria', { digits: r.code.split('').join(' ') })}>{r.code.slice(0, 3)} {r.code.slice(3)}</strong>
        </div>
        <div class="answer">
          <button type="button" class="primary" disabled={answering !== null} onclick={() => accept(r)}><Icon name="check" size={15} stroke={2} />{t('devices.list.accept')}</button>
          <button type="button" class="ghost" disabled={answering !== null} onclick={() => refuse(r)}>{t('devices.list.refuse')}</button>
        </div>
      </div>
    {/each}
    {#if requests.length}
      <p class="request-help">{t('devices.list.requestHelp')}</p>
    {/if}

    {#each devices as d (d.id)}
      {@const isLive = live?.device_id === d.id}
      {@const isOpen = opened === d.id}
      <article class="device" class:open={isOpen} class:live={isLive} aria-label={d.name}>
        <button type="button" class="head" aria-expanded={isOpen} aria-label={isOpen ? t('devices.list.close', { name: d.name }) : t('devices.list.open', { name: d.name })} onclick={() => toggle(d)}>
          <span class="device-icon" class:live={isLive}><Icon name={deviceIcon(d.kind)} size={22} stroke={1.6} /></span>
          <span class="who">
            <span class="name">{d.name}</span>
            {#if isOpen}
              <span class="meta">{[d.os, d.app].filter(Boolean).join(' · ')}</span>
            {:else}
              <span class="mono meta">{summary(d)}</span>
            {/if}
          </span>
          <span class="status">
            {#if isLive && live}
              <span class="pill live"><span class="dot"></span>{now - live.started_at >= 60 ? t('devices.list.inSessionSince', { duration: duration(now - live.started_at) }) : t('devices.list.inSession')}</span>
              {#if share?.origin === 'host' && guests}<span class="pill live">{tn('share.guests', guests)}</span>{/if}
              {#if live.latency_ms}<span class="mono latency live">{ms(live.latency_ms)}</span>{/if}
            {:else}
              <span class="pill">{d.sessions === 0 ? t('devices.list.paired', { ago: ago(d.last_seen) }) : capital(ago(d.last_seen))}</span>
              {#if d.latency_ms}<span class="mono latency">{ms(d.latency_ms)}</span>{/if}
            {/if}
          </span>
          <span class="chevron" aria-hidden="true"><Icon name="chevron" size={16} stroke={2} /></span>
        </button>

        {#if isOpen}
          <div class="body">
            <!-- Au début de la session -->
            <div class="set">
              <div>
                <h3>{t('devices.card.launchTitle')}</h3>
                <p>{t('devices.card.launchText')}</p>
              </div>
              <div class="launch">
                {#if d.launch}
                  <span class="appchip">
                    {#if d.launch.icon}<img src={api.appIconUrl(d.launch.icon)} alt="" width="26" height="26" />{:else}<Icon name="app" size={18} />{/if}
                    {d.launch.name}
                  </span>
                  <button type="button" class="ghost" onclick={() => (picking = d)}>{t('devices.card.change')}</button>
                {:else}
                  <span class="appchip none"><Icon name="other" size={18} />{t('devices.card.noApp')}</span>
                  <button type="button" class="ghost" onclick={() => (picking = d)}>{t('devices.card.choose')}</button>
                {/if}
              </div>
            </div>
            {#if d.launch}
              {@const closable = canCloseLaunch(d.launch)}
              <div class="opts">
                <div class="opt">
                  <div><span class="opt-title">{t('devices.card.closeAtEnd')}</span><p>{closable ? t('devices.card.closeAtEndHint') : t('devices.card.cannotClose')}</p></div>
                  <Switch label={t('devices.card.closeAtEnd')} checked={d.launch.close_at_end && closable} disabled={!closable || saving === d.id} onchange={(v) => setOption(d, { close_at_end: v })} />
                </div>
                <div class="opt">
                  <div><span class="opt-title">{t('devices.card.skipDesktop')}</span><p>{t('devices.card.skipDesktopHint')}</p></div>
                  <Switch label={t('devices.card.skipDesktop')} checked={d.launch.skip_desktop} disabled={saving === d.id} onchange={(v) => setOption(d, { skip_desktop: v })} />
                </div>
              </div>
            {/if}

            <!-- Accès -->
            <div class="set">
              <div>
                <h3>{t('devices.card.accessTitle')}</h3>
                <p>{d.own_account ? t('devices.card.ownAccessText', { user: d.user, when: when(d.first_seen) }) : t('devices.card.accountText', { user: d.user })}</p>
              </div>
              {#if d.own_account}
                <button type="button" class="ghost danger" onclick={() => forget(d)}><Icon name="key" size={15} />{t('devices.list.removeAccess')}</button>
              {/if}
            </div>

            <!-- Sessions -->
            <div class="set">
              <div>
                <h3>{t('devices.card.sessionsTitle')}</h3>
                <p>{sessionsText(d, isLive)}</p>
              </div>
              {#if isLive}
                <button type="button" class="ghost" disabled={ending || !!live?.end_requested} onclick={() => endSession(d)}>{live?.end_requested ? t('devices.card.endRequested') : t('devices.card.endSession')}</button>
              {/if}
            </div>

            {#if !d.own_account}
              <div class="foot"><button type="button" class="link" onclick={() => forget(d)}>{t('devices.list.forget')}</button></div>
            {/if}
          </div>
        {/if}
      </article>
    {:else}
      <p class="empty">{t('devices.list.empty')}</p>
    {/each}

    {#if kyber.loaded && multi}
      <div class="set multi">
        <div><h3>{multi.label}</h3><p>{multi.details}</p></div>
        <Switch label={multi.label} checked={!!value('network.multi_client')} onchange={(v) => setValues({ 'network.multi_client': v })} />
      </div>
    {/if}
  </section>

  <!-- Hors de chez vous : le relais et l'accès par le relais -->
  {#if relay}
    <section id="hors-de-chez-vous" aria-labelledby="t-away" class="card away">
      <div class="card-head">
        <h2 id="t-away">{t('devices.away.title')}</h2>
      </div>
      {#if relayLocked}
        <div class="notice warn" role="status"><Icon name="warning" size={16} /><span>{t('devices.away.locked')}</span></div>
      {/if}
      <div class="set">
        <div>
          <label class="title" for="relay-server">{t('devices.away.server')}</label>
          <p>{t('devices.away.serverHelp')}</p>
          {#if relay.server && relay.state === 'joined'}
            <span class="relay-state ok"><span class="dot"></span>{t('devices.away.joined')}</span>
          {:else if relay.server && relay.state === 'unreachable'}
            <span class="relay-state bad"><span class="dot"></span>{relay.reason ? t('devices.away.unreachable', { reason: relay.reason }) : t('devices.away.unreachablePlain')}</span>
          {/if}
        </div>
        <form class="relay-form" onsubmit={(e) => (e.preventDefault(), saveRelay({ server: (relayServer ?? '').trim() }))}>
          <input id="relay-server" class="mono" type="text" placeholder="share.kybernow.app:8443" autocomplete="off" autocapitalize="off" spellcheck="false" bind:value={relayServer} />
          <button type="submit" class="ghost small" disabled={relaySaving || !relayEdited}>{t('devices.away.save')}</button>
        </form>
      </div>
      <div class="set">
        <div><h3>{t('devices.away.access')}</h3><p>{t('devices.away.accessHelp')}</p></div>
        <Switch label={t('devices.away.access')} checked={relay.remote_access} disabled={relaySaving || !relay.server} onchange={(v) => saveRelay({ remote_access: v })} />
      </div>
    </section>
  {/if}
</div>

{#if picking}
  <LaunchPicker device={picking} onclose={() => (picking = null)} onsaved={(list) => (devices = list)} />
{/if}

<style>
  .space-head p a {
    color: var(--signal-light);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .add {
    display: flex;
    flex-wrap: wrap;
    gap: 28px;
    align-items: center;
  }

  .qr-wait {
    width: 168px;
    height: 168px;
    border-radius: 12px;
    background: var(--ink-3);
  }

  .add-text {
    flex: 1 1 360px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .add h2 {
    font-size: 26px;
    font-weight: 650;
    letter-spacing: -0.025em;
  }

  .add-text > p {
    font-size: 14.5px;
    line-height: 1.55;
    color: var(--text-2);
    max-width: 56ch;
  }

  .add-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: center;
  }

  .link {
    font-size: 11px;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  details {
    font-size: 13px;
    color: var(--faint);
  }

  summary {
    cursor: pointer;
    color: var(--muted);
  }

  details p {
    margin-top: 8px;
    line-height: 1.5;
    max-width: 60ch;
  }

  details .mono {
    color: var(--text-2);
  }

  .devices {
    padding-bottom: 14px;
  }

  .card-head {
    align-items: baseline;
  }

  .count {
    font-size: 12px;
    color: var(--faint);
  }

  /* Demande d'accès par code */

  .request {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    gap: 14px 20px;
    align-items: center;
    margin: 4px 0 0;
    padding: 16px;
    border-radius: 12px;
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .request + .request {
    margin-top: 10px;
  }

  .request .who {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .code {
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: flex-end;
  }

  .code .lbl {
    color: var(--signal-light);
    font-size: 9.5px;
  }

  .code strong {
    font-size: 34px;
    font-weight: 500;
    letter-spacing: 0.08em;
    color: var(--neon-core);
    text-shadow: var(--text-glow);
    white-space: nowrap;
  }

  .answer {
    grid-column: 2 / -1;
    display: flex;
    gap: 10px;
  }

  .request-help {
    margin: 10px 0 4px;
    font-size: 12.5px;
    color: var(--faint);
  }

  /* Une carte par appareil */

  .device {
    margin-top: 10px;
    border-radius: 14px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--line);
  }

  .device.live {
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .request-help + .device,
  .card-head + .device {
    margin-top: 6px;
  }

  .head {
    width: 100%;
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto 24px;
    gap: 16px;
    align-items: center;
    padding: 14px 16px;
    border: 0;
    border-radius: 14px;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .head:hover {
    background: rgb(255 255 255 / 0.025);
  }

  .device.open .head {
    border-radius: 14px 14px 0 0;
  }

  .device-icon {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--ink-3);
    color: var(--text-2);
    flex: none;
  }

  .head .device-icon {
    width: 44px;
    height: 44px;
  }

  .device-icon.live {
    background: rgb(109 91 255 / 0.14);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
    color: var(--signal-light);
  }

  .head .who {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name {
    font-size: 15.5px;
    font-weight: 650;
    overflow-wrap: anywhere;
  }

  .meta {
    font-size: 12.5px;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  .meta.mono {
    font-size: 11px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-content: flex-end;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 10px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    background: var(--ink-3);
    color: var(--muted);
    white-space: nowrap;
  }

  .pill.live {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--signal-light);
  }

  .pill .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--signal);
    box-shadow: 0 0 8px var(--signal);
  }

  .latency {
    font-size: 12.5px;
    color: var(--faint);
  }

  .latency.live {
    color: var(--signal-light);
  }

  .chevron {
    display: grid;
    place-items: center;
    color: var(--faint);
    transform: rotate(90deg);
    transition: transform 0.2s var(--ease-out);
  }

  .device.open .chevron {
    transform: rotate(-90deg);
  }

  .body {
    padding: 0 16px 6px;
  }

  .body .set {
    gap: 10px 28px;
  }

  .launch {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
  }

  .appchip {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 14px 0 9px;
    border-radius: 12px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    font-size: 14px;
    font-weight: 600;
    max-width: 100%;
  }

  .appchip img {
    border-radius: 7px;
  }

  .appchip.none {
    color: var(--faint);
    font-weight: 500;
  }

  .opts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(300px, 100%), 1fr));
    gap: 0 28px;
    padding: 0 0 8px;
  }

  .opt {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 0;
  }

  .opt-title {
    font-size: 14px;
    font-weight: 600;
  }

  .opt p {
    margin-top: 3px;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--faint);
  }

  .foot {
    display: flex;
    justify-content: flex-end;
    padding: 10px 0 8px;
    border-top: 1px solid var(--line);
  }

  .foot .link {
    border: 0;
    background: none;
    padding: 0;
    font: inherit;
    font-size: 12.5px;
    color: var(--faint);
    cursor: pointer;
  }

  .foot .link:hover {
    color: var(--bad-text);
  }

  .empty {
    font-size: 13.5px;
    color: var(--faint);
    padding: 14px 0 18px;
    border-top: 1px solid var(--line);
  }

  .multi {
    margin-top: 14px;
  }

  .share {
    padding: 26px 28px 22px;
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  /* Hors de chez vous */

  .away .notice {
    margin-bottom: 14px;
  }

  .relay-form {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .relay-form input {
    width: 260px;
    max-width: 100%;
    height: 36px;
    font-size: 13px;
  }

  .relay-state {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
    font-size: 13px;
    font-weight: 600;
  }

  .relay-state .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
  }

  .relay-state.ok {
    color: var(--ok-text);
  }

  .relay-state.ok .dot {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }

  .relay-state.bad {
    color: var(--bad-text);
  }

  .relay-state.bad .dot {
    background: var(--bad);
  }

  @media (max-width: 760px) {
    .share {
      padding: 20px 18px 16px;
    }
  }

  @media (max-width: 760px) {
    .request {
      grid-template-columns: 40px minmax(0, 1fr);
    }

    .code {
      grid-column: 1 / -1;
      align-items: flex-start;
    }

    .head {
      grid-template-columns: 40px minmax(0, 1fr) 24px;
      gap: 12px 14px;
      padding: 14px;
    }

    .head .device-icon {
      width: 40px;
      height: 40px;
    }

    .status {
      grid-column: 2;
      justify-content: flex-start;
      flex-wrap: wrap;
    }

    .body {
      padding: 0 14px 6px;
    }

    .launch {
      justify-content: flex-start;
    }

    .relay-form {
      flex-wrap: wrap;
    }

    .relay-form input {
      flex: 1 1 200px;
      width: auto;
    }
  }
</style>
