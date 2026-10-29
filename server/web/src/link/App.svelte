<!--
  KyberNow Link web (/link/) : l'accueil de l'ordinateur, ses réglages, et
  la session dans le navigateur. Servi par Admin, avec sa session : un
  seul mot de passe, celui d'Admin. Le lien d'un partage (`?join=<code>`)
  ouvre « Rejoindre » sans ce mot de passe : un invité n'a ni accueil, ni
  réglages, ni session d'Admin (KyberNow Share, docs/kybernow-share.md). Par
  le relais (`relay` de `/api/session`, docs/relais.md), il n'y a que
  « Rejoindre » : ni mot de passe, ni nom d'ordinateur avant l'accord de l'hôte.
  Prêt de l'ordinateur (docs/pret.md) : un ami emprunte avec un code de prêt
  (« Rejoindre ») ou avec son compte (`?borrow=<identifiant>`, « Ordinateur de
  {hôte} »), toujours sans le mot de passe d'Admin.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { backend, type Session as ConsoleSession } from './backend'
  import { loadPrefs, savePrefs, type Prefs } from './prefs'
  import { setRelay } from './plan'
  import { takeNotice, takeResume } from './station.svelte'
  import { codeFromUrl, forgetCodeInUrl, type GuestStart } from './share'
  import { loginFromUrl, type BorrowStart } from './lend'
  import Borrow from './Borrow.svelte'
  import Gate from './Gate.svelte'
  import Home from './Home.svelte'
  import Join from './Join.svelte'
  import Session from './Session.svelte'
  import { t } from '../lib/i18n/index.svelte'

  let session = $state<ConsoleSession | null>(null)
  let prefs = $state<Prefs>(loadPrefs())
  let inSession = $state(false)
  let loadError = $state(false)
  /** Code du lien d'un partage : « Rejoindre » s'ouvre dessus, sans mot de passe. */
  const linkCode = codeFromUrl()
  /** « Rejoindre un partage », ouvert par le lien ou par « Rejoindre avec un code ». */
  let joining = $state(linkCode !== null)
  /** Le code du lien ne sert qu'à la première fois. */
  let code = $state(linkCode ?? '')
  /** La session d'invité en cours (le partage rejoint). */
  let guest = $state<GuestStart | null>(null)
  /** Phrase laissée par un redémarrage ou une mise à jour de la Station. */
  const notice = takeNotice()
  /** Dernier recours de la connexion : elle reprend une fois le mot de passe donné. */
  let resume = takeResume()
  /** Identifiant du lien d'invitation d'un ami (`?borrow=lea`) : « Ordinateur de {hôte} ». */
  const borrowLogin = loginFromUrl()
  /** La session d'emprunt en cours, et l'écran d'où elle vient (code ou compte d'ami). */
  let borrowing = $state<BorrowStart | null>(null)
  let borrowFrom: 'join' | 'card' = 'card'
  /** Refus d'un emprunt qui n'a pas pu commencer, dit sur l'écran d'où il vient. */
  let borrowError = $state('')
  /** Le dernier compte essayé : la carte le reprend (le mot de passe en mémoire seulement). */
  let lastAccount = $state<{ login: string; password: string } | null>(null)
  let joinError = $state('')

  $effect(() => savePrefs($state.snapshot(prefs)))

  async function load() {
    try {
      session = await backend.session()
      // Par le relais, l'image passera en WebSocket (Admin n'y ouvre pas WebTransport).
      if (session.relay) setRelay(true)
      if (session.hostname) document.title = `${session.hostname} · KyberNow Link`
      if (session.authenticated && resume) {
        resume = false
        inSession = true
      }
    } catch {
      loadError = true
    }
  }

  onMount(load)

  /**
   * Fin d'une session d'emprunt : retour à « Rejoindre » ou à la carte, avec le refus s'il y en a un.
   * `over` : le prêt est fini (rendu, repris) ; un code n'y sert plus.
   */
  function endBorrow(error: string | null, over: boolean) {
    borrowing = null
    if (borrowFrom === 'join') {
      joining = true
      joinError = error ?? ''
      if (over) {
        code = ''
        forgetCodeInUrl()
      }
    } else {
      borrowError = error ?? ''
    }
    document.title = session?.hostname && !session.relay ? `${session.hostname} · KyberNow Link` : 'KyberNow Link'
  }

  function startBorrow(start: BorrowStart, from: 'join' | 'card') {
    borrowFrom = from
    if ('login' in start.request) lastAccount = { ...start.request }
    borrowError = ''
    joinError = ''
    borrowing = start
  }

  function endGuest() {
    guest = null
    joining = true
    code = ''
    forgetCodeInUrl()
    document.title = session?.hostname ? `${session.hostname} · KyberNow Link` : 'KyberNow Link'
  }
</script>

{#if loadError}
  <div class="center"><p class="muted">{t('web.app.noConsole')}</p></div>
{:else if guest}
  <Session hostname={guest.join.computer} bind:prefs {guest} onEnd={endGuest} />
{:else if borrowing}
  <Session hostname={session && !session.relay ? (session.hostname ?? '') : ''} bind:prefs borrow={borrowing} onEnd={() => endBorrow(null, false)} onLendEnd={endBorrow} />
{:else if borrowLogin !== null}
  <Borrow login={lastAccount?.login ?? borrowLogin} password={lastAccount?.password ?? ''} hostname={session && !session.relay ? (session.hostname ?? null) : null} error={borrowError} onBorrow={(start) => startBorrow(start, 'card')} />
{:else if joining || session?.relay}
  <!-- Le nom de l'ordinateur une fois la Station répondue ; jamais par le relais (connu après l'accord). -->
  <Join
    hostname={session && !session.relay ? (session.hostname ?? t('web.app.thisComputer')) : null}
    {code}
    error={joinError}
    onLend={(start) => startBorrow(start, 'join')}
    onJoined={(start) => {
      guest = start
      document.title = `${t('share.guest.on', { name: start.join.computer })} · KyberNow Link`
    }}
    onCancel={linkCode === null && !session?.relay
      ? () => {
          joining = false
          code = ''
        }
      : undefined}
  />
{:else if !session}
  <div class="center" aria-busy="true"></div>
{:else if !session.configured}
  <div class="center">
    <p>{t('web.app.noPassword')}</p>
    <a class="primary" href="/">{t('web.app.openConsole')}</a>
  </div>
{:else if !session.authenticated}
  <Gate hostname={session.hostname ?? null} {notice} onDone={load} onJoin={() => (joining = true)} />
{:else if inSession}
  <Session
    hostname={session.hostname ?? t('web.app.thisComputer')}
    bind:prefs
    onEnd={() => {
      inSession = false
      // La connexion à la console a pu expirer pendant la session : on
      // redemande alors le mot de passe (une console injoignable n'y change rien).
      backend
        .session()
        .then((s) => {
          if (!s.authenticated) session = s
        })
        .catch(() => {})
    }}
  />
{:else}
  <Home hostname={session.hostname ?? t('web.app.thisComputer')} bind:prefs {notice} onConnect={() => (inSession = true)} onSignedOut={load} onJoin={() => (joining = true)} />
{/if}

<style>
  .center {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 18px;
    padding: 24px;
    text-align: center;
  }
</style>
