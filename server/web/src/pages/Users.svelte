<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type User } from '../lib/api'
  import { markRestartPending, notify, errorText } from '../lib/store.svelte'

  let users = $state<User[]>([])
  let username = $state('')
  let password = $state('')
  let editing = $state<string | null>(null)
  let newPassword = $state('')
  let busy = $state(false)

  onMount(async () => {
    try {
      users = (await api.users()).users
    } catch (e) {
      notify('error', errorText(e))
    }
  })

  async function run(fn: () => Promise<{ users: User[] }>, message: string) {
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
    if (await run(() => api.upsertUser(username, password), `Utilisateur « ${username} » ajouté.`)) {
      username = ''
      password = ''
    }
  }

  async function savePassword(event: SubmitEvent, user: string) {
    event.preventDefault()
    if (await run(() => api.upsertUser(user, newPassword), `Mot de passe de « ${user} » modifié.`)) {
      editing = null
      newPassword = ''
    }
  }

  async function remove(user: string) {
    if (!confirm(`Supprimer l'utilisateur « ${user} » ? Il ne pourra plus se connecter.`)) return
    await run(() => api.deleteUser(user), `Utilisateur « ${user} » supprimé.`)
  }
</script>

<header>
  <h1>Utilisateurs</h1>
  <p class="muted">
    Comptes acceptés par Kyber pour la connexion par identifiant et mot de passe (clients web, macOS et
    ligne de commande).
  </p>
</header>

{#if users.some((u) => u.insecure_default)}
  <div class="notice warn">
    Le compte d'exemple « Default user » a encore son mot de passe d'origine, connu de tous. Supprimez-le ou
    changez son mot de passe.
  </div>
{/if}

<section class="card">
  <h2>Comptes</h2>
  <ul class="users">
    {#each users as user (user.username)}
      <li>
        <div class="row">
          <strong>{user.username}</strong>
          {#if user.insecure_default}<span class="badge warn">mot de passe par défaut</span>{/if}
          <span class="spacer"></span>
          <button disabled={busy} onclick={() => ((editing = editing === user.username ? null : user.username), (newPassword = ''))}>
            Changer le mot de passe
          </button>
          <button class="danger" disabled={busy || users.length <= 1} onclick={() => remove(user.username)}>
            Supprimer
          </button>
        </div>
        {#if editing === user.username}
          <form class="row edit" onsubmit={(e) => savePassword(e, user.username)}>
            <input
              type="password"
              placeholder="Nouveau mot de passe (8 caractères minimum)"
              autocomplete="new-password"
              minlength="8"
              bind:value={newPassword}
              required
            />
            <button class="primary" disabled={busy}>Enregistrer</button>
          </form>
        {/if}
      </li>
    {/each}
  </ul>
</section>

<section class="card">
  <h2>Ajouter un utilisateur</h2>
  <form class="row add" onsubmit={add}>
    <label class="field">
      Identifiant
      <input bind:value={username} autocomplete="off" required />
    </label>
    <label class="field">
      Mot de passe
      <input type="password" bind:value={password} autocomplete="new-password" minlength="8" required />
    </label>
    <button class="primary" disabled={busy}>Ajouter</button>
  </form>
  <p class="muted note">Kyber stocke les mots de passe sous forme d'empreinte SHA-256.</p>
</section>

<style>
  .users {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
  }

  .users li {
    padding: 12px 0;
    border-top: 1px solid var(--border);
  }

  .edit {
    margin-top: 10px;
  }

  .edit input {
    flex: 1;
    min-width: 220px;
  }

  .add {
    align-items: flex-end;
    margin-top: 12px;
  }

  .add .field {
    flex: 1;
    min-width: 180px;
  }

  .note {
    font-size: 0.82rem;
    margin-top: 10px;
  }
</style>
