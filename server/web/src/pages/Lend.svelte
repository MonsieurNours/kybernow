<!--
  Prêt (KyberNow Share, docs/pret.md) : prêter l'ordinateur à un ami quand on
  ne s'en sert pas. L'interrupteur et son avertissement, le prêt en cours (les
  demandes, « Reprendre l'ordinateur », le code pour une séance), les apps
  permises (Steam, puis celles du catalogue), et les comptes d'amis avec leurs
  créneaux et leur lien d'invitation. Textes de docs/link-ui.md.
-->
<script lang="ts">
  import { adminLend, api, type Launch, type LendFriend, type LendFriendInput, type LendSettings, type LendView } from '../lib/api'
  import { app, errorText, notify, refreshStatus } from '../lib/store.svelte'
  import { ago } from '../lib/format'
  import { dayName, slotHour, steamName } from '../lib/lend'
  import FriendForm from '../lib/FriendForm.svelte'
  import Icon from '../lib/Icon.svelte'
  import LaunchPicker from '../lib/LaunchPicker.svelte'
  import LendPanel from '../lib/LendPanel.svelte'
  import Qr from '../lib/Qr.svelte'
  import Switch from '../lib/Switch.svelte'
  import { t } from '../lib/i18n/index.svelte'

  let settings = $state<LendSettings | null>(null)
  let friends = $state<LendFriend[] | null>(null)
  let saving = $state(false)
  let picking = $state(false)
  /** Formulaire ouvert : `new`, ou l'identifiant du compte modifié. */
  let editing = $state<string | null>(null)
  /** Lien d'invitation montré (QR code). */
  let invite = $state<{ friend: LendFriend; link: string; web: string } | null>(null)
  /** La vue du prêt, quand `/api/status` ne la porte pas (relue ici). */
  let own = $state<LendView | null>(null)

  const view = $derived(app.status?.lend ?? own)
  const computer = $derived(app.status?.hostname ?? t('overview.thisComputer'))

  async function load() {
    try {
      ;[settings, friends] = await Promise.all([api.lendSettings(), api.lendFriends()])
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  async function loadView() {
    if (app.status?.lend !== undefined) return
    try {
      own = await adminLend.view()
    } catch {
      // 401 gérée globalement ; le prochain relevé le dira.
    }
  }

  $effect(() => {
    load()
    loadView()
    const timer = setInterval(loadView, 3000)
    return () => clearInterval(timer)
  })

  async function save(changes: Partial<Omit<LendSettings, 'steam_found'>>, done?: string) {
    saving = true
    try {
      settings = await api.setLendSettings(changes)
      if (done) notify('ok', done)
      refreshStatus()
      loadView()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      saving = false
    }
  }

  function addApp(launch: Launch) {
    if (!settings) return
    if (settings.apps.some((a) => a.kind === launch.kind && a.target === launch.target)) return
    save({ apps: [...settings.apps, launch] }, t('lend.apps.added', { app: launch.name }))
  }

  function removeApp(launch: Launch) {
    if (!settings) return
    save({ apps: settings.apps.filter((a) => !(a.kind === launch.kind && a.target === launch.target)) }, t('lend.apps.removed', { app: launch.name }))
  }

  async function saveFriend(friend: LendFriend | null, input: LendFriendInput) {
    try {
      if (friend) await api.updateLendFriend(friend.id, input)
      else await api.addLendFriend(input)
      friends = await api.lendFriends()
      editing = null
      notify('ok', t('lend.friends.saved', { name: input.name }))
    } catch (e) {
      notify('error', errorText(e))
      throw e
    }
  }

  async function removeFriend(f: LendFriend) {
    if (!confirm(t('lend.friends.confirmDelete', { name: f.name }))) return
    try {
      await api.deleteLendFriend(f.id)
      friends = (friends ?? []).filter((x) => x.id !== f.id)
      if (editing === f.id) editing = null
      notify('ok', t('lend.friends.deleted', { name: f.name }))
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  async function showInvite(f: LendFriend) {
    try {
      invite = { friend: f, ...(await api.lendInvite(f.id)) }
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  async function copyInvite() {
    if (!invite) return
    try {
      await navigator.clipboard.writeText(invite.web)
      notify('ok', t('share.copied'))
    } catch {
      notify('error', t('share.copyFailed'))
    }
  }

  /** « mer., sam. · 14 h – 18 h » pour chaque créneau. */
  function slotsLine(f: LendFriend): string {
    if (!f.slots.length) return t('lend.friends.noSlots')
    return f.slots
      .map((s) => t('lend.friends.slotLine', { days: s.days.map((d) => dayName(d, 'short')).join(', '), start: slotHour(s.start), end: slotHour(s.end) }))
      .join(' ; ')
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && invite) invite = null
  }
</script>

<svelte:window {onkeydown} />

<header class="space-head">
  <div>
    <span class="lbl">KyberNow Share</span>
    <h1>{t('lend.page.title')}</h1>
    <p>{t('lend.page.intro')}</p>
  </div>
</header>

<div class="space-body">
  <!-- Prêter l'ordinateur : l'interrupteur et l'avertissement -->
  <section aria-labelledby="t-lend-switch" class="card">
    <div class="set first">
      <div>
        <h2 id="t-lend-switch">{t('lend.page.switch')}</h2>
        <p>{t('lend.page.switchHelp')}</p>
      </div>
      <Switch label={t('lend.page.switch')} checked={!!settings?.enabled} disabled={!settings || saving} onchange={(v) => save({ enabled: v })} />
    </div>
    <div class="notice warn trust" role="note"><Icon name="warning" size={16} /><span>{t('lend.warning')}</span></div>
  </section>

  <!-- Prêt en cours : demandes, prêt, code pour une séance -->
  {#if view}
    <section id="en-cours" aria-labelledby="t-lend-now" class="card current" class:live={view.state !== 'free' || view.requests.length > 0}>
      <div class="card-head">
        <h2 id="t-lend-now">{t('lend.page.current')}</h2>
      </div>
      <LendPanel {view} {computer} />
    </section>
  {/if}

  <!-- Apps permises -->
  <section id="apps" aria-labelledby="t-lend-apps" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-lend-apps">{t('lend.apps.title')}</h2>
        <span class="sub">{t('lend.apps.sub')}</span>
      </div>
    </div>
    {#if settings}
      <div class="set">
        <div>
          <h3>{t('lend.apps.steam')}</h3>
          <p>{settings.steam_found === false ? t('lend.apps.steamMissing') : t('lend.apps.steamHelp')}</p>
        </div>
        <Switch label={t('lend.apps.steam')} checked={settings.steam} disabled={saving} onchange={(v) => save({ steam: v })} />
      </div>
      <div class="set apps">
        <div class="chips">
          {#each settings.apps as a (`${a.kind} ${a.target}`)}
            <span class="appchip">
              {#if a.icon}<img src={api.appIconUrl(a.icon)} alt="" width="26" height="26" />{:else}<Icon name="app" size={18} />{/if}
              <span class="app-name">{a.name}</span>
              <button type="button" class="icon-button" aria-label={t('lend.apps.remove', { app: a.name })} title={t('lend.apps.remove', { app: a.name })} disabled={saving} onclick={() => removeApp(a)}><Icon name="x" size={15} /></button>
            </span>
          {:else}
            <p class="empty">{t('lend.apps.none')}</p>
          {/each}
        </div>
        <button type="button" class="ghost" disabled={saving} onclick={() => (picking = true)}><Icon name="plus" size={15} stroke={2} />{t('lend.apps.add')}</button>
      </div>
    {:else}
      <p class="empty">{t('web.gate.wait')}</p>
    {/if}
  </section>

  <!-- Comptes d'amis -->
  <section id="amis" aria-labelledby="t-lend-friends" class="card friends">
    <div class="card-head">
      <div>
        <h2 id="t-lend-friends">{t('lend.friends.title')}</h2>
        <span class="sub">{t('lend.friends.sub')}</span>
      </div>
      {#if editing !== 'new'}
        <button type="button" class="ghost" onclick={() => (editing = 'new')}><Icon name="plus" size={15} stroke={2} />{t('lend.friends.add')}</button>
      {/if}
    </div>

    {#if editing === 'new'}
      <FriendForm onsave={(input) => saveFriend(null, input)} oncancel={() => (editing = null)} />
    {/if}

    {#each friends ?? [] as f (f.id)}
      {#if editing === f.id}
        <FriendForm friend={f} onsave={(input) => saveFriend(f, input)} oncancel={() => (editing = null)} />
      {:else}
        <article class="friend" aria-label={f.name}>
          <span class="device-icon"><Icon name="user" size={20} stroke={1.6} /></span>
          <div class="who">
            <span class="name">{f.name}</span>
            <span class="meta"><span class="mono">{f.login}</span> · {steamName(f.steam)} · {f.last_used ? t('lend.friends.lastUsed', { ago: ago(f.last_used) }) : t('lend.friends.never')}</span>
            <span class="meta">{slotsLine(f)}</span>
          </div>
          <div class="moves">
            <button type="button" class="ghost small" onclick={() => showInvite(f)}><Icon name="share" size={14} />{t('lend.friends.invite')}</button>
            <button type="button" class="ghost small" onclick={() => (editing = f.id)}>{t('lend.friends.edit')}</button>
            <button type="button" class="ghost small remove" onclick={() => removeFriend(f)}>{t('lend.friends.delete')}</button>
          </div>
        </article>
      {/if}
    {:else}
      {#if friends && editing !== 'new'}<p class="empty">{t('lend.friends.empty')}</p>{/if}
    {/each}
  </section>
</div>

{#if picking}
  <LaunchPicker pick={{ onpick: addApp }} onclose={() => (picking = false)} />
{/if}

{#if invite}
  <div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && (invite = null)}>
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="invite-title">
      <header>
        <div>
          <span class="lbl">{invite.friend.name}</span>
          <h2 id="invite-title" class="disp">{t('lend.friends.invite')}</h2>
        </div>
        <button type="button" class="icon-button" aria-label={t('lend.close')} onclick={() => (invite = null)}><Icon name="x" size={16} /></button>
      </header>
      <div class="invite">
        <Qr text={invite.link} size={168} label={t('lend.friends.inviteQr', { name: invite.friend.name })} />
        <div class="invite-text">
          <p>{t('lend.friends.inviteHelp', { login: invite.friend.login })}</p>
          <div class="link-row">
            <button type="button" class="primary" onclick={copyInvite}><Icon name="copy" size={15} stroke={2} />{t('share.copy')}</button>
          </div>
          <span class="mono link">{invite.web}</span>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .set.first {
    border-top: 0;
    padding-top: 0;
  }

  .set h2 {
    font-size: 23px;
  }

  .trust {
    margin: 0 0 16px;
    flex-wrap: nowrap;
    align-items: flex-start;
  }

  .trust > :global(svg) {
    margin-top: 2px;
  }

  .current.live {
    background: linear-gradient(135deg, rgb(109 91 255 / 0.1), rgb(109 91 255 / 0) 55%), var(--card);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .current {
    padding-bottom: 22px;
  }

  .apps {
    align-items: start;
  }

  /* La liste des apps, en pastilles (la première colonne d'un réglage est une colonne par défaut). */
  .set.apps > .chips {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  .appchip {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 4px 0 9px;
    border-radius: 12px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    font-size: 14px;
    font-weight: 600;
    max-width: 100%;
  }

  .appchip img {
    border-radius: 7px;
  }

  .app-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    font-size: 13.5px;
    color: var(--faint);
    padding: 4px 0 14px;
  }

  .friends {
    padding-bottom: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .friends .card-head {
    margin-bottom: 4px;
  }

  .friend {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    gap: 10px 16px;
    align-items: center;
    padding: 14px 16px;
    border-radius: 14px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--line);
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

  .who {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .name {
    font-size: 15.5px;
    font-weight: 650;
    overflow-wrap: anywhere;
  }

  .meta {
    font-size: 12.5px;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  .meta .mono {
    font-size: 11.5px;
    color: var(--text-2);
  }

  .moves {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }

  .moves .remove:hover:not(:disabled) {
    color: var(--bad-text);
  }

  /* Lien d'invitation */

  .overlay {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(5 7 10 / 0.6);
    backdrop-filter: blur(10px);
  }

  .dialog {
    width: min(620px, 100%);
    max-height: calc(100vh - 48px);
    overflow: auto;
    border-radius: var(--r-card);
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2), 0 30px 80px rgb(0 0 0 / 0.6);
    animation: rise 0.35s var(--ease-out) both;
  }

  .dialog header {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    padding: 24px 24px 8px;
  }

  .dialog header > div {
    flex: 1;
    min-width: 0;
  }

  .dialog h2 {
    margin-top: 6px;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  .invite {
    display: flex;
    flex-wrap: wrap;
    gap: 24px;
    align-items: center;
    padding: 12px 24px 26px;
  }

  .invite-text {
    flex: 1 1 240px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .invite-text p {
    font-size: 14px;
    line-height: 1.55;
    color: var(--text-2);
  }

  .link {
    font-size: 11px;
    color: var(--faint);
    overflow-wrap: anywhere;
    user-select: all;
  }

  @media (max-width: 760px) {
    .friend {
      grid-template-columns: 40px minmax(0, 1fr);
    }

    .moves {
      grid-column: 1 / -1;
      justify-content: flex-start;
    }

    .overlay {
      padding: 0;
    }

    .dialog {
      max-height: 100vh;
      border-radius: 0;
      height: 100%;
    }
  }
</style>
