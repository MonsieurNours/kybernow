<!--
  KyberNow Link web (/ecran/) : l'accueil de l'ordinateur, ses réglages, et
  la session dans le navigateur. Servi par Admin, avec sa session : un
  seul mot de passe, celui d'Admin.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { backend, type Session as ConsoleSession } from './backend'
  import { loadPrefs, savePrefs, type Prefs } from './prefs'
  import { takeNotice, takeResume } from './station.svelte'
  import Gate from './Gate.svelte'
  import Home from './Home.svelte'
  import Session from './Session.svelte'
  import { t } from '../lib/i18n/index.svelte'

  let session = $state<ConsoleSession | null>(null)
  let prefs = $state<Prefs>(loadPrefs())
  let inSession = $state(false)
  let loadError = $state(false)
  /** Phrase laissée par un redémarrage ou une mise à jour de la Station. */
  const notice = takeNotice()
  /** Dernier recours de la connexion : elle reprend une fois le mot de passe donné. */
  let resume = takeResume()

  $effect(() => savePrefs($state.snapshot(prefs)))

  async function load() {
    try {
      session = await backend.session()
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
</script>

{#if loadError}
  <div class="center"><p class="muted">{t('web.app.noConsole')}</p></div>
{:else if !session}
  <div class="center" aria-busy="true"></div>
{:else if !session.configured}
  <div class="center">
    <p>{t('web.app.noPassword')}</p>
    <a class="primary" href="/">{t('web.app.openConsole')}</a>
  </div>
{:else if !session.authenticated}
  <Gate hostname={session.hostname ?? null} {notice} onDone={load} />
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
  <Home hostname={session.hostname ?? t('web.app.thisComputer')} bind:prefs {notice} onConnect={() => (inSession = true)} onSignedOut={load} />
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
