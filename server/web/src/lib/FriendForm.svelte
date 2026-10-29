<!--
  Le compte d'un ami (page « Prêt » d'Admin, docs/pret.md) : prénom,
  identifiant, mot de passe (vide en modification : inchangé), compte Steam
  proposé, et les créneaux de la semaine pendant lesquels il emprunte sans
  demander. Admin vérifie aussi tout (identifiant unique, 6 caractères au
  moins) et dit pourquoi il refuse.
-->
<script lang="ts">
  import type { LendFriend, LendFriendInput, LendSlot, LendSteam } from './api'
  import { dayName, steamName } from './lend'
  import Icon from './Icon.svelte'
  import Segmented from './Segmented.svelte'
  import { t } from './i18n/index.svelte'

  let {
    friend = null,
    onsave,
    oncancel,
  }: {
    /** Le compte à modifier, ou `null` pour en créer un. */
    friend?: LendFriend | null
    /** Enregistre ; rejette si Admin refuse (le formulaire reste ouvert). */
    onsave: (input: LendFriendInput) => Promise<void>
    oncancel: () => void
  } = $props()

  // Les valeurs du compte au moment où le formulaire s'ouvre.
  // svelte-ignore state_referenced_locally
  let name = $state(friend?.name ?? '')
  // svelte-ignore state_referenced_locally
  let login = $state(friend?.login ?? '')
  let password = $state('')
  // svelte-ignore state_referenced_locally
  let steam = $state<LendSteam>(friend?.steam ?? 'friend')
  // svelte-ignore state_referenced_locally
  let slots = $state<LendSlot[]>(friend?.slots.map((s) => ({ ...s, days: [...s.days] })) ?? [])
  let saving = $state(false)
  let tried = $state(false)

  const id = Math.random().toString(36).slice(2, 8)
  const DAYS = [1, 2, 3, 4, 5, 6, 7]

  const slotOk = (s: LendSlot) => s.days.length > 0 && !!s.start && !!s.end && s.end > s.start
  const slotsOk = $derived(slots.every(slotOk))

  function toggleDay(s: LendSlot, d: number) {
    s.days = s.days.includes(d) ? s.days.filter((x) => x !== d) : [...s.days, d].sort((a, b) => a - b)
  }

  function addSlot() {
    slots.push({ days: [], start: '14:00', end: '18:00' })
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
      // Le refus est dit par la page ; le formulaire reste ouvert.
    } finally {
      saving = false
    }
  }
</script>

<form class="friend-form" onsubmit={submit}>
  <div class="grid">
    <div class="field">
      <label for="f-name-{id}">{t('lend.friends.name')}</label>
      <input id="f-name-{id}" bind:value={name} required maxlength="40" autocomplete="off" />
    </div>
    <div class="field">
      <label for="f-login-{id}">{t('lend.friends.login')}</label>
      <input
        id="f-login-{id}"
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
      <span class="hint">{t('lend.friends.loginHint')}</span>
    </div>
    <div class="field">
      <label for="f-pw-{id}">{t('lend.friends.password')}</label>
      <input
        id="f-pw-{id}"
        type="password"
        bind:value={password}
        required={!friend}
        minlength="6"
        autocomplete="new-password"
        placeholder={friend ? t('lend.friends.passwordKeep') : ''}
      />
      <span class="hint">{t('lend.friends.passwordHint')}</span>
    </div>
    <div class="field wide">
      <span class="label" id="f-steam-{id}">{t('lend.friends.steam')}</span>
      <Segmented label={t('lend.friends.steam')} options={(['friend', 'host'] as LendSteam[]).map((s) => ({ value: s, label: steamName(s) }))} value={steam} onchange={(s: LendSteam) => (steam = s)} />
    </div>
  </div>

  <fieldset class="slots">
    <legend>{t('lend.friends.slots')}</legend>
    <p class="hint">{t('lend.friends.slotsHelp')}</p>
    {#each slots as s, i (i)}
      <div class="slot" class:bad={tried && !slotOk(s)}>
        <div class="days" role="group" aria-label={t('lend.friends.days')}>
          {#each DAYS as d (d)}
            <button type="button" aria-pressed={s.days.includes(d)} aria-label={dayName(d)} onclick={() => toggleDay(s, d)}>{dayName(d, 'short')}</button>
          {/each}
        </div>
        <div class="hours">
          <input type="time" step="900" bind:value={s.start} aria-label={t('lend.friends.from')} required />
          <span aria-hidden="true">–</span>
          <input type="time" step="900" bind:value={s.end} aria-label={t('lend.friends.to')} required />
          <button type="button" class="icon-button" aria-label={t('lend.friends.removeSlot')} onclick={() => slots.splice(i, 1)}><Icon name="x" size={16} /></button>
        </div>
      </div>
    {/each}
    {#if tried && !slotsOk}<p class="error" role="alert">{t('lend.friends.slotInvalid')}</p>{/if}
    <button type="button" class="ghost small add-slot" onclick={addSlot}><Icon name="plus" size={14} stroke={2} />{t('lend.friends.addSlot')}</button>
  </fieldset>

  <div class="actions">
    <button type="button" class="ghost" disabled={saving} onclick={oncancel}>{t('lend.friends.cancel')}</button>
    <button type="submit" class="primary" disabled={saving}><Icon name="check" size={15} stroke={2} />{t('lend.friends.save')}</button>
  </div>
</form>

<style>
  .friend-form {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 18px;
    border-radius: 14px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(240px, 100%), 1fr));
    gap: 14px 18px;
  }

  .field.wide {
    grid-column: 1 / -1;
    align-items: flex-start;
  }

  .field .label {
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
    font-size: 14px;
    font-weight: 600;
  }

  .slot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 16px;
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--ink-3);
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
    min-width: 44px;
    height: 32px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: var(--muted);
    font-size: 12.5px;
  }

  .days button[aria-pressed='true'] {
    background: rgb(109 91 255 / 0.2);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.55);
    color: #fff;
  }

  .hours {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--faint);
  }

  .hours input {
    width: 112px;
    height: 36px;
    font-size: 13.5px;
  }

  .add-slot {
    align-self: flex-start;
  }

  .error {
    font-size: 13px;
    color: var(--bad-text);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
