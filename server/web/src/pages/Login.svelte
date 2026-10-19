<script lang="ts">
  import { api } from '../lib/api'
  import { errorText } from '../lib/store.svelte'
  import Mark from '../lib/Mark.svelte'
  import SignalBackground from '../lib/SignalBackground.svelte'

  let { configured, setupCodeFile, onDone }: { configured: boolean; setupCodeFile: string; onDone: () => void } =
    $props()

  let code = $state('')
  let password = $state('')
  let confirm = $state('')
  let error = $state('')
  let busy = $state(false)

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    error = ''
    if (!configured && password !== confirm) {
      error = 'Les deux mots de passe ne correspondent pas.'
      return
    }
    busy = true
    try {
      if (configured) await api.login(password)
      else await api.setup(code, password)
      onDone()
    } catch (e) {
      error = errorText(e)
    } finally {
      busy = false
    }
  }
</script>

<div class="wrap">
  <SignalBackground />
  <form class="card stack kn-enter" onsubmit={submit}>
    <div class="brand">
      <Mark size={64} animated id="kn-login" />
      <div>
        <h1>Kyber<span>Now</span></h1>
        <span class="eyebrow">Console d'administration</span>
      </div>
    </div>

    {#if configured}
      <p class="muted">Connectez-vous pour gérer le serveur de streaming de ce PC.</p>
      <label class="field">
        Mot de passe (celui choisi à l'installation)
        <!-- svelte-ignore a11y_autofocus -->
        <input type="password" autocomplete="current-password" bind:value={password} autofocus required />
      </label>
    {:else}
      <p class="muted">
        Première connexion : choisissez le mot de passe administrateur. Le code d'initialisation se trouve sur
        le PC, dans <code>{setupCodeFile}</code>.
      </p>
      <label class="field">
        Code d'initialisation
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={code} autocomplete="off" spellcheck="false" autofocus required />
      </label>
      <label class="field">
        Nouveau mot de passe (10 caractères minimum)
        <input type="password" autocomplete="new-password" bind:value={password} minlength="10" required />
      </label>
      <label class="field">
        Confirmation
        <input type="password" autocomplete="new-password" bind:value={confirm} minlength="10" required />
      </label>
    {/if}

    {#if error}
      <div class="notice danger">{error}</div>
    {/if}

    <button class="primary" type="submit" disabled={busy}>
      {configured ? 'Se connecter' : 'Définir le mot de passe'}
    </button>
  </form>
</div>

<style>
  .wrap {
    position: relative;
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 16px;
    overflow: hidden;
  }

  form {
    position: relative;
    width: min(440px, 100%);
    padding: 28px;
    gap: 18px;
    background: rgba(16, 21, 28, 0.94);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .brand h1 {
    margin: 0;
  }

  .brand h1 span {
    color: var(--accent);
  }

  form :global(button.primary) {
    min-height: 44px;
  }

  code {
    word-break: break-all;
  }
</style>
