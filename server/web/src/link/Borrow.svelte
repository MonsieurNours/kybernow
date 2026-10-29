<!--
  « Ordinateur de {hôte} » (prêt de l'ordinateur, docs/pret.md) : le lien
  d'invitation d'un ami (`/link/?borrow=lea`). Son identifiant (prérempli),
  son mot de passe (retenu dans ce navigateur seulement s'il le demande), puis
  l'état de l'ordinateur (« Disponible », « Occupé »…), son créneau, et
  « Emprunter » ou « Demander à {hôte} ». Sans mot de passe d'Admin. Textes de
  docs/link-ui.md (« Prêt de l'ordinateur », « L'ami »).
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { lend, LendRefusal, type LendStatus } from './backend'
  import { isIsolated } from './device'
  import { isActive, lendRefusalText, loadAccount, saveAccount, type BorrowStart } from './lend'
  import { loadName } from './share'
  import GateFrame from './GateFrame.svelte'
  import { tName } from './names'
  import { dayName, hourText } from '../lib/lend'
  import { t } from '../lib/i18n/index.svelte'

  let {
    login: initialLogin,
    password: initialPassword = '',
    hostname = null,
    error: initialError = '',
    onBorrow,
  }: {
    /** Identifiant du lien d'invitation (ou celui du dernier essai). */
    login: string
    /** Mot de passe du dernier essai, gardé en mémoire le temps de la page (jamais écrit sans « Retenir »). */
    password?: string
    /** Nom de l'ordinateur, s'il est connu (jamais par le relais). */
    hostname?: string | null
    /** Refus laissé par une session d'emprunt qui n'a pas pu commencer. */
    error?: string
    onBorrow: (start: BorrowStart) => void
  } = $props()

  const saved = untrack(() => loadAccount(initialLogin))
  let login = $state(untrack(() => initialLogin))
  let password = $state(untrack(() => initialPassword) || saved.password || '')
  let remember = $state(!!saved.password)
  let error = $state(untrack(() => initialError))
  let busy = $state(false)
  /** Identifiant et mot de passe acceptés : l'état de l'ordinateur, relu de temps en temps. */
  let status = $state<LendStatus | null>(null)
  /** Admin injoignable (ou le relais) une fois le compte accepté : « Hors ligne ». */
  let offline = $state(false)
  /** Trop d'essais : le bouton attend la fin du délai donné par la Station. */
  let waiting = $state(false)
  let waitTimer: ReturnType<typeof setTimeout> | null = null

  const host = $derived(status?.host ?? saved.host ?? null)
  const computer = $derived(status?.computer ?? saved.computer ?? hostname)
  /** Ce navigateur a le prêt en cours (fenêtre fermée, réseau perdu) : « Revenir ». */
  const mine = $derived(!!status && status.state === 'lent' && (status.mine === true || isActive({ login: login.trim() })))
  const cardState = $derived(offline ? 'offline' : (status?.state ?? null))
  const stateText = $derived(cardState ? t(`lend.friend.state.${cardState}`) : '')
  const slotLine = $derived.by(() => {
    const slot = status?.slot
    if (!slot?.next) return ''
    if (slot.now) return t('lend.friend.slotNow', { end: hourText(slot.next.end) })
    const start = new Date(slot.next.start * 1000)
    const day = dayName(start.getDay() === 0 ? 7 : start.getDay())
    return t('lend.friend.slotNext', { day, start: hourText(slot.next.start), end: hourText(slot.next.end) })
  })
  /** Ordinateur utilisé, hors de son créneau : il faut demander à l'hôte. */
  const asks = $derived(status?.state === 'busy' && !status.slot?.now)
  const canBorrow = $derived(!!status && !offline && (status.state === 'free' || status.state === 'busy' || mine))

  /** `keep` : le refus laissé par l'essai d'avant reste affiché (retour d'une session d'emprunt). */
  async function check(keep = false) {
    if (busy || !login.trim() || !password) return
    busy = true
    if (!keep) error = ''
    try {
      status = await lend.status(login.trim(), password)
      offline = false
      saveAccount(login.trim(), { host: status.host, computer: status.computer, password: remember ? password : null })
    } catch (e) {
      if (e instanceof LendRefusal && (e.kind === 'network' || e.kind === 'relay') && status) {
        offline = true
      } else {
        status = null
        error = lendRefusalText(e, host)
        if (e instanceof LendRefusal && e.kind === 'too_many') {
          waiting = true
          if (waitTimer) clearTimeout(waitTimer)
          waitTimer = setTimeout(() => (waiting = false), Math.min(e.retryAfter ?? 60, 90) * 1000)
        }
      }
    } finally {
      busy = false
    }
  }

  onMount(() => {
    // Mot de passe retenu : l'état tout de suite.
    if (password) check(true)
    // L'état relu toutes les 30 s (les essais sont comptés : 10 par minute et par adresse).
    const timer = setInterval(() => status && !waiting && check(true), 30_000)
    return () => {
      clearInterval(timer)
      if (waitTimer) clearTimeout(waitTimer)
    }
  })

  function submit(e: SubmitEvent) {
    e.preventDefault()
    check()
  }

  function rememberChanged() {
    // Décoché : le mot de passe est oublié tout de suite ; coché, il est retenu au prochain essai accepté.
    if (!remember) saveAccount(login.trim(), { password: null })
    else if (status) saveAccount(login.trim(), { password })
  }

  function borrow() {
    if (!status || !canBorrow) return
    if (!isIsolated) {
      error = t('web.home.blockedIsolation')
      return
    }
    error = ''
    onBorrow({ request: { login: login.trim(), password }, name: loadName(), host: status.host })
  }
</script>

<GateFrame>
  <div class="head">
    {#if computer}<span class="lbl host"><span class="dot {cardState ?? 'none'}"></span>{computer}</span>{/if}
    <h1 class="disp">{host ? tName('lend.friend.card', 'host', host) : t('lend.friend.titleUnknown')}</h1>
    {#if status}
      <p class="state">
        <span class="pill {cardState}">{stateText}</span>
        {#if slotLine}<span class="slot">{slotLine}</span>{/if}
      </p>
    {:else if host}
      <p>{t('lend.friend.invited', { host })}</p>
    {/if}
  </div>

  {#if status}
    {#if mine}<p class="hint">{t('lend.friend.comeBackHint')}</p>{/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="actions">
      {#if offline}
        <button type="button" class="primary" disabled={busy || waiting} onclick={() => check()}>{t('web.home.retry')}</button>
      {:else}
        <button type="button" class="primary" disabled={!canBorrow || busy} onclick={borrow}>
          {mine ? t('lend.friend.comeBack') : asks && host ? t('lend.friend.ask', { host }) : t('lend.friend.borrow')}
        </button>
      {/if}
      <p class="account">
        <span class="mono">{login}</span>
        <button type="button" class="link" onclick={() => (status = null)}>{t('lend.friends.edit')}</button>
      </p>
    </div>
  {:else}
    <form onsubmit={submit}>
      <label class="sr-only" for="lend-login">{t('lend.friend.login')}</label>
      <input
        id="lend-login"
        class="mono"
        bind:value={login}
        placeholder={t('lend.friend.login')}
        autocomplete="username"
        autocapitalize="off"
        spellcheck="false"
        required
      />
      <label class="sr-only" for="lend-pw">{t('lend.friend.password')}</label>
      <input id="lend-pw" type="password" bind:value={password} placeholder={t('lend.friend.password')} autocomplete="current-password" required />
      <label class="remember"><input type="checkbox" bind:checked={remember} onchange={rememberChanged} />{t('lend.friend.remember')}</label>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <button type="submit" class="primary" disabled={busy || waiting || !login.trim() || !password}>{busy ? t('web.gate.wait') : t('web.gate.continue')}</button>
    </form>
  {/if}
</GateFrame>

<style>
  .head .state {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
  }

  .pill {
    padding: 5px 12px;
    font-size: 13px;
    font-weight: 600;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: var(--muted);
  }

  .pill.free {
    background: var(--ok-bg);
    box-shadow: inset 0 0 0 1px var(--ok-line);
    color: var(--ok-text);
  }

  .pill.busy {
    background: var(--warn-bg);
    box-shadow: inset 0 0 0 1px var(--warn-line);
    color: var(--warn-text);
  }

  .pill.lent,
  .pill.locked,
  .pill.off,
  .pill.offline {
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
  }

  .slot {
    font-size: 14px;
    color: var(--text-2);
  }

  /* Le point du nom de l'ordinateur suit son état. */
  .head :global(.dot.none),
  .head :global(.dot.offline) {
    background: var(--ghost);
  }

  .head :global(.dot.busy) {
    background: var(--warn);
  }

  .head :global(.dot.lent),
  .head :global(.dot.locked),
  .head :global(.dot.off) {
    background: var(--bad);
  }

  .remember {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 14px;
    color: var(--text-2);
    cursor: pointer;
  }

  .remember input {
    width: 18px;
    height: 18px;
    padding: 0;
    margin: 0;
    accent-color: var(--signal-strong);
    box-shadow: none;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .account {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    color: var(--faint);
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 13px;
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .hint {
    font-size: 14px;
    color: var(--signal-light);
  }

  .error {
    font-size: 13.5px;
    color: var(--bad-text);
  }
</style>
