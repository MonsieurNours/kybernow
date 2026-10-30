<!--
  Prêt (KyberNow Share, docs/pret.md, textes de docs/link-ui.md) : prêter
  l'ordinateur à un ami quand on ne s'en sert pas. Prêt coupé : une présentation
  et « Activer le prêt ». Sinon, la carte d'état (ce qui se passe, l'action
  principale), puis trois onglets : « Prêter » (la semaine des créneaux, les
  derniers prêts), « Amis » (une carte par compte, le panneau d'un ami) et
  « Réglages » (apps permises, votre prénom, Steam, couper le prêt).
-->
<script lang="ts">
  import { adminLend, api, type Launch, type LendFriend, type LendFriendInput, type LendPast, type LendSettings, type LendView } from '../lib/api'
  import { app, errorText, notify, refreshStatus } from '../lib/store.svelte'
  import { ago, sinceText } from '../lib/format'
  import { avatarColor, dayDate, dayName, hourText, initial, lendAppName, slotHour, steamName } from '../lib/lend'
  import FriendDrawer from '../lib/FriendDrawer.svelte'
  import Icon from '../lib/Icon.svelte'
  import LaunchPicker from '../lib/LaunchPicker.svelte'
  import LendCodeDialog from '../lib/LendCodeDialog.svelte'
  import LendState from '../lib/LendState.svelte'
  import LendWeek from '../lib/LendWeek.svelte'
  import Switch from '../lib/Switch.svelte'
  import { t } from '../lib/i18n/index.svelte'

  type Tab = 'lend' | 'friends' | 'settings'

  let settings = $state<LendSettings | null>(null)
  let friends = $state<LendFriend[]>([])
  let history = $state<LendPast[]>([])
  let saving = $state(false)
  let picking = $state(false)
  let lending = $state(false)
  /** Le panneau d'un ami : `new`, le compte ouvert, ou rien. */
  let drawer = $state<LendFriend | 'new' | null>(null)
  let tab = $state<Tab>('lend')
  let hostName = $state('')
  /** La vue du prêt, quand `/api/status` ne la porte pas (relue ici). */
  let own = $state<LendView | null>(null)

  const view = $derived(app.status?.lend ?? own)

  async function load() {
    try {
      const [s, f, h] = await Promise.all([api.lendSettings(), api.lendFriends(), api.lendHistory()])
      settings = s
      friends = f
      history = h
      hostName = s.host_name ?? ''
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

  // Un prêt qui finit entre dans les derniers prêts.
  let wasLent = false
  $effect(() => {
    const lent = !!view?.borrower
    if (wasLent && !lent) api.lendHistory().then((h) => (history = h), () => {})
    wasLent = lent
  })

  async function save(changes: Partial<Omit<LendSettings, 'steam_found' | 'host_default'>>, done?: string) {
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
    if (!settings || settings.apps.some((a) => a.kind === launch.kind && a.target === launch.target)) return
    save({ apps: [...settings.apps, launch] }, t('lend.apps.added', { app: launch.name }))
  }

  function removeApp(launch: Launch) {
    if (!settings) return
    save({ apps: settings.apps.filter((a) => !(a.kind === launch.kind && a.target === launch.target)) }, t('lend.apps.removed', { app: launch.name }))
  }

  function saveHostName() {
    if (!settings || hostName.trim() === (settings.host_name ?? '')) return
    save({ host_name: hostName.trim() }, t('lend.admin.settings.hostNameSaved'))
  }

  async function saveFriend(friend: LendFriend | null, input: LendFriendInput) {
    try {
      if (friend) await api.updateLendFriend(friend.id, input)
      else await api.addLendFriend(input)
      friends = await api.lendFriends()
      drawer = null
      notify('ok', t('lend.friends.saved', { name: input.name }))
    } catch (e) {
      notify('error', errorText(e))
      throw e
    }
  }

  async function removeFriend(f: LendFriend) {
    try {
      await api.deleteLendFriend(f.id)
      friends = friends.filter((x) => x.id !== f.id)
      drawer = null
      notify('ok', t('lend.friends.deleted', { name: f.name }))
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  /** « mer., sam. 14 h – 18 h · … », ou « Sans créneau : … ». */
  function slotsLine(f: LendFriend): string {
    if (!f.slots.length) return t('lend.friends.noSlots')
    return f.slots.map((s) => t('lend.friends.slotLine', { days: s.days.map((d) => dayName(d, 'short')).join(', '), start: slotHour(s.start), end: slotHour(s.end) })).join(' · ')
  }

  /** Le dernier prêt d'un ami : « mar. 28 oct., 2 h 10 », sinon « il y a 3 jours », sinon « jamais emprunté ». */
  function lastLoan(f: LendFriend): string {
    const p = history.find((h) => h.friend === f.id)
    if (p) return `${dayDate(p.since)}, ${sinceText(p.end - p.since)}`
    return f.last_used ? ago(f.last_used) : t('lend.friends.never')
  }

  function when(p: LendPast): string {
    return `${dayDate(p.since)}, ${hourText(p.since)} – ${hourText(p.end)}`
  }
</script>

<header class="space-head">
  <div>
    <span class="lbl">KyberNow Share</span>
    <h1>{t('lend.page.title')}</h1>
    <p>{t('lend.admin.intro')}</p>
  </div>
</header>

<div class="space-body">
  {#if !settings}
    <p class="empty">{t('web.gate.wait')}</p>
  {:else if !settings.enabled}
    <!-- Prêt coupé : la présentation, et un seul bouton. -->
    <section class="intro" aria-labelledby="t-lend-off">
      <h2 id="t-lend-off" class="disp">{t('lend.admin.off.title')}</h2>
      <div class="how">
        <div><Icon name="key" size={26} stroke={1.7} /><strong>{t('lend.admin.off.code')}</strong><span>{t('lend.admin.off.codeText')}</span></div>
        <div><Icon name="user" size={26} stroke={1.7} /><strong>{t('lend.admin.off.friends')}</strong><span>{t('lend.admin.off.friendsText')}</span></div>
        <div><Icon name="clock" size={26} stroke={1.7} /><strong>{t('lend.admin.off.slots')}</strong><span>{t('lend.admin.off.slotsText')}</span></div>
      </div>
      <div class="notice warn trust" role="note"><Icon name="warning" size={16} /><span>{t('lend.admin.off.warning')}</span></div>
      <div><button type="button" class="primary" disabled={saving} onclick={() => save({ enabled: true })}>{t('lend.admin.off.enable')}</button></div>
    </section>
  {:else}
    {#if view}<LendState {view} {friends} onlend={() => (lending = true)} />{/if}

    <div class="ltabs" role="tablist">
      <button type="button" role="tab" aria-selected={tab === 'lend'} onclick={() => (tab = 'lend')}>{t('lend.admin.tabs.lend')}</button>
      <button type="button" role="tab" aria-selected={tab === 'friends'} onclick={() => (tab = 'friends')}>{t('lend.admin.tabs.friends')} <span class="count">{friends.length}</span></button>
      <button type="button" role="tab" aria-selected={tab === 'settings'} onclick={() => (tab = 'settings')}>{t('lend.admin.tabs.settings')}</button>
    </div>

    {#if tab === 'lend'}
      <div class="panel" role="tabpanel">
        <section class="block" aria-labelledby="t-week">
          <div class="block-head">
            <div>
              <h3 id="t-week" class="disp">{t('lend.admin.week.title')}</h3>
              <p>{t('lend.admin.week.sub')}</p>
            </div>
            <button type="button" class="ghost small" onclick={() => (tab = 'friends')}>{t('lend.admin.week.manage')}</button>
          </div>
          <LendWeek {friends} />
        </section>

        <section class="block" aria-labelledby="t-history">
          <div class="block-head"><h3 id="t-history" class="disp">{t('lend.admin.history.title')}</h3></div>
          <div class="surface scroll">
            {#if history.length}
              <table class="hist">
                <tbody>
                  {#each history as p, i (i)}
                    <tr>
                      <td><span class="who"><span class="avatar small" style="--av: {avatarColor(p.friend ?? p.name)}">{initial(p.name)}</span>{p.name}</span></td>
                      <td><span class="tag">{t(`lend.admin.kind.${p.kind}`)}</span></td>
                      <td class="dim">{lendAppName(p.app)}</td>
                      <td class="dim">{when(p)}</td>
                      <td class="dim">{t(`lend.admin.history.end.${p.reason}`, { name: p.name })}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {:else}
              <p class="empty pad">{t('lend.admin.history.empty')}</p>
            {/if}
          </div>
        </section>
      </div>
    {:else if tab === 'friends'}
      <div class="panel" role="tabpanel">
        <section class="block" aria-labelledby="t-friends">
          <div class="block-head">
            <div>
              <h3 id="t-friends" class="disp">{t('lend.friends.title')}</h3>
              <p>{t('lend.friends.sub')}</p>
            </div>
          </div>
          <div class="friends">
            {#each friends as f (f.id)}
              <button type="button" class="friend" onclick={() => (drawer = f)}>
                <span class="top">
                  <span class="avatar" style="--av: {avatarColor(f.id)}">{initial(f.name)}</span>
                  <span><strong>{f.name}</strong><span class="mono">{f.login}</span></span>
                </span>
                <dl>
                  <dt>{t('lend.friends.slots')}</dt><dd>{slotsLine(f)}</dd>
                  <dt>Steam</dt><dd>{steamName(f.steam)}</dd>
                  <dt>{t('lend.admin.friends.lastLoan')}</dt><dd>{lastLoan(f)}</dd>
                </dl>
              </button>
            {/each}
            <button type="button" class="friend add" onclick={() => (drawer = 'new')}><Icon name="plus" size={18} stroke={2} />{t('lend.friends.add')}</button>
          </div>
        </section>
      </div>
    {:else}
      <div class="panel" role="tabpanel">
        <section class="block" aria-labelledby="t-apps">
          <div class="block-head">
            <div>
              <h3 id="t-apps" class="disp">{t('lend.apps.title')}</h3>
              <p>{t('lend.apps.sub')}</p>
            </div>
          </div>
          <div class="apps">
            {#if settings.steam}
              <div class="app-t"><span class="ico steam">S</span><span class="app-name">Steam</span></div>
            {/if}
            {#each settings.apps as a (`${a.kind} ${a.target}`)}
              <div class="app-t">
                {#if a.icon}<img class="ico" src={api.appIconUrl(a.icon)} alt="" width="46" height="46" />{:else}<span class="ico" style="background: {avatarColor(a.target)}">{initial(a.name)}</span>{/if}
                <span class="app-name">{a.name}</span>
                <button type="button" class="rm" aria-label={t('lend.apps.remove', { app: a.name })} title={t('lend.apps.remove', { app: a.name })} disabled={saving} onclick={() => removeApp(a)}><Icon name="x" size={14} /></button>
              </div>
            {/each}
            <button type="button" class="app-t add" disabled={saving} onclick={() => (picking = true)}><Icon name="plus" size={22} stroke={2} />{t('lend.apps.add')}</button>
          </div>
        </section>

        <section class="block" aria-labelledby="t-settings">
          <div class="block-head"><h3 id="t-settings" class="disp">{t('lend.admin.settings.title')}</h3></div>
          <div class="surface">
            <div class="setrow">
              <div class="t">
                <label for="host-name"><strong>{t('lend.admin.settings.hostName')}</strong></label>
                <span>{t('lend.admin.settings.hostNameHelp', { name: hostName.trim() || settings.host_default || '' })}</span>
              </div>
              <input id="host-name" maxlength="40" autocomplete="off" placeholder={settings.host_default ?? ''} bind:value={hostName} onchange={saveHostName} onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
            </div>
            <div class="setrow">
              <div class="t">
                <strong>{t('lend.admin.settings.steam')}</strong>
                <span>{settings.steam_found === false ? t('lend.apps.steamMissing') : t('lend.admin.settings.steamHelp')}</span>
              </div>
              <Switch label={t('lend.admin.settings.steam')} checked={settings.steam} disabled={saving} onchange={(v) => save({ steam: v })} />
            </div>
            <div class="setrow">
              <div class="t">
                <strong>{t('lend.admin.settings.session')}</strong>
                <span>{t('lend.admin.settings.sessionHelp')}</span>
              </div>
            </div>
            <div class="setrow">
              <div class="t">
                <strong>{t('lend.admin.settings.off')}</strong>
                <span>{t('lend.admin.settings.offHelp')}</span>
              </div>
              <button type="button" class="danger" disabled={saving} onclick={() => save({ enabled: false })}>{t('lend.admin.settings.off')}</button>
            </div>
          </div>
        </section>

        <div class="notice warn trust" role="note"><Icon name="warning" size={16} /><span>{t('lend.warning')}</span></div>
      </div>
    {/if}
  {/if}
</div>

{#if picking}
  <LaunchPicker pick={{ onpick: addApp }} onclose={() => (picking = false)} />
{/if}

{#if lending}
  <LendCodeDialog {view} onchange={(v) => (own = v)} onclose={() => (lending = false)} />
{/if}

{#if drawer}
  {@const f = drawer === 'new' ? null : drawer}
  <FriendDrawer friend={f} onsave={(input) => saveFriend(f, input)} ondelete={removeFriend} onclose={() => (drawer = null)} />
{/if}

<style>
  .empty {
    font-size: 13.5px;
    color: var(--faint);
  }

  .empty.pad {
    padding: 18px 16px;
  }

  .trust {
    flex-wrap: nowrap;
    align-items: flex-start;
  }

  .trust > :global(svg) {
    margin-top: 2px;
    flex: none;
  }

  /* Prêt coupé */
  .intro {
    border-radius: 20px;
    padding: 36px;
    background: radial-gradient(70% 100% at 80% 0%, rgb(109 91 255 / 0.22), transparent 60%), var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2);
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .intro h2 {
    font-size: 34px;
    font-weight: 750;
    line-height: 1.05;
    letter-spacing: -0.03em;
    max-width: 20ch;
    text-wrap: balance;
  }

  .how {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }

  .how > div {
    padding: 18px;
    border-radius: 14px;
    background: rgb(255 255 255 / 0.03);
    box-shadow: inset 0 0 0 1px var(--line);
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--signal-light);
  }

  .how strong {
    font-size: 15px;
    color: var(--text);
  }

  .how span {
    color: var(--faint);
    font-size: 13px;
    line-height: 1.5;
  }

  /* Onglets */
  .ltabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--line);
    margin-top: -4px;
  }

  .ltabs button {
    position: relative;
    padding: 12px 14px;
    border: 0;
    background: none;
    color: var(--faint);
    font-weight: 600;
    font-size: 14px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .ltabs button:hover {
    color: var(--text-2);
  }

  .ltabs button[aria-selected='true'] {
    color: var(--text);
  }

  .ltabs button[aria-selected='true']::after {
    content: '';
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--signal);
    box-shadow: 0 0 10px var(--signal);
  }

  .count {
    font-family: var(--mono);
    font-size: 10.5px;
    padding: 1px 6px;
    border-radius: 6px;
    background: var(--ink-3);
    color: var(--text-2);
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 30px;
    animation: rise 0.35s var(--ease-out) both;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .block-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 12px;
    flex-wrap: wrap;
  }

  .block-head h3 {
    font-size: 18px;
    font-weight: 650;
    letter-spacing: -0.015em;
  }

  .block-head p {
    margin-top: 3px;
    color: var(--faint);
    font-size: 13px;
  }

  .surface {
    border-radius: 14px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }

  .scroll {
    overflow-x: auto;
  }

  /* Derniers prêts */
  .hist {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  .hist td {
    padding: 12px 16px;
    border-top: 1px solid var(--line);
    vertical-align: middle;
    white-space: nowrap;
  }

  .hist tr:first-child td {
    border-top: 0;
  }

  .hist .who {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 600;
  }

  .hist .dim {
    color: var(--faint);
  }

  .tag {
    display: inline-flex;
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 11.5px;
    background: var(--ink-3);
    color: var(--text-2);
  }

  .avatar {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    flex: none;
    font-weight: 700;
    font-stretch: 112%;
    color: var(--ink-0);
    background: var(--av);
  }

  .avatar.small {
    width: 28px;
    height: 28px;
    font-size: 12px;
  }

  /* Amis */
  .friends {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 12px;
  }

  .friend {
    text-align: left;
    padding: 16px;
    border: 0;
    border-radius: 14px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2);
    display: flex;
    flex-direction: column;
    gap: 12px;
    transition: transform 0.2s var(--ease-out), box-shadow 0.2s;
  }

  .friend:hover {
    transform: translateY(-2px);
    box-shadow: inset 0 0 0 1px var(--signal-line), 0 10px 30px rgb(0 0 0 / 0.35);
  }

  .friend .top {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }

  .friend .top > span:last-child {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .friend strong {
    font-size: 15px;
    overflow-wrap: anywhere;
  }

  .friend .mono {
    font-size: 11.5px;
    color: var(--faint);
  }

  .friend dl {
    margin: 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 4px 10px;
    font-size: 12.5px;
  }

  .friend dt {
    color: var(--faint);
  }

  .friend dd {
    margin: 0;
    color: var(--text-2);
  }

  .friend.add {
    align-items: center;
    justify-content: center;
    flex-direction: row;
    gap: 8px;
    min-height: 150px;
    color: var(--signal-light);
    font-weight: 600;
    box-shadow: none;
    border: 1.5px dashed var(--line-3);
  }

  /* Réglages */
  .apps {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
    gap: 10px;
  }

  .app-t {
    position: relative;
    aspect-ratio: 1;
    border: 0;
    border-radius: 14px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 10px;
    font-size: 13px;
    font-weight: 600;
    text-align: center;
  }

  .app-name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ico {
    width: 46px;
    height: 46px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    font-stretch: 125%;
    font-weight: 800;
    font-size: 20px;
    color: var(--ink-0);
    object-fit: contain;
  }

  .ico.steam {
    background: #1b2838;
    color: #c7d5e0;
  }

  .rm {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--faint);
    display: grid;
    place-items: center;
  }

  .rm:hover:not(:disabled) {
    background: rgb(255 107 107 / 0.15);
    color: var(--bad-text);
  }

  .app-t.add {
    border: 1.5px dashed var(--line-3);
    box-shadow: none;
    color: var(--signal-light);
  }

  .setrow {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 16px 18px;
    flex-wrap: wrap;
  }

  .setrow + .setrow {
    border-top: 1px solid var(--line);
  }

  .setrow .t {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1 1 320px;
  }

  .setrow .t span {
    color: var(--faint);
    font-size: 12.5px;
    line-height: 1.5;
  }

  .setrow input {
    width: 220px;
  }

  @media (max-width: 760px) {
    .how {
      grid-template-columns: 1fr;
    }

    .intro {
      padding: 22px;
    }

    .intro h2 {
      font-size: 28px;
    }
  }
</style>
