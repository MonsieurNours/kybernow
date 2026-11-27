<!--
  Étape 2 (première installation) · Votre compte KyberNow, facultative
  (maquette, rangée « Installateurs ») : la page du relais s'ouvre dans cette
  fenêtre (l'installateur pose sa vue sur le cadre) ; connecté, l'app
  démarrera connectée. « Plus tard » : l'installation, sans compte.
-->
<script lang="ts">
  import { isNative, send } from '../lib/backend'
  import { t } from '../lib/i18n/index.svelte'
  import { app, DEFAULT_RELAY, startInstall } from '../lib/state.svelte'

  const a = app.account
  let frame: HTMLDivElement | undefined = $state()
  let field: HTMLInputElement | undefined = $state()
  let timer: ReturnType<typeof setTimeout> | undefined

  function open(signup: boolean) {
    a.signup = signup
    a.failure = null
    a.phase = 'page'
  }

  function back() {
    send({ cmd: 'accountViewHide' })
    a.phase = 'choose'
  }

  function later() {
    send({ cmd: 'accountViewHide' })
    startInstall()
  }

  function type(value: string) {
    a.relayInput = value
    a.probe = null
    clearTimeout(timer)
    a.probing = !!value.trim()
    if (value.trim()) timer = setTimeout(() => send({ cmd: 'relayProbe', address: value.trim() }), 450)
  }

  function useRelay() {
    if (!a.probe?.ok) return
    a.relay = a.probe.address
    a.relayAccounts = a.probe.accounts
    a.phase = 'choose'
  }

  function defaultRelay() {
    a.relay = DEFAULT_RELAY
    a.relayAccounts = true
    a.relayInput = ''
    a.probe = null
    a.phase = 'choose'
  }

  // La vue de l'installateur suit le cadre de la page du relais.
  $effect(() => {
    if (!frame || a.phase !== 'page' || !isNative) return
    const el = frame
    const signup = a.signup
    const place = () => {
      const r = el.getBoundingClientRect()
      send({ cmd: 'accountView', relay: a.relay, signup, x: r.left, y: r.top, width: r.width, height: r.height })
    }
    const first = setTimeout(place, 480)
    const observer = new ResizeObserver(place)
    observer.observe(el)
    return () => {
      clearTimeout(first)
      observer.disconnect()
    }
  })

  $effect(() => {
    if (a.phase === 'relay') field?.focus()
  })

  /** Navigateur : la page du relais imitée. */
  function simulateSignIn() {
    app.account.signedIn = { relay: a.relay, name: 'Camille', email: 'camille@exemple.fr' }
    startInstall()
  }

  const probeText = $derived.by(() => {
    if (a.probing) return t('account.relay.checking')
    const p = a.probe
    if (!p) return ''
    if (p.error === 'bad_address') return t('account.relay.badAddress')
    if (!p.ok) return t('account.relay.unreachable', { reason: p.error ?? '' })
    return t('account.relay.joined', { version: p.version ?? '?', accounts: p.accounts ? t('account.relay.accountsOn') : t('account.relay.accountsOff') })
  })
</script>

{#if a.phase === 'choose'}
  <div class="account">
    <h1 class="title kn-enter">{t('account.title')}</h1>
    <p class="lead kn-enter kn-d1">{t('account.lead')}</p>
    {#if a.relayAccounts}
      <div class="cards kn-enter kn-d2">
        <button type="button" class="card first" onclick={() => open(false)}>
          <strong>{t('account.signin')}</strong>
          <span>{t('account.signinDetail')}</span>
        </button>
        <button type="button" class="card" onclick={() => open(true)}>
          <strong>{t('account.signup')}</strong>
          <span>{t('account.signupDetail')}</span>
        </button>
      </div>
    {:else}
      <div class="notice kn-enter kn-d2">{t('account.relay.noAccounts')}</div>
    {/if}
    <div class="relay-line kn-enter kn-d3">
      <span>{t('account.relayLabel')}</span>
      <span class="relay-name">{a.relay}</span>
      <span aria-hidden="true">·</span>
      <button type="button" class="text-button" onclick={() => (a.phase = 'relay')}>{t('account.otherRelay')}</button>
      {#if a.relay !== DEFAULT_RELAY}
        <span aria-hidden="true">·</span>
        <button type="button" class="text-button" onclick={defaultRelay}>{t('account.relay.back')}</button>
      {/if}
    </div>
  </div>
  <div class="footer">
    <span class="note">{t('account.laterNote')}</span>
    <div class="spacer"></div>
    <button type="button" class="ghost" onclick={later}>{t('account.later')}</button>
  </div>
{:else if a.phase === 'relay'}
  <div class="account">
    <h1 class="title">{t('account.relay.title')}</h1>
    <p class="lead">{t('account.relay.lead')}</p>
    <div class="field narrow">
      <label for="relay-address">{t('account.relay.address')}</label>
      <input
        id="relay-address"
        class="mono"
        bind:this={field}
        value={a.relayInput}
        oninput={(e) => type(e.currentTarget.value)}
        onkeydown={(e) => e.key === 'Enter' && useRelay()}
        placeholder={t('account.relay.example')}
        autocomplete="off"
        spellcheck="false"
        aria-invalid={!!a.probe && !a.probe.ok}
        aria-describedby="relay-hint relay-status"
      />
      <span id="relay-hint" class="hint">{t('account.relay.hint')}</span>
    </div>
    <div id="relay-status" class="status narrow" role="status" class:bad={a.probe && !a.probe.ok} class:empty={!probeText}>{probeText}</div>
  </div>
  <div class="footer">
    <div class="spacer"></div>
    <button type="button" class="ghost" onclick={defaultRelay}>{t('account.relay.back')}</button>
    <button type="button" class="primary" disabled={!a.probe?.ok} onclick={useRelay}>{t('account.relay.use')}</button>
  </div>
{:else if a.phase === 'page'}
  <div class="page">
    <div class="page-bar">
      <button type="button" class="text-button" onclick={back}>{t('account.back')}</button>
      <span>{a.relay}</span>
      {#if a.signup}
        <button type="button" class="text-button" onclick={() => (a.signup = false)}>{t('account.created')}</button>
      {:else}
        <span></span>
      {/if}
    </div>
    <div class="frame" bind:this={frame}>
      {#if !isNative}
        <div class="sim">
          <span class="lbl">{t('account.simulation')}</span>
          <p class="lead">{t(a.signup ? 'account.simulationSignup' : 'account.simulationLead', { relay: a.relay })}</p>
          {#if !a.signup}<button type="button" class="primary" onclick={simulateSignIn}>{t('account.simulationConnect')}</button>{/if}
        </div>
      {/if}
    </div>
  </div>
{:else}
  <div class="account">
    <h1 class="title">{t('account.failedTitle')}</h1>
    <p class="lead">{a.failure?.code === 'cancelled' ? t('account.cancelled') : t('account.failed', { reason: a.failure?.message ?? '' })}</p>
  </div>
  <div class="footer">
    <div class="spacer"></div>
    <button type="button" class="ghost" onclick={later}>{t('account.later')}</button>
    <button type="button" class="primary" onclick={() => open(a.signup)}>{t('account.retry')}</button>
  </div>
{/if}

<style>
  .account {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-top: 40px;
    max-width: 680px;
  }

  .account h1 {
    font-size: 32px;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }

  .card {
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 22px;
    border-radius: 16px;
    background: var(--ink-2);
    border: 1px solid var(--line-3);
    color: var(--text);
    cursor: pointer;
  }

  .card.first,
  .card:hover,
  .card:focus-visible {
    border-color: var(--signal);
  }

  .card strong {
    font-size: 18px;
    font-weight: 650;
  }

  .card span {
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }

  .relay-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 14px;
    color: var(--muted);
  }

  .relay-name {
    color: var(--text);
  }

  .narrow {
    max-width: 560px;
  }

  .field input.mono {
    font-family: var(--mono);
    font-size: 15px;
  }

  .status {
    min-height: 22px;
    padding: 14px 16px;
    border-radius: 12px;
    background: var(--ink-2);
    border: 1px solid var(--line);
    font-size: 15px;
    color: var(--text);
  }

  .status.empty {
    visibility: hidden;
  }

  .status.bad {
    color: var(--bad-text);
    border-color: var(--bad-line);
  }

  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 8px 0 20px;
  }

  .page-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 13px;
    color: var(--faint);
  }

  .frame {
    flex: 1;
    min-height: 0;
    border-radius: 16px;
    border: 1px solid var(--line-2);
    background: var(--ink-0);
    overflow: hidden;
    display: flex;
  }

  .sim {
    margin: auto;
    max-width: 420px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    text-align: center;
  }
</style>
