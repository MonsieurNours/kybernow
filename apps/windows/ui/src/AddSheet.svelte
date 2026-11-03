<!--
  Ajouter un ordinateur : le trouver (réseau, lien d'Admin, adresse),
  puis y avoir accès (code à accepter dans Admin, ou compte et mot de passe).
  Pas de QR code à la caméra : sur un PC, le lien d'Admin suffit.
  « Avec un compte d'ami » (prêt de l'ordinateur) : l'identifiant et le mot de passe que l'ami
  vous a donnés dans son Admin ; l'ordinateur s'ajoute à part, « Ordinateur de {hôte} ».

  Consoles PlayStation (docs/link-ui.md « Consoles PlayStation ») : l'onglet « PlayStation » (les
  comptes PlayStation Network, « Renommer », « Retirer », « Ajouter un compte » ; les consoles sur ce
  réseau et « Associer » ; « Envoyer à un appareil » et « Recevoir d'un autre appareil » entre
  appareils), les consoles trouvées aussi dans « Sur ce réseau », et « Par adresse » qui cherche
  d'abord une console à l'adresse (1,5 s), puis la Station comme avant. Associer une console ferme
  l'ajout et ouvre l'association (PairSheet, App.svelte).
-->
<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Switch from './lib/Switch.svelte'
  import type { Candidate, Computer, Found, PsAccount, PsFound, PsHandoffEvent, PsState, PsWaiting } from './lib/api'
  import { fingerprintGroups, invoke, listen, psAccountName, psConsoleCount, psGroupCode, psPlatform } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    discovered,
    computers,
    deviceName,
    initial,
    onclose,
    onadded,
    ps = { accounts: [], consoles: [], found: [], searched: false },
    initialWay = null,
    already = false,
    onpair = () => {},
    onaddaccount = () => {},
  }: {
    discovered: Found[]
    computers: Computer[]
    deviceName: string
    initial: Found | null
    onclose: () => void
    onadded: (id: string) => void
    /** Les consoles PlayStation : comptes, consoles gardées, consoles qui répondent. */
    ps?: PsState
    /** L'onglet ouvert d'abord (`playstation`). */
    initialWay?: 'playstation' | null
    /** « Ajouter un compte » a lu un compte déjà gardé : « Ce compte est déjà là. » */
    already?: boolean
    /** « Associer » une console : l'ajout se ferme, l'association s'ouvre. */
    onpair?: (found: PsFound) => void
    /** « Ajouter un compte » : l'association sans console, par-dessus l'ajout. */
    onaddaccount?: () => void
  } = $props()

  type Way = 'reseau' | 'lien' | 'adresse' | 'playstation'
  let way = $state<Way>(untrack(() => initialWay) ?? 'reseau')
  let candidate = $state<Candidate | null>(null)
  let access = $state<'code' | 'compte' | 'ami'>('code')
  let error = $state<string | null>(null)
  let working = $state(false)

  // Lien et adresse.
  let link = $state('')
  let host = $state('')
  let port = $state('9443')
  let seen = $state<Candidate | null>(null)

  // Code.
  let code = $state<string | null>(null)
  let pairState = $state<'pending' | 'refused' | 'expired'>('pending')
  // Interrogation d'Admin : un appel à la fois, relancé 1,5 s après
  // la réponse ; `round` invalide une interrogation dépassée.
  let timer: ReturnType<typeof setTimeout> | null = null
  let round = 0

  // Compte.
  let user = $state('')
  let password = $state('')
  // Compte d'ami (prêt de l'ordinateur) : « Retenir le mot de passe ».
  let remember = $state(true)

  // Ouvert depuis une carte « Sur le réseau » : l'ordinateur est déjà choisi.
  const first = untrack(() => initial)
  if (first) choose({ host: first.host, port: first.port, name: first.name, fingerprint: first.fingerprint })

  const known = (f: Found) => computers.some((c) => c.host === f.host && c.port === f.port)

  function stopPolling() {
    round++
    if (timer) clearTimeout(timer)
    timer = null
  }

  /** Ordinateur trouvé : l'accès par code commence aussitôt. */
  function choose(c: Candidate) {
    candidate = c
    error = null
    if (access === 'code') startCode()
  }

  onDestroy(() => {
    stopPolling()
    invoke('pair_cancel').catch(() => {})
  })

  async function run<T>(f: () => Promise<T>): Promise<T | undefined> {
    working = true
    error = null
    try {
      return await f()
    } catch (e) {
      error = String(e)
    } finally {
      working = false
    }
  }

  async function fromLink() {
    const c = await run(() => invoke<Candidate>('parse_link', { link }))
    if (c) choose(c)
  }

  /**
   * « Continuer » de « Par adresse » : une console à cette adresse d'abord (1,5 s : une Station et une console ne
   * répondent pas à la même adresse), qui ouvre son association ; sinon la Station, comme avant. Sans port d'Admin,
   * rien qui réponde : « Console injoignable. ».
   */
  async function fromAddress() {
    working = true
    error = null
    try {
      const f = await invoke<PsFound | null>('ps_probe_address', { host: host.trim() })
      if (f) {
        working = false
        onpair(f)
        return
      }
    } catch {
      // Rien de la console : la Station, comme avant.
    }
    working = false
    if (!port.trim()) {
      error = t('ps.unreachable')
      return
    }
    const c = await run(() => invoke<Candidate>('probe_address', { host: host.trim(), port: Number(port) || 9443 }))
    if (c) seen = c
  }

  // MARK: - Consoles PlayStation

  /** La console d'une réponse, si elle est gardée, et ses comptes associés. */
  const keptOf = (f: PsFound) => ps.consoles.find((c) => c.host_id === f.host_id) ?? null
  /** « Allumée · 192.168.1.110 », « En veille · Associée : Matthieu_PSN, Camille-92 ». */
  function foundDetail(f: PsFound) {
    const state = f.state === 'ready' ? t('ps.on') : t('ps.standby')
    const kept = keptOf(f)
    return kept?.accounts.length ? `${state} · ${t('ps.paired_with', { names: kept.accounts.map(psAccountName).join(', ') })}` : `${state} · ${f.address}`
  }

  // Renommer, retirer un compte.
  let renaming = $state<string | null>(null)
  let newName = $state('')
  let removing = $state<PsAccount | null>(null)

  async function psCall(f: () => Promise<unknown>) {
    error = null
    try {
      await f()
    } catch (e) {
      error = String(e)
    }
  }

  async function rename(a: PsAccount) {
    const name = newName.trim()
    renaming = null
    if (name && name !== a.name) await psCall(() => invoke('ps_account_rename', { account: a.id, name }))
  }

  async function removeAccount(a: PsAccount) {
    removing = null
    await psCall(() => invoke('ps_account_remove', { account: a.id }))
  }

  const removeQuestion = (a: PsAccount) =>
    t(a.consoles === 0 ? 'ps.remove_question_none' : a.consoles === 1 ? 'ps.remove_question_one' : 'ps.remove_question_other', { name: psAccountName(a), n: a.consoles })

  // « Envoyer à un appareil » : les appareils qui attendent, cherchés toutes les 3 s tant que l'onglet est ouvert.
  const hasAccounts = $derived(ps.accounts.length > 0)
  const psOpen = $derived(way === 'playstation' && !candidate)
  let waiting = $state<PsWaiting[]>([])
  let sendTo = $state<PsWaiting | null>(null)
  let sendCode = $state('')
  let sending = $state(false)
  let sent = $state<string | null>(null)
  let sendError = $state<string | null>(null)
  $effect(() => {
    if (!psOpen || !hasAccounts) return
    let stop = false
    let timer: ReturnType<typeof setTimeout> | null = null
    const look = async () => {
      try {
        const w = await invoke<PsWaiting[]>('ps_handoff_discover')
        if (!stop) waiting = w
      } catch {
        // Une recherche manquée : la suivante.
      }
      if (!stop) timer = setTimeout(look, 3000)
    }
    look()
    return () => {
      stop = true
      if (timer) clearTimeout(timer)
    }
  })

  function chooseDevice(d: PsWaiting) {
    sendTo = sendTo?.address === d.address ? null : d
    sendCode = ''
    sent = null
    sendError = null
  }

  async function send(e: Event) {
    e.preventDefault()
    if (!sendTo || sendCode.length !== 6 || sending) return
    sending = true
    sendError = null
    try {
      sent = await invoke<string>('ps_handoff_send', { address: sendTo.address, name: sendTo.name, code: sendCode })
      sendTo = null
      sendCode = ''
    } catch (e) {
      sendError = String(e)
    } finally {
      sending = false
    }
  }

  // « Recevoir d'un autre appareil » : un code à 6 chiffres, l'attente s'arrête à la fermeture.
  let receiveCode = $state<string | null>(null)
  let receiveExpired = $state(false)
  let received = $state<string | null>(null)
  let receiving = false

  async function startReceive() {
    error = null
    receiveExpired = false
    received = null
    try {
      receiveCode = await invoke<string | null>('ps_handoff_receive', { on: true })
      receiving = receiveCode !== null
    } catch (e) {
      error = String(e)
    }
  }

  function stopReceive() {
    receiveCode = null
    if (!receiving) return
    receiving = false
    invoke('ps_handoff_receive', { on: false }).catch(() => {})
  }

  onMount(() => {
    const off = listen<PsHandoffEvent>('ps-handoff', (e) => {
      if (e.event === 'received') {
        received = e.text
        stopReceive()
      } else if (e.event === 'new_code') {
        receiveCode = e.code
      } else {
        receiveExpired = true
        stopReceive()
      }
    })
    return () => off.then((f) => f())
  })
  onDestroy(stopReceive)

  async function startCode() {
    stopPolling()
    const mine = round
    pairState = 'pending'
    code = null
    const c = await run(() => invoke<string>('pair_start', { candidate }))
    if (!c || mine !== round) return
    code = c
    const poll = async () => {
      try {
        const r = await invoke<string>('pair_poll')
        if (mine !== round) return
        if (r === 'pending') {
          timer = setTimeout(poll, 1500)
          return
        }
        if (r === 'refused' || r === 'expired') pairState = r
        else onadded(r)
      } catch (e) {
        if (mine === round) error = String(e)
      }
    }
    timer = setTimeout(poll, 1500)
  }

  async function withAccount() {
    const id = await run(() => invoke<string>('add_computer', { candidate, user, password }))
    if (id) onadded(id)
  }

  /** « Avec un compte d'ami » : l'ordinateur d'un ami, à part de vos ordinateurs (son état vérifié par Admin). */
  async function withFriend() {
    const id = await run(() => invoke<string>('lend_add', { link: null, candidate, login: user, password, remember }))
    if (id) onadded(id)
  }

  function chooseAccess(a: 'code' | 'compte' | 'ami') {
    if (a === access) return
    access = a
    error = null
    if (a !== 'code') {
      stopPolling()
      invoke('pair_cancel').catch(() => {})
    } else startCode()
  }

  function back() {
    stopPolling()
    invoke('pair_cancel').catch(() => {})
    candidate = null
    seen = null
    code = null
    error = null
  }

  const ways: { value: Way; title: string; sub: string; icon: string }[] = $derived([
    { value: 'reseau', title: t('add.way_network'), sub: t('add.way_network_sub'), icon: 'wifi' },
    { value: 'lien', title: t('add.way_link'), sub: t('add.way_link_sub'), icon: 'link' },
    { value: 'adresse', title: t('add.way_address'), sub: t('add.way_address_sub'), icon: 'globe' },
    { value: 'playstation', title: t('ps.tab'), sub: t('ps.tab_sub'), icon: 'gamepad' },
  ])
</script>

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="add-title">
    <nav>
      <h2 id="add-title">{t('add.title')}</h2>
      {#each ways as w (w.value)}
        <button class="way" class:on={!candidate && way === w.value} onclick={() => { back(); if (w.value !== 'playstation') stopReceive(); way = w.value }}>
          <Icon name={w.icon} size={18} />
          <span><span class="t">{w.title}</span><span class="s">{w.sub}</span></span>
        </button>
      {/each}
      <p class="aside">{t('add.aside')}</p>
    </nav>

    <div class="pane">
      <button class="close" aria-label={t('common.close')} onclick={onclose}><Icon name="x" size={14} /></button>

      {#if !candidate}
        {#if way === 'reseau'}
          <div class="eyebrow">{t('add.on_this_network')}</div>
          {#if discovered.length === 0 && ps.found.length === 0}
            <p class="hint">{t('add.none_found')}</p>
          {:else}
            <ul class="found">
              <!-- Les consoles trouvées, avec les ordinateurs : « {nom} · PS5 », « Associer ». -->
              {#each ps.found as f (f.host_id)}
                {@const kept = keptOf(f)}
                {@const done = !!kept?.accounts.length && kept.free_accounts === 0}
                <li>
                  <button disabled={done} onclick={() => onpair(f)}>
                    <span class="name">{f.name} · {f.model}</span>
                    <span class="mono addr">{f.address}</span>
                    <span class="go">{done ? t('ps.paired_with', { names: kept!.accounts.map(psAccountName).join(', ') }) : kept?.accounts.length ? t('ps.pair_another') : t('ps.pair')}</span>
                  </button>
                </li>
              {/each}
              {#each discovered as f (f.id)}
                <li>
                  <button disabled={known(f)} onclick={() => choose({ host: f.host, port: f.port, name: f.name, fingerprint: f.fingerprint })}>
                    <span class="name">{f.name}</span>
                    <span class="mono addr">{f.host}:{f.port}</span>
                    <span class="go">{known(f) ? t('add.already_added') : t('add.choose')}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        {:else if way === 'lien'}
          <label class="col">
            <span class="eyebrow">{t('add.link_label')}</span>
            <textarea class="field area mono" rows="3" placeholder="kybernow://192.168.1.100:31544?name=…&fp=sha256:…" bind:value={link}></textarea>
          </label>
          <div><button class="primary" disabled={working || !link.trim()} onclick={fromLink}>{t('common.continue')}</button></div>
        {:else if way === 'playstation'}
          <!-- Comptes PlayStation Network, consoles sur ce réseau, envoi et réception entre appareils. -->
          <section class="ps">
            <h3>{t('ps.accounts_title')}</h3>
            {#if ps.accounts.length === 0}<p class="hint">{t('ps.no_account')}</p>{/if}
            {#each ps.accounts as a (a.id)}
              <div class="psrow">
                {#if renaming === a.id}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="field"
                    aria-label={t('ps.name_field')}
                    maxlength="40"
                    autofocus
                    bind:value={newName}
                    onkeydown={(e) => (e.key === 'Enter' ? rename(a) : e.key === 'Escape' && (renaming = null))}
                  />
                  <div class="acts">
                    <button class="ghost" onclick={() => rename(a)}>{t('ps.rename')}</button>
                    <button class="ghost" onclick={() => (renaming = null)}>{t('common.cancel')}</button>
                  </div>
                {:else}
                  <span class="txt"><span class="t">{psAccountName(a)}</span><span class="s">{psConsoleCount(a.consoles)}</span></span>
                  <div class="acts">
                    <button class="ghost" onclick={() => ((newName = a.name ?? ''), (removing = null), (renaming = a.id))}>{t('ps.rename')}</button>
                    <button class="ghost" onclick={() => ((renaming = null), (removing = a))}>{t('ps.remove')}</button>
                  </div>
                {/if}
              </div>
              {#if removing?.id === a.id}
                <div class="confirm" role="alertdialog" aria-label={removeQuestion(a)}>
                  <strong>{removeQuestion(a)}</strong>
                  <p class="hint">{t('ps.remove_help')}</p>
                  <div class="acts">
                    <button class="danger" onclick={() => removeAccount(a)}>{t('ps.remove')}</button>
                    <button class="ghost" onclick={() => (removing = null)}>{t('common.cancel')}</button>
                  </div>
                </div>
              {/if}
            {/each}
            {#if already}<div class="info" role="status">{t('ps.already_here')}</div>{/if}
            <div><button class="ghost add" onclick={onaddaccount}><Icon name="plus" size={14} />{t('ps.add_account')}</button></div>
          </section>

          <section class="ps">
            <h3>{t('ps.consoles_title')}</h3>
            {#if ps.found.length === 0}<p class="hint">{t('ps.no_console')}</p>{/if}
            {#each ps.found as f (f.host_id)}
              {@const kept = keptOf(f)}
              <div class="psrow" class:lit={!kept?.accounts.length}>
                <span class="txt"><span class="t">{f.name} · {f.model}</span><span class="s">{foundDetail(f)}</span></span>
                {#if !kept?.accounts.length}
                  <button class="primary small" onclick={() => onpair(f)}>{t('ps.pair')}</button>
                {:else if kept.free_accounts > 0}
                  <button class="ghost" onclick={() => onpair(f)}>{t('ps.pair_another')}</button>
                {/if}
              </div>
            {/each}
          </section>

          {#if hasAccounts}
            <section class="ps">
              <h3>{t('ps.send_title')}</h3>
              {#if waiting.length === 0}<p class="hint">{t('ps.no_device')}</p>{/if}
              {#each waiting as d (d.address)}
                <button class="psrow pick" class:lit={sendTo?.address === d.address} aria-expanded={sendTo?.address === d.address} onclick={() => chooseDevice(d)}>
                  <span class="txt"><span class="t">{d.name} · {psPlatform(d.platform)}</span></span>
                  <Icon name="send" size={16} />
                </button>
                {#if sendTo?.address === d.address}
                  <form class="send" onsubmit={send}>
                    <p class="hint">{t('ps.send_code', { name: d.name })}</p>
                    <div class="row">
                      <input
                        class="field code6 mono"
                        inputmode="numeric"
                        autocomplete="off"
                        maxlength="6"
                        placeholder="123456"
                        aria-label={t('ps.code6_field')}
                        title={t('ps.code6_field')}
                        value={sendCode}
                        oninput={(e) => (sendCode = (e.currentTarget as HTMLInputElement).value.replace(/\D/g, '').slice(0, 6))}
                        disabled={sending}
                      />
                      <button type="submit" class="primary small" disabled={sendCode.length !== 6 || sending}>{t('ps.send')}</button>
                    </div>
                    {#if sendError}<div class="error" role="alert">{sendError}</div>{/if}
                  </form>
                {/if}
              {/each}
              {#if sent}<div class="info" role="status">{sent}</div>{/if}
            </section>
          {/if}

          <section class="ps">
            <h3>{t('ps.receive_title')}</h3>
            {#if receiveCode}
              <div class="bigcode mono" aria-label={t('ps.code6_field') + ' ' + receiveCode}>{psGroupCode(receiveCode)}</div>
              <p class="hint">{t('ps.receive_help')}</p>
            {:else if receiveExpired}
              <p class="hint">{t('ps.code_expired')}</p>
              <div><button class="ghost" onclick={startReceive}>{t('add.new_code')}</button></div>
            {:else}
              {#if received}<div class="info" role="status">{received}</div>{/if}
              <div><button class="ghost add" onclick={startReceive}><Icon name="receive" size={16} />{t('ps.receive_title')}</button></div>
            {/if}
          </section>
        {:else}
          {#if !seen}
            <p class="hint">{t('ps.address_help')}</p>
            <div class="grid">
              <label class="col"><span class="eyebrow">{t('add.address')}</span><input class="field" placeholder="192.168.1.100" bind:value={host} /></label>
              <label class="col"><span class="eyebrow">{t('add.console_port')}</span><input class="field mono" inputmode="numeric" bind:value={port} /></label>
            </div>
            <div><button class="primary" disabled={working || !host.trim()} onclick={fromAddress}>{working ? t('common.searching') : t('common.continue')}</button></div>
          {:else}
            <div class="eyebrow">{t('add.fingerprint')}</div>
            <div class="fp mono">
              {#each fingerprintGroups(seen.fingerprint) as g, i}<span class:strong={i < 4}>{g}</span>{/each}
            </div>
            <p class="hint">{t('add.compare')}</p>
            <div class="row">
              <button class="primary" onclick={() => seen && choose(seen)}>{t('add.same')}</button>
              <button class="ghost" onclick={() => (seen = null)}>{t('common.cancel')}</button>
            </div>
          {/if}
        {/if}
      {:else}
        <div class="who">
          <div>
            <div class="eyebrow">{t('add.chosen')}</div>
            <div class="whoname">{candidate.name ?? candidate.host} <span class="mono addr">· {candidate.host}:{candidate.port}</span></div>
          </div>
          <button class="ghost" onclick={back}>{t('add.change')}</button>
        </div>

        <div class="tabs" role="tablist">
          <button role="tab" aria-selected={access === 'code'} onclick={() => chooseAccess('code')}>{t('add.with_code')}</button>
          <button role="tab" aria-selected={access === 'compte'} onclick={() => chooseAccess('compte')}>{t('add.with_account')}</button>
          <button role="tab" aria-selected={access === 'ami'} onclick={() => chooseAccess('ami')}>{t('lend.with_friend')}</button>
        </div>

        {#if access === 'code'}
          {#if code}
            <div class="digits" aria-label={t('add.code_label', { code })}>
              {#each code.split('') as d, i}{#if i === 3}<span class="gap"></span>{/if}<span class="d mono">{d}</span>{/each}
            </div>
            <ol>
              <li>{t('add.step1_before', { name: candidate.name ?? t('add.the_computer') })}<strong>{t('add.step1_strong')}</strong>{t('add.step1_after')}</li>
              <li>{t('add.step2_before')}<strong>{deviceName || t('add.this_pc')}</strong>{t('add.step2_after')}</li>
              <li>{t('add.step3')}</li>
            </ol>
            {#if pairState === 'pending'}
              <div class="wait"><span class="pulse"></span>{t('add.waiting')}</div>
            {:else}
              <div class="error">{pairState === 'refused' ? t('add.refused') : t('add.expired')}</div>
              <div><button class="ghost" onclick={startCode}>{t('add.new_code')}</button></div>
            {/if}
          {:else if working}
            <div class="wait"><span class="pulse"></span>{t('add.asking')}</div>
          {:else if error}
            <div><button class="ghost" onclick={startCode}>{t('add.retry')}</button></div>
          {/if}
        {:else if access === 'ami'}
          <div class="grid">
            <label class="col"><span class="eyebrow">{t('lend.username')}</span><input class="field" autocomplete="username" bind:value={user} /></label>
            <label class="col"><span class="eyebrow">{t('lend.password')}</span><input class="field" type="password" autocomplete="current-password" bind:value={password} /></label>
          </div>
          <div class="remember">
            <Switch checked={remember} label={t('lend.remember')} onchange={(v) => (remember = v)} />
            <span>{t('lend.remember')}</span>
          </div>
          <div><button class="primary" disabled={working || !user.trim() || !password} onclick={withFriend}>{working ? t('add.checking') : t('lend.add')}</button></div>
        {:else}
          <div class="grid">
            <label class="col"><span class="eyebrow">{t('add.account')}</span><input class="field" autocomplete="username" bind:value={user} /></label>
            <label class="col"><span class="eyebrow">{t('add.password')}</span><input class="field" type="password" autocomplete="current-password" bind:value={password} /></label>
          </div>
          <p class="hint">{t('add.account_hint')}</p>
          <div><button class="primary" disabled={working || !user.trim() || !password} onclick={withAccount}>{working ? t('add.checking') : t('add.add')}</button></div>
        {/if}
      {/if}

      {#if error}<div class="error" role="alert">{error}</div>{/if}
    </div>
  </div>
</div>

<style>
  .veil {
    position: absolute;
    inset: 0;
    z-index: 30;
    background: rgb(5 7 10 / 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 56px 24px 24px;
    animation: fade 0.25s var(--ease-out);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .sheet {
    width: min(980px, 100%);
    height: min(680px, 100%);
    display: flex;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
    overflow: hidden;
    animation: rise 0.32s var(--ease-out);
  }

  @keyframes rise {
    from {
      transform: translateY(18px) scale(0.985);
      opacity: 0;
    }
  }

  nav {
    width: 320px;
    flex: none;
    padding: 30px 18px;
    border-right: 1px solid var(--line);
    background: var(--ink-0);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h2 {
    margin: 0 10px 18px;
    font-size: 25px;
    font-weight: 640;
    font-stretch: 125%;
  }

  .way {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 13px 14px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: none;
    text-align: left;
    color: var(--muted);
  }

  .way > span {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .way .t {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  .way .s {
    font-size: 12px;
    color: var(--faint);
  }

  .way.on {
    border-color: var(--signal);
    background: var(--ink-3);
    box-shadow: 0 0 14px rgb(109 91 255 / 0.25);
  }

  .way.on :global(svg) {
    color: var(--signal);
  }

  .aside {
    margin: auto 8px 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--faint);
  }

  .pane {
    position: relative;
    flex: 1;
    padding: 40px 48px;
    display: flex;
    flex-direction: column;
    gap: 22px;
    overflow-y: auto;
  }

  .close {
    position: absolute;
    right: 16px;
    top: 16px;
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

  .close:hover {
    background: rgb(255 255 255 / 0.06);
  }

  .hint {
    margin: 0;
    font-size: 14px;
    line-height: 1.55;
    color: var(--muted);
  }

  .found {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .found button {
    width: 100%;
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 16px;
    padding: 16px 18px;
    border: 1px solid var(--line-2);
    border-radius: 12px;
    background: var(--ink-2);
    text-align: left;
  }

  .found button:hover:not(:disabled) {
    border-color: var(--signal);
  }

  .found .name {
    font-size: 16px;
    font-weight: 600;
  }

  .addr {
    font-size: 12px;
    color: var(--faint);
  }

  .go {
    font-size: 13px;
    color: var(--signal);
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 14px;
  }

  .area {
    height: auto;
    padding: 12px 14px;
    resize: none;
    font-size: 13px;
    line-height: 1.5;
  }

  .fp {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    font-size: 15px;
    color: var(--faint);
    user-select: text;
  }

  .fp .strong {
    color: var(--text);
    padding: 4px 6px;
    border-radius: 6px;
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.4);
  }

  .row {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .who {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    padding-right: 40px;
  }

  .whoname {
    margin-top: 6px;
    font-size: 20px;
    font-weight: 600;
  }

  .tabs {
    display: flex;
    gap: 6px;
    border-bottom: 1px solid var(--line);
  }

  .tabs button {
    height: 40px;
    padding: 0 14px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    font-size: 14px;
  }

  .tabs button[aria-selected='true'] {
    color: var(--text);
    border-bottom-color: var(--signal);
  }

  .digits {
    display: flex;
    gap: 10px;
  }

  .d {
    width: 64px;
    height: 84px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: var(--ink-2);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 40px;
    user-select: text;
  }

  .gap {
    width: 18px;
  }

  ol {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 15px;
    line-height: 1.5;
  }

  .wait {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--ink-0);
    font-size: 14px;
  }

  .pulse {
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background: var(--signal);
    box-shadow: 0 0 0 4px rgb(154 140 255 / 0.18), 0 0 14px var(--signal);
    animation: breathe 1.6s ease-in-out infinite;
  }

  @keyframes breathe {
    50% {
      box-shadow: 0 0 0 7px rgb(154 140 255 / 0.08), 0 0 6px var(--signal);
    }
  }

  .remember {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 14px;
  }

  /* Onglet PlayStation : des rangées comme celles de « Sur ce réseau ». */
  .ps {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .ps + .ps {
    margin-top: 8px;
  }

  h3 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  .psrow {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 14px 12px 18px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--ink-2);
    text-align: left;
    color: var(--muted);
  }

  .psrow.lit {
    border-color: rgb(154 140 255 / 0.5);
    background: var(--signal-soft);
  }

  .psrow.pick:hover {
    border-color: var(--signal);
  }

  .psrow .field {
    flex: 1;
    height: 38px;
  }

  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .txt .t {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .txt .s {
    font-size: 13px;
    color: var(--faint);
  }

  .acts {
    display: flex;
    gap: 8px;
  }

  .acts .ghost,
  .psrow > .ghost {
    height: 34px;
  }

  .primary.small {
    height: 38px;
    padding: 0 18px;
    font-size: 14px;
  }

  .ghost.add {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .confirm {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 18px;
    border: 1px solid rgb(255 107 107 / 0.4);
    border-radius: 12px;
    background: rgb(255 107 107 / 0.06);
    font-size: 14px;
  }

  .danger {
    height: 34px;
    padding: 0 14px;
    border: 0;
    border-radius: 8px;
    background: var(--bad);
    color: #1a0705;
    font-size: 14px;
    font-weight: 600;
  }

  .info {
    padding: 10px 14px;
    border: 1px solid rgb(154 140 255 / 0.4);
    border-radius: 10px;
    background: var(--signal-soft);
    font-size: 13px;
  }

  .send {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 0 4px 6px;
  }

  .code6 {
    width: 180px;
    font-size: 20px;
    letter-spacing: 0.14em;
  }

  .bigcode {
    font-size: 44px;
    letter-spacing: 0.08em;
    color: var(--text);
    user-select: text;
  }
</style>
