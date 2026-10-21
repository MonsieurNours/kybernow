<!--
  Accès : ajouter un appareil (QR code, lien), les demandes d'accès par code
  (Apple TV), les appareils qui se sont connectés, et les comptes qui
  permettent de se connecter.
-->
<script lang="ts">
  import { api, type Device, type PairRequest, type Pairing, type User } from '../lib/api'
  import { app, errorText, field, kyber, loadKyber, markRestartPending, notify, refreshStatus, setValues, value } from '../lib/store.svelte'
  import { ago, capital, deviceIcon, mode, ms } from '../lib/format'
  import Icon from '../lib/Icon.svelte'
  import Qr from '../lib/Qr.svelte'
  import Switch from '../lib/Switch.svelte'

  let pairing = $state<Pairing | null>(null)
  let devices = $state<Device[]>([])
  let users = $state<User[]>([])
  let menu = $state<string | null>(null)
  let editing = $state<string | null>(null)
  let newPassword = $state('')
  let username = $state('')
  let password = $state('')
  let busy = $state(false)

  const live = $derived(app.status?.activity?.live ?? null)
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
    return `${Math.floor(s / 60)}\u00a0min\u00a0${String(s % 60).padStart(2, '0')}`
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
          ? `${r.name} a reçu son accès. Le serveur redémarre quelques secondes pour en tenir compte.`
          : waiting
            ? `${r.name} a reçu son accès. Une session est en cours : le serveur redémarrera tout seul dès qu'elle finira, et ${r.name} pourra alors se connecter.`
            : `${r.name} a reçu son accès.`,
      )
      api.users().then((u) => (users = u.users)).catch(() => {})
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
      notify('ok', `Accès refusé à ${r.name}.`)
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
    api.users().then((r) => (users = r.users)).catch((e) => notify('error', errorText(e)))
    loadDevices()
    loadKyber()
    const timer = setInterval(loadDevices, 8000)
    return () => clearInterval(timer)
  })

  // Empreinte groupée par quatre caractères : les quatre premiers groupes suffisent à comparer.
  const fingerprint = $derived(
    ((pairing?.fingerprint ?? '').replace(/^sha256:/, '').replaceAll(':', '').toUpperCase().match(/.{1,4}/g) ?? []).slice(0, 4).join(' '),
  )

  async function copy() {
    if (!pairing) return
    try {
      await navigator.clipboard.writeText(pairing.link)
      notify('ok', 'Lien copié.')
    } catch {
      notify('error', 'Copie impossible : sélectionnez le lien à la main.')
    }
  }

  async function forget(d: Device) {
    menu = null
    const question = d.own_account
      ? `Retirer l'accès de ${d.name} ? Son compte propre (${d.user}) est supprimé : pour revenir, il devra être appairé de nouveau.`
      : `Oublier ${d.name} ? Il reste capable de se connecter avec son compte ; il disparaît seulement de cette liste.`
    if (!confirm(question)) return
    try {
      const res = await api.forgetDevice(d.id)
      devices = res.devices
      if (res.revoked) {
        if (res.restart_pending) markRestartPending()
        if (app.status) app.status.pending_access = res.pending_access
        api.users().then((u) => (users = u.users)).catch(() => {})
        notify(
          'ok',
          res.pending_access.some((p) => !p.grant && p.account === d.user)
            ? `Accès de ${d.name} retiré. Le serveur redémarrera pour en tenir compte dès la fin de la session en cours.`
            : `Accès de ${d.name} retiré.`,
        )
      } else {
        notify('ok', `${d.name} oublié.`)
      }
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  async function runUsers(fn: () => Promise<{ users: User[] }>, message: string) {
    busy = true
    try {
      users = (await fn()).users
      markRestartPending()
      notify('ok', message)
      return true
    } catch (e) {
      notify('error', errorText(e))
      return false
    } finally {
      busy = false
    }
  }

  async function add(event: SubmitEvent) {
    event.preventDefault()
    if (users.some((u) => u.username === username)) {
      notify('error', `« ${username} » existe déjà : changez plutôt son mot de passe.`)
      return
    }
    if (await runUsers(() => api.upsertUser(username, password), `Compte « ${username} » ajouté.`)) {
      username = ''
      password = ''
    }
  }

  async function savePassword(event: SubmitEvent, user: string) {
    event.preventDefault()
    if (await runUsers(() => api.upsertUser(user, newPassword), `Mot de passe de « ${user} » changé.`)) {
      editing = null
      newPassword = ''
    }
  }

  async function remove(user: string) {
    const device = users.find((u) => u.username === user)?.device
    const consequence = device ? `${device} ne pourra plus se connecter.` : "Les appareils qui l'utilisent ne pourront plus se connecter."
    if (!confirm(`Supprimer le compte « ${user} » ? ${consequence}`)) return
    await runUsers(() => api.deleteUser(user), `Compte « ${user} » supprimé.`)
  }

  const insecure = $derived(users.find((u) => u.insecure_default) ?? null)
  const multi = $derived(field('network.multi_client'))
</script>

<svelte:window onclick={() => (menu = null)} />

<header class="space-head">
  <div>
    <span class="lbl">Qui peut ouvrir cet ordinateur</span>
    <h1>Accès</h1>
    <p>Vos appareils, les comptes qui permettent de se connecter, et comment en ajouter un.</p>
  </div>
</header>

<div class="space-body">
  <!-- Ajouter un appareil -->
  <section aria-labelledby="t-add" class="card feature add">
    {#if pairing}<Qr text={pairing.link} label="QR code d'appairage" />{:else}<div class="qr-wait"></div>{/if}
    <div class="add-text">
      <h2 id="t-add" class="disp">Ajouter un appareil</h2>
      <p>Sur votre réseau, ouvrez KyberNow : cet ordinateur y est déjà. Ailleurs, ou par VPN, scannez ce code avec l'appareil, ou collez le lien dans l'app.</p>
      <div class="add-actions">
        <button type="button" class="primary" disabled={!pairing} onclick={copy}><Icon name="copy" size={15} stroke={2} />Copier le lien</button>
        {#if pairing}<span class="mono link">{pairing.link.replace(/\?.*/, '')}</span>{/if}
      </div>
      <details>
        <summary>Ce que contient le lien</summary>
        <p>L'adresse de cet ordinateur et l'empreinte de son certificat : l'app sait qu'elle parle bien à lui. Aucun mot de passe.{#if fingerprint} Empreinte : <span class="mono">{fingerprint}</span>{/if}</p>
      </details>
    </div>
  </section>

  <!-- Appareils -->
  <section id="appareils" aria-labelledby="t-dev" class="card">
    <div class="card-head">
      <h2 id="t-dev">Appareils</h2>
      <span class="sub">Ceux qui se sont déjà connectés</span>
    </div>
    {#each requests as r (r.id)}
      <div class="request" role="group" aria-labelledby="req-{r.id}">
        <span class="device-icon live"><Icon name={deviceIcon(r.kind)} size={20} stroke={1.6} /></span>
        <div class="who">
          <span class="name" id="req-{r.id}">{r.name} demande l'accès</span>
          <span class="mono meta">{[r.os, r.address, `encore\u00a0${left(r)}`].filter(Boolean).join(' · ')}</span>
        </div>
        <div class="code">
          <span class="lbl">Code sur l'écran de l'appareil</span>
          <strong class="mono" aria-label="Code {r.code.split('').join(' ')}">{r.code.slice(0, 3)} {r.code.slice(3)}</strong>
        </div>
        <div class="answer">
          <button type="button" class="primary" disabled={answering !== null} onclick={() => accept(r)}><Icon name="check" size={15} stroke={2} />Accepter</button>
          <button type="button" class="ghost" disabled={answering !== null} onclick={() => refuse(r)}>Refuser</button>
        </div>
      </div>
    {/each}
    {#if requests.length}
      <p class="request-help">Acceptez seulement si ce code est celui affiché sur votre appareil. Il reçoit son propre accès, que vous pourrez lui retirer ici.</p>
    {/if}
    {#each devices as d (d.id)}
      {@const isLive = live?.device_id === d.id}
      <div class="row">
        <span class="device-icon" class:live={isLive}><Icon name={deviceIcon(d.kind)} size={20} stroke={1.6} /></span>
        <div class="who">
          <span class="name">{d.name}</span>
          <span class="mono meta">{[d.os, d.own_account ? 'accès propre' : `compte ${d.user}`, d.last_mode ? mode(d.last_mode) : null].filter(Boolean).join(' · ')}</span>
        </div>
        <span class="when" class:live={isLive}>{isLive ? 'En session' : d.sessions === 0 ? `Appairé ${ago(d.last_seen)}` : capital(ago(d.last_seen))}{d.latency_ms && !isLive ? ` · ${ms(d.latency_ms)}` : ''}</span>
        <div class="menu-wrap">
          <button type="button" class="icon-button" aria-label="Options de {d.name}" aria-expanded={menu === d.id} onclick={(e) => (e.stopPropagation(), (menu = menu === d.id ? null : d.id))}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="5" cy="12" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="19" cy="12" r="1.8" /></svg>
          </button>
          {#if menu === d.id}
            <div class="menu" role="menu">
              <span class="menu-info">{d.sessions} session{d.sessions > 1 ? 's' : ''}{d.app ? ` · ${d.app}` : ''}</span>
              <button type="button" role="menuitem" onclick={() => forget(d)}><Icon name="trash" size={15} />{d.own_account ? "Retirer l'accès" : 'Oublier cet appareil'}</button>
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <p class="empty">Aucun appareil ne s'est encore connecté. Les apps KyberNow apparaissent ici dès leur première session.</p>
    {/each}

    {#if kyber.loaded && multi}
      <div class="set">
        <div><h3>{multi.label}</h3><p>{multi.details}</p></div>
        <Switch label={multi.label} checked={!!value('network.multi_client')} onchange={(v) => setValues({ 'network.multi_client': v })} />
      </div>
    {/if}
  </section>

  <!-- Comptes -->
  <section id="comptes" aria-labelledby="t-acc" class="card accounts">
    <div class="card-head">
      <h2 id="t-acc">Comptes</h2>
      <span class="sub">L'identifiant et le mot de passe saisis sur un appareil</span>
    </div>

    {#if insecure}
      <div class="notice warn" role="alert">
        <Icon name="warning" />
        <span>Le compte d'exemple « {insecure.username} » a toujours son mot de passe d'origine, connu de tous.</span>
        <button type="button" class="ghost small" disabled={busy || users.length <= 1} onclick={() => remove(insecure.username)}>Supprimer ce compte</button>
      </div>
    {/if}

    {#each users as u (u.username)}
      <div class="row account">
        <span class="avatar" class:warn={u.insecure_default}>{u.username.charAt(0).toUpperCase()}</span>
        <div class="who">
          <span class="name">{u.username}</span>
          <span class="sub" class:warn={u.insecure_default}>{u.insecure_default ? "Mot de passe d'origine" : u.device ? `Accès propre à ${u.device}` : 'Se connecter depuis un appareil'}</span>
        </div>
        {#if u.device}
          <span></span>
        {:else}
          <button type="button" class="ghost small" onclick={() => ((editing = editing === u.username ? null : u.username), (newPassword = ''))}>Changer le mot de passe</button>
        {/if}
        {#if users.length > 1}
          <button type="button" class="icon-button" aria-label="Supprimer le compte {u.username}" disabled={busy} onclick={() => remove(u.username)}><Icon name="trash" size={16} /></button>
        {:else}
          <span class="icon-spacer"></span>
        {/if}
      </div>
      {#if editing === u.username}
        <form class="inline" onsubmit={(e) => savePassword(e, u.username)}>
          <div class="field"><label for="np-{u.username}">Nouveau mot de passe de {u.username}</label><input id="np-{u.username}" type="password" minlength="8" required autocomplete="new-password" bind:value={newPassword} placeholder="8 caractères minimum" /></div>
          <button type="submit" class="ghost" disabled={busy}>Enregistrer</button>
          <button type="button" class="ghost" onclick={() => (editing = null)}>Annuler</button>
        </form>
      {/if}
    {/each}

    <form class="new" onsubmit={add}>
      <div class="field"><label for="u">Nouvel identifiant</label><input id="u" type="text" required autocomplete="off" bind:value={username} placeholder="ex. lea" /></div>
      <div class="field"><label for="p">Mot de passe</label><input id="p" type="password" minlength="8" required autocomplete="new-password" bind:value={password} placeholder="8 caractères minimum" /></div>
      <button type="submit" class="ghost" disabled={busy}>Ajouter le compte</button>
    </form>
    <p class="hash">Kyber ne garde que l'empreinte des mots de passe (SHA-256). Un changement de compte s'applique au redémarrage du serveur.</p>
  </section>
</div>

<style>
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

  .card-head {
    align-items: baseline;
  }

  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto auto;
    gap: 16px;
    align-items: center;
    padding: 14px 0;
    border-top: 1px solid var(--line);
  }

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
    margin: 10px 0 14px;
    font-size: 12.5px;
    color: var(--faint);
  }

  .device-icon {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--ink-3);
    color: var(--text-2);
  }

  .device-icon.live {
    background: rgb(109 91 255 / 0.14);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
    color: var(--signal-light);
  }

  .who {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .name {
    font-size: 14.5px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .meta {
    font-size: 11px;
    color: var(--faint);
  }

  .when {
    font-size: 12.5px;
    color: var(--faint);
    text-align: right;
  }

  .when.live {
    color: var(--signal-light);
  }

  .menu-wrap {
    position: relative;
  }

  .menu {
    position: absolute;
    right: 0;
    top: 38px;
    z-index: 5;
    min-width: 220px;
    padding: 6px;
    border-radius: 12px;
    background: rgb(14 20 26 / 0.96);
    backdrop-filter: blur(20px);
    box-shadow: inset 0 0 0 1px var(--line-2), 0 18px 40px rgb(0 0 0 / 0.5);
    display: flex;
    flex-direction: column;
  }

  .menu-info {
    padding: 6px 10px 8px;
    font-size: 12px;
    color: var(--faint);
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 38px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--bad-text);
    font-size: 13.5px;
    text-align: left;
  }

  .menu button:hover {
    background: rgb(255 255 255 / 0.06);
  }

  .empty {
    font-size: 13.5px;
    color: var(--faint);
    padding: 14px 0 18px;
    border-top: 1px solid var(--line);
  }

  .accounts {
    padding-bottom: 22px;
  }

  .accounts .notice {
    margin: 4px 0 8px;
  }

  .avatar {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--ink-3);
    font-size: 14px;
    font-weight: 600;
  }

  .avatar.warn {
    color: var(--faint);
  }

  .who .sub {
    font-size: 12.5px;
    color: var(--faint);
  }

  .who .sub.warn {
    color: var(--warn-text);
  }

  .icon-spacer {
    width: 34px;
  }

  .inline {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: end;
    padding: 0 0 16px 56px;
  }

  .inline .field {
    flex: 1 1 240px;
  }

  .new {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(200px, 100%), 1fr));
    gap: 12px;
    align-items: end;
    padding-top: 18px;
    margin-top: 4px;
    border-top: 1px solid var(--line);
  }

  .new .ghost {
    height: 40px;
  }

  .hash {
    margin-top: 12px;
    font-size: 12.5px;
    color: var(--faint);
  }

  @media (max-width: 760px) {
    .request {
      grid-template-columns: 40px minmax(0, 1fr);
    }

    .code {
      grid-column: 1 / -1;
      align-items: flex-start;
    }


    .row {
      grid-template-columns: 40px minmax(0, 1fr) auto;
    }

    .row .when {
      grid-column: 2;
      text-align: left;
    }

    .row.account .ghost {
      grid-column: 2 / -1;
      justify-self: start;
    }

    .inline {
      padding-left: 0;
    }
  }
</style>
