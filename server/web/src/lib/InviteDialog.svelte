<!--
  « Inviter un appareil… » (Appareils Link, docs/invitations.md) : en deux
  étapes, le nom et la durée, puis le code, le mot de passe (montré une seule
  fois), l'échéance et le QR code du lien des apps. Ouvert directement sur la
  seconde étape après « Nouveau mot de passe ». Avec le relais, le lien qui le
  porte est attendu 10 s au plus, comme pour le prêt.
-->
<script lang="ts">
  import { untrack } from 'svelte'
  import { api, type Invite, type InvitesView, type InviteShown } from './api'
  import { errorText, notify } from './store.svelte'
  import { copyText } from './lend'
  import { dateAt } from './format'
  import Icon from './Icon.svelte'
  import Qr from './Qr.svelte'
  import Segmented from './Segmented.svelte'
  import { t, tn } from './i18n/index.svelte'

  let {
    days,
    shown: initial = null,
    onchange,
    onclose,
  }: { days: number[]; shown?: InviteShown | null; onchange: (view: InvitesView) => void; onclose: () => void } = $props()

  // Ouvert sur la seconde étape quand Admin vient de rendre un mot de passe.
  // svelte-ignore state_referenced_locally
  let shown = $state<InviteShown | null>(initial)
  let name = $state('')
  // Sept jours proposés (docs/invitations.md).
  // svelte-ignore state_referenced_locally
  let length = $state(days.includes(7) ? 7 : (days[0] ?? 7))
  let busy = $state(false)
  let copied = $state(false)
  let now = $state(Date.now())

  const invite = $derived<Invite | null>(shown?.invite ?? null)
  const spaced = $derived(invite ? `${invite.code.slice(0, 3)} ${invite.code.slice(3)}` : '')

  $effect(() => {
    if (!shown) return
    const timer = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(timer)
  })

  /** L'invitation dont le relais prend le code (`null` sinon) : une valeur simple, qui ne change pas à chaque relecture. */
  const connectingId = $derived(invite?.relay?.state === 'connecting' ? invite.id : null)
  /** Depuis quand on attend le relais : posé par un effet, pas dans un `$derived` (WEB-27). */
  let waitFrom = $state(0)
  $effect(() => {
    const id = connectingId
    untrack(() => (waitFrom = id ? waitFrom || Date.now() : 0))
  })
  const relay = $derived.by(() => {
    const st = invite?.relay?.state ?? null
    if (st !== 'connecting') return st
    return !waitFrom || now - waitFrom < 10000 ? 'waiting' : 'unreachable'
  })

  // Tant que le relais prend le code, l'invitation est relue : son lien portera le relais. L'effet ne suit que
  // l'identifiant (WEB-27 : il se recréait à chaque relecture, qui réécrit `shown`).
  $effect(() => {
    const id = connectingId
    if (!id) return
    const timer = setInterval(async () => {
      try {
        const view = await api.invites()
        const fresh = view.invites.find((i) => i.id === id)
        untrack(() => {
          if (fresh && shown) shown = { ...shown, ...view, invite: fresh }
        })
        onchange(view)
      } catch {
        // 401 gérée globalement ; on réessaie au tour suivant.
      }
    }, 1500)
    return () => clearInterval(timer)
  })

  async function create(e: SubmitEvent) {
    e.preventDefault()
    if (!name.trim()) return
    busy = true
    try {
      const fresh = await api.createInvite(name.trim(), length)
      shown = fresh
      onchange(fresh)
    } catch (err) {
      notify('error', errorText(err))
    } finally {
      busy = false
    }
  }

  let copiedTimer: ReturnType<typeof setTimeout> | undefined
  async function copy() {
    if (!shown) return
    if (await copyText(shown.password)) {
      copied = true
      clearTimeout(copiedTimer)
      copiedTimer = setTimeout(() => (copied = false), 2000)
    } else {
      notify('error', t('devices.invites.copyFailed'))
    }
  }

  $effect(() => () => clearTimeout(copiedTimer))

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
  }
</script>

<svelte:window {onkeydown} />

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="invite-dialog-title">
    {#if !shown}
      <div class="steps" aria-hidden="true"><i class="on"></i><i></i></div>
      <h3 id="invite-dialog-title" class="disp">{t('devices.invites.dialogTitle')}</h3>
      <form onsubmit={create}>
        <div class="field">
          <label for="invite-name">{t('devices.invites.name')}</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="invite-name" bind:value={name} placeholder={t('devices.invites.namePlaceholder')} maxlength="40" autocomplete="off" autofocus required />
        </div>
        <div class="field">
          <span class="label">{t('devices.invites.duration')}</span>
          <Segmented label={t('devices.invites.duration')} options={days.map((d) => ({ value: String(d), label: tn('devices.invites.days', d) }))} value={String(length)} onchange={(v: string) => (length = Number(v))} />
          <p class="duration-help">{t('devices.invites.durationHelp')}</p>
        </div>
        <div class="foot">
          <button type="button" class="ghost" onclick={onclose}>{t('devices.invites.cancel')}</button>
          <button type="submit" class="primary" disabled={busy || !name.trim()}><Icon name="key" size={16} stroke={2} />{t('devices.invites.create')}</button>
        </div>
      </form>
    {:else if invite}
      <div class="steps" aria-hidden="true"><i class="on"></i><i class="on"></i></div>
      <h3 id="invite-dialog-title" class="disp">{invite.name}</h3>
      <div class="shown">
        <span class="qr">
          {#if relay === 'waiting'}<span class="spin" aria-label={t('share.relayPreparing')}></span>{:else}<Qr text={invite.link} size={128} label={t('devices.invites.qrLabel')} />{/if}
        </span>
        <div class="secrets">
          <div class="secret">
            <span class="lbl">{t('devices.invites.code')}</span>
            <strong class="code" aria-label={invite.code.split('').join(' ')}>{spaced}</strong>
          </div>
          <div class="secret">
            <span class="lbl">{t('devices.invites.password')}</span>
            <span class="password-row">
              <strong class="password">{shown.password}</strong>
              <button type="button" class="ghost small" onclick={copy}><Icon name={copied ? 'check' : 'copy'} size={14} stroke={2} />{copied ? t('devices.invites.copied') : t('devices.invites.copy')}</button>
            </span>
            <span class="warn">{t('devices.invites.writeDown')}</span>
          </div>
          <span class="until">{t('devices.invites.validUntil', { date: dateAt(invite.expires) })}</span>
        </div>
      </div>
      <p class="help">{t('devices.invites.appHelp')}</p>
      {#if relay === 'waiting'}<p class="help" role="status">{t('share.relayPreparing')}</p>{:else if relay === 'unreachable'}<p class="help">{t('share.relayUnreachable')}</p>{/if}
      {#if !shown.relay.set}
        <p class="note">{t('devices.invites.noRelay')}</p>
      {:else if shown.remote_access_turned_on || shown.relay.remote_access}
        <p class="note">{t('devices.invites.remoteOn')}</p>
      {/if}
      <div class="foot end">
        <button type="button" class="primary" onclick={onclose}>{t('devices.invites.done')}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: 16px;
    background: rgb(5 7 10 / 0.7);
    backdrop-filter: blur(8px);
    animation: fade 0.25s ease-out;
  }

  .dialog {
    width: min(560px, 100%);
    max-height: 100%;
    overflow: auto;
    border-radius: 18px;
    background: var(--ink-1);
    box-shadow: inset 0 0 0 1px var(--line-2), 0 30px 80px rgb(0 0 0 / 0.6);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    animation: rise 0.35s var(--ease-out) both;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .steps {
    display: flex;
    gap: 8px;
  }

  .steps i {
    height: 4px;
    flex: 1;
    border-radius: 4px;
    background: var(--ink-3);
  }

  .steps i.on {
    background: var(--signal);
    box-shadow: 0 0 8px var(--signal);
  }

  h3 {
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }

  .label {
    font-size: 12.5px;
    color: var(--muted);
  }

  .duration-help {
    font-size: 12.5px;
    color: var(--faint);
  }

  .shown {
    display: flex;
    flex-wrap: wrap;
    gap: 20px 24px;
    align-items: center;
  }

  .qr {
    width: 140px;
    height: 140px;
    flex: none;
    border-radius: 10px;
    background: #f2f8fa;
    padding: 6px;
    display: grid;
    place-items: center;
  }

  .spin {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 3px solid rgb(5 7 10 / 0.15);
    border-top-color: var(--signal-strong);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .secrets {
    flex: 1 1 240px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .secret {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .secret .lbl {
    color: var(--signal-light);
    font-size: 9.5px;
  }

  .code {
    font-family: var(--mono);
    font-size: 40px;
    font-weight: 500;
    letter-spacing: 0.08em;
    color: var(--neon-core);
    text-shadow: var(--text-glow);
    white-space: nowrap;
  }

  .password-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }

  .password {
    font-family: var(--mono);
    font-size: 22px;
    font-weight: 500;
    letter-spacing: 0.04em;
    user-select: all;
  }

  .warn {
    font-size: 12.5px;
    color: var(--warn-text);
  }

  .until {
    font-size: 13px;
    color: var(--text-2);
  }

  .help {
    color: var(--text-2);
    font-size: 13.5px;
  }

  .note {
    font-size: 13px;
    color: var(--faint);
  }

  .foot {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
  }

  .foot.end {
    justify-content: flex-end;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @media (max-width: 560px) {
    .code {
      font-size: 32px;
    }
  }
</style>
