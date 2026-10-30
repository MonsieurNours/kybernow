<!--
  Le compte d'un ami, dans un panneau sur le côté (page « Prêt » d'Admin,
  docs/pret.md) : prénom, identifiant, mot de passe (« Générer » ; vide en
  modification : inchangé), compte Steam proposé, créneaux (jours en pastilles,
  heures, « Ajouter un créneau »), lien d'invitation et son QR code,
  « Supprimer ». Admin vérifie aussi tout et dit pourquoi il refuse.
-->
<script lang="ts">
  import { api, type LendFriend, type LendFriendInput, type LendSlot, type LendSteam } from './api'
  import { errorText, notify } from './store.svelte'
  import { avatarColor, copyText, dayName, initial, steamName } from './lend'
  import Icon from './Icon.svelte'
  import Qr from './Qr.svelte'
  import Segmented from './Segmented.svelte'
  import { t } from './i18n/index.svelte'

  let {
    friend = null,
    onsave,
    ondelete,
    onclose,
  }: {
    /** Le compte à modifier, ou `null` pour en créer un. */
    friend?: LendFriend | null
    /** Enregistre ; rejette si Admin refuse (le panneau reste ouvert). */
    onsave: (input: LendFriendInput) => Promise<void>
    ondelete: (friend: LendFriend) => Promise<void>
    onclose: () => void
  } = $props()

  // Les valeurs du compte au moment où le panneau s'ouvre.
  // svelte-ignore state_referenced_locally
  let name = $state(friend?.name ?? '')
  // svelte-ignore state_referenced_locally
  let login = $state(friend?.login ?? '')
  let password = $state('')
  let shown = $state(false)
  // svelte-ignore state_referenced_locally
  let steam = $state<LendSteam>(friend?.steam ?? 'friend')
  // svelte-ignore state_referenced_locally
  let slots = $state<LendSlot[]>(friend?.slots.map((s) => ({ ...s, days: [...s.days] })) ?? [])
  let saving = $state(false)
  let tried = $state(false)
  let invite = $state<{ link: string; web: string } | null>(null)

  const DAYS = [1, 2, 3, 4, 5, 6, 7]
  const slotOk = (s: LendSlot) => s.days.length > 0 && !!s.start && !!s.end && s.end > s.start
  const slotsOk = $derived(slots.every(slotOk))

  $effect(() => {
    if (!friend) return
    api.lendInvite(friend.id).then(
      (v) => (invite = v),
      () => (invite = null),
    )
  })

  function toggleDay(s: LendSlot, d: number) {
    s.days = s.days.includes(d) ? s.days.filter((x) => x !== d) : [...s.days, d].sort((a, b) => a - b)
  }

  /** Un mot de passe facile à dicter : 10 lettres et chiffres sans ceux qui se confondent. */
  function generate() {
    const chars = 'abcdefghjkmnpqrstuvwxyz23456789'
    const bytes = crypto.getRandomValues(new Uint8Array(10))
    password = Array.from(bytes, (b) => chars[b % chars.length]).join('')
    shown = true
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    tried = true
    if (!slotsOk || saving) return
    saving = true
    try {
      await onsave({
        name: name.trim(),
        login: login.trim(),
        ...(password ? { password } : {}),
        steam,
        slots: $state.snapshot(slots),
        enabled: friend?.enabled ?? true,
      })
    } catch {
      // Le refus est dit par la page ; le panneau reste ouvert.
    } finally {
      saving = false
    }
  }

  async function remove() {
    if (!friend || !confirm(t('lend.friends.confirmDelete', { name: friend.name }))) return
    await ondelete(friend)
  }

  async function copy() {
    if (invite && (await copyText(invite.web))) notify('ok', t('share.copied'))
    else notify('error', t('share.copyFailed'))
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
  }

  $effect(() => {
    document.getElementById('fd-name')?.focus()
  })
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<form class="drawer" aria-label={friend ? friend.name : t('lend.admin.friends.newTitle')} onsubmit={submit}>
  <header>
    <span class="avatar" style="--av: {avatarColor(friend?.id ?? name)}">{friend || name ? initial(name || friend?.name || '') : '+'}</span>
    <h3 class="disp">{friend ? friend.name : t('lend.admin.friends.newTitle')}</h3>
    <button type="button" class="icon-button" aria-label={t('lend.close')} onclick={onclose}><Icon name="x" size={18} /></button>
  </header>

  <div class="body">
    <div class="row2">
      <div class="field">
        <label for="fd-name">{t('lend.friends.name')}</label>
        <input id="fd-name" bind:value={name} required maxlength="40" autocomplete="off" />
      </div>
      <div class="field">
        <label for="fd-login">{t('lend.friends.login')}</label>
        <input
          id="fd-login"
          class="mono"
          value={login}
          oninput={(e) => (login = e.currentTarget.value = e.currentTarget.value.toLowerCase())}
          required
          pattern={'[a-z0-9._\\-]{3,32}'}
          maxlength="32"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
        />
      </div>
    </div>
    <span class="hint">{t('lend.friends.loginHint')}</span>

    <div class="field">
      <label for="fd-pw">{t('lend.friends.password')}</label>
      <div class="inline">
        <input
          id="fd-pw"
          type={shown ? 'text' : 'password'}
          class:mono={shown}
          bind:value={password}
          required={!friend}
          minlength="6"
          autocomplete="new-password"
          placeholder={friend ? t('lend.friends.passwordKeep') : t('lend.friends.passwordHint')}
        />
        <button type="button" class="ghost" onclick={generate}>{t('lend.admin.friends.generate')}</button>
      </div>
      <span class="hint">{t('lend.admin.friends.passwordHelp')}</span>
    </div>

    <div class="field">
      <span class="label">{t('lend.friends.steam')}</span>
      <Segmented label={t('lend.friends.steam')} options={(['friend', 'host'] as LendSteam[]).map((s) => ({ value: s, label: steamName(s) }))} value={steam} onchange={(s: LendSteam) => (steam = s)} />
      <span class="hint">{t('lend.admin.friends.steamHelp')}</span>
    </div>

    <fieldset>
      <legend>{t('lend.friends.slots')}</legend>
      <span class="hint">{t('lend.friends.slotsHelp')}</span>
      {#each slots as s, i (i)}
        <div class="slot" class:bad={tried && !slotOk(s)}>
          <div class="days" role="group" aria-label={t('lend.friends.days')}>
            {#each DAYS as d (d)}
              <button type="button" aria-pressed={s.days.includes(d)} aria-label={dayName(d)} onclick={() => toggleDay(s, d)}>{dayName(d, 'short').replace('.', '')}</button>
            {/each}
          </div>
          <div class="hours">
            <input type="time" step="900" bind:value={s.start} aria-label={t('lend.friends.from')} required />
            <span aria-hidden="true">–</span>
            <input type="time" step="900" bind:value={s.end} aria-label={t('lend.friends.to')} required />
            <button type="button" class="icon-button" aria-label={t('lend.friends.removeSlot')} onclick={() => slots.splice(i, 1)}><Icon name="trash" size={15} /></button>
          </div>
        </div>
      {/each}
      {#if tried && !slotsOk}<p class="error" role="alert">{t('lend.friends.slotInvalid')}</p>{/if}
      <button type="button" class="ghost small add" onclick={() => slots.push({ days: [], start: '14:00', end: '18:00' })}><Icon name="plus" size={14} stroke={2} />{t('lend.friends.addSlot')}</button>
    </fieldset>

    {#if friend}
      <div class="field">
        <span class="label">{t('lend.friends.invite')}</span>
        <div class="invite">
          {#if invite}<span class="qr"><Qr text={invite.link} size={92} label={t('lend.friends.inviteQr', { name: friend.name })} /></span>{/if}
          <div>
            <p>{t('lend.friends.inviteHelp', { login: friend.login })}</p>
            <button type="button" class="ghost small" disabled={!invite} onclick={copy}><Icon name="copy" size={14} stroke={2} />{t('share.copy')}</button>
          </div>
        </div>
      </div>
    {/if}
  </div>

  <footer>
    {#if friend}<button type="button" class="danger" onclick={remove}><Icon name="trash" size={14} />{t('lend.friends.delete')}</button>{:else}<span></span>{/if}
    <div class="inline">
      <button type="button" class="ghost" disabled={saving} onclick={onclose}>{t('lend.friends.cancel')}</button>
      <button type="submit" class="primary" disabled={saving}><Icon name="check" size={15} stroke={2} />{friend ? t('lend.friends.save') : t('lend.admin.friends.create')}</button>
    </div>
  </footer>
</form>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(5 7 10 / 0.6);
    backdrop-filter: blur(4px);
    animation: fade 0.3s ease-out;
  }

  .drawer {
    position: fixed;
    z-index: 41;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(470px, 100%);
    background: var(--ink-1);
    box-shadow: -1px 0 0 var(--line-2), -30px 0 60px rgb(0 0 0 / 0.5);
    display: flex;
    flex-direction: column;
    animation: slide 0.4s var(--ease-out) both;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 20px 22px 16px;
    border-bottom: 1px solid var(--line);
  }

  header h3 {
    flex: 1;
    min-width: 0;
    font-size: 20px;
    font-weight: 650;
    overflow-wrap: anywhere;
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

  .body {
    flex: 1;
    overflow: auto;
    padding: 20px 22px 24px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .row2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .row2 + .hint {
    margin-top: -12px;
  }

  .inline {
    display: flex;
    gap: 8px;
  }

  .inline input {
    flex: 1;
    min-width: 0;
  }

  .label {
    font-size: 12.5px;
    color: var(--muted);
  }

  .hint {
    font-size: 12px;
    line-height: 1.45;
    color: var(--faint);
  }

  fieldset {
    margin: 0;
    padding: 0;
    border: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  legend {
    padding: 0;
    margin-bottom: 4px;
    font-size: 12.5px;
    color: var(--muted);
  }

  .slot {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: 12px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
  }

  .slot.bad {
    box-shadow: inset 0 0 0 1px var(--bad-line);
  }

  .days {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .days button {
    min-width: 40px;
    height: 34px;
    padding: 0 6px;
    border: 0;
    border-radius: 8px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--faint);
    font-size: 12px;
    text-transform: capitalize;
  }

  .days button[aria-pressed='true'] {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--text);
  }

  .hours {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--faint);
  }

  .hours input {
    width: 110px;
    font-family: var(--mono);
    font-size: 13px;
  }

  .hours .icon-button {
    margin-left: auto;
  }

  .add {
    align-self: flex-start;
  }

  .error {
    font-size: 13px;
    color: var(--bad-text);
  }

  .invite {
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 14px;
    border-radius: 12px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
  }

  .invite .qr {
    padding: 4px;
    border-radius: 8px;
    background: #f2f8fa;
    flex: none;
  }

  .invite p {
    margin: 0 0 10px;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }

  footer {
    display: flex;
    gap: 10px;
    justify-content: space-between;
    padding: 14px 22px 16px;
    border-top: 1px solid var(--line);
  }

  footer .danger {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  @keyframes slide {
    from {
      transform: translateX(100%);
    }
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @media (max-width: 520px) {
    .row2 {
      grid-template-columns: 1fr;
    }
  }
</style>
