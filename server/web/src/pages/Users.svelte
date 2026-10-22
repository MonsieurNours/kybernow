<!--
  Utilisateurs : les comptes qu'on saisit dans KyberNow Link (une personne par
  identifiant, plusieurs appareils peuvent l'utiliser), l'accès propre des
  appareils acceptés par code (en lecture seule : il se gère depuis leur
  carte), et l'ajout d'une personne.
-->
<script lang="ts">
  import { api, type Device, type User } from '../lib/api'
  import { errorText, markRestartPending, notify } from '../lib/store.svelte'
  import { deviceIcon } from '../lib/format'
  import Icon from '../lib/Icon.svelte'
  import { t, tn } from '../lib/i18n/index.svelte'

  let users = $state<User[]>([])
  let devices = $state<Device[]>([])
  let editing = $state<string | null>(null)
  let newPassword = $state('')
  let username = $state('')
  let password = $state('')
  let busy = $state(false)

  $effect(() => {
    api.users().then((r) => (users = r.users)).catch((e) => notify('error', errorText(e)))
    api.devices().then((r) => (devices = r.devices)).catch(() => {})
  })

  const people = $derived(users.filter((u) => !u.device))
  const deviceAccounts = $derived(users.filter((u) => u.device))
  const insecure = $derived(users.find((u) => u.insecure_default) ?? null)

  /** Les appareils qui se connectent avec ce compte. */
  function usedBy(u: User): string {
    const names = devices.filter((d) => d.user === u.username && !d.own_account).map((d) => d.name)
    if (!names.length) return t('users.accounts.unused')
    const list = names.length === 1 ? names[0] : names.slice(0, -1).join(', ') + t('users.accounts.and') + names[names.length - 1]
    return t('users.accounts.usedBy', { devices: list })
  }

  function kindOf(u: User): string {
    return deviceIcon(devices.find((d) => d.name === u.device)?.kind ?? 'other')
  }

  function edit(user: string) {
    editing = editing === user ? null : user
    newPassword = ''
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
      notify('error', t('users.accountExists', { user: username }))
      return
    }
    if (await runUsers(() => api.upsertUser(username, password), t('users.accountAdded', { user: username }))) {
      username = ''
      password = ''
    }
  }

  async function savePassword(event: SubmitEvent, user: string) {
    event.preventDefault()
    if (await runUsers(() => api.upsertUser(user, newPassword), t('users.passwordChanged', { user }))) {
      editing = null
      newPassword = ''
    }
  }

  async function remove(user: string) {
    const device = users.find((u) => u.username === user)?.device
    const consequence = device ? t('users.removeDeviceConsequence', { device }) : t('users.removeConsequence')
    if (!confirm(t('users.confirmRemove', { user, consequence }))) return
    await runUsers(() => api.deleteUser(user), t('users.accountRemoved', { user }))
  }
</script>

<header class="space-head">
  <div>
    <span class="lbl">{t('users.eyebrow')}</span>
    <h1>{t('users.title')}</h1>
    <p>{t('users.intro')} <a href="#/appareils">{t('users.introLink')}</a>.</p>
  </div>
</header>

<div class="space-body">
  <!-- Les comptes -->
  <section id="comptes" aria-labelledby="t-acc" class="card accounts">
    <div class="card-head">
      <div>
        <h2 id="t-acc">{t('users.accounts.title')}</h2>
        <span class="sub">{t('users.accounts.sub')}</span>
      </div>
      {#if users.length}
        <span class="mono count">{tn('users.accounts.people', people.length)}{deviceAccounts.length ? ` · ${tn('users.accounts.deviceAccounts', deviceAccounts.length)}` : ''}</span>
      {/if}
    </div>

    {#if insecure}
      <div class="notice warn" role="alert">
        <Icon name="warning" />
        <span><strong>{t('users.accounts.insecure', { user: insecure.username })}</strong>. {t('users.accounts.insecureHint')}</span>
        <span class="notice-actions">
          <button type="button" class="ghost small" disabled={busy} onclick={() => edit(insecure.username)}>{t('users.accounts.changePassword')}</button>
          <button type="button" class="ghost small danger" disabled={busy || users.length <= 1} onclick={() => remove(insecure.username)}>{t('users.accounts.delete')}</button>
        </span>
      </div>
    {/if}

    {#each users as u (u.username)}
      <div class="user">
        {#if u.device}
          <span class="avatar device"><Icon name={kindOf(u)} size={22} stroke={1.6} /></span>
        {:else}
          <span class="avatar" class:warn={u.insecure_default}>{u.username.charAt(0).toUpperCase()}</span>
        {/if}
        <div class="who">
          <span class="name">
            <span class:mono={!!u.device}>{u.username}</span>
            {#if u.device}<span class="pill">{t('users.accounts.devicePill')}</span>{/if}
          </span>
          <span class="sub" class:warn={u.insecure_default}>
            {u.device ? t('users.accounts.deviceText', { device: u.device }) : u.insecure_default ? t('users.accounts.originalPassword') : usedBy(u)}
          </span>
        </div>
        {#if u.device}
          <a class="ghost small" href="#/appareils">{t('users.accounts.seeDevice')}<Icon name="chevron" size={14} stroke={2} /></a>
        {:else}
          <div class="actions">
            <button type="button" class="ghost small" aria-expanded={editing === u.username} onclick={() => edit(u.username)}>{t('users.accounts.changePassword')}</button>
            {#if users.length > 1}
              <button type="button" class="icon-button" aria-label={t('users.accounts.deleteAria', { user: u.username })} disabled={busy} onclick={() => remove(u.username)}><Icon name="trash" size={16} /></button>
            {/if}
          </div>
        {/if}
      </div>
      {#if editing === u.username}
        <form class="inline" onsubmit={(e) => savePassword(e, u.username)}>
          <div class="field"><label for="np-{u.username}">{t('users.accounts.newPasswordFor', { user: u.username })}</label><input id="np-{u.username}" type="password" minlength="8" required autocomplete="new-password" bind:value={newPassword} placeholder={t('users.accounts.min8')} /></div>
          <button type="submit" class="primary" disabled={busy}>{t('users.accounts.save')}</button>
          <button type="button" class="ghost" disabled={busy} onclick={() => (editing = null)}>{t('users.accounts.cancel')}</button>
          <p class="hint">{t('users.accounts.passwordHint')}</p>
        </form>
      {/if}
    {/each}
  </section>

  <!-- Ajouter une personne -->
  <section aria-labelledby="t-new" class="card add">
    <div class="card-head">
      <div>
        <h2 id="t-new">{t('users.add.title')}</h2>
        <span class="sub">{t('users.add.sub')}</span>
      </div>
    </div>
    <form class="new" onsubmit={add}>
      <div class="field"><label for="u">{t('users.add.username')}</label><input id="u" type="text" required autocomplete="off" bind:value={username} placeholder={t('users.add.usernamePlaceholder')} /></div>
      <div class="field"><label for="p">{t('users.add.password')}</label><input id="p" type="password" minlength="8" required autocomplete="new-password" bind:value={password} placeholder={t('users.accounts.min8')} /></div>
      <button type="submit" class="primary" disabled={busy}><Icon name="plus" size={15} stroke={2.2} />{t('users.add.submit')}</button>
    </form>
    <p class="hash">{t('users.add.hash')}</p>
  </section>
</div>

<style>
  .space-head p a {
    color: var(--signal-light);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .card-head {
    align-items: baseline;
  }

  .count {
    font-size: 12px;
    color: var(--faint);
  }

  .accounts {
    padding-bottom: 10px;
  }

  .accounts .notice {
    margin: 4px 0 10px;
    flex-wrap: wrap;
  }

  .notice-actions {
    display: flex;
    gap: 8px;
    flex: none;
  }

  .user {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    gap: 16px 18px;
    align-items: center;
    padding: 16px 0;
    border-top: 1px solid var(--line);
  }

  .avatar {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
    font-size: 15px;
    font-weight: 650;
    color: var(--signal-light);
  }

  .avatar.warn {
    color: var(--warn-text);
  }

  .avatar.device {
    border-radius: 12px;
    color: var(--text-2);
  }

  .who {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    font-size: 15.5px;
    font-weight: 650;
    overflow-wrap: anywhere;
  }

  .name .mono {
    font-weight: 500;
    font-size: 14.5px;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    height: 24px;
    padding: 0 9px;
    border-radius: 999px;
    font-size: 11.5px;
    font-weight: 600;
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--signal-light);
    white-space: nowrap;
  }

  .who .sub {
    font-size: 13px;
    line-height: 1.5;
    color: var(--faint);
  }

  .who .sub.warn {
    color: var(--warn-text);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  a.ghost {
    text-decoration: none;
  }

  .inline {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 16px;
    align-items: end;
    margin: -6px 0 14px 62px;
    padding: 16px 18px;
    border-radius: 12px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .inline .field {
    flex: 1 1 260px;
  }

  .inline .primary,
  .inline .ghost {
    height: 40px;
  }

  .hint {
    flex-basis: 100%;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--faint);
  }

  .add {
    padding-bottom: 22px;
  }

  .new {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(200px, 100%), 1fr));
    gap: 12px 16px;
    align-items: end;
  }

  .new .primary {
    height: 40px;
    justify-content: center;
  }

  .hash {
    margin-top: 14px;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--faint);
  }

  @media (max-width: 760px) {
    .user {
      grid-template-columns: 44px minmax(0, 1fr);
    }

    .user .actions,
    .user a.ghost {
      grid-column: 2;
      justify-self: start;
    }

    .inline {
      margin-left: 0;
    }
  }
</style>
