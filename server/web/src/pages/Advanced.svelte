<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { markRestartPending, notify, errorText } from '../lib/store.svelte'

  let original = $state('')
  let text = $state('')
  let error = $state('')
  let busy = $state(false)

  let current = $state('')
  let next = $state('')
  let confirmNext = $state('')

  const dirty = $derived(text !== original)

  async function load() {
    try {
      original = text = (await api.rawConfig()).text
      error = ''
    } catch (e) {
      error = errorText(e)
    }
  }

  onMount(load)

  async function save() {
    busy = true
    try {
      await api.updateRawConfig(text)
      original = text
      error = ''
      markRestartPending()
      notify('ok', 'kyber_config.toml enregistré.')
    } catch (e) {
      error = errorText(e)
    } finally {
      busy = false
    }
  }

  async function changePassword(event: SubmitEvent) {
    event.preventDefault()
    if (next !== confirmNext) {
      notify('error', 'Les deux nouveaux mots de passe ne correspondent pas.')
      return
    }
    try {
      await api.changePassword(current, next)
      current = next = confirmNext = ''
      notify('ok', 'Mot de passe administrateur modifié. Les autres sessions ont été fermées.')
    } catch (e) {
      notify('error', errorText(e))
    }
  }
</script>

<header>
  <h1>Avancé</h1>
  <p class="muted">Édition directe du fichier de configuration et compte administrateur.</p>
</header>

<section class="card stack">
  <div class="row">
    <div>
      <h2>kyber_config.toml</h2>
      <p class="muted">La syntaxe est vérifiée avant l'enregistrement, pas le sens des valeurs.</p>
    </div>
    <span class="spacer"></span>
    <button disabled={!dirty || busy} onclick={() => (text = original)}>Annuler</button>
    <button class="primary" disabled={!dirty || busy} onclick={save}>Enregistrer</button>
  </div>
  {#if error}
    <div class="notice danger">{error}</div>
  {/if}
  <textarea bind:value={text} spellcheck="false" aria-label="Contenu de kyber_config.toml"></textarea>
</section>

<section class="card">
  <h2>Mot de passe administrateur</h2>
  <form class="stack pw" onsubmit={changePassword}>
    <label class="field">
      Mot de passe actuel
      <input type="password" autocomplete="current-password" bind:value={current} required />
    </label>
    <label class="field">
      Nouveau mot de passe (10 caractères minimum)
      <input type="password" autocomplete="new-password" minlength="10" bind:value={next} required />
    </label>
    <label class="field">
      Confirmation
      <input type="password" autocomplete="new-password" minlength="10" bind:value={confirmNext} required />
    </label>
    <div><button class="primary">Changer le mot de passe</button></div>
  </form>
</section>

<style>
  textarea {
    width: 100%;
    height: 60vh;
    font-family: var(--mono);
    font-size: 0.84rem;
    line-height: 1.45;
    resize: vertical;
    background: var(--surface-2);
  }

  .pw {
    max-width: 420px;
    margin-top: 12px;
  }
</style>
