<!--
  Compte KyberNow (docs/comptes.md) : le compte auquel l'ordinateur est
  rattaché, la clé du relais épinglée, « Accepter les comptes KyberNow »,
  et les comptes et appareils entrés par un jeton, qu'on bloque ici même si
  le relais dit oui. Pas rattaché : « Se connecter » demande un code au
  relais et ouvre sa console dans le navigateur (« Rattacher {nom} ») ; la
  page attend la confirmation, qui arrive par la connexion de contrôle.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { isQuiet } from '../lib/http'
  import { poll } from '../lib/poll'
  import { api, type AccountAccess, type AccountView } from '../lib/api'
  import { detail, who } from '../lib/account'
  import { dateAt } from '../lib/format'
  import { errorText, notify } from '../lib/store.svelte'
  import Icon from '../lib/Icon.svelte'
  import Switch from '../lib/Switch.svelte'
  import { t } from '../lib/i18n/index.svelte'

  let account = $state<AccountView | null>(null)
  let entries = $state<AccountAccess[]>([])
  let saving = $state(false)
  let busy = $state<string | null>(null)
  /** La page de la console ouverte par « Se connecter » (pour la rouvrir). */
  let attachUrl = $state<string | null>(null)

  /** `quiet` : relevé périodique, une Station injoignable n'y fait pas de toast toutes les 5 s (WEB-07). */
  async function load(quiet = false) {
    try {
      const before = account
      account = await api.account()
      if (before && !before.attached && account.attached) {
        // Confirmé dans la console : le relais l'a dit par la connexion de contrôle.
        attachUrl = null
        const o = account.owner
        notify('ok', t('account.detached.done', { name: o ? o.email || o.name || o.id : '' }))
      }
      entries = (await api.accountAccess()).entries
    } catch (e) {
      if (!(quiet && isQuiet(e))) notify('error', errorText(e))
    }
  }

  onMount(() => {
    load()
    // Les venues et la liste bougent pendant que la page est ouverte.
    return poll(() => load(true), 5000, { immediate: false })
  })

  const owner = $derived(account?.owner ?? null)
  const ownerName = $derived(owner ? owner.email || owner.name || owner.id : '')

  async function signIn() {
    // La fenêtre s'ouvre pendant le clic (sinon le navigateur la bloque), l'adresse suit.
    const win = window.open('', '_blank')
    saving = true
    try {
      const { url } = await api.attachAccount()
      attachUrl = url
      if (win) {
        win.opener = null
        win.location.href = url
      } else {
        window.open(url, '_blank', 'noopener')
      }
      account = await api.account()
    } catch (e) {
      win?.close()
      notify('error', errorText(e))
    } finally {
      saving = false
    }
  }

  async function cancelAttach() {
    try {
      account = await api.setAccount({ cancel_attach: true })
      attachUrl = null
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  async function setAccept(on: boolean) {
    saving = true
    try {
      account = await api.setAccount({ accept: on })
      notify('ok', t(on ? 'account.accept.on' : 'account.accept.off'))
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      saving = false
    }
  }

  async function detach() {
    if (!confirm(t('account.attached.confirmDetach'))) return
    saving = true
    try {
      account = await api.setAccount({ detach: true })
      entries = []
      notify('ok', t('account.attached.detached'))
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      saving = false
    }
  }

  async function block(e: AccountAccess, blocked: boolean, whole = false) {
    busy = e.id
    try {
      entries = (await api.blockAccess(e.id, blocked, whole)).entries
      const name = whole ? e.name || who(e) : who(e)
      notify('ok', t(!blocked ? 'account.access.unblocked' : whole ? 'account.access.blockedAll' : 'account.access.blocked', { name }))
    } catch (err) {
      notify('error', errorText(err))
    } finally {
      busy = null
    }
  }
</script>

<header class="space-head">
  <div>
    <span class="lbl">{t('account.eyebrow')}</span>
    <h1>{t('account.title')}</h1>
    <p>{t('account.intro')}</p>
  </div>
</header>

<div class="space-body">
  {#if account && !account.attached}
    <!-- Pas rattaché : se connecter à la console du relais. -->
    <section aria-labelledby="t-acc" class="card feature detached">
      <h2 id="t-acc">{t('account.detached.title')}</h2>
      <p>{t('account.detached.text')}</p>
      {#if account.attaching}
        <!-- En attente de la confirmation dans la console du relais. -->
        <p class="waiting" role="status"><span class="spinner" aria-hidden="true"></span>{t('account.detached.waiting')}</p>
        <div class="actions">
          {#if attachUrl}
            <a class="ghost small" href={attachUrl} target="_blank" rel="noopener noreferrer">{t('account.detached.reopen')}</a>
          {/if}
          <button type="button" class="ghost small" onclick={cancelAttach}>{t('account.detached.cancel')}</button>
        </div>
      {:else}
        <div class="actions">
          <button type="button" class="primary" disabled={!account.can_attach || saving} onclick={signIn}>
            <Icon name="user" size={16} />{t('account.detached.signIn')}
          </button>
          {#if !account.can_attach}<span class="soon">{t('account.detached.noRelay')}</span>{/if}
        </div>
      {/if}
      <p class="hint">{t('account.detached.optional')}</p>
    </section>
  {:else if account}
    <!-- Rattaché : à qui, le relais et sa clé, l'interrupteur. -->
    <section aria-labelledby="t-acc" class="card attached">
      <div class="owner">
        <div>
          <span class="lbl" id="t-acc">{owner?.kind === 'organization' ? t('account.attached.organization') : t('account.attached.to')}</span>
          <strong>{ownerName}</strong>
          <span class="line">
            {t('account.attached.relayLine', { relay: account.relay ?? '', date: account.pinned_at ? dateAt(account.pinned_at) : '—' })}
          </span>
          {#each account.keys as k (k.kid)}
            <span class="mono key" title={k.fingerprint ?? ''}>{t('account.attached.key')} · {k.fingerprint?.slice(0, 7 + 3 * 8 - 1) ?? k.kid}…</span>
          {/each}
          {#if account.test}
            <span class="test"><Icon name="warning" size={14} />{t('account.attached.test')}</span>
          {/if}
        </div>
        <div class="owner-actions">
          {#if account.console}
            <a class="ghost small" href={account.console} target="_blank" rel="noopener noreferrer">{t('account.attached.console')}</a>
          {/if}
          <button type="button" class="danger small" disabled={saving} onclick={detach}>{t('account.attached.detach')}</button>
        </div>
      </div>
      <div class="set">
        <div>
          <h3>{t('account.accept.title')}</h3>
          <p>{t('account.accept.help')}</p>
        </div>
        <Switch label={t('account.accept.title')} checked={account.accept} disabled={saving} onchange={setAccept} />
      </div>
    </section>
  {/if}

  {#if account && (account.attached || entries.length)}
    <!-- Garde-fou 2 : la liste locale, et le blocage qui vaut même si le relais dit oui. -->
    <section aria-labelledby="t-access" class="card access">
      <div class="card-head">
        <div>
          <h2 id="t-access">{t('account.access.title')}</h2>
          <span class="sub">{t('account.access.sub')}</span>
        </div>
      </div>
      {#each entries as e (e.id)}
        <div class="entry" class:blocked={e.blocked !== null}>
          <div class="who">
            <span class="name">{who(e) || e.account}</span>
            <span class="detail">{detail(e)}</span>
          </div>
          <div class="entry-actions">
            {#if e.blocked !== null}
              <button
                type="button"
                class="danger small"
                aria-label={t('account.access.unblockAria', { name: who(e) })}
                disabled={busy !== null}
                onclick={() => block(e, false, e.account_blocked)}>{t('account.access.unblock')}</button
              >
            {:else}
              {#if !e.owner}
                <button
                  type="button"
                  class="ghost small"
                  aria-label={t('account.access.blockAccountAria', { name: e.name || who(e) })}
                  disabled={busy !== null}
                  onclick={() => block(e, true, true)}>{t('account.access.blockAccount')}</button
                >
              {/if}
              <button
                type="button"
                class="ghost small"
                aria-label={t('account.access.blockAria', { name: who(e) })}
                disabled={busy !== null}
                onclick={() => block(e, true)}>{t('account.access.block')}</button
              >
            {/if}
          </div>
        </div>
      {:else}
        <p class="empty">{t('account.access.none')}</p>
      {/each}
    </section>
  {/if}
</div>

<style>
  .detached {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .detached > p {
    font-size: 14.5px;
    line-height: 1.55;
    color: var(--text-2);
    max-width: 60ch;
  }

  .detached .hint {
    font-size: 13px;
    color: var(--faint);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 16px;
  }

  .soon {
    font-size: 13px;
    color: var(--faint);
  }

  .waiting {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 14.5px;
    line-height: 1.55;
    color: var(--text-2);
    max-width: 60ch;
  }

  .spinner {
    flex: none;
    width: 14px;
    height: 14px;
    border: 2px solid var(--line);
    border-top-color: var(--muted);
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }

  .owner {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 16px 24px;
    padding-bottom: 18px;
  }

  .owner > div:first-child {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .owner .lbl {
    font-size: 13.5px;
    color: var(--faint);
  }

  .owner strong {
    font-size: 19px;
    font-weight: 650;
    overflow-wrap: anywhere;
  }

  .owner .line {
    font-size: 13.5px;
    color: var(--muted);
  }

  .key {
    font-size: 12px;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  .test {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--warn-text);
  }

  .owner-actions,
  .entry-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .danger.small {
    height: 34px;
    padding: 0 12px;
    font-size: 13px;
  }

  .access {
    padding-bottom: 14px;
  }

  .entry {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 10px 16px;
    padding: 12px 0;
    border-top: 1px solid var(--line);
  }

  .entry .who {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .entry .name {
    font-size: 15px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .entry .detail {
    font-size: 13px;
    color: var(--faint);
  }

  .entry.blocked .name {
    color: var(--muted);
  }

  .empty {
    padding: 14px 0 6px;
    border-top: 1px solid var(--line);
    font-size: 13.5px;
    color: var(--faint);
  }
</style>
