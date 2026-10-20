<!--
  Client web KyberNow (/ecran/) : l'accueil de l'ordinateur, ses réglages, et
  la session dans le navigateur. Servi par la console, avec sa session : un
  seul mot de passe, celui de la console.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { backend, type Session as ConsoleSession } from './backend'
  import { loadPrefs, savePrefs, type Prefs } from './prefs'
  import Gate from './Gate.svelte'
  import Home from './Home.svelte'
  import Session from './Session.svelte'

  let session = $state<ConsoleSession | null>(null)
  let prefs = $state<Prefs>(loadPrefs())
  let inSession = $state(false)
  let loadError = $state('')

  $effect(() => savePrefs($state.snapshot(prefs)))

  async function load() {
    try {
      session = await backend.session()
      if (session.hostname) document.title = `${session.hostname} · KyberNow`
    } catch {
      loadError = 'La console ne répond pas.'
    }
  }

  onMount(load)
</script>

{#if loadError}
  <div class="center"><p class="muted">{loadError}</p></div>
{:else if !session}
  <div class="center" aria-busy="true"></div>
{:else if !session.configured}
  <div class="center">
    <p>La console de cet ordinateur n'a pas encore de mot de passe.</p>
    <a class="primary" href="/">Ouvrir la console</a>
  </div>
{:else if !session.authenticated}
  <Gate hostname={session.hostname ?? null} onDone={load} />
{:else if inSession}
  <Session hostname={session.hostname ?? 'Cet ordinateur'} bind:prefs onEnd={() => (inSession = false)} />
{:else}
  <Home hostname={session.hostname ?? 'Cet ordinateur'} bind:prefs onConnect={() => (inSession = true)} onSignedOut={load} />
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
